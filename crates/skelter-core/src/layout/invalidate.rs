//! Invalidation: dirty bits, layout boundaries and the per-window queues (ported from
//! svelters-egui's `scene/dirty.rs`).
//!
//! - A **content** change of a node (children inserted or removed, a leaf's measured size) clears
//!   the layout caches from the node up to the nearest **layout boundary** and queues that boundary
//!   in its window's `relayout` queue. A **style** change of a node (its own size may change) clears
//!   its cache and continues as a content change of its parent.
//! - A boundary is a node whose size cannot depend on its content: a window root, a node flagged
//!   [`NodeFlags::LAYOUT_BOUNDARY`], or a node with a definite width and height that cannot be shrunk
//!   below them by its automatic minimum size (`flex-shrink: 0`, or definite min sizes), that does
//!   not clip, and is not baseline-aligned. Relayout of a boundary runs at its last size.
//! - A text change queues the leaf in `remeasure`; only a changed measured size invalidates.
//! - Detached trees have no queues: invalidation clears caches up to their top, so they are laid out
//!   fresh when attached.

use crate::document::Document;
use crate::tree::{Dirty, NodeFlags};
use skelter_base::ids::WindowId;

impl Document {
    /// True if node `i`'s size cannot depend on its content.
    pub(crate) fn is_boundary(&self, i: usize) -> bool {
        let core = &self.core[i];
        if core.flags.intersects(NodeFlags::ROOT | NodeFlags::LAYOUT_BOUNDARY) {
            return true;
        }
        let st = &self.layout[i].style;
        let px = |t: usize| t == taffy::CompactLength::LENGTH_TAG;
        let clips = st.overflow.x != taffy::Overflow::Visible || st.overflow.y != taffy::Overflow::Visible;
        if !px(st.size.width.tag()) || !px(st.size.height.tag()) || clips || st.align_self == Some(taffy::AlignItems::BASELINE) {
            return false;
        }
        if st.flex_shrink != 0.0 && (!px(st.min_size.width.into_raw().tag()) || !px(st.min_size.height.into_raw().tag())) {
            return false;
        }
        match core.parent.and_then(|p| self.idx(p)) {
            Some(pi) => self.layout[pi].style.align_items != taffy::AlignItems::BASELINE,
            None => true,
        }
    }

    /// The content of node `i` changed (children, measured size): clear caches up to the nearest
    /// boundary and queue it (attached), or up to the top (detached).
    pub(crate) fn invalidate_content(&mut self, mut i: usize) {
        let window = self.core[i].window;
        let mut first = true;
        loop {
            self.layout[i].cache.clear();
            self.core[i].dirty.insert(if first { Dirty::LAYOUT } else { Dirty::LAYOUT | Dirty::DESC_LAYOUT });
            first = false;
            if let Some(w) = window
                && self.is_boundary(i)
            {
                self.queue_relayout(w, i);
                return;
            }
            match self.core[i].parent {
                Some(p) => i = self.ix(p),
                None => return,
            }
        }
    }

    /// The style (own size) of node `i` changed.
    pub(crate) fn invalidate_style(&mut self, i: usize) {
        self.layout[i].cache.clear();
        self.core[i].dirty.insert(Dirty::LAYOUT);
        match self.core[i].parent {
            Some(p) => {
                let pi = self.ix(p);
                self.invalidate_content(pi)
            }
            None => {
                if let Some(w) = self.core[i].window {
                    self.queue_relayout(w, i);
                }
            }
        }
    }

    fn queue_relayout(&mut self, w: WindowId, i: usize) {
        if self.core[i].flags.contains(NodeFlags::IN_RELAYOUT) {
            return;
        }
        let Some(id) = self.slots.id_at(i) else { return };
        let Some(ws) = self.window_mut(w) else { return };
        ws.dirty.relayout.push(id);
        self.core[i].flags.insert(NodeFlags::IN_RELAYOUT);
    }

    /// The content a leaf measures changed: queue it for remeasure in a laid-out window; elsewhere
    /// drop its measurement (it is measured at its next layout).
    pub(crate) fn mark_measure(&mut self, i: usize) {
        let window = self.core[i].window;
        let laid_out = window.and_then(|w| self.window(w)).is_some_and(|w| w.laid_out);
        if !laid_out {
            self.layout[i].intrinsic = None;
            self.invalidate_content(i);
            return;
        }
        self.core[i].dirty.insert(Dirty::MEASURE);
        if !self.core[i].flags.contains(NodeFlags::IN_REMEASURE) {
            let id = self.slots.id_at(i).expect("live");
            if let Some(ws) = window.and_then(|w| self.window_mut(w)) {
                ws.dirty.remeasure.push(id);
            }
            self.core[i].flags.insert(NodeFlags::IN_REMEASURE);
        }
    }
}
