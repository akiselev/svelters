//! Scroll containers: nodes whose computed `overflow` scrolls on an axis. The offset is native-owned
//! state of the document (architecture §4.5): wheel defaults, the compositor's flings
//! ([`Document::apply_scroll`]) and `scrollTo` from JS write it; a write marks only the node's
//! property-tree entries (a `Property` change: no relayout, no re-record).

use crate::document::Document;
use crate::tree::Dirty;
use skelter_base::geom::{Rect, Size, Vec2};
use skelter_base::ids::{NodeId, PropNodeId, WindowId};
use svelters_style::StateMask;

/// A scroll container's geometry from the last layout ([`NodeView::scroll_geometry`](crate::NodeView::scroll_geometry)):
/// what a kind that draws its own scroll bars (or scrolls by its own rules) needs.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct ScrollGeometry {
    /// The viewport: the padding box, in the node's local coordinates.
    pub viewport: Rect,
    /// The scrollable content's size, at least the viewport's.
    pub content: Size,
    /// The current offset (content scrolled out at the top left).
    pub offset: Vec2,
    /// The axes it scrolls on (x, y).
    pub axes: (bool, bool),
}

impl ScrollGeometry {
    /// A geometry (for tests and hosts that drive a kind without a document).
    pub fn new(viewport: Rect, content: Size, offset: Vec2, axes: (bool, bool)) -> Self {
        ScrollGeometry { viewport, content, offset, axes }
    }
}

/// `:state(sv.overflow-top)`: a scroll container scrolled away from its top (content lies beyond
/// that edge). svelters' own provider states (R15's edge shadows in base.css); skelter-core keeps
/// the top four provider bits for them, so a host allocates its own from bit 32 up to bit 59.
pub const OVERFLOW_TOP: StateMask = StateMask(1 << 60);
/// `:state(sv.overflow-bottom)`: content lies beyond the bottom edge.
pub const OVERFLOW_BOTTOM: StateMask = StateMask(1 << 61);
/// `:state(sv.overflow-left)`: scrolled away from the left edge.
pub const OVERFLOW_LEFT: StateMask = StateMask(1 << 62);
/// `:state(sv.overflow-right)`: content lies beyond the right edge.
pub const OVERFLOW_RIGHT: StateMask = StateMask(1 << 63);
/// The four overflow states.
pub const OVERFLOW: StateMask = StateMask(OVERFLOW_TOP.0 | OVERFLOW_BOTTOM.0 | OVERFLOW_LEFT.0 | OVERFLOW_RIGHT.0);

/// The overflow state named `name` inside `:state()` (`sv.overflow-top`).
pub fn overflow_state(name: &str) -> Option<StateMask> {
    Some(match name {
        "sv.overflow-top" => OVERFLOW_TOP,
        "sv.overflow-bottom" => OVERFLOW_BOTTOM,
        "sv.overflow-left" => OVERFLOW_LEFT,
        "sv.overflow-right" => OVERFLOW_RIGHT,
        _ => return None,
    })
}

/// The overflow states of a scroll container at `offset` over `content` in `viewport` on `axes`
/// (half a point of slack, as the widgets' `Scroller::away_from_start`).
pub fn overflow_states(offset: Vec2, content: Size, viewport: Size, axes: (bool, bool)) -> StateMask {
    let mut m = 0;
    if axes.1 {
        if offset.y > 0.5 {
            m |= OVERFLOW_TOP.0;
        }
        if offset.y < content.h - viewport.h - 0.5 {
            m |= OVERFLOW_BOTTOM.0;
        }
    }
    if axes.0 {
        if offset.x > 0.5 {
            m |= OVERFLOW_LEFT.0;
        }
        if offset.x < content.w - viewport.w - 0.5 {
            m |= OVERFLOW_RIGHT.0;
        }
    }
    StateMask(m)
}

impl Document {
    /// Sets node `i`'s `:state(sv.overflow-*)` bits from its scroll geometry while it is a scroll
    /// container, and clears them when it stops being one (`was`: it was one). Other nodes' bits are
    /// left alone: a kind that scrolls by its own offset without core (`table`) sets them itself.
    pub(crate) fn refresh_overflow_states(&mut self, i: usize, was: bool) {
        let Some(node) = self.slots.id_at(i) else { return };
        let bits = match self.scroll_viewport(i) {
            Some(view) => overflow_states(self.scroll_offset(node), self.scroll_content(i), view, self.scroll_axes(i)),
            None if was => StateMask::NONE,
            None => return,
        };
        if StateMask(self.state[i].0 & OVERFLOW.0) != bits {
            self.set_state(node, StateMask(OVERFLOW.0 & !bits.0), false);
            self.set_state(node, bits, true);

        }
    }

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

    /// Node `node`'s [`ScrollGeometry`] when it is a scroll container.
    pub fn scroll_geometry(&self, node: NodeId) -> Option<ScrollGeometry> {
        let i = self.idx(node)?;
        let view = self.scroll_viewport(i)?;
        let b = self.style.computed(i).border_widths;
        Some(ScrollGeometry {
            viewport: Rect::new(b.left, b.top, b.left + view.w, b.top + view.h),
            content: self.scroll_content(i),
            offset: self.scroll_offset(node),
            axes: self.scroll_axes(i),
        })
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

    /// Sets a scroll container's offset (clamped), as native state. An axis pinned to the end
    /// ([`Document::pin_scroll_end`]) that this moves away from the end is unpinned.
    pub fn set_scroll_offset(&mut self, node: NodeId, v: Vec2) {
        let Some(i) = self.idx(node) else { return };
        let v = self.clamp_scroll(i, v);
        if let Some(&(px, py)) = self.scroll_pins.get(&node) {
            let end = self.scroll_end(i);
            let pins = (px && v.x >= end.x - 0.5, py && v.y >= end.y - 0.5);
            if pins == (false, false) {
                self.scroll_pins.remove(&node);
            } else {
                self.scroll_pins.insert(node, pins);
            }
        }
        if self.scroll_offset(node) == v {
            return;
        }
        self.stamp();
        self.scroll.insert(node, v);
        // a kind that scrolls itself places its accessibility parts (text runs) by the offset: they
        // re-send with it (a scroll container's children re-send as they move)
        let own = if self.self_scroll.contains_key(&node) { Dirty::A11Y } else { Dirty::EMPTY };
        // its foreground (bars, edge shadows) follows the offset: recorded alone, not the content
        if self.record[i].fore.is_some() {
            self.fore_dirty.insert(node);
        }
        self.mark(node, Dirty::PROPERTY.union(own));
        if let Some(w) = self.core[i].window
            && let Some(ws) = self.window_mut(w)
        {
            ws.rebuild = true;
        }
    }

    /// The largest offset of node `i` on each axis it scrolls (its content's end).
    fn scroll_end(&self, i: usize) -> Vec2 {
        self.clamp_scroll(i, Vec2::new(f32::MAX, f32::MAX))
    }

    /// SV86: keeps scroll container `node` at the end of its content on the axes set (x, y), as a
    /// terminal or a chat log does: it scrolls there now and, after every layout, follows the
    /// content as it grows or shrinks, until a scroll moves it away from the end on that axis (the
    /// user's wheel or drag, a fling, a `scrollTo` short of the end), which unpins that axis.
    /// `(false, false)` unpins. Not pinned by default.
    pub fn pin_scroll_end(&mut self, node: NodeId, x: bool, y: bool) {
        let Some(i) = self.idx(node) else { return };
        if !x && !y {
            self.scroll_pins.remove(&node);
            return;
        }
        self.scroll_pins.insert(node, (x, y));
        let cur = self.scroll_offset(node);
        let end = self.scroll_end(i);
        self.scroll_fast(node, Vec2::new(if x { end.x } else { cur.x }, if y { end.y } else { cur.y }));
    }

    /// The axes (x, y) `node` is pinned to the end on ([`Document::pin_scroll_end`]).
    pub fn scroll_pinned(&self, node: NodeId) -> (bool, bool) {
        self.scroll_pins.get(&node).copied().unwrap_or((false, false))
    }

    /// After layout: the pinned scroll containers of `window` follow the end of their content.
    pub(crate) fn follow_scroll_pins(&mut self, window: WindowId) {
        let pins: Vec<(NodeId, (bool, bool))> = self.scroll_pins.iter().map(|(&n, &p)| (n, p)).collect();
        for (node, (x, y)) in pins {
            let Some(i) = self.idx(node) else { continue };
            if self.core[i].window != Some(window) {
                continue;
            }
            let cur = self.scroll_offset(node);
            let end = self.scroll_end(i);
            let to = Vec2::new(if x { end.x } else { cur.x }, if y { end.y } else { cur.y });
            if to != cur {
                self.set_scroll_offset(node, to);
                // its bars (a kind that draws them reads the offset in its prepare) move with it
                self.mark(node, Dirty::PAINT);
            }
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
            self.fast.push(skelter_base::commit::FastCommit::ScrollOffset { window, node: sc, offset, revision: self.revision, input_seq: self.input_seq });
        }
    }

    /// The fast commits queued since the last call (kinds' scrolls and carets), for the runner to
    /// send with `CompositorHandle::fast` before the next commit.
    pub fn take_fast_commits(&mut self) -> Vec<skelter_base::commit::FastCommit> {
        std::mem::take(&mut self.fast)
    }
}
