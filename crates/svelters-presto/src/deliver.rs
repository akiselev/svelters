//! Event-time delivery (architecture §11.1, compiler-and-js §3.10): what one input produced in the
//! tree ([`Dispatched`]: `Document::dispatch`'s result) reaches JS before the input's handling ends,
//! not at the next frame through an outbox.
//!
//! Order, as the host's checkpoint delivers (REACTIVITY §5.3): observed native-owned prop changes
//! first (one batch), then the events in order. A cancelable event runs its listeners synchronously
//! (`call_sync`) so its result can cancel the native default; the events before it are delivered
//! first so handlers see them in order. One checkpoint at the end delivers the rest and drains the
//! microtasks. Lane RUNTIME's "dispatch and drain" host entry replaces the closing checkpoint once
//! it lands; the order stays.

use crate::backend::PrestoBackend;
use crate::shim::Dispatched;
use crate::value;
use svelters_abi::*;

/// What delivering one [`Dispatched`] did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Delivered {
    /// Events that reached a listener.
    pub events: usize,
    /// Observed prop changes announced.
    pub changes: usize,
    /// Per input event, in order: a synchronous listener cancelled its native default.
    pub cancelled: Vec<bool>,
}

impl PrestoBackend {
    /// Delivers `d` to the attached host now. Events on nodes JS has no handle for, or no listener
    /// on, and changes nobody observes, are dropped here.
    pub fn deliver(&self, d: Dispatched) -> Delivered {
        let mut out = Delivered::default();
        let Some(host) = self.host() else { return out };
        let reg = self.registry().clone();
        for (node, prop) in &d.changed {
            let (Some(h), Some(k)) = (self.handle(*node), self.0.doc.borrow().kind(*node)) else { continue };
            let wire = self.wire().prop_wire(&reg, k, *prop);
            if wire != 0 && self.0.observed.borrow().contains_key(&(h, wire)) {
                host.prop_changed(h, wire);
                out.changes += 1;
            }
        }
        let mut posted = false;
        for ev in &d.events {
            let mut cancelled = false;
            let target = self.handle(ev.node).zip(self.0.doc.borrow().kind(ev.node));
            if let Some((h, k)) = target
                && let Some(spec) = reg.signal_spec(ev.signal)
                && self.0.doc.borrow().listening(ev.node, ev.signal).is_some()
            {
                let wire = self.wire().signal_wire(&reg, k, ev.signal);
                let args: Vec<Value> = ev.args.iter().zip(spec.args.iter().map(|(_, t)| *t).chain(std::iter::repeat(crate::shim::ValueType::Str))).map(|(a, t)| value::to_abi(a, t)).collect();
                if wire != 0 {
                    if spec.sync {
                        if posted {
                            host.checkpoint();
                            posted = false;
                        }
                        cancelled = matches!(host.call_sync(h, wire, &args), Ok(Value::Bool(true)));
                    } else {
                        host.post_event(h, wire, &args);
                        posted = true;
                    }
                    out.events += 1;
                }
            }
            out.cancelled.push(cancelled);
        }
        if out.events > 0 || out.changes > 0 {
            host.checkpoint();
        }
        out
    }

    /// Delivers whatever native code produced outside an input (a kind's timer, an animation end):
    /// the tree's pending output.
    pub fn pump(&self) -> Delivered {
        let d = self.0.doc.borrow_mut().take_dispatched();
        if d.is_empty() {
            return Delivered::default();
        }
        self.deliver(d)
    }
}
