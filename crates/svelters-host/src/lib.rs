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
//! Layout: `core` is engine-agnostic (handles, name caches, event queue, timers, checkpoint),
//! `engine` is the boundary trait, `quickjs` is the rquickjs implementation.
//!
//! Threading: a `Host` is pinned to the thread that created it (the toolkit's main thread).
//! The host starts no threads. Timers run on the toolkit: after each checkpoint the host arms the
//! backend's one-shot `schedule_wake` for the earliest deadline.

mod core;
mod engine;
mod quickjs;
pub mod testing;

pub use crate::core::{ConsoleLine, HostConfig, Timing};
pub use crate::engine::{ErrorKind, JobStatus, JsEngine, JsError};

use crate::core::Core;
use std::cell::RefCell;
use std::marker::PhantomData;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use svelters_abi::{AbiError, BackendRef, HostCallbacks, HostVtable, Value, svl_handle, svl_id};

type SharedEngine = Rc<RefCell<Box<dyn JsEngine>>>;

struct Callbacks {
    core: Rc<Core>,
    engine: SharedEngine,
}

impl HostCallbacks for Callbacks {
    fn post_event(&self, h: svl_handle, signal: svl_id, args: &[Value]) {
        self.core.push_event(h, signal, args.to_vec());
        // A live backend reports a native event from its loop and expects the host to schedule
        // the checkpoint that delivers it. Scripted backends call `checkpoint` themselves.
        if !self.core.cfg.scripted {
            self.core.backend().wake();
        }
    }

    fn checkpoint(&self) {
        core::checkpoint(&self.core, &self.engine);
    }

    fn call_sync(&self, _h: svl_handle, _signal: svl_id, _args: &[Value]) -> Result<Value, String> {
        Err("call_sync is not supported yet (M4)".into())
    }

    fn on_destroyed(&self, h: svl_handle, _reason: u32) {
        self.core.forget_handle(h);
    }
}

/// The host: JS engine + core state bound to one backend.
pub struct Host<'a> {
    core: Rc<Core>,
    engine: SharedEngine,
    vt: HostVtable,
    _backend: PhantomData<BackendRef<'a>>,
}

impl<'a> Host<'a> {
    /// Creates a host with the QuickJS engine. The backend must outlive the host (`'a`).
    pub fn new(backend: BackendRef<'a>, cfg: HostConfig) -> Result<Host<'a>, JsError> {
        Host::with_engine(backend, cfg, |core| {
            Ok(Box::new(quickjs::QjsEngine::new(core)?))
        })
    }

    pub(crate) fn with_engine(
        backend: BackendRef<'a>,
        cfg: HostConfig,
        make: impl FnOnce(&Rc<Core>) -> Result<Box<dyn JsEngine>, JsError>,
    ) -> Result<Host<'a>, JsError> {
        // SAFETY: `'a` ties the host to the backend borrow, so the erased reference cannot outlive it.
        let backend_static = unsafe { backend.erase_lifetime() };
        let core = Rc::new(Core::new(backend_static, cfg));
        let engine: SharedEngine = Rc::new(RefCell::new(make(&core)?));
        let vt = HostVtable::new(Callbacks {
            core: core.clone(),
            engine: engine.clone(),
        });

        Ok(Host {
            core,
            engine,
            vt,
            _backend: PhantomData,
        })
    }

    /// Loads and evaluates an ESM entry module. Imports resolve relative to the entry file's
    /// directory and cannot leave it. Installs `globalThis.__svelters_host` first.
    pub fn load_bundle(&self, path: impl AsRef<Path>) -> Result<(), JsError> {
        core::load_bundle(&self.core, &self.engine, path.as_ref())
    }

    /// Runs the backend's loop. Returns when the backend quits.
    pub fn run(&self) -> Result<(), AbiError> {
        self.core.backend().run(&self.vt)
    }

    /// Runs one checkpoint directly (what the backend triggers through `svl_host.checkpoint`).
    pub fn checkpoint(&self) {
        core::checkpoint(&self.core, &self.engine);
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
        // A pending backend wakeup would call back into a vtable that is about to be freed.
        self.core.disarm();
    }
}
