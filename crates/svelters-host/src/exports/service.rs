//! Services (E3, `docs/notes/egui-rust-exports.md` §6.4, §8): long-lived Rust work that starts once per
//! process and feeds JS through a store or an object it returns.
//!
//! ```ignore
//! #[svelters::service(module = "telemetry")]                 // JS: import { telemetry } from 'svelters:app/telemetry'
//! fn telemetry(cx: ServiceCx) -> Store<Telemetry> {
//!     let store = Store::new(Telemetry::default());
//!     let s = store.clone();
//!     cx.thread(move |stop| while !stop.wait(Duration::from_millis(50)) {
//!         s.update(|t| t.samples += 1);
//!     });
//!     store
//! }
//! ```
//!
//! - **Start:** every service starts at app start: when the process creates its first [`crate::Host`]
//!   (or at [`start_services`], called earlier by an app that wants it). `#[svelters::service(lazy)]`
//!   starts on first use instead: when JS first imports its module.
//! - **Hot reload** never touches a service: the new JS runtime gets the same Rust value.
//! - **Stop:** [`shutdown_services`] (the app calls it after its loop returns): fires every service's
//!   [`StopToken`], runs the `on_stop` hooks, aborts its tokio tasks and joins its threads with a timeout.
//!
//! The service function runs once, on the thread that starts it; it must return quickly and leave the
//! long-running part to [`ServiceCx::thread`] (or [`ServiceCx::spawn`] with the `tokio` feature).

use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Set when the services are told to stop. Cheap to clone; `Send + Sync`.
#[derive(Clone, Default)]
pub struct StopToken {
    inner: Arc<(Mutex<bool>, Condvar)>,
}

impl StopToken {
    /// True once the services are stopping.
    pub fn is_stopped(&self) -> bool {
        *lock(&self.inner.0)
    }

    /// Sleeps for `d` or until the stop, whichever comes first: true if stopped (a service loop's tick).
    pub fn wait(&self, d: Duration) -> bool {
        let end = Instant::now() + d;
        let mut g = lock(&self.inner.0);
        while !*g {
            let left = end.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return false;
            }
            g = self.inner.1.wait_timeout(g, left).unwrap_or_else(|e| e.into_inner()).0;
        }
        true
    }

    fn stop(&self) {
        *lock(&self.inner.0) = true;
        self.inner.1.notify_all();
    }
}

struct Running {
    name: String,
    stop: StopToken,
    threads: Vec<(String, JoinHandle<()>)>,
    hooks: Vec<Box<dyn FnOnce() + Send>>,
}

struct Registry {
    started: bool,
    stopped: bool,
    running: Vec<Arc<Mutex<Running>>>,
}

fn registry() -> &'static Mutex<Registry> {
    static R: OnceLock<Mutex<Registry>> = OnceLock::new();
    R.get_or_init(|| Mutex::new(Registry { started: false, stopped: false, running: Vec::new() }))
}

/// What a service function gets: where to put its long-running work so that shutdown can stop it.
pub struct ServiceCx {
    run: Arc<Mutex<Running>>,
    stop: StopToken,
}

impl ServiceCx {
    /// `module/name` of the service.
    pub fn name(&self) -> String {
        lock(&self.run).name.clone()
    }

    /// The token shutdown fires.
    pub fn stop_token(&self) -> StopToken {
        self.stop.clone()
    }

    /// Runs `f` on a named thread of its own; shutdown fires the token it gets and joins the thread.
    pub fn thread(&self, f: impl FnOnce(StopToken) + Send + 'static) {
        let stop = self.stop.clone();
        let name = format!("svelters-service-{}", lock(&self.run).name.replace('/', "-"));
        let h = std::thread::Builder::new().name(name.clone()).spawn(move || f(stop)).expect("spawn a service thread");
        lock(&self.run).threads.push((name, h));
    }

    /// Runs `f` at shutdown (after the token fired, before the threads are joined).
    pub fn on_stop(&self, f: impl FnOnce() + Send + 'static) {
        lock(&self.run).hooks.push(Box::new(f));
    }

    /// Spawns `fut` on tokio (the handle given to `set_tokio_handle`, else a small runtime of ours);
    /// shutdown aborts it.
    #[cfg(feature = "tokio")]
    pub fn spawn(&self, fut: impl std::future::Future<Output = ()> + Send + 'static) {
        let h = super::offload::tokio_handle().spawn(fut);
        self.on_stop(move || h.abort());
    }
}

/// The slot behind one `#[svelters::service]`: the value it returned, made once per process.
pub struct ServiceSlot<V> {
    value: OnceLock<V>,
}

impl<V> Default for ServiceSlot<V> {
    fn default() -> Self {
        ServiceSlot::new()
    }
}

impl<V> ServiceSlot<V> {
    pub const fn new() -> ServiceSlot<V> {
        ServiceSlot { value: OnceLock::new() }
    }

    /// The service's value, starting the service if it has not run yet.
    pub fn get_or_start(&self, module: &str, name: &str, f: fn(ServiceCx) -> V) -> &V {
        self.value.get_or_init(|| {
            let stop = StopToken::default();
            let run = Arc::new(Mutex::new(Running { name: format!("{module}/{name}"), stop: stop.clone(), threads: Vec::new(), hooks: Vec::new() }));
            {
                let mut r = lock(registry());
                if r.stopped {
                    // Started after shutdown (a late import): stopped from the start.
                    stop.stop();
                }
                r.running.push(run.clone());
            }
            f(ServiceCx { run, stop })
        })
    }

    /// Whether the service has started.
    pub fn is_started(&self) -> bool {
        self.value.get().is_some()
    }
}

/// One `#[svelters::service]`, as the macro registers it.
pub struct ServiceDef {
    pub module: &'static str,
    pub name: &'static str,
    /// `#[svelters::service(lazy)]`: started on first use, not at app start.
    pub lazy: bool,
    /// Starts it (idempotent).
    pub start: fn(),
}

inventory::collect!(ServiceDef);

/// Starts every service that is not `lazy` (once per process; later calls do nothing). The host calls it
/// when the process creates its first `Host`.
pub fn start_services() {
    {
        let mut r = lock(registry());
        if r.started {
            return;
        }
        r.started = true;
    }
    let mut defs: Vec<&ServiceDef> = inventory::iter::<ServiceDef>.into_iter().filter(|d| !d.lazy).collect();
    defs.sort_by_key(|d| (d.module, d.name));
    for d in defs {
        (d.start)();
    }
}

/// What [`shutdown_services`] did.
#[derive(Debug, Default)]
pub struct ShutdownReport {
    /// Services that were running.
    pub services: usize,
    /// Threads joined in time.
    pub joined: usize,
    /// Threads still running at the timeout (left detached).
    pub timed_out: Vec<String>,
}

/// Stops every started service: fires the stop tokens, runs the `on_stop` hooks (tokio tasks are
/// aborted), and joins the service threads, waiting at most `timeout` in all. Services never stop at a
/// hot reload; call this when the app shuts down, after the toolkit loop returned.
pub fn shutdown_services(timeout: Duration) -> ShutdownReport {
    let running = {
        let mut r = lock(registry());
        r.stopped = true;
        std::mem::take(&mut r.running)
    };
    let mut report = ShutdownReport { services: running.len(), ..Default::default() };
    let mut threads = Vec::new();
    for run in &running {
        let (stop, hooks, ts) = {
            let mut g = lock(run);
            (g.stop.clone(), std::mem::take(&mut g.hooks), std::mem::take(&mut g.threads))
        };
        stop.stop();
        for h in hooks {
            h();
        }
        threads.extend(ts);
    }
    let end = Instant::now() + timeout;
    for (name, t) in threads {
        while !t.is_finished() && Instant::now() < end {
            std::thread::sleep(Duration::from_millis(1));
        }
        if t.is_finished() {
            let _ = t.join();
            report.joined += 1;
        } else {
            report.timed_out.push(name);
        }
    }
    report
}

/// `module/name` of the services started so far, in start order (diagnostics, tests).
pub fn running_services() -> Vec<String> {
    lock(registry()).running.iter().map(|r| lock(r).name.clone()).collect()
}
