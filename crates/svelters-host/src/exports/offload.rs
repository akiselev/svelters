//! Thread offload for exports (E1, `docs/notes/egui-rust-exports.md` §6): the thread a call runs on is
//! chosen by its signature.
//!
//! | Declared as | Runs on |
//! | --- | --- |
//! | `fn` | the JS thread, synchronously (E0) |
//! | `#[js(worker)] fn` | rayon (the global pool, or [`set_worker_pool`]) |
//! | `#[js(blocking)] fn` | a blocking pool of plain threads (file/DB I/O, sleeps) |
//! | `async fn`, `#[js(local)] async fn` | the JS thread: polled eagerly at the call, then whenever its waker fires (decision 2026-10-21) |
//! | `#[js(tokio)] async fn` | tokio (feature `tokio`; [`set_tokio_handle`], else a small runtime of our own) |
//!
//! Every offloaded member returns a `Promise` and takes a trailing `{ signal, onProgress }`. Workers
//! never see JS values: arguments are converted to owned Rust values on the JS thread, and results go
//! back through the host's completion queue ([`Completions`]), which wakes the backend once per burst.
//! The host drains it at the start of every checkpoint and settles the promises directly in the
//! engine, in one entry per checkpoint. An abort rejects the promise at once (decision 2026-10-21); a
//! late result is dropped on the JS thread without ever becoming a JS value.

/// The rayon `#[js(worker)]` jobs run on.
pub use rayon;

use super::{CallCx, IntoJs, NativeError, TypeBuilder, TypeRef, Value, host_error};
use std::collections::VecDeque;
use std::future::Future;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::task::{Context, Poll, Wake, Waker};
use std::time::Duration;
use svelters_abi::{SVL_NULL, WakeHandle};

/// Where an offloaded export runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Exec {
    /// `#[js(worker)]`: rayon.
    Worker,
    /// `#[js(blocking)]`: the blocking pool.
    Blocking,
    /// `async fn` / `#[js(local)] async fn`: the JS thread.
    Local,
    /// `#[js(tokio)] async fn`: tokio.
    Tokio,
}

impl Exec {
    /// The `runsOn` name in the schema and typings.
    pub fn name(self) -> &'static str {
        match self {
            Exec::Worker => "worker",
            Exec::Blocking => "blocking",
            Exec::Local => "local",
            Exec::Tokio => "tokio",
        }
    }
}

/// Turns a finished job's result into a value, on the JS thread (objects need the host's table).
pub type SendSettle = Box<dyn FnOnce(&CallCx<'_>) -> Result<Value, NativeError> + Send>;
/// The same for JS-thread futures, whose results need not be `Send`.
pub type LocalSettle = Box<dyn FnOnce(&CallCx<'_>) -> Result<Value, NativeError>>;
/// What a thread-pool job or a tokio task produces.
pub type Outcome = Result<SendSettle, NativeError>;
pub(crate) type LocalFuture = Pin<Box<dyn Future<Output = Result<LocalSettle, NativeError>>>>;
type SendFuture = Pin<Box<dyn Future<Output = Outcome> + Send>>;

/// The work an offloaded call starts (built by the generated start thunk).
pub enum Job {
    /// A synchronous closure for rayon ([`Exec::Worker`]) or the blocking pool ([`Exec::Blocking`]).
    Pool(Exec, Box<dyn FnOnce() -> SendSettle + Send>),
    /// A future polled on the JS thread.
    Local(LocalFuture),
    /// A `Send` future spawned on tokio.
    Tokio(SendFuture),
}

// ---- the cancel token and progress slot a job shares with the host ----

/// What a running job shares with the host: the cancel flag, cancel hooks and the progress slot.
pub struct OpShared {
    pub(crate) id: u64,
    pub(crate) generation: u64,
    cancelled: AtomicBool,
    hooks: Mutex<Vec<Box<dyn FnOnce() + Send>>>,
    progress_pending: AtomicBool,
    progress: OnceLock<Arc<dyn ProgressSource>>,
    pub(crate) queue: Arc<Completions>,
}

impl OpShared {
    pub(crate) fn new(id: u64, generation: u64, queue: Arc<Completions>) -> Arc<OpShared> {
        Arc::new(OpShared {
            id,
            generation,
            cancelled: AtomicBool::new(false),
            hooks: Mutex::new(Vec::new()),
            progress_pending: AtomicBool::new(false),
            progress: OnceLock::new(),
            queue,
        })
    }

    pub(crate) fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    /// Fires the token once: sets the flag, then runs the hooks (wakers of `cancelled()`, a tokio abort).
    pub(crate) fn cancel(&self) {
        if self.cancelled.swap(true, Ordering::AcqRel) {
            return;
        }
        let hooks = std::mem::take(&mut *lock(&self.hooks));
        for h in hooks {
            h();
        }
    }

    /// Runs `f` when the token fires (at once if it already has).
    pub(crate) fn on_cancel(&self, f: Box<dyn FnOnce() + Send>) {
        let mut g = lock(&self.hooks);
        if self.is_cancelled() {
            drop(g);
            f();
        } else {
            g.push(f);
        }
    }

    /// The latest progress value, if one arrived since the last take (JS thread).
    pub(crate) fn take_progress(&self) -> Option<SendSettle> {
        self.progress_pending.store(false, Ordering::Release);
        self.progress.get().and_then(|p| p.take())
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

trait ProgressSource: Send + Sync {
    fn take(&self) -> Option<SendSettle>;
}

impl<P: IntoJs + Send + 'static> ProgressSource for Mutex<Option<P>> {
    fn take(&self) -> Option<SendSettle> {
        let p = lock(self).take()?;
        Some(Box::new(move |cx: &CallCx<'_>| p.into_js(cx)))
    }
}

/// An offloaded call was cancelled (aborted, superseded, its owner disposed or its runtime reloaded).
///
/// The promise was rejected already when a job sees this, so what the job returns afterwards is
/// dropped. `?` turns it into a [`NativeError`] (`svelters` / `cancelled`), a `String` or a boxed
/// error; an export with its own error type can map it, or use [`WorkCx::abort_if_cancelled`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Cancelled;

impl std::fmt::Display for Cancelled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("the operation was cancelled")
    }
}

impl std::error::Error for Cancelled {}

impl super::ExportError for Cancelled {
    fn code(&self) -> String {
        "cancelled".into()
    }
    fn message(&self) -> String {
        self.to_string()
    }
    /// None in the module's domain: a cancellation is `svelters` / `cancelled` (the typings say so).
    fn codes() -> Vec<&'static str> {
        Vec::new()
    }
    fn into_native(self, _domain: &str) -> NativeError {
        host_error("cancelled", self.to_string())
    }
}

impl From<Cancelled> for NativeError {
    fn from(c: Cancelled) -> NativeError {
        host_error("cancelled", c.to_string())
    }
}

impl From<Cancelled> for String {
    fn from(c: Cancelled) -> String {
        c.to_string()
    }
}

/// The context an offloaded export may take as a parameter (it is injected, never a JS argument):
/// cancellation and progress.
///
/// ```ignore
/// #[svelters::export(module = "sim")]
/// #[js(worker)]
/// pub fn step(n: u32, cx: WorkCx<f32>) -> Result<u32, NativeError> {
///     for i in 0..n {
///         cx.check()?;                                  // cancelled → stop early (the result is dropped)
///         if i % 64 == 0 { cx.progress(i as f32 / n as f32); }   // latest value, once per checkpoint
///     }
///     Ok(n)
/// }
/// ```
///
/// `P` (default `()`: no progress) is what `options.onProgress` receives in JS. Only cooperative
/// code is interrupted: a job that never checks runs to the end, and its result is then dropped.
pub struct WorkCx<P = ()> {
    op: Arc<OpShared>,
    slot: Arc<Mutex<Option<P>>>,
}

impl<P> Clone for WorkCx<P> {
    fn clone(&self) -> Self {
        WorkCx { op: self.op.clone(), slot: self.slot.clone() }
    }
}

impl<P> WorkCx<P> {
    /// True once the call was cancelled.
    pub fn is_cancelled(&self) -> bool {
        self.op.is_cancelled()
    }

    /// `Err(Cancelled)` once the call was cancelled.
    pub fn check(&self) -> Result<(), Cancelled> {
        if self.is_cancelled() { Err(Cancelled) } else { Ok(()) }
    }

    /// Unwinds out of the job once the call was cancelled (no panic message is printed; the host
    /// treats the unwind as a cancellation). For deep call stacks whose error types have no
    /// `Cancelled` variant.
    pub fn abort_if_cancelled(&self) {
        if self.is_cancelled() {
            std::panic::resume_unwind(Box::new(Cancelled));
        }
    }

    /// A future that completes when the call is cancelled (for `async` exports: `select!` on it).
    pub fn cancelled(&self) -> CancelledFuture {
        CancelledFuture { op: self.op.clone(), registered: Arc::new(Mutex::new(None)) }
    }
}

impl<P: IntoJs + Send + 'static> WorkCx<P> {
    /// Reports progress: the latest value wins, and JS's `onProgress` sees it at most once per
    /// checkpoint. Costs a lock and an atomic; the first report after a delivery wakes the host.
    pub fn progress(&self, p: P) {
        *lock(&self.slot) = Some(p);
        if !self.op.progress_pending.swap(true, Ordering::AcqRel) {
            self.op.queue.push_progress(self.op.id);
        }
    }
}

/// [`WorkCx::cancelled`].
pub struct CancelledFuture {
    op: Arc<OpShared>,
    registered: Arc<Mutex<Option<Waker>>>,
}

impl Future for CancelledFuture {
    type Output = ();
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        if self.op.is_cancelled() {
            return Poll::Ready(());
        }
        let first = lock(&self.registered).replace(cx.waker().clone()).is_none();
        if first {
            let slot = self.registered.clone();
            self.op.on_cancel(Box::new(move || {
                if let Some(w) = lock(&slot).take() {
                    w.wake();
                }
            }));
        }
        if self.op.is_cancelled() { Poll::Ready(()) } else { Poll::Pending }
    }
}

/// Parameters the host injects into an offloaded call (`WorkCx<P>`).
pub trait Injected: Sized {
    #[doc(hidden)]
    fn inject(op: &Arc<OpShared>) -> Self;
    /// The progress type (`None` for `()`).
    #[doc(hidden)]
    fn progress(b: &mut TypeBuilder) -> Option<TypeRef>;
}

impl<P: IntoJs + Send + 'static> Injected for WorkCx<P> {
    fn inject(op: &Arc<OpShared>) -> Self {
        let slot: Arc<Mutex<Option<P>>> = Arc::new(Mutex::new(None));
        let _ = op.progress.set(slot.clone());
        WorkCx { op: op.clone(), slot }
    }
    fn progress(b: &mut TypeBuilder) -> Option<TypeRef> {
        let t = P::describe(b);
        if t.slot.tag == SVL_NULL && t.ts == "void" { None } else { Some(t) }
    }
}

// ---- the completion queue ----

#[derive(Default)]
struct Queue {
    done: VecDeque<(u64, u64, Outcome)>,
    progress: Vec<u64>,
    ready: Vec<u64>,
    /// E2: transferred buffers whose tokens were dropped: freed on the JS thread at the next drain.
    released: Vec<u64>,
}

/// The host's completion queue: finished jobs, progress notifications and woken local futures.
/// Any thread pushes; the JS thread drains it at the start of every checkpoint. A burst of pushes
/// costs one backend `wake`.
pub struct Completions {
    q: Mutex<Queue>,
    wake_pending: AtomicBool,
    /// Cleared when the host goes away: later pushes reach nobody (and wake nobody).
    waker: Mutex<Option<WakeHandle>>,
}

impl Completions {
    pub(crate) fn new(waker: WakeHandle) -> Arc<Completions> {
        Arc::new(Completions { q: Mutex::new(Queue::default()), wake_pending: AtomicBool::new(false), waker: Mutex::new(Some(waker)) })
    }

    fn wake(&self) {
        if !self.wake_pending.swap(true, Ordering::AcqRel) {
            if let Some(w) = &*lock(&self.waker) {
                w.wake();
            }
        }
    }

    pub(crate) fn push_done(&self, id: u64, generation: u64, outcome: Outcome) {
        lock(&self.q).done.push_back((id, generation, outcome));
        self.wake();
    }

    fn push_progress(&self, id: u64) {
        lock(&self.q).progress.push(id);
        self.wake();
    }

    fn push_ready(&self, id: u64) {
        lock(&self.q).ready.push(id);
        self.wake();
    }

    /// E2: a transfer token was dropped (any thread): its JS memory is freed at the next drain.
    pub(crate) fn push_release(&self, id: u64) {
        lock(&self.q).released.push(id);
        self.wake();
    }

    /// Takes the finished results whose generation `keep` refuses (a runtime that went away), for the
    /// JS thread to drop before that runtime is (E2: transfer tokens in them are let go first). The
    /// caller drops them outside the lock: dropping a token pushes its release to this queue.
    pub(crate) fn take_done_except(&self, keep: impl Fn(u64) -> bool) -> Vec<(u64, u64, Outcome)> {
        let mut q = lock(&self.q);
        let (k, g): (VecDeque<_>, VecDeque<_>) = std::mem::take(&mut q.done).into_iter().partition(|(_, generation, _)| keep(*generation));
        q.done = k;
        g.into_iter().collect()
    }

    /// E2: the released transfers since the last take.
    pub(crate) fn take_released(&self) -> Vec<u64> {
        std::mem::take(&mut lock(&self.q).released)
    }

    /// The host is going away: no more wakes.
    pub(crate) fn detach(&self) {
        *lock(&self.waker) = None;
    }

    /// Something to drain (cheap check at checkpoint start).
    pub(crate) fn pending(&self) -> bool {
        self.wake_pending.load(Ordering::Acquire)
    }

    /// Takes up to `max` completions plus every progress and ready notification.
    #[allow(clippy::type_complexity)]
    pub(crate) fn take(&self, max: usize) -> (Vec<(u64, u64, Outcome)>, Vec<u64>, Vec<u64>, bool) {
        self.wake_pending.store(false, Ordering::Release);
        let mut q = lock(&self.q);
        let n = q.done.len().min(max);
        let done: Vec<_> = q.done.drain(..n).collect();
        let more = !q.done.is_empty();
        (done, std::mem::take(&mut q.progress), std::mem::take(&mut q.ready), more)
    }

    /// Puts unprocessed completions back at the front (budget exhausted) and wakes again.
    pub(crate) fn requeue(&self, rest: Vec<(u64, u64, Outcome)>) {
        if rest.is_empty() {
            return;
        }
        {
            let mut q = lock(&self.q);
            for c in rest.into_iter().rev() {
                q.done.push_front(c);
            }
        }
        self.rewake();
    }

    /// Wakes the backend again for leftover work (even if a wake is pending: it was consumed).
    pub(crate) fn rewake(&self) {
        self.wake_pending.store(false, Ordering::Release);
        self.wake();
    }
}

// ---- spawning ----

fn panic_message(p: &(dyn std::any::Any + Send)) -> Option<String> {
    if p.is::<Cancelled>() {
        return None;
    }
    Some(p.downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| p.downcast_ref::<String>().cloned()).unwrap_or_else(|| "panic".into()))
}

/// A caught panic as the error JS sees (`svelters` / `panic`); a [`WorkCx::abort_if_cancelled`]
/// unwind is a cancellation.
/// Names an offloaded member in messages (`app:<module>.<member>`), formatted only when needed.
#[derive(Clone, Copy)]
pub(crate) struct Label(pub svelters_abi::svl_id, pub &'static str);

impl std::fmt::Display for Label {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", super::schema().kind_name(self.0).unwrap_or_default(), self.1)
    }
}

pub(crate) fn panic_error(p: Box<dyn std::any::Any + Send>, what: &dyn std::fmt::Display) -> NativeError {
    match panic_message(&*p) {
        Some(m) => host_error("panic", format!("{what}: Rust panic: {m}")),
        None => Cancelled.into(),
    }
}

static WORKER_POOL: OnceLock<rayon::ThreadPool> = OnceLock::new();

/// Runs `#[js(worker)]` jobs on `pool` instead of rayon's global pool. Only the first call counts;
/// returns false if a pool was set already.
pub fn set_worker_pool(pool: rayon::ThreadPool) -> bool {
    WORKER_POOL.set(pool).is_ok()
}

fn spawn_worker(f: impl FnOnce() + Send + 'static) {
    match WORKER_POOL.get() {
        Some(p) => p.spawn(f),
        None => rayon::spawn(f),
    }
}

/// Plain threads for `#[js(blocking)]` jobs: grown on demand up to [`BLOCKING_MAX`], idle threads
/// exit after 10 s.
struct BlockingPool {
    st: Mutex<BlockingState>,
    cv: Condvar,
}

#[derive(Default)]
struct BlockingState {
    jobs: VecDeque<Box<dyn FnOnce() + Send>>,
    threads: usize,
    idle: usize,
}

/// The most threads the blocking pool starts.
pub const BLOCKING_MAX: usize = 64;
const BLOCKING_IDLE: Duration = Duration::from_secs(10);

fn blocking_pool() -> &'static BlockingPool {
    static P: OnceLock<BlockingPool> = OnceLock::new();
    P.get_or_init(|| BlockingPool { st: Mutex::new(BlockingState::default()), cv: Condvar::new() })
}

fn spawn_blocking(f: Box<dyn FnOnce() + Send>) {
    let pool = blocking_pool();
    let mut st = lock(&pool.st);
    st.jobs.push_back(f);
    if st.jobs.len() > st.idle && st.threads < BLOCKING_MAX {
        st.threads += 1;
        let n = st.threads;
        drop(st);
        let spawned = std::thread::Builder::new().name(format!("svelters-blocking-{n}")).spawn(blocking_loop);
        if spawned.is_err() {
            lock(&pool.st).threads -= 1;
        }
        return;
    }
    drop(st);
    pool.cv.notify_one();
}

fn blocking_loop() {
    let pool = blocking_pool();
    loop {
        let job = {
            let mut st = lock(&pool.st);
            loop {
                if let Some(j) = st.jobs.pop_front() {
                    break Some(j);
                }
                st.idle += 1;
                let (g, to) = pool.cv.wait_timeout(st, BLOCKING_IDLE).unwrap_or_else(|e| e.into_inner());
                st = g;
                st.idle -= 1;
                if to.timed_out() && st.jobs.is_empty() {
                    st.threads -= 1;
                    break None;
                }
            }
        };
        match job {
            Some(j) => j(),
            None => return,
        }
    }
}

/// Starts a pool job: skipped if cancelled while queued; a panic becomes `NativeError` (`panic`).
pub(crate) fn spawn_pool(exec: Exec, f: Box<dyn FnOnce() -> SendSettle + Send>, op: Arc<OpShared>, what: Label) {
    let run = move || {
        let outcome = if op.is_cancelled() {
            Err(Cancelled.into())
        } else {
            catch_unwind(AssertUnwindSafe(f)).map_err(|p| panic_error(p, &what))
        };
        op.queue.push_done(op.id, op.generation, outcome);
    };
    match exec {
        Exec::Blocking => spawn_blocking(Box::new(run)),
        _ => spawn_worker(run),
    }
}

/// Polls `fut` with panics caught.
pub(crate) struct CatchUnwind<F>(pub F);

impl<F: Future + Unpin> Future for CatchUnwind<F> {
    type Output = std::thread::Result<F::Output>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let inner = &mut self.0;
        match catch_unwind(AssertUnwindSafe(|| Pin::new(inner).poll(cx))) {
            Ok(Poll::Pending) => Poll::Pending,
            Ok(Poll::Ready(v)) => Poll::Ready(Ok(v)),
            Err(p) => Poll::Ready(Err(p)),
        }
    }
}

/// The waker of a JS-thread future: queues its op id and wakes the host.
pub(crate) struct LocalWake {
    pub id: u64,
    pub queue: Arc<Completions>,
}

impl Wake for LocalWake {
    fn wake(self: Arc<Self>) {
        self.queue.push_ready(self.id);
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.queue.push_ready(self.id);
    }
}

/// Polls a JS-thread future once with panics caught.
pub(crate) fn poll_local(fut: &mut LocalFuture, waker: &Waker, what: Label) -> Poll<Result<LocalSettle, NativeError>> {
    let mut cx = Context::from_waker(waker);
    match catch_unwind(AssertUnwindSafe(|| fut.as_mut().poll(&mut cx))) {
        Ok(p) => p,
        Err(p) => Poll::Ready(Err(panic_error(p, &what))),
    }
}

/// Pushes `Err(cancelled)` if a tokio task is dropped before it finished (aborted, or its runtime
/// shut down), so the op's slot is released.
struct DoneGuard {
    op: Arc<OpShared>,
    sent: bool,
}

impl Drop for DoneGuard {
    fn drop(&mut self) {
        if !self.sent {
            self.op.queue.push_done(self.op.id, self.op.generation, Err(Cancelled.into()));
        }
    }
}

#[cfg(feature = "tokio")]
mod tokio_rt {
    use super::*;

    static HANDLE: OnceLock<tokio::runtime::Handle> = OnceLock::new();
    static OWNED: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

    /// Runs `#[js(tokio)]` exports on `handle` (the app's runtime). Only the first call counts.
    pub fn set_tokio_handle(handle: tokio::runtime::Handle) -> bool {
        HANDLE.set(handle).is_ok()
    }

    pub(super) fn handle() -> tokio::runtime::Handle {
        if let Some(h) = HANDLE.get() {
            return h.clone();
        }
        OWNED
            .get_or_init(|| {
                tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(2)
                    .thread_name("svelters-tokio")
                    .enable_all()
                    .build()
                    .expect("svelters: start the tokio runtime for #[js(tokio)] exports")
            })
            .handle()
            .clone()
    }
}

#[cfg(feature = "tokio")]
pub use tokio_rt::set_tokio_handle;

/// Spawns a tokio task; an abort drops the future at its next `.await`.
pub(crate) fn spawn_tokio(fut: SendFuture, op: Arc<OpShared>, what: Label) {
    #[cfg(feature = "tokio")]
    {
        let o = op.clone();
        let task = async move {
            let mut guard = DoneGuard { op: o, sent: false };
            let r = CatchUnwind(fut).await;
            let outcome = r.unwrap_or_else(|p| Err(panic_error(p, &what)));
            guard.sent = true;
            guard.op.queue.push_done(guard.op.id, guard.op.generation, outcome);
        };
        let jh = tokio_rt::handle().spawn(task);
        let abort = jh.abort_handle();
        op.on_cancel(Box::new(move || abort.abort()));
    }
    #[cfg(not(feature = "tokio"))]
    {
        let _ = (fut, what);
        let guard = DoneGuard { op, sent: false };
        drop(guard);
    }
}

// ---- futures for JS-thread async exports ----

struct SpawnState<T> {
    result: Option<std::thread::Result<T>>,
    waker: Option<Waker>,
}

/// A value computed on another thread, as a future (see [`worker`], [`blocking`]). Dropping it
/// does not stop the computation.
pub struct Spawned<T> {
    st: Arc<Mutex<SpawnState<T>>>,
}

impl<T> Future for Spawned<T> {
    type Output = T;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<T> {
        let mut st = lock(&self.st);
        match st.result.take() {
            Some(Ok(v)) => Poll::Ready(v),
            Some(Err(p)) => {
                drop(st);
                std::panic::resume_unwind(p)
            }
            None => {
                st.waker = Some(cx.waker().clone());
                Poll::Pending
            }
        }
    }
}

fn spawned<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static, exec: Exec) -> Spawned<T> {
    let st = Arc::new(Mutex::new(SpawnState { result: None, waker: None }));
    let s = st.clone();
    let run = move || {
        let r = catch_unwind(AssertUnwindSafe(f));
        let w = {
            let mut g = lock(&s);
            g.result = Some(r);
            g.waker.take()
        };
        if let Some(w) = w {
            w.wake();
        }
    };
    match exec {
        Exec::Blocking => spawn_blocking(Box::new(run)),
        _ => spawn_worker(run),
    }
    Spawned { st }
}

/// Runs `f` on the worker pool (rayon) and awaits its result: how a JS-thread `async fn` export
/// offloads CPU work without blocking the UI. A panic in `f` resumes in the awaiting future.
pub fn worker<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Spawned<T> {
    spawned(f, Exec::Worker)
}

/// [`worker`] on the blocking pool (I/O, sleeps).
pub fn blocking<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Spawned<T> {
    spawned(f, Exec::Blocking)
}

// ---- what the macros call ----

/// `#[js(worker)]`/`#[js(blocking)]`: a closure run on a pool; its result converts on the JS thread.
pub fn pool_job<R: IntoJs + Send + 'static>(exec: Exec, f: impl FnOnce() -> R + Send + 'static) -> Job {
    Job::Pool(
        exec,
        Box::new(move || {
            let r = f();
            Box::new(move |cx: &CallCx<'_>| r.into_js(cx)) as SendSettle
        }),
    )
}

/// `async fn` (JS thread).
pub fn local_job<R: IntoJs + 'static>(fut: impl Future<Output = Result<R, NativeError>> + 'static) -> Job {
    Job::Local(Box::pin(async move {
        let r = fut.await?;
        Ok(Box::new(move |cx: &CallCx<'_>| r.into_js(cx)) as LocalSettle)
    }))
}

/// Implemented for `Send + 'static` futures when the `tokio` feature of `svelters` is on.
#[diagnostic::on_unimplemented(
    message = "#[js(tokio)] needs the `tokio` feature of the `svelters` crate, and a `Send + 'static` future",
    note = "enable it with `svelters = {{ ..., features = [\"tokio\"] }}`, or drop #[js(tokio)] to run the async fn on the JS thread"
)]
pub trait TokioFuture: Future + Send + 'static {}
#[cfg(feature = "tokio")]
impl<F: Future + Send + 'static> TokioFuture for F {}

/// `#[js(tokio)] async fn`.
pub fn tokio_job<R: IntoJs + Send + 'static, F: TokioFuture<Output = Result<R, NativeError>>>(fut: F) -> Job {
    Job::Tokio(Box::pin(async move {
        let r = fut.await?;
        Ok(Box::new(move |cx: &CallCx<'_>| r.into_js(cx)) as SendSettle)
    }))
}

/// Offloaded parameters and results cross threads.
#[diagnostic::on_unimplemented(
    message = "`{Self}` can't cross to a worker thread",
    label = "offloaded parameters and results must be `Send + 'static`",
    note = "a UI object (`Obj<T>`, `&T`) or a JS callback stays on the JS thread: pass plain data or a shared object (`Arc<T>`), or use a plain `async fn` (JS thread)"
)]
pub trait WorkerSend {}
impl<T: Send + 'static> WorkerSend for T {}

/// The receiver of a `#[js(worker)]`/`#[js(blocking)]`/`#[js(tokio)]` method.
#[diagnostic::on_unimplemented(
    message = "worker methods on `{Self}` need `#[svelters::object(shared)]` and `Self: Send + Sync`",
    note = "a UI object (`#[svelters::object]`) lives on the JS thread: use a plain `async fn` there, or make the object shared (`Arc<Self>`, interior mutability)"
)]
pub trait OffloadSelf {}
impl<T: super::SharedObject> OffloadSelf for T {}

/// Compile-time checks the generated code makes.
pub const fn assert_worker_send<T: WorkerSend + ?Sized>() {}
pub const fn assert_offload_self<T: OffloadSelf>() {}
