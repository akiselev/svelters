//! The boundary between the engine-agnostic host core and a JavaScript engine.
//!
//! The core (`core.rs`) owns the handle table, name caches, event queue, timer heap, checkpoint
//! logic and op forwarding to the backend. An engine implements [`JsEngine`] to run JS on the
//! core's behalf, and calls back into the core's `op_*`, `timer_*` and `console` methods from its
//! native `__svelters_host`, timer and `console` bindings. `quickjs.rs` is the only
//! implementation.

use std::fmt;
use std::path::Path;
use svelters_abi::{NativeError, SlotDesc, Value};

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

/// One entry of a `__svelters_changed` batch (ABI v2.6, REACTIVITY.md §5.2): an observed property
/// whose value differs from the last one the host delivered or read back. `value` is `None` for a
/// `SVL_M_PROPORTIONAL` property (an invalidation: JS reads it live if it needs it).
#[derive(Clone, Debug)]
pub struct ChangedProp {
    pub handle: u64,
    pub prop: String,
    pub value: Option<Value>,
}

/// How one offloaded export call ends, or what it reports (E1): settled directly in the engine at
/// checkpoint start, a whole batch per engine entry.
#[derive(Clone, Debug)]
pub enum OpOutcome {
    /// Resolve the call's Promise with the value, converted as the slot says.
    Resolve(Value, &'static SlotDesc),
    /// Reject it with a `NativeError`.
    Reject(NativeError),
    /// Call its `options.onProgress` with the value (the Promise stays pending).
    Progress(Value, &'static SlotDesc),
    /// E3: hand items to the call's stream iterator (each converted as the slot says).
    Items(Vec<Value>, &'static SlotDesc),
}

/// One entry of an [`JsEngine::settle_ops`] batch.
#[derive(Clone, Debug)]
pub struct OpSettle {
    pub id: u64,
    pub outcome: OpOutcome,
}

pub trait JsEngine {
    /// Evaluates the entry module. Imports resolve relative to the entry file's directory and may
    /// not leave it. Jobs may remain queued afterwards.
    fn load_module(&mut self, entry: &Path) -> Result<(), JsError>;

    /// Calls `globalThis.__svelters_dispatch(handle, signal, args, deferred, flags)`. `deferred`
    /// marks a sync signal that could not run synchronously (its result is ignored); `flags` are
    /// the `SVL_LISTEN_*` bits of the connection that fired.
    fn dispatch(
        &mut self,
        handle: u64,
        signal: &str,
        args: &[Value],
        deferred: bool,
        flags: u32,
    ) -> Result<(), JsError>;

    /// Calls `globalThis.__svelters_dispatch_sync(handle, signal, args, flags)` and returns its
    /// result: `Null` for no decision, a bool for a stop flag, or the handler's explicit value.
    fn dispatch_sync(
        &mut self,
        handle: u64,
        signal: &str,
        args: &[Value],
        flags: u32,
    ) -> Result<Value, JsError>;

    /// Runs the animation-frame callbacks (`__svelters_internal.frame`).
    fn run_frame(&mut self) -> Result<(), JsError>;

    /// Calls `globalThis.__svelters_gone(handle, reason)` if the renderer installed it: the
    /// native object behind `handle` died (`reason` is a `gone_name`).
    fn dispatch_gone(&mut self, handle: u64, reason: &str) -> Result<(), JsError>;

    /// ABI v2.6: calls `globalThis.__svelters_changed([h, prop, value, h, prop, value, ...])` once
    /// with the whole batch, if the runtime installed it (REACTIVITY.md §5.2). `value` is
    /// `undefined` for a proportional property.
    fn dispatch_changed(&mut self, batch: &[ChangedProp]) -> Result<(), JsError>;

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

    /// Settles offloaded export calls (E1) in one engine entry: resolves or rejects their Promises
    /// and delivers progress. Ids the runtime does not know (an older runtime's) are ignored.
    /// Reactions run as jobs afterwards, like every other completion.
    fn settle_ops(&mut self, batch: &[OpSettle]) -> Result<(), JsError> {
        let _ = batch;
        Err(JsError::new(ErrorKind::Host, "this engine does not run offloaded exports"))
    }
}
