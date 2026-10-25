//! Event-time delivery inside presto-core's dispatch (EV1; decided by the coordinator, 2026-10-25):
//! the cancelable DOM-style events (pointer down/up, click, wheel, keys) and kinds' `emit_sync`
//! signals reach JS before their native default, through `Document::set_sync_delivery`, so
//! `preventDefault` skips the default. presto-core needs only the answer (prevented, stopped) at that
//! point.
//!
//! The deliverer runs while presto-core's dispatch holds the document, so the JS it runs cannot reach
//! the tree. Its ops are deferred: they are checked against the kind maps (which need no document),
//! queued in order, and applied right after the dispatch returns, in `js_transaction(revision)`.
//! Handles are the backend's, so a node created in a listener has its handle at once. A read of a
//! prop the listener wrote answers the written value; a read of anything else fails with a message
//! that says why (the tree is mid-dispatch). Ops that fail when applied are collected
//! ([`PrestoBackend::take_deferred_errors`]).

use crate::backend::PrestoBackend;
use crate::value::{self, PHASE_BUBBLE, PHASE_CAPTURE, PHASE_TARGET};
use presto_core::{JsEvent, Outcome, Phase};
use std::collections::HashMap;
use std::rc::Rc;
use svelters_abi::*;

/// `call_sync`'s answer bits for a propagating event (svelters-egui's `events.rs`).
const STOP: i64 = 1;
const PREVENT: i64 = 2;
const IMMEDIATE: i64 = 4;

type Op = Box<dyn FnOnce(&PrestoBackend) -> StrResult<()>>;

/// The ops JS made while the tree was mid-dispatch.
#[derive(Default)]
pub struct Deferred {
    ops: Vec<Op>,
    /// The props written, for reads in the same listeners.
    written: HashMap<(svl_handle, svl_id), Value>,
}

impl PrestoBackend {
    pub(crate) fn deferring(&self) -> bool {
        self.0.sync.borrow().is_some()
    }

    pub(crate) fn defer(&self, op: impl FnOnce(&PrestoBackend) -> StrResult<()> + 'static) {
        if let Some(d) = self.0.sync.borrow_mut().as_mut() {
            d.ops.push(Box::new(op));
        }
    }

    pub(crate) fn defer_write(&self, h: svl_handle, prop: svl_id, v: Value) {
        if let Some(d) = self.0.sync.borrow_mut().as_mut() {
            d.written.insert((h, prop), v.clone());
            d.ops.push(Box::new(move |b: &PrestoBackend| b.write_prop(h, prop, &v)));
        }
    }

    pub(crate) fn deferred_read(&self, h: svl_handle, prop: svl_id) -> StrResult<Value> {
        let sync = self.0.sync.borrow();
        if let Some(v) = sync.as_ref().and_then(|d| d.written.get(&(h, prop))) {
            return Ok(v.clone());
        }
        let km = self.kind_map(h)?;
        let name = km.prop(prop).map_or("?", |p| p.spec.name);
        Err(format!("<{}>.{name} cannot be read inside a cancelable event's listener on presto (the tree is mid-dispatch); read it after the event", km.tag()))
    }

    /// Ops that failed when applied after a dispatch, since the last call.
    pub fn take_deferred_errors(&self) -> Vec<String> {
        std::mem::take(&mut self.0.deferred_errors.borrow_mut())
    }

    /// Applies the deferred ops in order, as a JS task that saw `revision`.
    pub(crate) fn apply_deferred(&self, revision: Option<presto_base::ids::Revision>) {
        let Some(d) = self.0.sync.borrow_mut().take() else { return };
        let prev = self.0.seen.replace(revision.or(self.0.seen.get()));
        for op in d.ops {
            if let Err(e) = op(self) {
                self.0.deferred_errors.borrow_mut().push(e);
            }
        }
        self.0.seen.set(prev);
    }

    pub(crate) fn install_sync_delivery(&self) {
        let weak = Rc::downgrade(&self.0);
        self.0.doc.borrow_mut().set_sync_delivery(Box::new(move |_doc: &mut presto_core::Document, ev: &JsEvent| match weak.upgrade() {
            Some(inner) => PrestoBackend(inner).deliver_cancelable(ev),
            None => Outcome::default(),
        }));
    }

    /// One cancelable event, along its path, without the document: what presto-core's dispatch calls
    /// (see the module docs). JS ops from here on are deferred until the next dispatch return, pump
    /// or `with_doc`.
    pub fn deliver_cancelable(&self, ev: &JsEvent) -> Outcome {
        let mut out = Outcome::default();
        let Some(host) = self.host() else { return out };
        self.0.sync.borrow_mut().get_or_insert_with(Deferred::default);
        let target = self.handle(ev.target).map_or(0, |h| h as i64);
        for d in &ev.path {
            let Some(h) = self.handle(d.node) else { continue };
            let Ok(m) = self.kind_map(h) else { continue };
            let Some(sm) = m.of_native_signal(ev.signal) else { continue };
            let phase = match d.phase {
                Phase::Capture => PHASE_CAPTURE,
                Phase::Target => PHASE_TARGET,
                Phase::Bubble => PHASE_BUBBLE,
            };
            let args = value::signal_args(sm.spec, ev.arg.as_ref(), target, phase);
            match host.call_sync(h, sm.wire, &args) {
                Ok(Value::Bool(true)) => {
                    out.prevented = ev.cancelable;
                    out.stopped = true;
                }
                Ok(Value::I64(bits)) => {
                    out.prevented |= ev.cancelable && bits & PREVENT != 0;
                    out.stopped |= bits & (STOP | IMMEDIATE) != 0;
                }
                _ => {}
            }
            if out.stopped {
                break;
            }
        }
        out
    }
}
