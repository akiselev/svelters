//! Event-time delivery inside skelter-core's dispatch (EV1; decided by the coordinator, 2026-10-25,
//! skelter-core's lent-document `SyncDelivery`): the cancelable DOM-style events (pointer down/up,
//! click, wheel, keys) and kinds' `emit_sync` signals reach JS before their native default, so
//! `preventDefault` skips it, kinds' own cancelable signals included.
//!
//! skelter-core takes the deliverer out of the document and calls it with `&mut Document`, no other
//! borrow outstanding. For the call, the backend's ops use that document ([`SkelterBackend::doc`] and
//! [`SkelterBackend::doc_mut`] hand out the lent one, with the `RefCell` borrow rules checked), so a
//! listener reads the live tree, writes (in `js_transaction` at the event's revision, which the open
//! dispatch transaction shares) and builds nodes as anywhere else. The path is fixed when the event
//! is built: listener nodes gone by their turn are skipped. No delivery nests: a cancelable signal
//! raised meanwhile comes back in the dispatch's events.

use crate::backend::SkelterBackend;
use crate::value::{self, PHASE_BUBBLE, PHASE_CAPTURE, PHASE_TARGET};
use skelter_core::{Document, JsEvent, Outcome, Phase};
use std::cell::{Cell, Ref, RefMut};
use std::ops::{Deref, DerefMut};
use std::rc::Rc;
use svelters_abi::*;

/// `call_sync`'s answer bits for a propagating event (svelters-egui's `events.rs`).
const STOP: i64 = 1;
const PREVENT: i64 = 2;
const IMMEDIATE: i64 = 4;

/// A shared borrow of the backend's document (its own, or the one lent to the deliverer).
pub enum DocRef<'a> {
    Own(Ref<'a, Document>),
    Lent(&'a Document, &'a Cell<isize>),
}

impl Deref for DocRef<'_> {
    type Target = Document;
    fn deref(&self) -> &Document {
        match self {
            DocRef::Own(r) => r,
            DocRef::Lent(d, _) => d,
        }
    }
}

impl Drop for DocRef<'_> {
    fn drop(&mut self) {
        if let DocRef::Lent(_, n) = self {
            n.set(n.get() - 1);
        }
    }
}

/// An exclusive borrow of the backend's document.
pub enum DocMut<'a> {
    Own(RefMut<'a, Document>),
    Lent(&'a mut Document, &'a Cell<isize>),
}

impl Deref for DocMut<'_> {
    type Target = Document;
    fn deref(&self) -> &Document {
        match self {
            DocMut::Own(r) => r,
            DocMut::Lent(d, _) => d,
        }
    }
}

impl DerefMut for DocMut<'_> {
    fn deref_mut(&mut self) -> &mut Document {
        match self {
            DocMut::Own(r) => r,
            DocMut::Lent(d, _) => d,
        }
    }
}

impl Drop for DocMut<'_> {
    fn drop(&mut self) {
        if let DocMut::Lent(_, n) = self {
            n.set(0);
        }
    }
}

impl SkelterBackend {
    /// The document, shared.
    ///
    /// # Panics
    /// Like `RefCell::borrow`, while it is borrowed exclusively.
    pub fn doc(&self) -> DocRef<'_> {
        let p = self.0.lent.get();
        if p.is_null() {
            return DocRef::Own(self.0.doc.borrow());
        }
        let n = &self.0.lent_borrows;
        assert!(n.get() >= 0, "svelters-skelter: the lent document is borrowed exclusively");
        n.set(n.get() + 1);
        // SAFETY: `lent` is the `&mut Document` skelter-core passed to the deliverer, valid for its
        // call (`deliver_lent` clears it before returning); `lent_borrows` enforces the borrow rules
        // between the guards made from it.
        DocRef::Lent(unsafe { &*p }, n)
    }

    /// The document, exclusively.
    ///
    /// # Panics
    /// Like `RefCell::borrow_mut`, while it is borrowed.
    pub fn doc_mut(&self) -> DocMut<'_> {
        let p = self.0.lent.get();
        if p.is_null() {
            return DocMut::Own(self.0.doc.borrow_mut());
        }
        let n = &self.0.lent_borrows;
        assert!(n.get() == 0, "svelters-skelter: the lent document is already borrowed");
        n.set(-1);
        // SAFETY: as in `doc`; no other guard exists (checked above).
        DocMut::Lent(unsafe { &mut *p }, n)
    }

    pub(crate) fn install_sync_delivery(&self) {
        let weak = Rc::downgrade(&self.0);
        self.doc_mut().set_sync_delivery(Box::new(move |doc: &mut Document, ev: &JsEvent| match weak.upgrade() {
            Some(inner) => SkelterBackend(inner).deliver_lent(doc, ev),
            None => Outcome::default(),
        }));
    }

    /// What skelter-core's dispatch calls for a cancelable event: `doc` is lent for the call.
    fn deliver_lent(&self, doc: &mut Document, ev: &JsEvent) -> Outcome {
        if !self.0.taps.borrow().is_empty() {
            // the event taps see the events delivered at event time too (A2)
            self.0.tapped.borrow_mut().push(ev.clone());
        }
        let prev = self.0.lent.replace(doc as *mut Document);
        let borrows = self.0.lent_borrows.replace(0);
        let out = self.deliver_cancelable(ev);
        self.0.lent_borrows.set(borrows);
        self.0.lent.set(prev);
        out
    }

    /// One cancelable event along its path, its listeners run now; returns what they did. JS writes
    /// are a task that saw the event's revision.
    pub fn deliver_cancelable(&self, ev: &JsEvent) -> Outcome {
        let mut out = Outcome::default();
        let Some(host) = self.host() else { return out };
        let prev = self.0.seen.replace(Some(self.doc().revision()));
        let target = self.handle(ev.target).map_or(0, |h| h as i64);
        for d in &ev.path {
            // a listener node removed by an earlier listener is skipped
            if !self.doc().contains(d.node) {
                continue;
            }
            let Some(h) = self.handle(d.node) else { continue };
            let Ok(m) = self.kind_map(h) else { continue };
            let Some(sm) = m.of_native_signal(ev.signal) else { continue };
            let phase = match d.phase {
                Phase::Capture => PHASE_CAPTURE,
                Phase::Target => PHASE_TARGET,
                Phase::Bubble => PHASE_BUBBLE,
            };
            let args = value::signal_args(sm.spec, ev.arg.as_ref(), target, phase);
            // FQ-65: a signal the surface does not declare sync (a checkbox's `changed`) is called
            // now only so it can be cancelled: its listeners get its arguments as when posted
            // (as on egui), the event last (`SVL_SIGNAL_ARGS`).
            let wire = if sm.spec.sync { sm.wire } else { sm.wire | SVL_SIGNAL_ARGS << SVL_SIGNAL_FLAGS_SHIFT };
            self.count(|t| t.syncs += 1);
            match host.call_sync(h, wire, &args) {
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
        self.0.seen.set(prev);
        out
    }
}
