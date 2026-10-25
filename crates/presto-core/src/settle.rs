//! Settling (ARCH-EMBED §5.4): why a document is not idle, as bits, for wait-idle in tests, agents and
//! cadatomic's idle barrier. Compositor-only cosmetic animation (the caret blink) is not busy; a
//! running transition is.

use crate::document::Document;
use crate::tree::Dirty;

/// Why the document is busy. The values of the bits it shares with svelters-egui's `Busy` are the
/// same (`LAYOUT`, `FRAME`, `EVENTS`), so callers that merge both read one bit set.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Busy(pub u32);

impl Busy {
    /// JS events queued and not taken (`take_events`).
    pub const EVENTS: Busy = Busy(1);
    /// A frame is needed: record, property or accessibility work is dirty.
    pub const FRAME: Busy = Busy(32);
    /// Style or layout work is queued.
    pub const LAYOUT: Busy = Busy(128);
    /// A transition or kind animation is running.
    pub const ANIMATING: Busy = Busy(1 << 13);
    /// A commit was built and the compositor has not applied it yet.
    pub const COMMIT_IN_FLIGHT: Busy = Busy(1 << 14);

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
    pub const fn contains(self, o: Busy) -> bool {
        self.0 & o.0 == o.0
    }
    pub fn insert(&mut self, o: Busy) {
        self.0 |= o.0;
    }
}

impl Document {
    /// Why the document is not settled (empty = settled).
    pub fn busy(&self) -> Busy {
        let mut b = Busy::default();
        if !self.input_events.is_empty() {
            b.insert(Busy::EVENTS);
        }
        if !self.anim.running.is_empty() {
            b.insert(Busy::ANIMATING);
        }
        for w in &self.windows {
            if w.commit_in_flight() {
                b.insert(Busy::COMMIT_IN_FLIGHT);
            }
            if w.dirty.full || !w.dirty.relayout.is_empty() || !w.dirty.remeasure.is_empty() {
                b.insert(Busy::LAYOUT);
            }
            if w.rebuild || !w.pending.is_empty() {
                b.insert(Busy::FRAME);
            }
            if let Some(i) = self.idx(w.root) {
                let d = self.core[i].dirty;
                if d.intersects(Dirty::STYLE | Dirty::DESC_STYLE | Dirty::STYLE_SUBTREE) {
                    b.insert(Busy::LAYOUT);
                }
                if d.intersects(Dirty::RECORD.union(Dirty::DESC_RECORD)) {
                    b.insert(Busy::FRAME);
                }
            }
        }
        b
    }
}
