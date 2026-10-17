//! The boundary between the engine-agnostic host core and a JavaScript engine.
//!
//! The core (`core.rs`) owns the handle table, name caches, event queue, timer heap, checkpoint
//! logic and op forwarding to the backend. An engine implements [`JsEngine`] to run JS on the
//! core's behalf, and calls back into the core's `op_*`, `timer_*` and `console` methods from its
//! native `__svelters_host`, timer and `console` bindings. `quickjs.rs` is the only
//! implementation.

use std::fmt;
use std::path::Path;
use svelters_abi::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    /// An uncaught exception from a module, event handler, timer or job.
    Exception,
    /// A promise rejected with no handler attached.
    UnhandledRejection,
    /// The interrupt handler stopped a runaway script (turn budget or external flag).
    Interrupted,
    /// The memory limit was hit.
    OutOfMemory,
    /// The bundle could not be found, read, resolved or parsed.
    Load,
    /// A host-side failure (backend error, bad configuration).
    Host,
}

/// An error with the JS stack when there is one. Source-map translation is M2.
#[derive(Clone, Debug)]
pub struct JsError {
    pub kind: ErrorKind,
    pub message: String,
    pub stack: Option<String>,
}

impl JsError {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> JsError {
        JsError {
            kind,
            message: message.into(),
            stack: None,
        }
    }
}

impl fmt::Display for JsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)?;
        if let Some(s) = &self.stack {
            write!(f, "\n{s}")?;
        }
        Ok(())
    }
}

impl std::error::Error for JsError {}

/// Outcome of draining the engine's job queue.
#[derive(Clone, Copy, Debug, Default)]
pub struct JobStatus {
    pub ran: usize,
    /// Jobs remain because `max` was reached.
    pub pending: bool,
}

pub trait JsEngine {
    /// Evaluates the entry module. Imports resolve relative to the entry file's directory and may
    /// not leave it. Jobs may remain queued afterwards.
    fn load_module(&mut self, entry: &Path) -> Result<(), JsError>;

    /// Calls `globalThis.__svelters_dispatch(handle, signal, args, deferred)`. `deferred` marks
    /// a sync signal that could not run synchronously (its result is ignored).
    fn dispatch(
        &mut self,
        handle: u64,
        signal: &str,
        args: &[Value],
        deferred: bool,
    ) -> Result<(), JsError>;

    /// Calls `globalThis.__svelters_dispatch_sync(handle, signal, args)` and returns its result:
    /// `Null` for no decision, a bool for a stop flag, or the handler's explicit value.
    fn dispatch_sync(
        &mut self,
        handle: u64,
        signal: &str,
        args: &[Value],
    ) -> Result<Value, JsError>;

    /// Runs the animation-frame callbacks (`__svelters_internal.frame`).
    fn run_frame(&mut self) -> Result<(), JsError>;

    /// Calls `globalThis.__svelters_gone(handle, reason)` if the renderer installed it: the
    /// native object behind `handle` died (`reason` is a `gone_name`).
    fn dispatch_gone(&mut self, handle: u64, reason: &str) -> Result<(), JsError>;

    /// Runs the callback registered for a timer id.
    fn fire_timer(&mut self, id: u32) -> Result<(), JsError>;

    /// Runs at most `max` queued jobs (promise reactions, microtasks).
    fn run_jobs(&mut self, max: usize) -> JobStatus;

    /// Runs a full garbage collection (finalization callbacks are queued as jobs). A test and
    /// shutdown hook; the engine collects on its own otherwise.
    fn collect_garbage(&mut self) {}

    /// Errors the engine collected outside a call it returned from: job exceptions and unhandled
    /// rejections. Drains the list.
    fn take_errors(&mut self) -> Vec<JsError>;
}
