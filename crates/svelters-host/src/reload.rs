//! Hot reload: watch the bundle file and swap the running app for the new build without leaving
//! the process or the toolkit loop.
//!
//! A watcher thread (inotify through `notify`) debounces file events, sets a flag and calls the
//! backend's thread-safe `wake`. The next checkpoint, on the toolkit thread, sees the flag and
//! reloads before doing any other work:
//!
//! 1. Set aside the old toplevels and the old runtime's timers and drop queued events.
//! 2. Create a fresh JS runtime and load the bundle into it. The new app creates and presents its
//!    windows while the old ones are still up, so the toolkit loop never sees zero windows.
//! 3. If the load succeeds, drop the old runtime and destroy the old toplevels with the ordinary
//!    destroy op, descendants first.
//! 4. If it fails (syntax error, throw during mount), destroy whatever the failed load created,
//!    put the previous runtime and its timers back, print the source-mapped error and keep
//!    running the previous version until the next change.
//!
//! This is a full reload: all JS state is lost. A reload never invokes Cargo.

use crate::core::{self, Core, Timers};
use crate::engine::{ErrorKind, JsEngine, JsError};
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use svelters_abi::WakeHandle;

/// Quiet time after the last file event before a reload starts. Vite writes the bundle and its
/// map as separate writes, and editors may write twice.
const DEBOUNCE: Duration = Duration::from_millis(120);

/// What one reload attempt did.
#[derive(Clone, Debug)]
pub struct ReloadReport {
    /// 1 for the first reload of this host, counting failures.
    pub generation: u64,
    pub ok: bool,
    /// Load and teardown time inside the host (not the build, not the toolkit's repaint).
    pub elapsed: Duration,
    /// The load error, already mapped to original sources. `None` on success.
    pub error: Option<JsError>,
}

pub(crate) type SharedEngine = Rc<RefCell<Box<dyn JsEngine>>>;
pub(crate) type MakeEngine = Rc<dyn Fn(&Rc<Core>) -> Result<Box<dyn JsEngine>, JsError>>;

pub(crate) struct Reloader {
    make: MakeEngine,
    pending: Arc<AtomicBool>,
    bundle: RefCell<Option<PathBuf>>,
    watch: RefCell<Option<Watch>>,
    hook: RefCell<Option<Rc<dyn Fn(&ReloadReport)>>>,
    generation: Cell<u64>,
}

impl Reloader {
    pub(crate) fn new(make: MakeEngine) -> Rc<Reloader> {
        Rc::new(Reloader {
            make,
            pending: Arc::new(AtomicBool::new(false)),
            bundle: RefCell::new(None),
            watch: RefCell::new(None),
            hook: RefCell::new(None),
            generation: Cell::new(0),
        })
    }

    pub(crate) fn set_bundle(&self, path: &Path) {
        *self.bundle.borrow_mut() = Some(std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf()));
    }

    pub(crate) fn set_hook(&self, f: Rc<dyn Fn(&ReloadReport)>) {
        *self.hook.borrow_mut() = Some(f);
    }

    /// Requests a reload at the next checkpoint (same flag the watcher thread sets).
    pub(crate) fn request(&self) {
        self.pending.store(true, Ordering::SeqCst);
    }

    pub(crate) fn take_pending(&self) -> bool {
        self.pending.swap(false, Ordering::SeqCst)
    }

    /// Starts watching the bundle recorded by `set_bundle`. The thread only sets the flag and
    /// calls `wake`.
    pub(crate) fn start_watch(&self, wake: WakeHandle) -> Result<(), JsError> {
        let host_err = |m: String| JsError::new(ErrorKind::Host, m);
        let bundle = self
            .bundle
            .borrow()
            .clone()
            .ok_or_else(|| host_err("watch: load a bundle first".into()))?;
        let dir = bundle.parent().ok_or_else(|| host_err("watch: bundle has no directory".into()))?.to_path_buf();
        let name = bundle.file_name().ok_or_else(|| host_err("watch: bundle has no file name".into()))?.to_os_string();
        let mut map_name: OsString = name.clone();
        map_name.push(".map");

        let (tx, rx) = mpsc::channel::<()>();
        let mut watcher: RecommendedWatcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            let Ok(ev) = res else { return };
            if !matches!(ev.kind, EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)) {
                return;
            }
            if ev.paths.iter().any(|p| p.file_name().is_some_and(|n| n == name || n == map_name)) {
                let _ = tx.send(());
            }
        })
        .map_err(|e| host_err(format!("watch {}: {e}", dir.display())))?;
        watcher
            .watch(&dir, RecursiveMode::NonRecursive)
            .map_err(|e| host_err(format!("watch {}: {e}", dir.display())))?;

        let pending = self.pending.clone();
        let thread = std::thread::Builder::new()
            .name("svelters-watch".into())
            .spawn(move || {
                while rx.recv().is_ok() {
                    loop {
                        match rx.recv_timeout(DEBOUNCE) {
                            Ok(()) => {}
                            Err(RecvTimeoutError::Timeout) => break,
                            Err(RecvTimeoutError::Disconnected) => return,
                        }
                    }
                    pending.store(true, Ordering::SeqCst);
                    wake.wake();
                }
            })
            .map_err(|e| host_err(format!("spawn watch thread: {e}")))?;
        *self.watch.borrow_mut() = Some(Watch { watcher: Some(watcher), thread: Some(thread) });
        Ok(())
    }

    /// Stops the watcher thread (joins it, so the backend may be dropped afterwards).
    pub(crate) fn stop_watch(&self) {
        self.watch.borrow_mut().take();
    }

    /// Swaps the running app for a fresh load of the bundle. Must not be called while JS runs.
    pub(crate) fn reload(&self, core: &Rc<Core>, engine: &SharedEngine) -> Option<ReloadReport> {
        let path = self.bundle.borrow().clone()?;
        // A build that clears its output directory first is mid-write: the file's reappearance is
        // another event, so wait for it instead of reporting a failure.
        if !path.is_file() {
            return None;
        }
        let t0 = Instant::now();
        let generation = self.generation.get() + 1;
        self.generation.set(generation);

        let old_roots: HashSet<u64> = core.roots().into_iter().collect();
        core.clear_events();
        let old_timers = core.swap_timers(Timers::default());

        let loaded = (self.make)(core).and_then(|fresh| {
            let old_engine = std::mem::replace(&mut *engine.borrow_mut(), fresh);
            match core::load_bundle(core, engine, &path) {
                Ok(()) => Ok(old_engine),
                Err(e) => {
                    *engine.borrow_mut() = old_engine;
                    Err(e)
                }
            }
        });

        let error = match loaded {
            Ok(old_engine) => {
                drop(old_engine);
                drop(old_timers);
                for h in old_roots {
                    core.destroy_subtree(h);
                }
                None
            }
            Err(e) => {
                for h in core.roots() {
                    if !old_roots.contains(&h) {
                        core.destroy_subtree(h);
                    }
                }
                drop(core.swap_timers(old_timers));
                let e = core.map_error(e);
                core.report(e.clone());
                Some(e)
            }
        };

        let report = ReloadReport { generation, ok: error.is_none(), elapsed: t0.elapsed(), error };
        if core.cfg.echo_console {
            if report.ok {
                eprintln!(
                    "[svelters] reloaded {} in {:.1} ms (#{generation})",
                    path.display(),
                    report.elapsed.as_secs_f64() * 1e3
                );
            } else {
                eprintln!("[svelters] reload #{generation} failed; the previous version keeps running until the next change");
            }
        }
        let hook = self.hook.borrow().clone();
        if let Some(h) = hook {
            h(&report);
        }
        Some(report)
    }
}

struct Watch {
    watcher: Option<RecommendedWatcher>,
    thread: Option<JoinHandle<()>>,
}

impl Drop for Watch {
    fn drop(&mut self) {
        // Dropping the watcher drops the event closure and with it the channel sender, which ends
        // the thread's loop.
        drop(self.watcher.take());
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}
