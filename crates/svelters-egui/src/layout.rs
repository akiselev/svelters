//! Layout: taffy run directly on the scene's columns (D2; plans/research/EGUI.md §1.3).
//!
//! The scene implements taffy's low-level traits (no mirrored `TaffyTree`): a node's taffy id is its
//! [`NodeId`](crate::scene::NodeId), its style, cache and layouts live in its `LayoutBox`. Leaves are
//! measured with egui's own fonts plus the widget chrome of their kind, before the pass, so the first
//! frame is placed right.
//!
//! Layout runs per root, only for queued work (ARCH-SCENE §7.1, `scene/dirty.rs`): a whole root when
//! it is new, presented or resized; otherwise only the layout boundaries whose content changed, each
//! at its last size ([`run_root`]). Placing rects walks children by index (no per-layout clones).
//!
//! Writer lanes: SCENE (the adapter), WIDGETS (per-kind measure), STYLE (computed style).

pub use crate::scene::dirty::invalidate;
use crate::scene::id::IDX_MASK;
use crate::scene::node::Payload;
use crate::scene::{Common, Dirty, NodeFlags, NodeId as SceneId, RootIx, Scene, Visual, W};
use egui::{Rect, Vec2, pos2, vec2};
use taffy::prelude::{AvailableSpace, Dimension, Display, FlexDirection, FlexWrap, Layout, LengthPercentage, LengthPercentageAuto, Size, Style};
use taffy::{
    AlignContent, AlignItems, CacheTree, LayoutInput, LayoutOutput, LayoutPartialTree, Line, NodeId, RequestedAxis, RunMode, SizingMode, compute_cached_layout, compute_flexbox_layout,
    compute_hidden_layout, compute_leaf_layout, compute_root_layout, round_layout,
};

/// A node's taffy style from its props.
pub fn style_of(c: &Common, w: &W, vis: &Visual) -> Style {
    let len = |v: Option<f64>| v.map(|x| Dimension::length(x as f32)).unwrap_or(Dimension::auto());
    let lpa = |v: Option<f64>| v.map(|x| LengthPercentageAuto::length(x as f32)).unwrap_or(LengthPercentageAuto::auto());
    let container = w.is_container();
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
    if let Some([cw, ch]) = vis.clip {
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
    s
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
            W::Ext(x) => x.measure(self, crate::provider::AvailableSpace::MaxContent),
        }
    }

    /// Height-for-width of a leaf at `width` (labels that wrap).
    fn wrapped(&self, w: &W, width: f32) -> Option<Vec2> {
        match w {
            W::Label { text, wrap: "wrap", .. } => Some(self.text_size(text, Some(width))),
            W::Ext(x) if x.entry().caps.contains(crate::provider::Caps::HEIGHT_FOR_WIDTH) => {
                Some(x.measure(self, crate::provider::AvailableSpace::Definite(width)))
            }
            _ => None,
        }
    }
}

// ---- the taffy adapter over the scene's columns ----

#[inline]
fn ix(id: NodeId) -> usize {
    (u64::from(id) & IDX_MASK) as usize
}

#[inline]
fn tid(id: SceneId) -> NodeId {
    NodeId::from(id.to_raw())
}

/// The scene seen as a taffy tree for one layout run.
struct Tree<'a> {
    s: &'a mut Scene,
    m: &'a Metrics,
    /// Leaf computations (cache misses).
    leaf_layouts: u64,
    /// Leaves measured with egui's fonts.
    fonts: u64,
}

pub struct ChildIter<'a>(std::slice::Iter<'a, SceneId>);

impl Iterator for ChildIter<'_> {
    type Item = NodeId;
    fn next(&mut self) -> Option<NodeId> {
        self.0.next().map(|&c| tid(c))
    }
}

impl taffy::TraversePartialTree for Tree<'_> {
    type ChildIter<'b>
        = ChildIter<'b>
    where
        Self: 'b;

    fn child_ids(&self, id: NodeId) -> Self::ChildIter<'_> {
        ChildIter(self.s.children[ix(id)].iter())
    }
    fn child_count(&self, id: NodeId) -> usize {
        self.s.children[ix(id)].len()
    }
    fn get_child_id(&self, id: NodeId, index: usize) -> NodeId {
        tid(self.s.children[ix(id)][index])
    }
}

impl taffy::TraverseTree for Tree<'_> {}

impl LayoutPartialTree for Tree<'_> {
    type CoreContainerStyle<'b>
        = &'b Style
    where
        Self: 'b;
    type CustomIdent = String;

    fn get_core_container_style(&self, id: NodeId) -> Self::CoreContainerStyle<'_> {
        &self.s.layout[ix(id)].style
    }
    /// CS5e: `calc()` lengths of the computed style (`style.rs` interns them).
    fn resolve_calc_value(&self, val: *const (), basis: f32) -> f32 {
        crate::style::resolve_calc(val, basis)
    }
    fn set_unrounded_layout(&mut self, id: NodeId, layout: &Layout) {
        self.s.layout[ix(id)].unrounded = *layout;
    }
    fn compute_child_layout(&mut self, id: NodeId, inputs: LayoutInput) -> LayoutOutput {
        // Hidden: `display: none`, or inside a hidden ancestor (taffy passes HIDDEN to children).
        if inputs.run_mode == RunMode::PerformHiddenLayout || self.s.layout[ix(id)].style.display == Display::None {
            return compute_hidden_layout(self, id);
        }
        compute_cached_layout(self, id, inputs, |tree, id, inputs| {
            let i = ix(id);
            if tree.s.w(i).is_container() {
                // CS5e: `display: grid` from the computed style
                if tree.s.layout[i].style.display == Display::Grid {
                    return taffy::compute_grid_layout(tree, id, inputs);
                }
                return compute_flexbox_layout(tree, id, inputs);
            }
            tree.leaf_layouts += 1;
            let m = tree.m;
            let s = &mut *tree.s;
            let w = match &s.payload[i] {
                Payload::Builtin(w) => w,
                Payload::Vacant => unreachable!("vacant payload in layout"),
            };
            let lb = &mut s.layout[i];
            let mut fonts = 0;
            let intrinsic = *lb.intrinsic.get_or_insert_with(|| {
                fonts += 1;
                m.intrinsic(w)
            });
            let out = compute_leaf_layout(inputs, &lb.style, |_, _| 0.0, |known, avail| {
                if let Size { width: Some(w), height: Some(hh) } = known {
                    return Size { width: w, height: hh };
                }
                let limit = known.width.or(match avail.width {
                    AvailableSpace::Definite(x) => Some(x),
                    AvailableSpace::MinContent => Some(0.0),
                    AvailableSpace::MaxContent => None,
                });
                let mut sz = intrinsic;
                if let Some(lim) = limit
                    && lim < intrinsic.x
                    && let Some(wr) = m.wrapped(w, lim)
                {
                    sz = wr;
                }
                Size { width: known.width.unwrap_or(sz.x.ceil()), height: known.height.unwrap_or(sz.y.ceil()) }
            });
            tree.fonts += fonts;
            out
        })
    }
}

impl taffy::LayoutContainingBlock for Tree<'_> {
    type OofItemStyle<'b>
        = &'b Style
    where
        Self: 'b;
    fn get_oof_item_style(&self, id: NodeId) -> Self::OofItemStyle<'_> {
        &self.s.layout[ix(id)].style
    }
    // No absolute positioning yet (SG4: hoisted children in the cold column).
    fn clear_hoisted_children(&mut self, _id: NodeId) {}
    fn add_hoisted_children(&mut self, _id: NodeId, _hoisted: &[NodeId]) {}
}

impl CacheTree for Tree<'_> {
    fn cache_get(&mut self, id: NodeId, inputs: &LayoutInput) -> Option<LayoutOutput> {
        self.s.layout[ix(id)].cache.get(inputs)
    }
    fn cache_store(&mut self, id: NodeId, inputs: &LayoutInput, out: LayoutOutput) {
        self.s.layout[ix(id)].cache.store(inputs, out)
    }
    fn cache_clear(&mut self, id: NodeId) {
        self.s.layout[ix(id)].cache.clear();
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
        &self.s.layout[ix(id)].style
    }
    fn get_flexbox_child_style(&self, id: NodeId) -> Self::FlexboxItemStyle<'_> {
        &self.s.layout[ix(id)].style
    }
}

impl taffy::LayoutGridContainer for Tree<'_> {
    type GridContainerStyle<'b>
        = &'b Style
    where
        Self: 'b;
    type GridItemStyle<'b>
        = &'b Style
    where
        Self: 'b;
    fn get_grid_container_style(&self, id: NodeId) -> Self::GridContainerStyle<'_> {
        &self.s.layout[ix(id)].style
    }
    fn get_grid_child_style(&self, id: NodeId) -> Self::GridItemStyle<'_> {
        &self.s.layout[ix(id)].style
    }
}

impl taffy::RoundTree for Tree<'_> {
    fn get_unrounded_layout(&self, id: NodeId) -> Layout {
        self.s.layout[ix(id)].unrounded
    }
    fn set_final_layout(&mut self, id: NodeId, layout: &Layout) {
        self.s.layout[ix(id)].layout = *layout;
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

/// What one layout run did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LayoutRun {
    /// The whole root was laid out.
    pub full: bool,
    /// Boundaries laid out on their own.
    pub boundaries: u64,
    /// Taffy leaf computations (cache misses).
    pub leaf_layouts: u64,
    /// Leaves measured with egui's fonts.
    pub fonts: u64,
}

impl Tree<'_> {
    /// The whole tree under `top` for a content area of `size` (the top is definite: EGUI.md §1.3,
    /// an `auto` root re-lays out every row).
    fn whole(&mut self, top: SceneId, size: Vec2) {
        self.s.layout[top.index() as usize].style.size = Size { width: Dimension::length(size.x), height: Dimension::length(size.y) };
        let id = tid(top);
        compute_root_layout(self, id, Size { width: AvailableSpace::Definite(size.x), height: AvailableSpace::Definite(size.y) });
        round_layout(self, id);
    }

    /// Boundary `b`'s content at its current size (its own place and size do not change).
    fn boundary(&mut self, b: SceneId) {
        let i = b.index() as usize;
        let ul = self.s.layout[i].unrounded;
        let (w, h) = (ul.size.width, ul.size.height);
        let parent_size = match self.s.core[i].parent {
            Some(p) => {
                let pl = self.s.layout[p.index() as usize].unrounded;
                Size { width: Some(pl.size.width), height: Some(pl.size.height) }
            }
            None => Size::NONE,
        };
        self.compute_child_layout(
            tid(b),
            LayoutInput {
                run_mode: RunMode::PerformLayout,
                sizing_mode: SizingMode::InherentSize,
                axis: RequestedAxis::Both,
                known_dimensions: Size { width: Some(w), height: Some(h) },
                known_dimensions_are_definite: Size { width: true, height: true },
                parent_size,
                available_space: Size { width: AvailableSpace::Definite(w), height: AvailableSpace::Definite(h) },
                vertical_margins_are_collapsible: Line::FALSE,
            },
        );
    }

    /// Rounds `b`'s subtree exactly as a whole-tree `round_layout` would: taffy rounds cumulative
    /// positions, so `b` is rounded from its absolute unrounded position (its own final layout,
    /// relative to its parent, does not change and is restored).
    fn round_from(&mut self, b: SceneId) {
        let i = b.index() as usize;
        let (mut x, mut y) = (0.0, 0.0);
        let mut a = self.s.core[i].parent;
        while let Some(p) = a {
            let pi = p.index() as usize;
            x += self.s.layout[pi].unrounded.location.x;
            y += self.s.layout[pi].unrounded.location.y;
            a = self.s.core[pi].parent;
        }
        let (saved_u, saved_f) = (self.s.layout[i].unrounded, self.s.layout[i].layout);
        self.s.layout[i].unrounded.location.x += x;
        self.s.layout[i].unrounded.location.y += y;
        round_layout(self, tid(b));
        self.s.layout[i].unrounded = saved_u;
        self.s.layout[i].layout = saved_f;
    }
}

/// Lays out the whole tree under `top` for a content area of `size`, places it, and returns the
/// number of leaf computations (0 when every cached result was reused).
pub fn compute(scene: &mut Scene, m: &Metrics, top: SceneId, size: Vec2) -> usize {
    let mut t = Tree { s: scene, m, leaf_layouts: 0, fonts: 0 };
    t.whole(top, size);
    let (leaf, fonts) = (t.leaf_layouts, t.fonts);
    scene.counters.leaves_measured += fonts;
    scene.counters.leaf_layouts += leaf;
    scene.counters.nodes_placed += place(scene, top, pos2(0.0, 0.0));
    leaf as usize
}

/// Absolute rects (relative to the root's content origin) from taffy's parent-relative layouts,
/// for `top`'s subtree placed at `origin`; clears the layout dirty bits on the way. Iterative,
/// children by index (no clones). Returns the nodes placed.
fn place(s: &mut Scene, top: SceneId, origin: egui::Pos2) -> u64 {
    let mut stack = vec![(top.index() as usize, origin)];
    let mut n = 0;
    while let Some((i, o)) = stack.pop() {
        let l = s.layout[i].layout;
        let min = o + vec2(l.location.x, l.location.y);
        let rect = Rect::from_min_size(min, vec2(l.size.width, l.size.height));
        // SG2: `Layout` records, only when an observer wants them
        if s.journal.wants(crate::scene::ChangeMask::LAYOUT) && s.core[i].rect != rect {
            let before = s.core[i].rect;
            if let Some(node) = s.slots.id_at(i) {
                s.record(crate::scene::Change::Layout { node, before, after: rect }, crate::scene::Origin::Host);
            }
        }
        s.core[i].rect = rect;
        s.core[i].dirty.remove(Dirty::LAYOUT | Dirty::DESC_LAYOUT | Dirty::CHILDREN);
        n += 1;
        for k in 0..s.children[i].len() {
            stack.push((s.children[i][k].index() as usize, min));
        }
    }
    n
}

/// True if `b` or an ancestor is `display: none` (its layout is the hidden one).
fn hidden(s: &Scene, b: SceneId) -> bool {
    let mut a = Some(b);
    while let Some(x) = a {
        let i = x.index() as usize;
        if s.layout[i].style.display == Display::None {
            return true;
        }
        a = s.core[i].parent;
    }
    false
}

/// Serves root `r`'s layout queue for a content area of `size`: the whole root when it is new,
/// resized or its top changed, plus every queued boundary at its current size. `None`: nothing was
/// due.
pub(crate) fn run_root(s: &mut Scene, m: &Metrics, r: RootIx, size: Vec2) -> Option<LayoutRun> {
    let e = s.roots.get_mut(r)?;
    let top = e.top;
    let mut full = e.dirty.full || e.size != Some(size);
    let queue = std::mem::take(&mut e.dirty.relayout);
    e.dirty.full = false;
    e.size = Some(size);
    let ti = s.ix(top);
    full |= s.layout[ti].cache.is_empty();
    let mut bs: Vec<SceneId> = Vec::new();
    for b in queue {
        let Some(i) = s.idx(b) else { continue };
        if !s.core[i].flags.contains(NodeFlags::IN_RELAYOUT) || s.core[i].root != r {
            continue;
        }
        s.core[i].flags.remove(NodeFlags::IN_RELAYOUT);
        if b == top || !s.is_boundary(i) {
            // the top, or a node that stopped being a boundary since it was queued
            full = true;
            continue;
        }
        bs.push(b);
    }
    if !full && bs.is_empty() {
        return None;
    }
    if full {
        // A whole-root run reuses the caches above a queued boundary and would not reach it:
        // clear them, so the one run lays out everything.
        for &b in &bs {
            let mut a = Some(b);
            while let Some(x) = a {
                let i = x.index() as usize;
                s.layout[i].cache.clear();
                a = s.core[i].parent;
            }
        }
    }
    let mut t = Tree { s, m, leaf_layouts: 0, fonts: 0 };
    let mut run = LayoutRun { full, ..Default::default() };
    if !full {
        for &b in &bs {
            if hidden(t.s, b) {
                // inside a hidden subtree: the hidden (zero) layout, as a whole-root run gives it
                compute_hidden_layout(&mut t, tid(b));
            } else {
                t.boundary(b);
                run.boundaries += 1;
            }
        }
    }
    if full {
        t.whole(top, size);
    } else {
        for &b in &bs {
            t.round_from(b);
        }
    }
    (run.leaf_layouts, run.fonts) = (t.leaf_layouts, t.fonts);
    let mut placed = 0;
    if full {
        placed += place(s, top, pos2(0.0, 0.0));
    } else {
        for &b in &bs {
            let o = s.core[b.index() as usize].parent.map(|p| s.core[p.index() as usize].rect.min).unwrap_or(pos2(0.0, 0.0));
            placed += place(s, b, o);
        }
    }
    // SG2: `Damage::relayout` for observers of this root's layout
    if s.journal.wants(crate::scene::ChangeMask::LAYOUT) {
        let laid: Vec<SceneId> = if full { vec![top] } else { bs.clone() };
        for b in laid {
            s.journal.laid_out(&s.core, b);
        }
    }
    let c = &mut s.counters;
    c.layouts += full as u64;
    c.boundaries_laid_out += run.boundaries;
    c.leaf_layouts += run.leaf_layouts;
    c.leaves_measured += run.fonts;
    c.nodes_placed += placed;
    Some(run)
}

/// Re-measures root `r`'s queued leaves; invalidates layout only for the ones whose size changed (a
/// counter going from 1 to 2 usually keeps its width: EGUI.md §1.3). True if any did.
pub(crate) fn remeasure(s: &mut Scene, m: &Metrics, r: RootIx) -> bool {
    let Some(e) = s.roots.get_mut(r) else { return false };
    let q = std::mem::take(&mut e.dirty.remeasure);
    let mut changed = false;
    for id in q {
        let Some(i) = s.idx(id) else { continue };
        if !s.core[i].flags.contains(NodeFlags::IN_REMEASURE) || s.core[i].root != r {
            continue;
        }
        s.core[i].flags.remove(NodeFlags::IN_REMEASURE);
        s.core[i].dirty.remove(Dirty::MEASURE);
        let old = s.layout[i].intrinsic.take();
        let new = m.intrinsic(s.w(i));
        s.counters.leaves_measured += 1;
        s.layout[i].intrinsic = Some(new);
        // A wrapping label that does not fit its last width may wrap differently with new text.
        let wraps = matches!(s.w(i), W::Label { wrap: "wrap", .. }) && new.x > s.core[i].rect.width() + 0.5;
        if old != Some(new) || wraps {
            changed = true;
            s.invalidate_content(id);
        }
    }
    changed
}

