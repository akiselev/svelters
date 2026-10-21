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
pub mod exports;
mod quickjs;
mod reload;
mod sourcemap;
pub mod testing;

pub use crate::core::{ConsoleLine, HostConfig, Timing};
pub use crate::engine::{ChangedProp, ErrorKind, JobStatus, JsEngine, JsError};
pub use crate::reload::ReloadReport;

use crate::core::Core;
use crate::reload::{MakeEngine, Reloader};
use std::cell::{Cell, RefCell};
use std::marker::PhantomData;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use svelters_abi::{
    AbiError, BackendRef, HostCallbacks, HostVtable, SyncError, Value, svl_handle, svl_id,
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
    /// The hook registered by `on_started`, armed by the next `run`/`start`.
    on_started: RefCell<Option<StartedHook<'a>>>,
    /// The backend was attached through `start` and not detached since.
    attached: Cell<bool>,
    _backend: PhantomData<BackendRef<'a>>,
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
            on_started: RefCell::new(None),
            attached: Cell::new(false),
            _backend: PhantomData,
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
        self.ensure_attached();
        self.reloader.set_bundle(path.as_ref());
        core::load_bundle(&self.core, &self.engine, path.as_ref()).map_err(|e| self.core.map_error(e))
    }

    /// Hot reload: watches the bundle loaded by `load_bundle` (and its `.map`) and reloads it in
    /// this process whenever it changes, see `reload.rs`. Requires a backend whose `wake` can be
    /// called from another thread, and a running loop (`run`) to deliver the reload.
    pub fn watch(&self) -> Result<(), JsError> {
        self.reloader.start_watch(self.core.backend().wake_handle())
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

    /// Returns and resets the time split counters (all zero unless `HostConfig::timing`).
    pub fn take_timing(&self) -> Timing {
        self.core.take_timing()
    }

    /// Setting this flag from any thread interrupts the running script once.
    pub fn interrupt_flag(&self) -> Arc<AtomicBool> {
        self.core.interrupt.clone()
    }
}

impl Drop for Host<'_> {
    fn drop(&mut self) {
        self.reloader.stop_watch();
        // A backend attached through `start` must not call the vtable that is about to be freed.
        self.stop();
        // A pending backend wakeup would call back into a vtable that is about to be freed.
        self.core.disarm();
    }
}
