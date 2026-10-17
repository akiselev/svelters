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
//! The only cross-thread call is the timer thread invoking the backend's thread-safe `wake`.

mod core;
mod engine;
mod quickjs;
pub mod testing;

pub use crate::core::{ConsoleLine, HostConfig};
pub use crate::engine::{ErrorKind, JobStatus, JsEngine, JsError};

use crate::core::{Core, Sched};
use std::cell::RefCell;
use std::marker::PhantomData;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::thread::JoinHandle;
use svelters_abi::{AbiError, BackendRef, HostCallbacks, HostVtable, Value, svl_handle, svl_id};

type SharedEngine = Rc<RefCell<Box<dyn JsEngine>>>;

struct Callbacks {
    core: Rc<Core>,
    engine: SharedEngine,
}

impl HostCallbacks for Callbacks {
    fn post_event(&self, h: svl_handle, signal: svl_id, args: &[Value]) {
        self.core.push_event(h, signal, args.to_vec());
    }

    fn checkpoint(&self) {
        core::checkpoint(&self.core, &self.engine);
    }

    fn call_sync(&self, _h: svl_handle, _signal: svl_id, _args: &[Value]) -> Result<Value, String> {
        Err("call_sync is not supported yet (M4)".into())
    }

    fn on_destroyed(&self, h: svl_handle) {
        self.core.forget_handle(h);
    }
}

/// The host: JS engine + core state bound to one backend.
pub struct Host<'a> {
    core: Rc<Core>,
    engine: SharedEngine,
    vt: HostVtable,
    timer_thread: Option<JoinHandle<()>>,
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
        // SAFETY: `'a` ties the host to the backend borrow, and `Drop` joins the only other user
        // of the erased reference (the timer thread) before `'a` can end.
        let backend_static = unsafe { backend.erase_lifetime() };
        let sched = Sched::new();
        let core = Rc::new(Core::new(backend_static, cfg, sched.clone()));
        let engine: SharedEngine = Rc::new(RefCell::new(make(&core)?));
        let vt = HostVtable::new(Callbacks {
            core: core.clone(),
            engine: engine.clone(),
        });

        let wake = backend.wake_handle();
        let timer_thread = std::thread::Builder::new()
            .name("svelters-timers".into())
            .spawn(move || sched.run(wake))
            .map_err(|e| JsError::new(ErrorKind::Host, format!("spawn timer thread: {e}")))?;

        Ok(Host {
            core,
            engine,
            vt,
            timer_thread: Some(timer_thread),
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

    /// Setting this flag from any thread interrupts the running script once.
    pub fn interrupt_flag(&self) -> Arc<AtomicBool> {
        self.core.interrupt.clone()
    }
}

impl Drop for Host<'_> {
    fn drop(&mut self) {
        self.core.sched.shutdown();
        if let Some(t) = self.timer_thread.take() {
            let _ = t.join();
        }
    }
}
