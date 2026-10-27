//! Input filters (S1): native code that sees pointer and wheel events after hit testing and before
//! JS and the kinds' handlers, and may consume them. A tool that owns the pointer over a native
//! canvas, or an overlay that walks the user through the UI, takes the events it wants here instead
//! of patching each kind. Filters are Rust only: a JS filter on every pointer move would cost a
//! crossing per move.
//!
//! Filters run highest [`priority`](Document::add_input_filter) first, in the order added within a
//! priority, until one returns [`Filtered::Consumed`]. A consumed event changes nothing else: no
//! hover, press, focus, kind handler or JS event. [`Dispatched::filtered_by`] names the owner of the
//! filter that consumed it, so a recorder can note it. No filter runs during an IME composition or
//! an in-app drag, nor for keys (the key hook, [`Document::set_key_hook`], is their twin).

use crate::document::Document;
use crate::input::Dispatched;
use skelter_base::event::{InputEnvelope, InputEvent};
use skelter_base::geom::Point;
use skelter_base::ids::{NodeId, WindowId};
use std::sync::Arc;

/// What a filter decided.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Filtered {
    /// The event goes on: to the next filter, then the normal dispatch.
    Continue,
    /// The filter took the event: nothing else sees it.
    Consumed,
}

/// What a filter sees besides the input. Fields are added over time, so it is built only here.
#[non_exhaustive]
pub struct FilterCx<'a> {
    pub doc: &'a mut Document,
    pub window: WindowId,
    /// The hit target, or the node holding the pointer's capture; `None` over nothing.
    pub target: Option<NodeId>,
    /// The event's position in the target's local space.
    pub local: Option<Point>,
}

/// A pointer and wheel filter ([`Document::add_input_filter`]).
pub type InputFilter = Box<dyn FnMut(&mut FilterCx<'_>, &InputEnvelope) -> Filtered>;

struct Entry {
    priority: i16,
    owner: Arc<str>,
    f: InputFilter,
}

/// The document's filters, sorted by priority (highest first), stable within a priority.
#[derive(Default)]
pub(crate) struct Filters {
    entries: Vec<Entry>,
    /// Filters are running (taken out of `entries`).
    running: bool,
    /// Owners removed while the filters ran.
    removed: Vec<Arc<str>>,
}

impl Filters {
    fn insert(&mut self, e: Entry) {
        let at = self.entries.iter().position(|x| x.priority < e.priority).unwrap_or(self.entries.len());
        self.entries.insert(at, e);
    }
}

impl Document {
    /// Adds a pointer and wheel filter owned by `owner` (a stable id; [`Document::remove_input_filters`]
    /// removes all of an owner's filters). Higher `priority` runs first; equal priorities run in the
    /// order added. A consumed event changes nothing else; no filter runs during an IME
    /// composition or an in-app drag.
    pub fn add_input_filter(&mut self, priority: i16, owner: impl Into<Arc<str>>, f: impl FnMut(&mut FilterCx<'_>, &InputEnvelope) -> Filtered + 'static) {
        self.input_filters.insert(Entry { priority, owner: owner.into(), f: Box::new(f) });
    }

    /// Removes every filter `owner` added (a filter may remove its own while it runs).
    pub fn remove_input_filters(&mut self, owner: &str) {
        self.input_filters.entries.retain(|e| &*e.owner != owner);
        if self.input_filters.running {
            self.input_filters.removed.push(owner.into());
        }
    }

    /// The owners of the filters, in the order they run.
    pub fn input_filter_owners(&self) -> Vec<Arc<str>> {
        self.input_filters.entries.iter().map(|e| e.owner.clone()).collect()
    }

    /// Runs the filters on `ev`; true when one consumed it (`out` says which).
    pub(crate) fn filter_input(&mut self, ev: &InputEnvelope, out: &mut Dispatched) -> bool {
        if self.input_filters.entries.is_empty() || self.input_filters.running || self.input.composing {
            return false;
        }
        let window = ev.window;
        let (target, pos) = match &ev.event {
            InputEvent::Pointer(p) => {
                // an in-app drag keeps the pointer until it ends
                if self.input.drag.is_some() {
                    return false;
                }
                let captured = self.input.capture.get(&p.pointer).copied().filter(|&n| self.contains(n));
                (captured.or_else(|| self.hit_test(window, p.pos)), p.pos)
            }
            InputEvent::Wheel(w) => (self.hit_test_ex(window, w.pos, true), w.pos),
            _ => return false,
        };
        let local = target.map(|t| self.to_local(window, t, pos));
        let mut entries = std::mem::take(&mut self.input_filters.entries);
        self.input_filters.running = true;
        let mut by = None;
        for e in &mut entries {
            let mut cx = FilterCx { doc: self, window, target, local };
            if (e.f)(&mut cx, ev) == Filtered::Consumed {
                by = Some(e.owner.clone());
                break;
            }
        }
        self.input_filters.running = false;
        // removals made while running apply to the filters that ran; filters added join after them
        let added = std::mem::take(&mut self.input_filters.entries);
        let removed = std::mem::take(&mut self.input_filters.removed);
        entries.retain(|e| !removed.contains(&e.owner));
        self.input_filters.entries = entries;
        for e in added {
            self.input_filters.insert(e);
        }
        let Some(owner) = by else { return false };
        out.target = target;
        out.handled = true;
        out.filtered_by = Some(owner);
        true
    }
}
