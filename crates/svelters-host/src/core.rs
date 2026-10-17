//! Engine-agnostic host core: handle table, name→ID caches, event queue, timer heap, error and
//! console sinks, op forwarding to the backend, and the checkpoint algorithm.
//!
//! Nothing here names an engine type. An engine reaches the core through the `op_*`, `timer_*`,
//! `console` and `report` methods and is driven through [`JsEngine`].
//!
//! # Re-entrancy
//!
//! `checkpoint` can be called while JS is already running: a backend op issued from JS may open a
//! modal dialog whose nested toolkit loop calls `host.checkpoint`. The core guards re-entry
//! instead of nesting. Every entry into JS (`checkpoint`, bundle load) holds a [`JsGuard`]; a
//! checkpoint requested while a guard is held does no work, sets `deferred`, and returns. When the
//! outermost guard drops it calls `backend.wake()` so the deferred work runs as an ordinary
//! checkpoint from the toolkit loop. No `RefCell` is borrowed across a call into the backend or
//! the engine, so backend callbacks (`post_event`, `on_destroyed`, nested `checkpoint`) cannot
//! double-borrow.

use crate::engine::{ErrorKind, JsEngine, JsError};
use std::cell::{Cell, RefCell};
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet, VecDeque};
use std::rc::Rc;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};
use crate::convert::TypeInfo;
use svelters_abi::{BackendRef, MemberDesc, Value, WakeHandle, svl_handle, svl_id};

mod ops;
pub(crate) use ops::{Name, OpError, OpKind};

#[derive(Clone, Debug)]
pub struct HostConfig {
    /// QuickJS heap limit in bytes. 0 means unlimited.
    pub memory_limit: usize,
    /// JS stack limit in bytes.
    pub max_stack_size: usize,
    /// Wall-clock budget for one entry into JS (bundle load, one checkpoint step). The interrupt
    /// handler stops scripts that exceed it. `None` disables the time limit.
    pub turn_budget: Option<Duration>,
    /// Jobs (promise reactions) run per checkpoint before the host yields and calls `wake`.
    pub max_jobs_per_checkpoint: usize,
    /// Echo `console.*` and reported errors to stderr.
    pub echo_console: bool,
    pub echo_errors: bool,
    /// Scripted mode for op-log fixtures, matching the Node TestHost's stepping model:
    /// - timers run on a virtual clock and never wake the backend;
    /// - a checkpoint with queued events only delivers them (no job drain, no timers);
    /// - a checkpoint with no queued events drains jobs and fires the earliest timer, repeating
    ///   until both are empty (at most `max_virtual_timer_fires` timer firings);
    /// - loading a bundle drains nothing.
    pub scripted: bool,
    pub max_virtual_timer_fires: usize,
    /// Accumulate [`Timing`] counters (two clock reads per host op and per entry into JS).
    pub timing: bool,
}

impl Default for HostConfig {
    fn default() -> Self {
        HostConfig {
            memory_limit: 256 * 1024 * 1024,
            max_stack_size: 1024 * 1024,
            turn_budget: Some(Duration::from_secs(10)),
            max_jobs_per_checkpoint: 10_000,
            echo_console: true,
            echo_errors: true,
            scripted: false,
            max_virtual_timer_fires: 10_000,
            timing: false,
        }
    }
}

/// Time split counters, enabled by [`HostConfig::timing`].
///
/// `js_ns` is the wall time spent inside entries into JS (bundle load, checkpoints). `ops_ns` is
/// the part of that spent inside host ops (marshalling, handle table, name resolution and the
/// backend call itself), so JS-only time is `js_ns - ops_ns`; subtract the backend's own
/// toolkit time from `ops_ns` to get host/ABI time.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Timing {
    pub js_ns: u64,
    pub ops_ns: u64,
    pub ops: u64,
    pub checkpoints: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConsoleLine {
    pub level: String,
    pub text: String,
}

struct Event {
    handle: svl_handle,
    signal: svl_id,
    args: Vec<Value>,
}

/// An event with its signal name resolved, ready for `__svelters_dispatch`.
pub(crate) struct DispatchEvent {
    pub handle: svl_handle,
    pub signal: String,
    pub args: Vec<Value>,
}

#[derive(Default)]
struct Timers {
    heap: BinaryHeap<Reverse<(Instant, u64, u32)>>,
    live: HashMap<u32, (u64, Option<Duration>)>,
    seq: u64,
}

impl Timers {
    fn set(&mut self, id: u32, delay: Duration, repeat: bool, now: Instant) {
        self.seq += 1;
        let interval = repeat.then_some(delay);
        self.live.insert(id, (self.seq, interval));
        self.heap.push(Reverse((now + delay, self.seq, id)));
    }

    fn next_deadline(&mut self) -> Option<Instant> {
        while let Some(Reverse((d, seq, id))) = self.heap.peek().copied() {
            if self.live.get(&id).map(|l| l.0) == Some(seq) {
                return Some(d);
            }
            self.heap.pop();
        }
        None
    }

    fn take_due(&mut self, now: Instant) -> Vec<(u32, u64)> {
        let mut due = Vec::new();
        while let Some(Reverse((d, seq, id))) = self.heap.peek().copied() {
            if d > now {
                break;
            }
            self.heap.pop();
            if self.live.get(&id).map(|l| l.0) == Some(seq) {
                due.push((id, seq));
            }
        }
        due
    }

    /// Marks `(id, seq)` as firing. Returns false if the timer was cleared or replaced since
    /// `take_due`. Intervals are rescheduled; one-shots are dropped.
    fn begin_fire(&mut self, id: u32, seq: u64, now: Instant) -> bool {
        match self.live.get(&id).copied() {
            Some((s, interval)) if s == seq => {
                match interval {
                    Some(iv) => {
                        self.seq += 1;
                        self.live.insert(id, (self.seq, interval));
                        self.heap.push(Reverse((now + iv, self.seq, id)));
                    }
                    None => {
                        self.live.remove(&id);
                    }
                }
                true
            }
            _ => false,
        }
    }
}

struct SchedState {
    deadline: Option<Instant>,
    gen_: u64,
    fired_gen: u64,
    shutdown: bool,
}

/// Timer thread state. The thread sleeps until the earliest timer deadline and calls the
/// backend's thread-safe `wake` once per deadline update, so the host never polls.
pub(crate) struct Sched {
    m: Mutex<SchedState>,
    cv: Condvar,
}

impl Sched {
    pub(crate) fn new() -> Arc<Sched> {
        Arc::new(Sched {
            m: Mutex::new(SchedState {
                deadline: None,
                gen_: 0,
                fired_gen: 0,
                shutdown: false,
            }),
            cv: Condvar::new(),
        })
    }

    fn lock(&self) -> MutexGuard<'_, SchedState> {
        self.m.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn set(&self, deadline: Option<Instant>, force: bool) {
        let mut g = self.lock();
        if force || g.deadline != deadline {
            g.deadline = deadline;
            g.gen_ += 1;
            self.cv.notify_all();
        }
    }

    pub(crate) fn shutdown(&self) {
        self.lock().shutdown = true;
        self.cv.notify_all();
    }

    pub(crate) fn run(&self, wake: WakeHandle) {
        let mut g = self.lock();
        loop {
            if g.shutdown {
                return;
            }
            match g.deadline {
                Some(d) if g.fired_gen != g.gen_ => {
                    let now = Instant::now();
                    if now >= d {
                        g.fired_gen = g.gen_;
                        drop(g);
                        wake.wake();
                        g = self.lock();
                    } else {
                        g = self
                            .cv
                            .wait_timeout(g, d - now)
                            .unwrap_or_else(|e| e.into_inner())
                            .0;
                    }
                }
                _ => g = self.cv.wait(g).unwrap_or_else(|e| e.into_inner()),
            }
        }
    }
}

#[derive(Default)]
struct CoreState {
    /// handle → kind id
    handles: HashMap<svl_handle, svl_id>,
    kinds: HashMap<String, svl_id>,
    /// kind id → name, for error messages
    kind_names: HashMap<svl_id, String>,
    /// (name kind, scope kind) → name → id
    names: HashMap<(u32, svl_id), HashMap<Box<str>, svl_id>>,
    /// Descriptor caches: `None` records that the backend has no descriptor.
    members: HashMap<(u32, svl_id, svl_id), Option<Rc<MemberDesc>>>,
    types: HashMap<svl_id, Option<Rc<TypeInfo>>>,
    /// Live boxed-value handles, and ones the JS side dropped (released at the next checkpoint).
    boxed: HashSet<svl_handle>,
    release_queue: Vec<svl_handle>,
    /// (kind id, signal id) → name, filled by `listen`
    signal_names: HashMap<(svl_id, svl_id), String>,
    events: VecDeque<Event>,
    timers: Timers,
    console: Vec<ConsoleLine>,
    errors: Vec<JsError>,
}

pub(crate) struct Core {
    backend: BackendRef<'static>,
    pub(crate) cfg: HostConfig,
    st: RefCell<CoreState>,
    busy: Cell<u32>,
    deferred: Cell<bool>,
    pub(crate) sched: Arc<Sched>,
    pub(crate) interrupt: Arc<AtomicBool>,
    epoch: Instant,
    vnow: Cell<Duration>,
    timing: Cell<Timing>,
    checkpoints: Cell<u64>,
}

/// Held while JS runs. See the module docs on re-entrancy.
pub(crate) struct JsGuard<'a>(&'a Core, Option<Instant>);

impl Drop for JsGuard<'_> {
    fn drop(&mut self) {
        let c = self.0;
        if let Some(t0) = self.1 {
            let mut t = c.timing.get();
            t.js_ns += t0.elapsed().as_nanos() as u64;
            c.timing.set(t);
        }
        c.busy.set(c.busy.get() - 1);
        if c.busy.get() == 0 && c.deferred.replace(false) {
            c.backend.wake();
        }
    }
}

impl Core {
    pub(crate) fn new(backend: BackendRef<'static>, cfg: HostConfig, sched: Arc<Sched>) -> Core {
        Core {
            backend,
            cfg,
            st: RefCell::new(CoreState::default()),
            busy: Cell::new(0),
            deferred: Cell::new(false),
            sched,
            interrupt: Arc::new(AtomicBool::new(false)),
            epoch: Instant::now(),
            vnow: Cell::new(Duration::ZERO),
            timing: Cell::new(Timing::default()),
            checkpoints: Cell::new(0),
        }
    }

    pub(crate) fn take_timing(&self) -> Timing {
        self.timing.replace(Timing::default())
    }

    pub(crate) fn checkpoint_count(&self) -> u64 {
        self.checkpoints.get()
    }

    /// Runs one host op, adding its duration to the timing counters when enabled.
    fn timed<R>(&self, f: impl FnOnce() -> R) -> R {
        if !self.cfg.timing {
            return f();
        }
        let t0 = Instant::now();
        let r = f();
        let mut t = self.timing.get();
        t.ops_ns += t0.elapsed().as_nanos() as u64;
        t.ops += 1;
        self.timing.set(t);
        r
    }

    /// True when nothing is waiting for the host: no queued events and no live timers.
    pub(crate) fn is_idle(&self) -> bool {
        let st = self.st.borrow();
        self.busy.get() == 0 && st.events.is_empty() && st.timers.live.is_empty()
    }

    /// The timer clock: wall time, or the virtual clock in scripted mode.
    pub(crate) fn now(&self) -> Instant {
        if self.cfg.scripted {
            self.epoch + self.vnow.get()
        } else {
            Instant::now()
        }
    }

    pub(crate) fn backend(&self) -> BackendRef<'static> {
        self.backend
    }

    /// Enters JS. `None` if JS is already running (re-entry); the request is then recorded as
    /// deferred.
    pub(crate) fn enter(&self) -> Option<JsGuard<'_>> {
        if self.busy.get() > 0 {
            self.deferred.set(true);
            return None;
        }
        self.busy.set(1);
        Some(JsGuard(self, self.cfg.timing.then(Instant::now)))
    }

    // ---- sinks ----

    pub(crate) fn report(&self, err: JsError) {
        if self.cfg.echo_errors {
            eprintln!("[svelters] {err}");
        }
        self.st.borrow_mut().errors.push(err);
    }

    pub(crate) fn report_all(&self, errs: Vec<JsError>) {
        for e in errs {
            self.report(e);
        }
    }

    pub(crate) fn console(&self, level: &str, text: String) {
        if self.cfg.echo_console {
            eprintln!("[{level}] {text}");
        }
        self.st.borrow_mut().console.push(ConsoleLine {
            level: level.to_string(),
            text,
        });
    }

    pub(crate) fn take_errors(&self) -> Vec<JsError> {
        std::mem::take(&mut self.st.borrow_mut().errors)
    }

    pub(crate) fn take_console(&self) -> Vec<ConsoleLine> {
        std::mem::take(&mut self.st.borrow_mut().console)
    }

    // ---- events ----

    pub(crate) fn push_event(&self, handle: svl_handle, signal: svl_id, args: Vec<Value>) {
        self.st.borrow_mut().events.push_back(Event {
            handle,
            signal,
            args,
        });
    }

    pub(crate) fn has_events(&self) -> bool {
        !self.st.borrow().events.is_empty()
    }

    /// Drains the queue, resolving signal ids to names. Events that cannot be mapped are
    /// reported and dropped.
    pub(crate) fn take_events(&self) -> Vec<DispatchEvent> {
        let events: Vec<Event> = self.st.borrow_mut().events.drain(..).collect();
        let mut out = Vec::new();
        for e in events {
            let name = {
                let st = self.st.borrow();
                st.handles
                    .get(&e.handle)
                    .and_then(|k| st.signal_names.get(&(*k, e.signal)))
                    .cloned()
            };
            match name {
                Some(signal) => out.push(DispatchEvent {
                    handle: e.handle,
                    signal,
                    args: e.args,
                }),
                None => self.report(JsError::new(
                    ErrorKind::Host,
                    format!(
                        "dropped event for handle {} signal id {}: no listener or stale handle",
                        e.handle, e.signal
                    ),
                )),
            }
        }
        out
    }

    /// The backend reports that a native object went away.
    pub(crate) fn forget_handle(&self, h: svl_handle) {
        self.st.borrow_mut().handles.remove(&h);
    }

    // ---- timers ----

    pub(crate) fn timer_set(&self, id: u32, delay: Duration, repeat: bool) {
        let now = self.now();
        // Wall-clock timers have a 1 ms floor; the virtual clock orders by exact due time.
        let delay = if self.cfg.scripted {
            delay
        } else {
            delay.max(Duration::from_millis(1))
        };
        let deadline = {
            let mut st = self.st.borrow_mut();
            st.timers.set(id, delay, repeat, now);
            st.timers.next_deadline()
        };
        self.arm_sched(deadline, false);
    }

    fn arm_sched(&self, deadline: Option<Instant>, force: bool) {
        if !self.cfg.scripted {
            self.sched.set(deadline, force);
        }
    }

    pub(crate) fn timer_clear(&self, id: u32) {
        let deadline = {
            let mut st = self.st.borrow_mut();
            st.timers.live.remove(&id);
            st.timers.next_deadline()
        };
        self.arm_sched(deadline, false);
    }

    fn timers_take_due(&self, now: Instant) -> Vec<(u32, u64)> {
        self.st.borrow_mut().timers.take_due(now)
    }

    fn timer_begin_fire(&self, id: u32, seq: u64, now: Instant) -> bool {
        self.st.borrow_mut().timers.begin_fire(id, seq, now)
    }

    fn sync_sched(&self) {
        let d = self.st.borrow_mut().timers.next_deadline();
        self.arm_sched(d, true);
    }

    fn next_timer_deadline(&self) -> Option<Instant> {
        self.st.borrow_mut().timers.next_deadline()
    }

    fn advance_clock_to(&self, t: Instant) {
        let off = t.saturating_duration_since(self.epoch);
        if off > self.vnow.get() {
            self.vnow.set(off);
        }
    }
}

/// Runs one checkpoint (see "Checkpoint semantics" in `backend-abi.md`):
/// 1. deliver queued events to JS, draining jobs after each;
/// 2. fire due timers, draining jobs after each;
/// 3. drain remaining jobs, up to `max_jobs_per_checkpoint` in total;
/// 4. `wake` if jobs or events remain, and re-arm the timer thread.
///
/// Ops reach the backend directly while JS runs (no batching in P). Scripted mode has its own
/// stepping, see [`HostConfig::scripted`].
pub(crate) fn checkpoint(core: &Core, engine: &RefCell<Box<dyn JsEngine>>) {
    let Some(_guard) = core.enter() else { return };
    core.checkpoints.set(core.checkpoints.get() + 1);
    if core.cfg.timing {
        let mut t = core.timing.get();
        t.checkpoints += 1;
        core.timing.set(t);
    }
    let mut eng = engine.borrow_mut();
    if core.cfg.scripted {
        checkpoint_scripted(core, &mut eng);
        core.process_releases();
        return;
    }
    let mut budget = core.cfg.max_jobs_per_checkpoint;
    let mut pending = false;

    let drain = |eng: &mut Box<dyn JsEngine>, budget: &mut usize| {
        let st = eng.run_jobs(*budget);
        *budget = budget.saturating_sub(st.ran);
        core.report_all(eng.take_errors());
        st.pending
    };

    for ev in core.take_events() {
        if let Err(e) = eng.dispatch(ev.handle, &ev.signal, &ev.args) {
            core.report(e);
        }
        pending |= drain(&mut eng, &mut budget);
    }

    let now = core.now();
    for (id, seq) in core.timers_take_due(now) {
        if !core.timer_begin_fire(id, seq, now) {
            continue;
        }
        if let Err(e) = eng.fire_timer(id) {
            core.report(e);
        }
        pending |= drain(&mut eng, &mut budget);
    }

    pending |= drain(&mut eng, &mut budget);
    drop(eng);

    // GC'd boxed values were only queued by the finalization callbacks: release them here,
    // outside any entry into JS.
    core.process_releases();
    core.sync_sched();
    if pending || core.has_events() {
        core.backend.wake();
    }
}

/// Scripted-mode checkpoint; see [`HostConfig::scripted`].
fn checkpoint_scripted(core: &Core, eng: &mut Box<dyn JsEngine>) {
    const MAX_DRAIN_ROUNDS: usize = 100;
    let events = core.take_events();
    if !events.is_empty() {
        for ev in events {
            if let Err(e) = eng.dispatch(ev.handle, &ev.signal, &ev.args) {
                core.report(e);
            }
            core.report_all(eng.take_errors());
        }
        return;
    }

    let drain = |eng: &mut Box<dyn JsEngine>| {
        for _ in 0..MAX_DRAIN_ROUNDS {
            let st = eng.run_jobs(core.cfg.max_jobs_per_checkpoint);
            core.report_all(eng.take_errors());
            if !st.pending {
                return;
            }
        }
        core.report(JsError::new(ErrorKind::Host, "job queue did not drain"));
    };

    let mut fires = 0;
    drain(eng);
    while let Some(deadline) = core.next_timer_deadline() {
        core.advance_clock_to(deadline);
        let now = core.now();
        for (id, seq) in core.timers_take_due(now) {
            if !core.timer_begin_fire(id, seq, now) {
                continue;
            }
            fires += 1;
            if let Err(e) = eng.fire_timer(id) {
                core.report(e);
            }
            drain(eng);
        }
        if fires >= core.cfg.max_virtual_timer_fires {
            core.report(JsError::new(
                ErrorKind::Host,
                "virtual clock: timers did not quiesce (live interval?)",
            ));
            break;
        }
    }
}

/// Loads the entry module under the re-entry guard, then drains jobs.
pub(crate) fn load_bundle(
    core: &Core,
    engine: &RefCell<Box<dyn JsEngine>>,
    entry: &std::path::Path,
) -> Result<(), JsError> {
    let Some(_guard) = core.enter() else {
        return Err(JsError::new(
            ErrorKind::Host,
            "load_bundle called while JS is running",
        ));
    };
    let mut eng = engine.borrow_mut();
    let res = eng.load_module(entry);
    let mut pending = false;
    if res.is_ok() && !core.cfg.scripted {
        let st = eng.run_jobs(core.cfg.max_jobs_per_checkpoint);
        pending = st.pending;
    }
    core.report_all(eng.take_errors());
    drop(eng);
    core.sync_sched();
    if !core.cfg.scripted && (pending || core.has_events()) {
        core.backend.wake();
    }
    res
}

/// Runs queued jobs to exhaustion (no timers, no events). For harnesses that need a final
/// flush after the backend loop returns.
pub(crate) fn drain_jobs(core: &Core, engine: &RefCell<Box<dyn JsEngine>>) {
    let Some(_guard) = core.enter() else { return };
    let mut eng = engine.borrow_mut();
    for _ in 0..100 {
        let st = eng.run_jobs(core.cfg.max_jobs_per_checkpoint);
        core.report_all(eng.take_errors());
        if !st.pending {
            return;
        }
    }
    core.report(JsError::new(ErrorKind::Host, "job queue did not drain"));
}
