//! Invalidation (ARCH-SCENE §7.1): dirty bits, layout boundaries and the per-root queues that
//! replace the backend's global `layout_dirty`/`remeasure` flags.
//!
//! - A **content** change of a node (children inserted or removed, a leaf's measured size) clears
//!   the layout caches from the node up to the nearest **layout boundary** and queues that boundary
//!   in its root's `relayout` queue. A **style** change of a node (its own size may change) clears its
//!   cache and continues as a content change of its parent.
//! - A boundary is a node whose size cannot depend on its content: a root's top, a node flagged
//!   [`NodeFlags::LAYOUT_BOUNDARY`], or a node with a definite width and height that cannot be
//!   shrunk below them by its automatic minimum size (`flex-shrink: 0`, or definite min sizes) and
//!   is not baseline-aligned. Relayout of a boundary (`layout.rs`) runs at its last size.
//! - A text change queues the leaf in `remeasure`; only a changed measured size invalidates.
//! - Detached trees have no queues: invalidation clears caches up to their top, so they are laid
//!   out fresh when attached.
//!
//! Writer lane: SCENE.

use super::Scene;
use super::id::{NodeId, RootIx};
use super::node::{Dirty, NodeFlags};

/// The queued layout work of one root.
#[derive(Default, Debug)]
pub(crate) struct RootDirty {
    /// Boundaries whose content changed (flag `IN_RELAYOUT`; stale or moved entries are skipped).
    pub relayout: Vec<NodeId>,
    /// Leaves whose content changed (flag `IN_REMEASURE`).
    pub remeasure: Vec<NodeId>,
    /// Lay out the whole root (new root, presented, or its top's own style changed).
    pub full: bool,
}

/// Clears the layout cache of `h` and its ancestors, up to the nearest layout boundary (pre-SG1
/// name, kept for callers of `layout::invalidate`).
pub fn invalidate(scene: &mut Scene, h: super::Handle) {
    if let Some(id) = NodeId::from_raw(h).filter(|&id| scene.idx(id).is_some()) {
        scene.invalidate_content(id);
    }
}

impl Scene {
    /// True if node `i`'s size cannot depend on its content.
    pub(crate) fn is_boundary(&self, i: usize) -> bool {
        let core = &self.core[i];
        if core
            .flags
            .intersects(NodeFlags::ROOT | NodeFlags::LAYOUT_BOUNDARY)
        {
            return true;
        }
        let c = self.common(i);
        if c.width.is_none()
            || c.height.is_none()
            || self.vis(i).clip.is_some()
            || c.align_self == "baseline"
        {
            return false;
        }
        if c.flex_shrink != 0.0 && (c.min_width.is_none() || c.min_height.is_none()) {
            return false;
        }
        match core.parent.and_then(|p| self.idx(p)) {
            Some(pi) => self.common(pi).align_items != "baseline",
            None => true,
        }
    }

    /// The content of `id` changed (children, measured size): clear caches up to the nearest
    /// boundary and queue it (attached), or up to the top (detached).
    pub(crate) fn invalidate_content(&mut self, id: NodeId) {
        let mut i = self.ix(id);
        let root = self.core[i].root;
        let mut first = true;
        loop {
            self.layout[i].cache.clear();
            self.core[i].dirty.insert(if first {
                Dirty::LAYOUT
            } else {
                Dirty::LAYOUT | Dirty::DESC_LAYOUT
            });
            first = false;
            if !root.is_detached() && self.is_boundary(i) {
                self.queue_relayout(root, i);
                return;
            }
            match self.core[i].parent {
                Some(p) => i = self.ix(p),
                None => return,
            }
        }
    }

    /// The style (own size) of `id` changed.
    pub(crate) fn invalidate_style(&mut self, id: NodeId) {
        let i = self.ix(id);
        self.layout[i].cache.clear();
        self.core[i].dirty.insert(Dirty::LAYOUT);
        match self.core[i].parent {
            Some(p) => self.invalidate_content(p),
            None => {
                let root = self.core[i].root;
                if !root.is_detached() {
                    self.queue_relayout(root, i);
                }
            }
        }
    }

    fn queue_relayout(&mut self, root: RootIx, i: usize) {
        if self.core[i].flags.contains(NodeFlags::IN_RELAYOUT) {
            return;
        }
        let Some(id) = self.slots.id_at(i) else {
            return;
        };
        self.core[i].flags.insert(NodeFlags::IN_RELAYOUT);
        self.roots
            .get_mut(root)
            .expect("live root")
            .dirty
            .relayout
            .push(id);
    }

    /// The content a leaf measures changed: queue it for remeasure in a laid-out root; elsewhere
    /// drop its measurement (it is measured at its next layout).
    pub(crate) fn mark_measure(&mut self, id: NodeId) {
        let i = self.ix(id);
        let root = self.core[i].root;
        let queued = self.roots.get(root).is_some_and(|e| e.laid_out());
        if !queued {
            self.layout[i].intrinsic = None;
            self.invalidate_content(id);
            return;
        }
        self.core[i].dirty.insert(Dirty::MEASURE);
        if !self.core[i].flags.contains(NodeFlags::IN_REMEASURE) {
            self.core[i].flags.insert(NodeFlags::IN_REMEASURE);
            self.roots
                .get_mut(root)
                .expect("live root")
                .dirty
                .remeasure
                .push(id);
        }
    }

    /// Marks root `r` for a full layout (a window was presented).
    pub(crate) fn mark_full(&mut self, r: RootIx) {
        if let Some(e) = self.roots.get_mut(r) {
            e.dirty.full = true;
        }
    }
}
