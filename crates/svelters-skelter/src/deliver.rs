//! Event-time delivery (architecture §11.1, compiler-and-js §3.10): what one input produced in the
//! tree reaches JS before the input's handling ends, not at the next frame through an outbox.
//!
//! The cancelable events are delivered inside the dispatch, before their native default (`sync.rs`);
//! skelter-core does not repeat them in its result. The rest comes from skelter-core: the events of
//! [`Document::dispatch`] (and those it queued
//! outside dispatch, [`Document::take_events`]: focus moves, kinds' signals) with their propagation
//! paths, and the native-owned prop changes of its change journal ([`Document::drain_changes`],
//! `Change::Prop { native: true }`). [`Output`] is that, gathered.
//!
//! Order, as the host's checkpoint delivers (REACTIVITY §5.3): observed native changes first (one
//! batch), then the events in order, each along its path (capture, target, bubble). A synchronous
//! event runs its listeners with `call_sync` so their answer can stop propagation and cancel the
//! native default (egui's bits: 1 stop, 2 prevent, 4 stop immediately; `true` for the non-propagating
//! `bool_stop` signals); the events before it are delivered first so handlers see them in order. One
//! checkpoint at the end delivers the rest and drains the microtasks. JS writes during delivery are a
//! JS task that saw the dispatch's revision. Lane RUNTIME's "dispatch and drain" host entry replaces
//! the closing checkpoint once it lands; the order stays.
//!
//! [`Document::dispatch`]: skelter_core::Document::dispatch
//! [`Document::take_events`]: skelter_core::Document::take_events
//! [`Document::drain_changes`]: skelter_core::Document::drain_changes

use crate::backend::SkelterBackend;
use crate::surface;
use crate::value::{self, PHASE_BUBBLE, PHASE_CAPTURE, PHASE_TARGET};
use skelter_base::event::InputEnvelope;
use skelter_base::ids::{NodeId, Revision};
use skelter_core::journal::Change;
use skelter_core::{Delivery, Dispatched, JsEvent, Phase, PropId};
use smallvec::SmallVec;
use svelters_abi::*;

/// `call_sync`'s answer bits for a propagating event (svelters-egui's `events.rs`).
const STOP: i64 = 1;
const PREVENT: i64 = 2;
const IMMEDIATE: i64 = 4;

/// What the tree produced for JS, in order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Output {
    pub events: Vec<JsEvent>,
    /// Native-owned props the kinds changed.
    pub changed: Vec<(NodeId, PropId)>,
    /// The revision the events were dispatched at.
    pub revision: Option<Revision>,
}

impl Output {
    pub fn is_empty(&self) -> bool {
        self.events.is_empty() && self.changed.is_empty()
    }

    pub(crate) fn append(&mut self, o: Output) {
        self.events.extend(o.events);
        self.changed.extend(o.changed);
        self.revision = self.revision.max(o.revision);
    }
}

/// What delivering one [`Output`] did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Delivered {
    /// Listener calls made.
    pub events: usize,
    /// Observed prop changes announced.
    pub changes: usize,
    /// Per event, in order: a synchronous listener cancelled its native default.
    pub cancelled: Vec<bool>,
}

impl SkelterBackend {
    /// Runs one input through skelter-core (hit testing, native defaults, focus) and delivers what it
    /// produced now.
    pub fn dispatch(&self, ev: &InputEnvelope) -> (Dispatched, Delivered) {
        let d = self.doc_mut().dispatch(ev);
        self.run_taps(ev, &d.events);
        // SV82: another backend's listeners on the path get their part at their next pump
        let mut out = self.route(Output { events: d.events.clone(), changed: Vec::new(), revision: Some(d.revision) });
        out.append(self.collect());
        (d, self.deliver(out))
    }

    /// Queues native output produced outside the tree (an embedder's own event, a test's) for the
    /// next [`SkelterBackend::pump`].
    pub fn queue(&self, out: Output) {
        let mine = self.route(out);
        self.0.pending.borrow_mut().append(mine);
    }

    /// Everything the tree and the queue hold for JS, taken. The tree's output is shared with the
    /// other backends over the document by owner (SV82): theirs waits in their queues.
    fn collect(&self) -> Output {
        let mut tree = Output::default();
        {
            let mut doc = self.doc_mut();
            tree.events.extend(doc.take_events());
            for c in doc.drain_changes() {
                if let Change::Prop { node, prop, native: true, revision } = c {
                    tree.changed.push((node, prop));
                    tree.revision = tree.revision.max(Some(revision));
                }
            }
        }
        let tree = self.route(tree);
        let mut out = std::mem::take(&mut *self.0.pending.borrow_mut());
        out.events.extend(tree.events);
        out.changed.extend(tree.changed);
        out.revision = out.revision.max(tree.revision);
        out
    }

    /// Delivers whatever native code produced outside an input (a kind's timer, an animation end, a
    /// programmatic focus move).
    pub fn pump(&self) -> Delivered {
        let ends = self.deliver_animation_ends();
        let out = self.collect();
        if out.is_empty() {
            if ends > 0
                && let Some(h) = self.host()
            {
                self.count(|t| t.checkpoints += 1);
                h.checkpoint();
            }
            return Delivered { events: ends, ..Delivered::default() };
        }
        let mut d = self.deliver(out);
        d.events += ends;
        d
    }

    /// The event skelter-core lists for `signal` emitted on `node` by its kind: target phase, `node`'s
    /// own listeners; `None` when it has none (skelter-core lists no event without a listener).
    pub fn target_event(&self, node: NodeId, signal: &str, args: Vec<surface::Value>) -> Option<JsEvent> {
        let doc = self.doc();
        let m = self.0.kinds.by_core(doc.kind_of(node)?)?;
        let sm = m.signal_named(signal)?;
        let sid = sm.native?;
        let path: SmallVec<[Delivery; 4]> = doc.listeners(node).iter().filter(|l| l.signal == sid).map(|l| Delivery { node, phase: Phase::Target, flags: l.flags }).collect();
        if path.is_empty() {
            return None;
        }
        Some(JsEvent { signal: sid, target: node, arg: value::signal_arg(sm.spec, args), path, cancelable: sm.spec.cancelable, sync: sm.spec.sync })
    }

    /// Delivers `out` to the attached host now. Deliveries on nodes JS has no handle for, and changes
    /// nobody observes, are dropped here.
    pub fn deliver(&self, out: Output) -> Delivered {
        let mut done = Delivered::default();
        let Some(host) = self.host() else { return done };
        let kinds = self.kinds().clone();
        let kind_of = |n: NodeId| self.doc().kind_of(n).and_then(|k| kinds.by_core(k));
        for (node, prop) in &out.changed {
            let (Some(h), Some(m)) = (self.handle(*node), kind_of(*node)) else { continue };
            for pm in m.of_native_prop(*prop) {
                if self.0.observed.borrow().contains_key(&(h, pm.wire)) {
                    self.count(|t| t.changes += 1);
                    host.prop_changed(h, pm.wire);
                    done.changes += 1;
                }
            }
        }
        let prev = self.0.seen.replace(out.revision);
        let mut posted = false;
        for ev in &out.events {
            let mut cancelled = false;
            let target = self.handle(ev.target).map_or(0, |h| h as i64);
            for d in &ev.path {
                let (Some(h), Some(m)) = (self.handle(d.node), kind_of(d.node)) else { continue };
                let Some(sm) = m.of_native_signal(ev.signal) else { continue };
                let phase = match d.phase {
                    Phase::Capture => PHASE_CAPTURE,
                    Phase::Target => PHASE_TARGET,
                    Phase::Bubble => PHASE_BUBBLE,
                };
                let args = value::signal_args(sm.spec, ev.arg.as_ref(), target, phase);
                done.events += 1;
                // A signal the surface does not declare sync is posted even when skelter-core
                // delivers it at event time (the text field's `changed`): its listeners get its
                // arguments alone, as on egui, not a `NativeEvent` first. Posting keeps the order:
                // a later sync event flushes the posted ones first (QC1-EXP-002, QC1-EXP-005).
                if !ev.sync || !sm.spec.sync {
                    self.count(|t| t.posts += 1);
                    host.post_event(h, sm.wire, &args);
                    posted = true;
                    continue;
                }
                if posted {
                    self.count(|t| t.checkpoints += 1);
                    host.checkpoint();
                    posted = false;
                }
                self.count(|t| t.syncs += 1);
                match host.call_sync(h, sm.wire, &args) {
                    Ok(Value::Bool(true)) => {
                        cancelled = true;
                        break;
                    }
                    Ok(Value::I64(bits)) => {
                        cancelled |= ev.cancelable && bits & PREVENT != 0;
                        if bits & (STOP | IMMEDIATE) != 0 {
                            break;
                        }
                    }
                    _ => {}
                }
            }
            done.cancelled.push(cancelled);
        }
        if done.events > 0 || done.changes > 0 {
            self.count(|t| t.checkpoints += 1);
            host.checkpoint();
        }
        self.0.seen.set(prev);
        done
    }
}
