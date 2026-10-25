//! Scroll containers: nodes whose computed `overflow` scrolls on an axis. The offset is native-owned
//! state of the document (architecture §4.5): wheel defaults, the compositor's flings
//! ([`Document::apply_scroll`]) and `scrollTo` from JS write it; a write marks only the node's
//! property-tree entries (a `Property` change: no relayout, no re-record).

use crate::document::Document;
use crate::tree::Dirty;
use presto_base::geom::{Size, Vec2};
use presto_base::ids::{NodeId, PropNodeId, WindowId};

impl Document {
    /// The axes node `i` scrolls on: its CSS `overflow`, or its kind's [`SelfScroll`](crate::SelfScroll).
    pub(crate) fn scroll_axes(&self, i: usize) -> (bool, bool) {
        let s = self.style.computed(i);
        let own = self.slots.id_at(i).and_then(|n| self.self_scroll.get(&n));
        (s.overflow_x.scrolls() || own.is_some_and(|o| o.x), s.overflow_y.scrolls() || own.is_some_and(|o| o.y))
    }

    /// Node `i`'s viewport (padding box) size when it is a scroll container.
    pub(crate) fn scroll_viewport(&self, i: usize) -> Option<Size> {
        let s = self.style.computed(i);
        if self.scroll_axes(i) == (false, false) {
            return None;
        }
        let l = &self.layout[i].layout;
        let b = s.border_widths;
        Some(Size::new((l.size.width - b.left - b.right).max(0.0), (l.size.height - b.top - b.bottom).max(0.0)))
    }

    /// The scrollable content size of node `i`: the reachable extent of taffy's scrollable overflow
    /// rect (measured from the padding box's scroll origin), at least the viewport.
    pub(crate) fn scroll_content(&self, i: usize) -> Size {
        let r = &self.layout[i].layout.scrollable_overflow_rect;
        let v = self.scroll_viewport(i).unwrap_or(Size::ZERO);
        // a virtualising node scrolls over its model's extent, not its realised rows'
        let (w, h) = match self.slots.id_at(i).and_then(|n| self.virtual_extent(n)) {
            Some(e) => (e.x.max(r.right), e.y.max(r.bottom)),
            None => (r.right, r.bottom),
        };
        // a kind that scrolls itself says how big its content is
        let (w, h) = match self.slots.id_at(i).and_then(|n| self.self_scroll.get(&n)) {
            Some(o) => (w.max(o.content.w), h.max(o.content.h)),
            None => (w, h),
        };
        Size::new(w.max(v.w), h.max(v.h))
    }

    /// `v` clamped to node `i`'s scroll range, on the axes it scrolls.
    pub(crate) fn clamp_scroll(&self, i: usize, v: Vec2) -> Vec2 {
        let Some(view) = self.scroll_viewport(i) else { return Vec2::ZERO };
        let c = self.scroll_content(i);
        let (sx, sy) = self.scroll_axes(i);
        let x = if sx { v.x.clamp(0.0, (c.w - view.w).max(0.0)) } else { 0.0 };
        let y = if sy { v.y.clamp(0.0, (c.h - view.h).max(0.0)) } else { 0.0 };
        Vec2::new(x, y)
    }

    /// The scroll offset of a scroll container (zero for other nodes).
    pub fn scroll_offset(&self, node: NodeId) -> Vec2 {
        self.scroll.get(&node).copied().unwrap_or(Vec2::ZERO)
    }

    /// Sets a scroll container's offset (clamped), as native state.
    pub fn set_scroll_offset(&mut self, node: NodeId, v: Vec2) {
        let Some(i) = self.idx(node) else { return };
        let v = self.clamp_scroll(i, v);
        if self.scroll_offset(node) == v {
            return;
        }
        self.stamp();
        self.scroll.insert(node, v);
        self.mark(node, Dirty::PROPERTY);
        if let Some(w) = self.core[i].window
            && let Some(ws) = self.window_mut(w)
        {
            ws.rebuild = true;
        }
    }

    /// The compositor moved a scroll offset (fling, smooth scroll): take it as native state.
    pub fn apply_scroll(&mut self, window: WindowId, node: PropNodeId, offset: Vec2) {
        let Some(n) = self.window(window).and_then(|w| w.scroll_nodes.get(&node).copied()) else { return };
        self.set_scroll_offset(n, offset);
    }

    /// Sets a scroll container's offset as [`Document::set_scroll_offset`] does and also sends it
    /// to the compositor at once ([`Document::take_fast_commits`]); the next commit agrees.
    pub fn scroll_fast(&mut self, node: NodeId, v: Vec2) {
        self.set_scroll_offset(node, v);
        let Some(i) = self.idx(node) else { return };
        if let (Some(window), Some(sc)) = (self.core[i].window, self.record[i].scroll) {
            let offset = self.scroll_offset(node);
            self.fast.push(presto_base::commit::FastCommit::ScrollOffset { window, node: sc, offset, revision: self.revision, input_seq: self.input_seq });
        }
    }

    /// The fast commits queued since the last call (kinds' scrolls and carets), for the runner to
    /// send with `CompositorHandle::fast` before the next commit.
    pub fn take_fast_commits(&mut self) -> Vec<presto_base::commit::FastCommit> {
        std::mem::take(&mut self.fast)
    }
}
