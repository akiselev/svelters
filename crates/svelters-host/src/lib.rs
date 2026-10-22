//! svelters-host: runs an ESM bundle in embedded QuickJS-NG and forwards its host ops to a
//! `svl_backend`.
//!
//! ```ignore
//! let vt = BackendVtable::new(my_backend);      // or any `svl_backend` via BackendRef::from_raw
//! let host = Host::new(vt.backend_ref(), HostConfig::default())?;
//! host.load_bundle("fixtures/counter/dist/app.js")?;
//! host.run()?;                                   // the backend's loop; returns on quit
//! ```
//!
//! Where the toolkit must be up before JS mounts (iOS/Android, embedders, test harnesses), load
//! in the started hook instead: `host.run_with(|h| h.load_bundle(path).unwrap())?`, or the
//! non-blocking `host.start_with(...)` (ABI v2.7) and drive the loop yourself; see [`Host`].
//!
//! Layout: `core` is engine-agnostic (handles, name caches, event queue, timers, checkpoint),
//! `engine` is the boundary trait, `quickjs` is the rquickjs implementation.
//!
//! Threading: a `Host` is pinned to the thread that created it (the toolkit's main thread).
//! The host starts no threads. Timers run on the toolkit: after each checkpoint the host arms the
//! backend's one-shot `schedule_wake` for the earliest deadline.

mod convert;
mod core;
mod engine;
mod hooks;
pub mod exports;
pub mod platform;
mod quickjs;
mod reload;
mod sourcemap;
pub mod testing;

pub use crate::core::{ConsoleLine, HostConfig, OffloadStats, ReactiveStats, Timing};
pub use crate::engine::{
    BufferStats, ChangedProp, ErrorKind, HostBusy, JobStatus, JsEngine, JsError, OpOutcome, OpSettle, RootMessage,
};
pub use crate::engine::HotOutcome;
pub use crate::reload::ReloadReport;
/// Q1: the module loader (providers; `SourceProvider` with its `source` feature).
pub use svelters_modules as modules;
pub use hooks::ModuleHooks;
pub use platform::{ClipboardBackend, ClipboardKind, MimeData, Platform, ShellBackend};
pub use svelters_modules::{BundleProvider, ModuleCode, ModuleProvider};

/// The provider of a built bundle; a missing entry is a load error.
fn bundle_provider(path: &Path) -> Result<Rc<dyn ModuleProvider>, JsError> {
    Ok(Rc::new(BundleProvider::new(path).map_err(|m| JsError::new(ErrorKind::Load, m))?))
}

use crate::core::Core;
use crate::reload::{MakeEngine, Reloader};
use std::cell::{Cell, RefCell};
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;
use svelters_abi::{
    AbiError, BackendRef, BackendVtable, HostCallbacks, HostVtable, SyncError, Value, svl_handle, svl_id,
};

type SharedEngine = Rc<RefCell<Box<dyn JsEngine>>>;

/// The "started" state shared by the host and its callbacks (G0, ABI v2.7).
///
/// The toolkit is up at the first host callback the backend delivers from `run`/`start`, or when
/// `start` returns. `hook` holds the pending `on_started` trampoline while `run`/`start` is active.
#[derive(Default)]
struct Started {
    /// The armed trampoline (see [`Host::run_with`]); empty outside `run`/`start`.
    hook: RefCell<Option<Box<dyn FnOnce()>>>,
    /// The toolkit is up (a callback arrived, or `start` returned) and not stopped since.
    up: Cell<bool>,
    /// Inside `run`, or between `start` and `stop`: callbacks count as "toolkit up". Callbacks
    /// made while the host is only pre-attached for mount (`load_bundle` before `run`) do not.
    active: Cell<bool>,
}

struct Callbacks {
    core: Rc<Core>,
    engine: SharedEngine,
    reloader: Rc<Reloader>,
    started: Rc<Started>,
}

impl Callbacks {
    /// Marks the toolkit up and fires a pending started hook, before the callback is handled.
    /// While JS runs (a callback from inside a host op) the hook waits for the next callback, so it
    /// can load a bundle (the core's re-entry guard is not held here).
    fn toolkit_up(&self) {
        if !self.started.active.get() {
            return;
        }
        self.started.up.set(true);
        if self.core.js_running() {
            return;
        }
        let hook = self.started.hook.borrow_mut().take();
        if let Some(hook) = hook {
            hook();
        }
    }
}

impl HostCallbacks for Callbacks {
    fn post_event(&self, h: svl_handle, signal: svl_id, args: &[Value]) {
        self.toolkit_up();
        self.core.push_event(h, signal, args.to_vec());
        // A live backend reports a native event from its loop and expects the host to schedule
        // the checkpoint that delivers it. Scripted backends call `checkpoint` themselves.
        if !self.core.cfg.scripted {
            self.core.backend().wake();
        }
    }

    fn checkpoint(&self) {
        self.toolkit_up();
        // A reload runs first, and only from the toolkit loop: while JS is running (a nested
        // loop inside a host op) the flag stays set and the deferred wake comes back here.
        if !self.core.js_running() && self.reloader.take_pending() {
            self.reloader.reload(&self.core, &self.engine);
        }
        core::checkpoint(&self.core, &self.engine);
    }

    fn call_sync(&self, h: svl_handle, signal: svl_id, args: &[Value]) -> Result<Value, SyncError> {
        self.toolkit_up();
        core::call_sync(&self.core, &self.engine, h, signal, args)
    }

    fn frame(&self) {
        self.toolkit_up();
        core::frame(&self.core, &self.engine);
    }

    fn prop_changed(&self, h: svl_handle, prop: svl_id) {
        self.toolkit_up();
        // Marks (h, prop) dirty; the next checkpoint reads, compares and delivers it in one batch.
        // A live backend gets one wake per turn: on the first insert into an empty dirty set.
        if self.core.prop_changed(h, prop) && !self.core.cfg.scripted {
            self.core.backend().wake();
        }
    }

    fn invoke_callback(&self, callback: u64, args: &[Value]) -> Result<Value, String> {
        self.toolkit_up();
        self.core.invoke_callback(callback, args)
    }

    // Not a "toolkit up" signal: a release can come from teardown paths (finalizers, dropped closures).
    fn release_callback(&self, callback: u64) {
        self.core.release_callback_from_native(callback);
    }

    fn on_destroyed(&self, h: svl_handle, reason: u32) {
        self.toolkit_up();
        self.core.native_gone(h, reason);
        // Delivering the death is a checkpoint's job; a live backend needs one scheduled.
        if !self.core.cfg.scripted {
            self.core.backend().wake();
        }
    }
}

/// The user's started hook, registered by [`Host::on_started`].
type StartedHook<'a> = Box<dyn FnOnce(&Host<'a>) + 'a>;

/// The host: JS engine + core state bound to one backend.
///
/// # Start orders
///
/// - **Desktop** (the backend owns the loop and the toolkit can create windows before it runs):
///   [`Host::load_bundle`] then [`Host::run`].
/// - **Platforms that own `main`, embedders, test harnesses** (iOS/Android: the platform brings the
///   UI up later; egui/kittest: the caller drives the frame loop): the toolkit must be up before JS
///   mounts. Use [`Host::run_with`] (blocking loop) or [`Host::start_with`] (non-blocking, ABI v2.7
///   `start`; the caller then drives the loop, calling [`Host::checkpoint`]/[`Host::frame`] or
///   letting the backend call them) and load the bundle in the started hook.
///
/// "Started" is the host's notion: the first host callback the backend delivers from `run`/`start`
/// (a checkpoint, a frame, an event...), or the return of `start` when none came during it.
pub struct Host<'a> {
    core: Rc<Core>,
    engine: SharedEngine,
    vt: HostVtable,
    reloader: Rc<Reloader>,
    started: Rc<Started>,
    /// LD1: extra schemes, native modules and the `svelters:app/*` filter for every load.
    module_hooks: RefCell<ModuleHooks>,
    /// The hook registered by `on_started`, armed by the next `run`/`start`.
    on_started: RefCell<Option<StartedHook<'a>>>,
    /// The backend was attached through `start` and not detached since.
    attached: Cell<bool>,
    _backend: PhantomData<BackendRef<'a>>,
    /// [`Host::new_owned`]: the backend's vtable, owned by the host. Last field: dropped after
    /// everything else that could still call the backend (the core, the engine, the callbacks).
    owned: Option<Box<BackendVtable>>,
}

/// Disarms an unfired started trampoline when `run`/`start` returns (or unwinds).
struct DisarmStarted<'s>(&'s Started);

impl Drop for DisarmStarted<'_> {
    fn drop(&mut self) {
        let hook = self.0.hook.borrow_mut().take();
        drop(hook);
    }
}

impl<'a> Host<'a> {
    /// Creates a host with the QuickJS engine. The backend must outlive the host (`'a`).
    pub fn new(backend: BackendRef<'a>, cfg: HostConfig) -> Result<Host<'a>, JsError> {
        Host::with_engine(backend, cfg, Rc::new(|core| {
            Ok(Box::new(quickjs::QjsEngine::new(core)?))
        }))
    }

    pub(crate) fn with_engine(
        backend: BackendRef<'a>,
        cfg: HostConfig,
        make: MakeEngine,
    ) -> Result<Host<'a>, JsError> {
        // E3: the app's services start with its first host (once per process; later hosts find them running).
        crate::exports::start_services();
        // SAFETY: `'a` ties the host to the backend borrow, so the erased reference cannot outlive it.
        let backend_static = unsafe { backend.erase_lifetime() };
        let core = Rc::new(Core::new(backend_static, cfg));
        let engine: SharedEngine = Rc::new(RefCell::new(make(&core)?));
        let reloader = Reloader::new(make);
        let started = Rc::new(Started::default());
        let vt = HostVtable::new(Callbacks {
            core: core.clone(),
            engine: engine.clone(),
            reloader: reloader.clone(),
            started: started.clone(),
        });

        Ok(Host {
            core,
            engine,
            vt,
            reloader,
            started,
            module_hooks: RefCell::new(ModuleHooks::default()),
            on_started: RefCell::new(None),
            attached: Cell::new(false),
            _backend: PhantomData,
            owned: None,
        })
    }

    /// Loads and evaluates an ESM entry module. Imports resolve relative to the entry file's
    /// directory and cannot leave it. Installs `globalThis.__svelters_host` first.
    ///
    /// The path is remembered for hot reload. A load error is returned with its stack mapped to
    /// the original sources when the bundle has a `.map` next to it.
    ///
    /// On a backend with ABI v2.7 `start`, a host that is not running yet (the desktop order: load,
    /// then `run`) is attached first, so host callbacks made during mount (a native filter or sorter
    /// calling `invoke_callback` while a model is built, a wake) reach it. That pre-attach does not
    /// count as "started"; `run` attaches the same callbacks again, `Drop` detaches.
    pub fn load_bundle(&self, path: impl AsRef<Path>) -> Result<(), JsError> {
        self.load_from(bundle_provider(path.as_ref())?)
    }

    /// LD1: sets the extra schemes, native modules and `svelters:app/*` visibility that apply to the
    /// next load (`load_from`, `load_entry_from`, `load_bundle`, `load_entry`). The default serves
    /// this binary's exports and every `svelters:app/*` module.
    /// DX2: the clipboard and shell this host's `svelters:clipboard` and `svelters:shell` use
    /// (`HostConfig::platform`), for host-side callers: EV3's paste command, a Rust panel's copy.
    pub fn platform(&self) -> &platform::Platform {
        &self.core.cfg.platform
    }

    pub fn set_module_hooks(&self, hooks: ModuleHooks) {
        *self.core.app_gate.borrow_mut() = Some(hooks.app_gate());
        *self.module_hooks.borrow_mut() = hooks;
    }

    fn compose_modules(&self, inner: Rc<dyn ModuleProvider>) -> Result<Rc<dyn ModuleProvider>, JsError> {
        self.module_hooks.borrow().compose(inner).map_err(|m| JsError::new(ErrorKind::Host, m))
    }

    /// Q1: loads and evaluates the entry module of `provider` ([`BundleProvider`], or
    /// `svelters_modules::SourceProvider` for an app compiled from source), resolving and loading
    /// every import through it. Remembered for hot reload, like [`Host::load_bundle`]; load errors
    /// (compiler diagnostics included) are returned with their stacks mapped to the sources.
    pub fn load_from(&self, provider: Rc<dyn ModuleProvider>) -> Result<(), JsError> {
        self.ensure_attached();
        let provider = self.compose_modules(provider)?;
        self.reloader.set_provider(provider.clone());
        core::load_bundle(&self.core, &self.engine, &provider).map_err(|e| self.core.map_error(e))
    }

    /// Hot reload: watches the files of the app loaded by `load_bundle` or `load_from` (a bundle
    /// and its `.map`; an app from source: its module graph and manifests) and reloads it in this
    /// process whenever one of them changes, see `reload.rs`. Requires a backend whose `wake` can be
    /// called from another thread, and a running loop (`run`) to deliver the reload.
    pub fn watch(&self) -> Result<(), JsError> {
        self.reloader.start_watch(self.core.backend().wake_handle())
    }

    /// Applies edits to source files now, as the watcher would after its debounce: swaps the
    /// changed modules in place when the app accepts them, else (and when nothing accepts) reloads
    /// the whole app. For embedders and editors that know which files they saved (H1). `None`
    /// when JS is running, no app is loaded or the files do not affect it. An embedder's entries
    /// do not survive the full-reload fallback; its apps must accept their changes.
    pub fn apply_changes(&self, changed: Vec<PathBuf>) -> Option<ReloadReport> {
        if self.core.js_running() {
            return None;
        }
        self.reloader.note_changed(changed);
        self.reloader.reload(&self.core, &self.engine)
    }

    /// Reloads at the next checkpoint, as if the watcher had fired (also wakes the backend).
    pub fn request_reload(&self) {
        self.reloader.request();
        self.core.backend().wake();
    }

    /// Called after every reload attempt, successful or not, on the toolkit thread.
    pub fn on_reload(&self, f: impl Fn(&ReloadReport) + 'static) {
        self.reloader.set_hook(Rc::new(f));
    }

    /// Live handles that are not inserted under another handle: the app's toplevels.
    pub fn toplevels(&self) -> Vec<svl_handle> {
        self.core.roots()
    }

    /// Live handles of the TOPLEVEL lifetime class, transient dialogs included.
    pub fn toplevel_handles(&self) -> Vec<svl_handle> {
        self.core.toplevel_handles()
    }

    /// Runs the backend's loop. Returns when the backend quits.
    ///
    /// With a hook registered by [`Host::on_started`], the host wakes the backend just before its
    /// loop (so loops that only checkpoint on a pending wake deliver a first callback) and fires the
    /// hook at the first callback; a hook still unfired when the loop returns is dropped. Without a
    /// hook `run` is unchanged.
    pub fn run(&self) -> Result<(), AbiError> {
        let _disarm = DisarmStarted(&self.started);
        if self.arm_started() {
            self.core.backend().wake();
        }
        self.started.active.set(true);
        let res = self.core.backend().run(&self.vt);
        self.started.active.set(false);
        self.started.up.set(false);
        res
    }

    /// Attaches the host through `start` before mount, when the backend supports it and the host
    /// is neither attached nor running (see [`Host::load_bundle`]). Does not fire the started hook.
    fn ensure_attached(&self) {
        if self.attached.get() || self.started.active.get() {
            return;
        }
        let backend = self.core.backend();
        if backend.has_start() && backend.start(&self.vt).is_ok() {
            self.attached.set(true);
        }
    }

    /// [`Host::on_started`] then [`Host::run`]: the order for platforms where the toolkit must be
    /// up before the app mounts. Typically `host.run_with(|h| h.load_bundle(path).unwrap())`.
    pub fn run_with(&self, on_started: impl FnOnce(&Host<'a>) + 'a) -> Result<(), AbiError> {
        self.on_started(on_started);
        self.run()
    }

    /// Registers a hook that fires once when the toolkit is up: at the first host callback the
    /// backend delivers from the next [`Host::run`]/[`Host::start`] (before that callback is handled;
    /// a callback made while JS runs defers it to the next one), or when `start` returns if no
    /// callback came during it. The hook may load the bundle. Replaces a hook not yet armed; called
    /// on a host already started through `start`, it fires at once.
    pub fn on_started(&self, f: impl FnOnce(&Host<'a>) + 'a) {
        if self.started.up.get() {
            f(self);
            return;
        }
        *self.on_started.borrow_mut() = Some(Box::new(f));
    }

    /// Moves the registered hook into the callbacks' slot as a trampoline. True if one was armed.
    fn arm_started(&self) -> bool {
        let Some(f) = self.on_started.borrow_mut().take() else { return false };
        let host: *const Host<'a> = self;
        // SAFETY (deref of `host`): the trampoline runs only from the started slot, which `run`
        // and `start` empty before they return (`DisarmStarted`), and both borrow `self` for their
        // whole duration, so the host is alive and has not moved when it runs.
        let tramp: Box<dyn FnOnce() + 'a> = Box::new(move || f(unsafe { &*host }));
        // SAFETY: only the lifetime bound is erased. The slot is emptied before `run`/`start`
        // returns, so the closure (and what it borrows for `'a`) is never used or dropped after
        // `'a`; the host itself cannot outlive `'a`.
        let tramp: Box<dyn FnOnce()> = unsafe { std::mem::transmute(tramp) };
        *self.started.hook.borrow_mut() = Some(tramp);
        true
    }

    /// ABI v2.7: attaches this host to the backend and brings the toolkit up without running its
    /// loop, then returns. The caller drives the loop (the backend's toolkit loop, or
    /// [`Host::checkpoint`]/[`Host::frame`] from an embedder's frame); the backend calls the host
    /// as it would from `run`. Fires a registered [`Host::on_started`] hook before returning.
    /// An error if the backend has no `start` (the hook then stays registered). Calling it again
    /// while started (or from inside `run`) is a no-op.
    pub fn start(&self) -> Result<(), AbiError> {
        let backend = self.core.backend();
        if !backend.has_start() {
            return Err(AbiError::plain(-1, "this backend does not implement start (ABI v2.7)"));
        }
        if self.started.active.get() {
            return Ok(());
        }
        let _disarm = DisarmStarted(&self.started);
        self.arm_started();
        self.started.active.set(true);
        if let Err(e) = backend.start(&self.vt) {
            self.started.active.set(false);
            return Err(e);
        }
        self.attached.set(true);
        self.started.up.set(true);
        let hook = self.started.hook.borrow_mut().take();
        if let Some(hook) = hook {
            hook();
        }
        Ok(())
    }

    /// [`Host::on_started`] then [`Host::start`]; the hook has run when this returns `Ok`.
    pub fn start_with(&self, on_started: impl FnOnce(&Host<'a>) + 'a) -> Result<(), AbiError> {
        self.on_started(on_started);
        self.start()
    }

    /// Detaches the host from a backend attached by [`Host::start`] (or pre-attached by
    /// [`Host::load_bundle`]): `start(NULL)`, no host callback afterwards. Idempotent; `Drop` does
    /// it too. Not for use inside `run` (quit the backend instead).
    pub fn stop(&self) {
        if self.attached.replace(false) {
            self.core.backend().detach();
        }
        self.started.active.set(false);
        self.started.up.set(false);
    }

    /// True once the toolkit is up: from the first host callback of `run`/`start` (or the return
    /// of `start`) until `run` returns or [`Host::stop`].
    pub fn is_started(&self) -> bool {
        self.started.up.get()
    }

    /// The `svl_host` callbacks this host gives its backend, for embedders and tests that pump the
    /// toolkit loop themselves instead of calling [`Host::run`] (attach them to the backend first,
    /// or use [`Host::start`] on a backend implementing ABI v2.7 `start`).
    pub fn host_callbacks(&self) -> *const svelters_abi::svl_host {
        self.vt.as_raw()
    }

    /// Runs one checkpoint directly (what the backend triggers through `svl_host.checkpoint`).
    pub fn checkpoint(&self) {
        core::checkpoint(&self.core, &self.engine);
    }

    /// Runs the animation-frame callbacks directly (what the backend triggers through
    /// `svl_host.frame`), for embedders that own the frame clock.
    pub fn frame(&self) {
        core::frame(&self.core, &self.engine);
    }

    /// Runs a full garbage collection. Values JS dropped without `dispose()` are queued for release
    /// by their finalization callbacks (jobs) and handed to the backend at the next checkpoint.
    pub fn collect_garbage(&self) {
        let Some(_guard) = self.core.enter() else { return };
        self.engine.borrow_mut().collect_garbage();
    }

    /// Number of callbacks that are still registered.
    pub fn live_callbacks(&self) -> usize {
        self.core.live_callbacks()
    }

    /// Number of (handle, prop) pairs observed natively through `host.observe` (ABI v2.6).
    pub fn observed_props(&self) -> usize {
        self.core.observed_count()
    }

    /// Sends a line per app-export op (`construct`, `call`, `static`, `set`; E0) to `sink`: a test
    /// harness appends them to the backend's op log, so the log shows exported calls in order with
    /// the native ops (`svelters_host::testing` does this for the log backend).
    pub fn set_op_log(&self, sink: impl Fn(&str) + 'static) {
        *self.core.op_log.borrow_mut() = Some(Rc::new(sink));
    }

    /// Number of live exported Rust objects (`svelters:app/*`) this host holds.
    pub fn live_app_objects(&self) -> usize {
        self.core.app_count()
    }

    /// Offloaded export calls (E1) not finished yet: pending, waiting for a `max_in_flight` slot, or
    /// cancelled but still running on a pool (their results will be dropped).
    pub fn ops_in_flight(&self) -> usize {
        self.core.ops_in_flight()
    }

    /// Counters of the offload path: calls started, promises settled (by checkpoint drains or at
    /// once), late results dropped, and the drains' time.
    pub fn offload_stats(&self) -> OffloadStats {
        self.core.offload_stats()
    }

    /// Counters of the reactive exports (E3): store fields marked, delivered and dropped by the compare,
    /// change batches with store fields, `Emitter` events queued and dropped.
    pub fn reactive_stats(&self) -> ReactiveStats {
        self.core.reactive_stats()
    }

    /// Keys of the app objects JS persisted with `persist(key, ...)` (E3); they survive hot reloads.
    pub fn persisted(&self) -> Vec<String> {
        self.core.persisted_keys()
    }

    /// Counters of the zero-copy buffer paths (E2): transfers, reclaims, buffers handed back, releases
    /// on the JS thread, views handed out, fallback copies, live transfer tokens.
    pub fn buffer_stats(&self) -> BufferStats {
        let mut s = self.engine.borrow().buffer_stats();
        s.leaked_runtimes = quickjs::leaked_runtimes();
        s
    }

    /// Number of live boxed values held by JS wrappers (released, disposed or collected ones excluded).
    pub fn live_boxed(&self) -> usize {
        self.core.live_boxed()
    }

    /// Runs queued JS jobs to exhaustion without firing timers or events.
    pub fn drain_jobs(&self) {
        core::drain_jobs(&self.core, &self.engine);
    }

    /// Drains reported errors (uncaught exceptions, unhandled rejections, interrupts).
    pub fn take_errors(&self) -> Vec<JsError> {
        self.core.take_errors()
    }

    /// Drains captured `console.*` output.
    pub fn take_console(&self) -> Vec<ConsoleLine> {
        self.core.take_console()
    }

    /// True when no events are queued and no timers are pending (and JS is not running).
    pub fn is_idle(&self) -> bool {
        self.core.is_idle()
    }

    /// Number of checkpoints that ran so far.
    pub fn checkpoint_count(&self) -> u64 {
        self.core.checkpoint_count()
    }

    /// Everything the host is waiting for (ARCH-EMBED §5.4): queued events and changes, jobs,
    /// offloaded calls, store deliveries, a requested frame, the next timer. Replaces
    /// [`Host::is_idle`]'s partial answer for embedders and idle barriers.
    pub fn busy(&self) -> HostBusy {
        let jobs = self.engine.try_borrow().map(|e| e.jobs_pending()).unwrap_or(true);
        self.core.busy(jobs)
    }

    /// The wall-clock budget armed for each following entry into JS (the interrupt handler stops a
    /// script that exceeds it with an `Interrupted` error). Starts as `HostConfig::turn_budget`;
    /// an embedder switches it per entry (ARCH-EMBED §4.2 `Budgets`).
    pub fn set_turn_budget(&self, budget: Option<Duration>) {
        self.core.budget.set(budget);
    }

    pub fn turn_budget(&self) -> Option<Duration> {
        self.core.budget.get()
    }

    // ---- EM1: roots (ARCH-EMBED §4.2/§4.3) ----

    /// Evaluates an ESM bundle like [`Host::load_bundle`] but keeps its module namespace, so
    /// [`Host::mount_root`] can name its exports. The bundle is an embed bundle
    /// (`svelteConfig({ embed })`): it exports components and installs `__svelters_embed`. Not
    /// watched for reload. Returns the entry's index.
    pub fn load_entry(&self, path: impl AsRef<Path>) -> Result<u32, JsError> {
        self.load_entry_from(bundle_provider(path.as_ref())?)
    }

    /// [`Host::load_entry`] from a module provider (Q1): an embed entry compiled from source.
    pub fn load_entry_from(&self, provider: Rc<dyn ModuleProvider>) -> Result<u32, JsError> {
        self.ensure_attached();
        let provider = self.compose_modules(provider)?;
        self.reloader.set_provider(provider.clone());
        self.reloader.set_embedded();
        core::load_entry(&self.core, &self.engine, &provider).map_err(|e| self.core.map_error(e))
    }

    /// Mounts export `export` of entry `entry` into the native node `target` (an embedder's slot)
    /// as root `root`, with `props` (a plain object, made reactive by the runtime). One entry into
    /// JS under the current turn budget, then a job drain. A first-render error is returned and
    /// the runtime has already torn the root down.
    pub fn mount_root(&self, entry: u32, export: &str, root: u32, target: svl_handle, props: &[(String, Value)]) -> Result<(), JsError> {
        core::root_call(&self.core, &self.engine, Some(target), |e| e.mount_root(entry, export, root, target, props))
            .map_err(|e| self.core.map_error(e))
    }

    /// Shallow-merges `patch` into root `root`'s props (`Value::Null` sets null; a key absent
    /// stays). A `"$$delete"` entry holding a list of key strings removes those keys (D10).
    pub fn root_props(&self, root: u32, patch: &[(String, Value)]) -> Result<(), JsError> {
        self.root_call_js("props", root, Some(patch)).map(|_| ())
    }

    /// Updates the root's `svelters:embed` info fields (visible, width, height, scale, viewport,
    /// focused) seen by its components.
    pub fn root_info(&self, root: u32, patch: &[(String, Value)]) -> Result<(), JsError> {
        self.root_call_js("info", root, Some(patch)).map(|_| ())
    }

    /// Unmounts root `root`: its natives are removed from the target and destroyed, its handlers
    /// and effects disposed. Unknown roots are a no-op.
    pub fn unmount_root(&self, root: u32) -> Result<(), JsError> {
        self.root_call_js("unmount", root, None).map(|_| ())
    }

    /// Tells root `root` that its natives now live under `target` (EM2 `move_root`: the embedder
    /// moved them there); the runtime inserts its new natives into `target` from then on.
    pub fn retarget_root(&self, root: u32, target: svl_handle) -> Result<(), JsError> {
        self.root_call_js("retarget", root, Some(&[("target".to_string(), Value::F64(target as f64))])).map(|_| ())
    }

    /// Runs root `root`'s `onCloseRequest` handlers; true if the root may close.
    pub fn close_root(&self, root: u32) -> Result<bool, JsError> {
        self.root_call_js("close", root, None).map(|v| !matches!(v, Value::Bool(false)))
    }

    fn root_call_js(&self, method: &str, root: u32, patch: Option<&[(String, Value)]>) -> Result<Value, JsError> {
        core::root_call(&self.core, &self.engine, None, |e| e.root_call(method, root, patch)).map_err(|e| self.core.map_error(e))
    }

    /// Drains what mounted roots reported (faults from their boundaries, title/dirty, emits).
    pub fn take_root_messages(&self) -> Vec<RootMessage> {
        self.core.take_root_messages()
    }

    /// Returns and resets the time split counters (all zero unless `HostConfig::timing`).
    pub fn take_timing(&self) -> Timing {
        self.core.take_timing()
    }

    /// Setting this flag from any thread interrupts the running script once.
    pub fn interrupt_flag(&self) -> Arc<AtomicBool> {
        self.core.interrupt.clone()
    }
}

impl Host<'static> {
    /// A host that owns its backend's vtable (boxed, so its address is stable) and drops it after
    /// detaching: no `'static` borrow, leak or `unsafe` at the call site (ARCH-EMBED §4.2, K-7).
    pub fn new_owned<B: svelters_abi::Backend + 'static>(backend: B, cfg: HostConfig) -> Result<Host<'static>, JsError> {
        let vt = Box::new(BackendVtable::new(backend));
        // SAFETY: the vtable is boxed and owned by the host (`owned`, its last field), so it lives
        // at the same address until after everything that could call it is dropped.
        let r: BackendRef<'static> = unsafe { vt.backend_ref().erase_lifetime() };
        let mut host = Host::new(r, cfg)?;
        host.owned = Some(vt);
        Ok(host)
    }
}

impl Drop for Host<'_> {

    fn drop(&mut self) {
        // E1: workers that finish later post to a queue nobody drains and wake nobody.
        self.core.ops_shutdown();
        // E2: app objects go before the JS runtime, so a transferred buffer one of them holds is let go
        // first (the runtime is only leaked when Rust really keeps its memory).
        self.core.app_release_all(&self.core.app_handles());
        // E3: store writers and emitters on other threads reach nobody either; persisted objects are
        // dropped here, also before the runtime.
        self.core.reactive_shutdown();
        self.reloader.stop_watch();
        // A backend attached through `start` must not call the vtable that is about to be freed.
        self.stop();
        // A pending backend wakeup would call back into a vtable that is about to be freed.
        self.core.disarm();
    }
}
