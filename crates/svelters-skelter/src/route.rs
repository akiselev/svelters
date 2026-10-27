//! SV82: several backends over one document ([`SkelterBackend::shared`], one host each). The document
//! has one sync-delivery slot and one queue of events and changes, and a backend delivers only on
//! nodes it holds handles for. So every backend over a document joins that document's [`Router`]:
//! it owns the slot, and splits what the tree produced by owner (the backend holding the node's
//! handle), so each backend gets its own events and changes, whichever one pumps.
//!
//! An event's path is cut into runs of one owner, in path order; each run reaches its owner as an
//! event of its own (same signal, target and argument). A cancelable event stops at the first run
//! whose listeners stop it; `preventDefault` in any run cancels the default. A node two backends
//! hold (both adopted it) reaches both. Deliveries no backend owns are dropped, as before.
//!
//! With one backend nothing is split: it gets everything, as before routing existed.

use crate::backend::{Inner, SkelterBackend};
use crate::deliver::Output;
use skelter_base::ids::NodeId;
use skelter_core::{Delivery, Document, JsEvent, Outcome};
use smallvec::SmallVec;
use std::cell::RefCell;
use std::rc::{Rc, Weak};

/// The backends over one document.
#[derive(Default)]
pub(crate) struct Router {
    members: RefCell<Vec<Weak<Inner>>>,
}

thread_local! {
    /// Each shared document's router.
    static ROUTERS: RefCell<Vec<(Weak<RefCell<Document>>, Weak<Router>)>> = const { RefCell::new(Vec::new()) };
}

impl Router {
    /// The router of `doc`, made on first use.
    pub(crate) fn of(doc: &Rc<RefCell<Document>>) -> Rc<Router> {
        ROUTERS.with(|rs| {
            let mut rs = rs.borrow_mut();
            rs.retain(|(d, r)| d.strong_count() > 0 && r.strong_count() > 0);
            if let Some(r) = rs.iter().find(|(d, _)| std::ptr::eq(d.as_ptr(), Rc::as_ptr(doc))).and_then(|(_, r)| r.upgrade()) {
                return r;
            }
            let r = Rc::new(Router::default());
            rs.push((Rc::downgrade(doc), Rc::downgrade(&r)));
            r
        })
    }

    /// Adds a backend; the first one installs the document's deliverer.
    pub(crate) fn join(self: &Rc<Router>, b: &SkelterBackend) {
        let first = {
            let mut m = self.members.borrow_mut();
            m.retain(|w| w.strong_count() > 0);
            m.push(Rc::downgrade(&b.0));
            m.len() == 1
        };
        if first {
            let weak = Rc::downgrade(self);
            b.doc_mut().set_sync_delivery(Box::new(move |doc: &mut Document, ev: &JsEvent| match weak.upgrade() {
                Some(r) => r.deliver_sync(doc, ev),
                None => Outcome::default(),
            }));
        }
    }

    /// The live backends, in join order.
    pub(crate) fn members(&self) -> Vec<SkelterBackend> {
        self.members.borrow().iter().filter_map(Weak::upgrade).map(SkelterBackend).collect()
    }

    fn deliver_sync(&self, doc: &mut Document, ev: &JsEvent) -> Outcome {
        let members = self.members();
        if members.len() <= 1 {
            return members.first().map_or_else(Outcome::default, |b| b.deliver_lent(doc, ev));
        }
        let mut out = Outcome::default();
        for (i, part) in split(ev, &members) {
            let o = members[i].deliver_lent(doc, &part);
            out.prevented |= o.prevented;
            if o.stopped {
                out.stopped = true;
                break;
            }
        }
        out
    }
}

/// `ev` cut into runs of one owner, in path order: (member index, its event).
pub(crate) fn split(ev: &JsEvent, members: &[SkelterBackend]) -> Vec<(usize, JsEvent)> {
    let mut runs: Vec<(usize, SmallVec<[Delivery; 4]>)> = Vec::new();
    for d in &ev.path {
        for (i, b) in members.iter().enumerate() {
            if b.handle(d.node).is_none() {
                continue;
            }
            match runs.last_mut() {
                Some((j, path)) if *j == i => path.push(*d),
                _ => runs.push((i, SmallVec::from_slice(&[*d]))),
            }
        }
    }
    runs.into_iter().map(|(i, path)| (i, JsEvent { path, ..ev.clone() })).collect()
}

impl SkelterBackend {
    /// SV82: hands `out` to the backends that own its parts; returns this backend's part. The
    /// others' parts wait in their queues for their next pump, and they are woken.
    pub(crate) fn route(&self, out: Output) -> Output {
        let members = self.0.router.members();
        if members.len() <= 1 {
            return out;
        }
        let me = members.iter().position(|b| Rc::ptr_eq(&b.0, &self.0));
        let mut parts: Vec<Output> = members.iter().map(|_| Output::default()).collect();
        for ev in &out.events {
            for (i, part) in split(ev, &members) {
                parts[i].events.push(part);
            }
        }
        for &(node, prop) in &out.changed {
            for (i, b) in members.iter().enumerate() {
                if owns(b, node) {
                    parts[i].changed.push((node, prop));
                }
            }
        }
        let mut mine = Output { revision: out.revision, ..Output::default() };
        for (i, mut part) in parts.into_iter().enumerate() {
            if Some(i) == me {
                mine.events = part.events;
                mine.changed = part.changed;
            } else if !part.is_empty() {
                part.revision = out.revision;
                members[i].0.pending.borrow_mut().append(part);
                members[i].wake_for_output();
            }
        }
        mine
    }
}

impl SkelterBackend {
    /// Another backend over the document queued output for this one: its loop runs a pass.
    fn wake_for_output(&self) {
        self.0.wake.wake();
        svelters_abi::Backend::request_frame(self);
    }
}

fn owns(b: &SkelterBackend, n: NodeId) -> bool {
    b.handle(n).is_some()
}
