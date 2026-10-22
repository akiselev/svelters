//! Hot reload: watch the app's files and swap the running app for the new version without leaving
//! the process or the toolkit loop.
//!
//! The module provider names the files to watch (Q1): a bundle and its map, or the module graph
//! of an app loaded from source plus its manifests. A watcher thread (inotify through `notify`)
//! on their directories keeps the events for those files, debounces them, sets a flag and calls
//! the backend's thread-safe `wake`. The next checkpoint, on the toolkit thread, sees the flag,
//! asks the provider whether the changed files affect the app (`invalidate`) and reloads before
//! doing any other work:
//!
//! 1. Set aside the old toplevels and the old runtime's timers and drop queued events.
//! 2. Create a fresh JS runtime and load the app into it (unchanged modules come from the
//!    provider's transform cache). The new app creates and presents its
//!    windows while the old ones are still up, so the toolkit loop never sees zero windows.
//! 3. If the load succeeds, drop the old runtime and destroy the old toplevels with the ordinary
//!    destroy op, descendants first.
//! 4. If it fails (syntax error, throw during mount), destroy whatever the failed load created,
//!    put the previous runtime and its timers back, print the source-mapped error and keep
//!    running the previous version until the next change.
//!
//! This is a full reload: all JS state is lost. A reload never invokes Cargo. After each attempt the
//! watched set follows the new module graph; after a failed one, any change in a watched directory
//! retries (the fix may be a file the graph does not reach yet).

use crate::core::{self, Core, Timers};
use crate::engine::{ErrorKind, HotOutcome, JsEngine, JsError, ModuleProvider};
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
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
    /// H1: the modules swapped in place (no runtime was replaced). `None` for a full reload.
    pub hot: Option<Vec<String>>,
    /// H1: why a full reload ran although the app supports in-place updates (no accepting module,
    /// the swap budget, ...).
    pub fallback: Option<String>,
}

pub(crate) type SharedEngine = Rc<RefCell<Box<dyn JsEngine>>>;
pub(crate) type MakeEngine = Rc<dyn Fn(&Rc<Core>) -> Result<Box<dyn JsEngine>, JsError>>;

pub(crate) struct Reloader {
    make: MakeEngine,
    pending: Arc<AtomicBool>,
    provider: RefCell<Option<Rc<dyn ModuleProvider>>>,
    /// Watched files that changed since the last reload (filled by the watcher thread).
    changed: Arc<Mutex<Vec<PathBuf>>>,
    /// The last attempt failed: any event in a watched directory counts.
    retry: Arc<AtomicBool>,
    watch: RefCell<Option<Watch>>,
    hook: RefCell<Option<Rc<dyn Fn(&ReloadReport)>>>,
    generation: Cell<u64>,
    /// An embedder's entries are loaded (`load_entry`): a full reload would discard them, so it is
    /// refused (the embedder decides what to do).
    embedded: Cell<bool>,
}

impl Reloader {
    pub(crate) fn new(make: MakeEngine) -> Rc<Reloader> {
        Rc::new(Reloader {
            make,
            pending: Arc::new(AtomicBool::new(false)),
            provider: RefCell::new(None),
            changed: Arc::default(),
            retry: Arc::default(),
            watch: RefCell::new(None),
            hook: RefCell::new(None),
            generation: Cell::new(0),
            embedded: Cell::new(false),
        })
    }

    pub(crate) fn set_embedded(&self) {
        self.embedded.set(true);
    }

    pub(crate) fn set_provider(&self, provider: Rc<dyn ModuleProvider>) {
        *self.provider.borrow_mut() = Some(provider);
    }

    /// Records changed files as the watcher thread does (without the debounce or the wake).
    pub(crate) fn note_changed(&self, paths: Vec<PathBuf>) {
        self.changed.lock().unwrap().extend(paths);
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

    /// Starts watching the files of the provider recorded by `set_provider`. The thread only
    /// records changed paths, sets the flag and calls `wake`.
    pub(crate) fn start_watch(&self, wake: WakeHandle) -> Result<(), JsError> {
        let host_err = |m: String| JsError::new(ErrorKind::Host, m);
        if self.provider.borrow().is_none() {
            return Err(host_err("watch: load an app first".into()));
        }
        let files: Arc<Mutex<HashSet<PathBuf>>> = Arc::default();
        let (tx, rx) = mpsc::channel::<()>();
        let (filter, changed, retry) = (files.clone(), self.changed.clone(), self.retry.clone());
        let watcher: RecommendedWatcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            let Ok(ev) = res else { return };
            if !matches!(ev.kind, EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)) {
                return;
            }
            let all = retry.load(Ordering::SeqCst);
            let hits: Vec<PathBuf> = {
                let files = filter.lock().unwrap();
                ev.paths.into_iter().filter(|p| all || files.contains(p)).collect()
            };
            if !hits.is_empty() {
                changed.lock().unwrap().extend(hits);
                let _ = tx.send(());
            }
        })
        .map_err(|e| host_err(format!("watch: {e}")))?;

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
        *self.watch.borrow_mut() = Some(Watch { watcher: Some(watcher), thread: Some(thread), files, dirs: HashSet::new() });
        self.sync_watch().map_err(host_err)
    }

    /// Points the watcher at the provider's current files: their directories, non-recursively.
    fn sync_watch(&self) -> Result<(), String> {
        let Some(provider) = self.provider.borrow().clone() else { return Ok(()) };
        let mut watch = self.watch.borrow_mut();
        let Some(w) = watch.as_mut() else { return Ok(()) };
        let files: HashSet<PathBuf> = provider.watch_files().into_iter().collect();
        let mut first_err = None;
        for dir in files.iter().filter_map(|f| f.parent()) {
            if w.dirs.contains(dir) || !dir.is_dir() {
                continue;
            }
            match w.watcher.as_mut().map(|x| x.watch(dir, RecursiveMode::NonRecursive)) {
                Some(Ok(())) => {
                    w.dirs.insert(dir.to_path_buf());
                }
                Some(Err(e)) => {
                    first_err.get_or_insert_with(|| format!("watch {}: {e}", dir.display()));
                }
                None => {}
            }
        }
        *w.files.lock().unwrap() = files;
        first_err.map_or(Ok(()), Err)
    }

    /// Stops the watcher thread (joins it, so the backend may be dropped afterwards).
    pub(crate) fn stop_watch(&self) {
        self.watch.borrow_mut().take();
    }

    /// Swaps the running app for a fresh load from its provider. Must not be called while JS
    /// runs. Does nothing when the changed files do not affect the app (the provider decides: a
    /// bundle that is mid-write, a file the graph no longer reaches). A reload requested without
    /// changed files (`request`) always runs.
    pub(crate) fn reload(&self, core: &Rc<Core>, engine: &SharedEngine) -> Option<ReloadReport> {
        let provider = self.provider.borrow().clone()?;
        let changed = std::mem::take(&mut *self.changed.lock().unwrap());
        let affected = provider.invalidate(&changed);
        // After a failure a source app retries on any change; a bundle still waits for its file.
        let retry = self.retry.load(Ordering::SeqCst) && provider.map_dir().is_none();
        if !changed.is_empty() && !affected && !retry {
            return None;
        }
        let t0 = Instant::now();
        let generation = self.generation.get() + 1;
        self.generation.set(generation);

        // H1: a change to source modules is first tried as an in-place swap.
        let mut fallback = None;
        if provider.hmr() && !changed.is_empty() {
            let ids: Vec<String> = changed.iter().flat_map(|p| provider.modules_at(std::slice::from_ref(p))).collect();
            let all_modules = changed.iter().all(|p| !provider.modules_at(std::slice::from_ref(p)).is_empty());
            if all_modules {
                match core::hot_update(core, engine, &provider, &ids) {
                    Ok(HotOutcome::Applied { modules }) => {
                        let report = ReloadReport { generation, ok: true, elapsed: t0.elapsed(), error: None, hot: Some(modules), fallback: None };
                        return Some(self.finish(core, &provider, report));
                    }
                    Ok(HotOutcome::Reload(why)) => fallback = Some(why),
                    Err(e) => {
                        // the previous versions keep running; the next change retries
                        let e = core.map_error(e);
                        core.report(e.clone());
                        let report = ReloadReport { generation, ok: false, elapsed: t0.elapsed(), error: Some(e), hot: Some(Vec::new()), fallback: None };
                        return Some(self.finish(core, &provider, report));
                    }
                }
            } else {
                fallback = Some("a changed file is not a module of the app".into());
            }
        }

        if self.embedded.get() {
            // the runtime holds the embedder's entries and roots: replacing it is the embedder's call
            let why = fallback.clone().unwrap_or_else(|| "a full reload was requested".into());
            let e = JsError::new(ErrorKind::Host, format!("not applied in place ({why}); a full reload would discard the embedded roots"));
            core.report(e.clone());
            let report = ReloadReport { generation, ok: false, elapsed: t0.elapsed(), error: Some(e), hot: None, fallback };
            return Some(self.finish(core, &provider, report));
        }

        let old_roots: HashSet<u64> = core.roots().into_iter().collect();
        core.clear_events();
        let old_timers = core.swap_timers(Timers::default());

        // App objects the old runtime held die with it unless the new one receives them again (E3: a
        // persisted object, an app-wide store, a service), and so do its offloaded calls (E1): results
        // tagged with its generation are dropped. Its observations of and listeners on app objects are
        // set aside while the new bundle loads.
        let old_apps = core.app_handles();
        let old_gen = core.ops_begin_generation();
        let old_app_gen = core.app_begin_generation();
        let stash = core.reactive_stash();
        let loaded = (self.make)(core).and_then(|fresh| {
            let old_engine = std::mem::replace(&mut *engine.borrow_mut(), fresh);
            match core::load_bundle(core, engine, &provider) {
                Ok(()) => Ok(old_engine),
                Err(e) => {
                    *engine.borrow_mut() = old_engine;
                    Err(e)
                }
            }
        });

        let error = match loaded {
            Ok(old_engine) => {
                // The old runtime's calls are cancelled before it is dropped, so workers holding its
                // transferred buffers (E2) see the cancel while the drop waits for them. Its app objects
                // go first too, except the ones the new runtime received again (E3: persisted objects,
                // app-wide stores, services keep their handles).
                core.ops_end_generation(old_gen, true);
                drop(stash);
                core.app_release_stale(&old_apps);
                // E3 + E2: memory of the old runtime that Rust still holds (a JS `Buffer<T>` kept in a
                // persisted object, a store, a service or a static, or a worker ignoring the cancel)
                // makes the drop below wait 1 s and then leak the whole runtime. Say so, and how to avoid it.
                let pinned = old_engine.buffer_stats().tokens_live;
                if pinned > 0 {
                    core.console(
                        "warn",
                        format!(
                            "svelters: {pinned} buffer(s) from the old JS runtime are still held by Rust at reload \
                             (a Buffer<T> kept in a persisted object, a store, a service or a static, or a worker that ignores \
                             its cancel): the runtime cannot be freed and will be leaked after 1 s. Keep Rust-owned memory \
                             instead: Buffer::into_vec() copies JS memory into Rust."
                        ),
                    );
                }
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
                core.ops_end_generation(old_gen, false);
                core.app_restore_generation(old_app_gen, &old_apps);
                core.reactive_restore(stash);
                let e = core.map_error(e);
                core.report(e.clone());
                Some(e)
            }
        };

        let report = ReloadReport { generation, ok: error.is_none(), elapsed: t0.elapsed(), error, hot: None, fallback };
        Some(self.finish(core, &provider, report))
    }

    /// Bookkeeping after an attempt: the retry flag, the watched set, the log line and the hook.
    fn finish(&self, core: &Rc<Core>, provider: &Rc<dyn ModuleProvider>, report: ReloadReport) -> ReloadReport {
        let generation = report.generation;
        self.retry.store(!report.ok, Ordering::SeqCst);
        if let Err(e) = self.sync_watch() {
            core.console("warn", format!("svelters: {e}"));
        }
        if core.cfg.echo_console {
            if report.ok {
                eprintln!(
                    "[svelters] {} {} in {:.1} ms (#{generation}){}",
                    if report.hot.is_some() { "updated in place" } else { "reloaded" },
                    provider.describe(),
                    report.elapsed.as_secs_f64() * 1e3,
                    report.fallback.as_deref().map(|w| format!(" (full reload: {w})")).unwrap_or_default()
                );
            } else {
                eprintln!("[svelters] reload #{generation} failed; the previous version keeps running until the next change");
            }
        }
        let hook = self.hook.borrow().clone();
        if let Some(h) = hook {
            h(&report);
        }
        report
    }
}

struct Watch {
    watcher: Option<RecommendedWatcher>,
    thread: Option<JoinHandle<()>>,
    /// The files whose events count (shared with the watcher's callback).
    files: Arc<Mutex<HashSet<PathBuf>>>,
    /// The directories watched so far.
    dirs: HashSet<PathBuf>,
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
