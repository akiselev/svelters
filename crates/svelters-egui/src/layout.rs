//! Layout: taffy run directly on the scene arena (D2; plans/research/EGUI.md §1.3).
//!
//! The arena implements taffy's low-level traits (no mirrored `TaffyTree`): a node's taffy id is its
//! handle, its style, cache and layout live in the node. Leaves are measured with egui's own fonts
//! plus the widget chrome of their kind, before the pass, so the first frame is placed right.
//! Layout runs only when something is dirty: a style prop, a structural change, a leaf whose
//! measured size changed, or a new window size.

use crate::scene::{Arena, Handle, Node, W};
use egui::{Rect, Vec2, pos2, vec2};
use taffy::prelude::*;
use taffy::{
    AlignContent, AlignItems, CacheTree, LayoutInput, LayoutOutput, compute_cached_layout, compute_flexbox_layout,
    compute_hidden_layout, compute_leaf_layout, compute_root_layout, round_layout,
};

/// Rebuilds a node's taffy style from its props.
pub fn restyle(n: &mut Node) {
    let c = &n.c;
    let len = |v: Option<f64>| v.map(|x| Dimension::length(x as f32)).unwrap_or(Dimension::auto());
    let lpa = |v: Option<f64>| v.map(|x| LengthPercentageAuto::length(x as f32)).unwrap_or(LengthPercentageAuto::auto());
    let container = matches!(n.w, W::Box | W::Window { .. });
    let mut s = Style {
        display: if !c.visible {
            Display::None
        } else {
            Display::Flex
        },
        size: Size { width: len(c.width), height: len(c.height) },
        min_size: Size { width: lpa(c.min_width), height: lpa(c.min_height) },
        max_size: Size { width: lpa(c.max_width), height: lpa(c.max_height) },
        flex_grow: c.flex_grow as f32,
        flex_shrink: c.flex_shrink as f32,
        flex_basis: len(c.flex_basis),
        align_self: match c.align_self {
            "start" => Some(AlignItems::START),
            "end" => Some(AlignItems::END),
            "center" => Some(AlignItems::CENTER),
            "stretch" => Some(AlignItems::STRETCH),
            "baseline" => Some(AlignItems::BASELINE),
            _ => None,
        },
        margin: taffy::Rect::length(c.margin as f32),
        ..Style::default()
    };
    // TR1 clip channel: the node occupies at most the clipped size (paint is cut at it, render.rs)
    if let Some([cw, ch]) = n.vis.clip {
        if cw >= 0.0 {
            s.max_size.width = LengthPercentageAuto::length(cw);
            s.min_size.width = LengthPercentageAuto::length(0.0);
            s.size.width = Dimension::auto();
        }
        if ch >= 0.0 {
            s.max_size.height = LengthPercentageAuto::length(ch);
            s.min_size.height = LengthPercentageAuto::length(0.0);
            s.size.height = Dimension::auto();
        }
    }
    if container {
        let column = c.flex_direction.starts_with("column");
        s.flex_direction = match c.flex_direction {
            "row" => FlexDirection::Row,
            "row-reverse" => FlexDirection::RowReverse,
            "column-reverse" => FlexDirection::ColumnReverse,
            _ => FlexDirection::Column,
        };
        s.flex_wrap = match c.flex_wrap {
            "wrap" => FlexWrap::Wrap,
            "wrap-reverse" => FlexWrap::WrapReverse,
            _ => FlexWrap::NoWrap,
        };
        // CSS `gap` is `row-gap column-gap`; taffy's `gap.width` is the column gap.
        let _ = column;
        s.gap = Size {
            width: LengthPercentage::length(c.column_gap.unwrap_or(c.gap) as f32),
            height: LengthPercentage::length(c.row_gap.unwrap_or(c.gap) as f32),
        };
        s.padding = taffy::Rect::length(c.padding as f32);
        s.align_items = match c.align_items {
            "start" => AlignItems::START,
            "end" => AlignItems::END,
            "center" => AlignItems::CENTER,
            "baseline" => AlignItems::BASELINE,
            _ => AlignItems::STRETCH,
        };
        s.justify_content = match c.justify_content {
            "end" => AlignContent::END,
            "center" => AlignContent::CENTER,
            "space-between" => AlignContent::SPACE_BETWEEN,
            "space-around" => AlignContent::SPACE_AROUND,
            "space-evenly" => AlignContent::SPACE_EVENLY,
            _ => AlignContent::START,
        };
    }
    n.style = s;
}

/// What the measure functions need from egui.
pub struct Metrics {
    pub ctx: egui::Context,
    pub style: std::sync::Arc<egui::Style>,
}

impl Metrics {
    pub fn new(ctx: &egui::Context) -> Metrics {
        Metrics { ctx: ctx.clone(), style: ctx.global_style() }
    }

    fn font(&self) -> egui::FontId {
        egui::TextStyle::Body.resolve(&self.style)
    }

    fn text_size(&self, text: &str, wrap: Option<f32>) -> Vec2 {
        let font = self.font();
        self.ctx.fonts_mut(|f| match wrap {
            Some(w) => f.layout(text.to_string(), font, egui::Color32::WHITE, w.max(0.0)).size(),
            None => f.layout_no_wrap(text.to_string(), font, egui::Color32::WHITE).size(),
        })
    }

    fn row_height(&self) -> f32 {
        let font = self.font();
        self.ctx.fonts_mut(|f| f.row_height(&font))
    }

    /// The size the widget of `w` asks for with no wrapping (content + chrome).
    pub fn intrinsic(&self, w: &W) -> Vec2 {
        let sp = &self.style.spacing;
        let interact = sp.interact_size;
        match w {
            W::Label { text, .. } => self.text_size(text, None),
            W::Button { text, small, .. } => {
                let t = self.text_size(text, None);
                let pad = if *small { vec2(sp.button_padding.x, 0.0) } else { sp.button_padding };
                let h = t.y + 2.0 * pad.y;
                vec2(t.x + 2.0 * pad.x, if *small { h } else { h.max(interact.y) })
            }
            W::Checkbox { text, .. } => {
                let t = self.text_size(text, None);
                let icon = sp.icon_width + if text.is_empty() { 0.0 } else { sp.icon_spacing };
                vec2(t.x + icon, t.y.max(interact.y))
            }
            W::TextEdit { multiline, .. } => {
                // TextEdit's default margin is (4, 2)
                let rows = if *multiline { 4.0 } else { 1.0 };
                vec2(sp.text_edit_width, (rows * self.row_height() + 4.0).max(interact.y))
            }
            W::Slider { text, show_value, .. } => {
                let mut wdt = sp.slider_width;
                if *show_value {
                    wdt += sp.item_spacing.x + interact.x;
                }
                if !text.is_empty() {
                    wdt += sp.item_spacing.x + self.text_size(text, None).x;
                }
                vec2(wdt, interact.y)
            }
            W::Box | W::Window { .. } => Vec2::ZERO,
        }
    }

    /// Height-for-width of a leaf at `width` (labels that wrap).
    fn wrapped(&self, w: &W, width: f32) -> Option<Vec2> {
        match w {
            W::Label { text, wrap: "wrap", .. } => Some(self.text_size(text, Some(width))),
            _ => None,
        }
    }
}

/// The arena seen as a taffy tree for one layout run.
struct Tree<'a> {
    arena: &'a mut Arena,
    m: &'a Metrics,
    measured: usize,
}

fn h(id: NodeId) -> Handle {
    u64::from(id)
}

impl Tree<'_> {
    fn node(&self, id: NodeId) -> &Node {
        self.arena.n(h(id))
    }
    fn node_mut(&mut self, id: NodeId) -> &mut Node {
        self.arena.n_mut(h(id))
    }
}

pub struct ChildIter<'a>(std::slice::Iter<'a, Handle>);

impl Iterator for ChildIter<'_> {
    type Item = NodeId;
    fn next(&mut self) -> Option<NodeId> {
        self.0.next().map(|&c| NodeId::from(c))
    }
}

impl taffy::TraversePartialTree for Tree<'_> {
    type ChildIter<'b>
        = ChildIter<'b>
    where
        Self: 'b;

    fn child_ids(&self, id: NodeId) -> Self::ChildIter<'_> {
        ChildIter(self.node(id).children.iter())
    }
    fn child_count(&self, id: NodeId) -> usize {
        self.node(id).children.len()
    }
    fn get_child_id(&self, id: NodeId, index: usize) -> NodeId {
        NodeId::from(self.node(id).children[index])
    }
}

impl taffy::TraverseTree for Tree<'_> {}

impl taffy::LayoutPartialTree for Tree<'_> {
    type CoreContainerStyle<'b>
        = &'b Style
    where
        Self: 'b;
    type CustomIdent = String;

    fn get_core_container_style(&self, id: NodeId) -> Self::CoreContainerStyle<'_> {
        &self.node(id).style
    }
    fn set_unrounded_layout(&mut self, id: NodeId, layout: &Layout) {
        self.node_mut(id).unrounded = *layout;
    }
    fn compute_child_layout(&mut self, id: NodeId, inputs: LayoutInput) -> LayoutOutput {
        if self.node(id).style.display == Display::None {
            return compute_hidden_layout(self, id);
        }
        compute_cached_layout(self, id, inputs, |tree, id, inputs| {
            if matches!(tree.node(id).w, W::Box | W::Window { .. }) {
                return compute_flexbox_layout(tree, id, inputs);
            }
            tree.measured += 1;
            let m = tree.m;
            let n = tree.arena.n_mut(h(id));
            let intrinsic = *n.intrinsic.get_or_insert_with(|| m.intrinsic(&n.w));
            let w = &n.w;
            compute_leaf_layout(inputs, &n.style, |_, _| 0.0, |known, avail| {
                if let Size { width: Some(w), height: Some(hh) } = known {
                    return Size { width: w, height: hh };
                }
                let limit = known.width.or(match avail.width {
                    AvailableSpace::Definite(x) => Some(x),
                    AvailableSpace::MinContent => Some(0.0),
                    AvailableSpace::MaxContent => None,
                });
                let mut s = intrinsic;
                if let Some(lim) = limit
                    && lim < intrinsic.x
                    && let Some(wr) = m.wrapped(w, lim)
                {
                    s = wr;
                }
                Size { width: known.width.unwrap_or(s.x.ceil()), height: known.height.unwrap_or(s.y.ceil()) }
            })
        })
    }
}

impl taffy::LayoutContainingBlock for Tree<'_> {
    type OofItemStyle<'b>
        = &'b Style
    where
        Self: 'b;
    fn get_oof_item_style(&self, id: NodeId) -> Self::OofItemStyle<'_> {
        &self.node(id).style
    }
    // No absolute positioning in G1: nothing is ever hoisted.
    fn clear_hoisted_children(&mut self, _id: NodeId) {}
    fn add_hoisted_children(&mut self, _id: NodeId, _hoisted: &[NodeId]) {}
}

impl CacheTree for Tree<'_> {
    fn cache_get(&mut self, id: NodeId, inputs: &LayoutInput) -> Option<LayoutOutput> {
        self.node_mut(id).cache.get(inputs)
    }
    fn cache_store(&mut self, id: NodeId, inputs: &LayoutInput, out: LayoutOutput) {
        self.node_mut(id).cache.store(inputs, out)
    }
    fn cache_clear(&mut self, id: NodeId) {
        self.node_mut(id).cache.clear();
    }
}

impl taffy::LayoutFlexboxContainer for Tree<'_> {
    type FlexboxContainerStyle<'b>
        = &'b Style
    where
        Self: 'b;
    type FlexboxItemStyle<'b>
        = &'b Style
    where
        Self: 'b;
    fn get_flexbox_container_style(&self, id: NodeId) -> Self::FlexboxContainerStyle<'_> {
        &self.node(id).style
    }
    fn get_flexbox_child_style(&self, id: NodeId) -> Self::FlexboxItemStyle<'_> {
        &self.node(id).style
    }
}

impl taffy::RoundTree for Tree<'_> {
    fn get_unrounded_layout(&self, id: NodeId) -> Layout {
        self.node(id).unrounded
    }
    fn set_final_layout(&mut self, id: NodeId, layout: &Layout) {
        self.node_mut(id).layout = *layout;
    }
    fn is_out_of_flow(&self, _id: NodeId) -> bool {
        false
    }
    fn hoisted_child_count(&self, _id: NodeId) -> usize {
        0
    }
    fn get_hoisted_child_id(&self, _id: NodeId, _index: usize) -> NodeId {
        unreachable!("nothing is hoisted")
    }
}

/// Clears the layout cache of `h` and its ancestors (a style or structure change below them).
pub fn invalidate(arena: &mut Arena, mut h: Handle) {
    while let Some(n) = arena.get_mut(h) {
        n.cache.clear();
        match n.parent {
            Some(p) => h = p,
            None => break,
        }
    }
}

/// Lays out the window `root` for a content area of `size`. Returns the number of leaves measured
/// (0 when every cached result was reused).
pub fn compute(arena: &mut Arena, m: &Metrics, root: Handle, size: Vec2) -> usize {
    {
        let n = arena.n_mut(root);
        // The window root is definite (EGUI.md §1.3: an `auto` root re-lays out every row).
        n.style.size = Size { width: Dimension::length(size.x), height: Dimension::length(size.y) };
    }
    let mut t = Tree { arena, m, measured: 0 };
    let id = NodeId::from(root);
    compute_root_layout(&mut t, id, Size { width: AvailableSpace::Definite(size.x), height: AvailableSpace::Definite(size.y) });
    round_layout(&mut t, id);
    let measured = t.measured;
    place(arena, root, pos2(0.0, 0.0));
    measured
}

/// Absolute rects (relative to the window's content origin) from taffy's parent-relative layouts.
fn place(arena: &mut Arena, h: Handle, origin: egui::Pos2) {
    let n = arena.n_mut(h);
    let l = n.layout;
    let min = origin + vec2(l.location.x, l.location.y);
    n.rect = Rect::from_min_size(min, vec2(l.size.width, l.size.height));
    let kids = n.children.clone();
    for c in kids {
        place(arena, c, min);
    }
}

/// Re-measures the leaves whose content changed; invalidates layout only for the ones whose size
/// did (a counter going from 1 to 2 usually keeps its width: EGUI.md §1.3). True if any did.
pub fn remeasure(arena: &mut Arena, m: &Metrics, leaves: &[Handle]) -> bool {
    let mut changed = false;
    for &h in leaves {
        let Some(n) = arena.get_mut(h) else { continue };
        let old = n.intrinsic.take();
        let new = m.intrinsic(&n.w);
        n.intrinsic = Some(new);
        // A wrapping label that does not fit its last width may wrap differently with new text.
        let wraps = matches!(n.w, W::Label { wrap: "wrap", .. }) && new.x > n.rect.width() + 0.5;
        if old != Some(new) || wraps {
            changed = true;
            invalidate(arena, h);
        }
    }
    changed
}

