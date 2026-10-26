//! Layout: taffy 0.15 run directly on the document's columns through its low-level traits (no
//! mirrored `TaffyTree`), ported from svelters-egui's `layout.rs`. A node's taffy id is its
//! [`NodeId`]; its style, cache and layouts live in its `LayoutBox`. Leaves are measured by their
//! kind ([`Kind::measure`](crate::Kind::measure)) through skelter-base's
//! [`TextMeasure`](skelter_base::text::TextMeasure).
//!
//! Layout runs per window, only for queued work (`invalidate.rs`): the whole window when it is new
//! or resized, otherwise the remeasured leaves (the measure guard: a leaf whose max-content size did
//! not change costs no layout) and the layout boundaries whose content changed, each at its last
//! size. `position: absolute` boxes are hoisted to their containing block, as taffy records them,
//! and placed relative to it. After layout, a node whose position moved gets a `PROPERTY` mark (its
//! transform node moves) and one whose size changed a `PAINT` mark.

mod invalidate;

use crate::cx::MeasureCx;
use crate::document::Document;
use crate::tree::{Dirty, NodeFlags};
use skelter_base::geom::Size as PSize;
use skelter_base::ids::{NodeId as PNode, WindowId};
use taffy::{
    AvailableSpace, CacheTree, Display, LayoutInput, LayoutOutput, LayoutPartialTree, Line, NodeId, RequestedAxis, RunMode, Size, SizingMode, Style,
    compute_block_layout, compute_cached_layout, compute_flexbox_layout, compute_grid_layout, compute_hidden_layout, compute_leaf_layout,
    compute_oof_layout, compute_root_layout, round_layout,
};

fn tid(id: PNode) -> NodeId {
    NodeId::from(id.0)
}

fn ix(id: NodeId) -> usize {
    PNode(u64::from(id)).index() as usize
}

/// What one layout run did (counters for tests and the inspector).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LayoutRun {
    /// The whole window was laid out.
    pub full: bool,
    /// Boundaries laid out on their own.
    pub boundaries: u64,
    /// Leaves remeasured, and those whose size changed (or that may wrap differently).
    pub remeasured: u64,
    pub resized: u64,
    /// Leaf computations (cache misses).
    pub leaf_layouts: u64,
}

/// The document seen as a taffy tree for one layout run.
pub(crate) struct Tree<'a> {
    pub doc: &'a mut Document,
    pub leaf_layouts: u64,
}

pub struct ChildIter<'a>(std::slice::Iter<'a, PNode>);

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
        ChildIter(self.doc.layout_children(ix(id)).iter())
    }
    fn child_count(&self, id: NodeId) -> usize {
        self.doc.layout_children(ix(id)).len()
    }
    fn get_child_id(&self, id: NodeId, index: usize) -> NodeId {
        tid(self.doc.layout_children(ix(id))[index])
    }
}

impl taffy::TraverseTree for Tree<'_> {}

/// Stands in for the document's text measure while a kind measures with it.
struct NoMeasure;

impl skelter_base::text::TextMeasure for NoMeasure {
    fn measure(&mut self, _spec: &skelter_base::text::TextSpec<'_>, _max_width: Option<f32>) -> skelter_base::geom::Size {
        skelter_base::geom::Size::ZERO
    }
    fn content_widths(&mut self, _spec: &skelter_base::text::TextSpec<'_>) -> (f32, f32) {
        (0.0, 0.0)
    }
}

/// Calls `f` with node `i`'s kind and a measure context (its view, with `style` standing in for its
/// computed box while a restyle computes it). The kind sees the whole document (its children's
/// views): the services it uses mutably are lent out of it for the call.
pub(crate) fn with_measure_cx<R>(doc: &mut Document, i: usize, style: Option<&crate::style::ComputedBox>, f: impl FnOnce(&dyn crate::Kind, &mut MeasureCx) -> R) -> Option<R> {
    let k = doc.kinds[i].take()?;
    let mut text = std::mem::replace(&mut doc.text, Box::new(NoMeasure));
    let mut services = std::mem::take(&mut doc.services);
    let out = {
        let view = doc.view_at(i);
        let view = match style {
            Some(s) => crate::cx::NodeView { style: s, ..view },
            None => view,
        };
        let mut cx = MeasureCx { node: view, text: &mut *text, services: &mut services };
        f(&*k, &mut cx)
    };
    doc.text = text;
    doc.services = services;
    doc.kinds[i] = Some(k);
    Some(out)
}

/// The measure of a leaf: its kind's `measure` with the node's view.
fn measure_leaf(doc: &mut Document, i: usize, known: Size<Option<f32>>, avail: Size<AvailableSpace>) -> Size<f32> {
    with_measure_cx(doc, i, None, |k, cx| k.measure(cx, known, avail)).unwrap_or(Size::ZERO)
}

/// Adds `pad` (points) to the length parts of `style`'s padding (a kind's chrome).
pub(crate) fn add_padding(style: &mut Style, pad: skelter_base::geom::Insets) {
    let add = |lp: &mut taffy::LengthPercentage, v: f32| {
        if let taffy::style::ExpandedLengthPercentage::Length(l) = lp.expand() {
            *lp = taffy::LengthPercentage::length(l + v);
        }
    };
    add(&mut style.padding.left, pad.left);
    add(&mut style.padding.right, pad.right);
    add(&mut style.padding.top, pad.top);
    add(&mut style.padding.bottom, pad.bottom);
}

impl LayoutPartialTree for Tree<'_> {
    type CoreContainerStyle<'b>
        = &'b Style
    where
        Self: 'b;
    type CustomIdent = String;

    fn get_core_container_style(&self, id: NodeId) -> Self::CoreContainerStyle<'_> {
        &self.doc.layout[ix(id)].style
    }
    /// The cascade never emits `calc()` lengths (mixed lengths are resolved there).
    fn resolve_calc_value(&self, _val: *const (), _basis: f32) -> f32 {
        0.0
    }
    fn set_unrounded_layout(&mut self, id: NodeId, layout: &taffy::Layout) {
        self.doc.layout[ix(id)].unrounded = *layout;
    }
    fn compute_child_layout(&mut self, id: NodeId, inputs: LayoutInput) -> LayoutOutput {
        if inputs.run_mode == RunMode::PerformHiddenLayout || self.doc.layout[ix(id)].style.display == Display::None {
            return compute_hidden_layout(self, id);
        }
        compute_cached_layout(self, id, inputs, |tree, id, inputs| {
            let i = ix(id);
            let container = !tree.doc.layout_children(i).is_empty();
            if container {
                let mut out = match tree.doc.layout[i].style.display {
                    Display::Grid => compute_grid_layout(tree, id, inputs),
                    Display::Block => compute_block_layout(tree, id, inputs, None),
                    _ => compute_flexbox_layout(tree, id, inputs),
                };
                if inputs.run_mode == RunMode::PerformLayout {
                    compute_oof_layout(tree, id, &mut out);
                }
                return out;
            }
            tree.leaf_layouts += 1;
            let style = tree.doc.layout[i].style.clone();
            let doc = &mut *tree.doc;
            // the measure guard's reference: the leaf's max-content size
            if doc.layout[i].intrinsic.is_none() {
                let m = measure_leaf(doc, i, Size::NONE, Size { width: AvailableSpace::MaxContent, height: AvailableSpace::MaxContent });
                doc.layout[i].intrinsic = Some(PSize::new(m.width, m.height));
            }
            compute_leaf_layout(inputs, &*style, |_, _| 0.0, |known, avail| {
                if let Size { width: Some(w), height: Some(h) } = known {
                    return Size { width: w, height: h };
                }
                let s = measure_leaf(doc, i, known, avail);
                Size { width: known.width.unwrap_or(s.width.ceil()), height: known.height.unwrap_or(s.height.ceil()) }
            })
        })
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
        &self.doc.layout[ix(id)].style
    }
    fn get_flexbox_child_style(&self, id: NodeId) -> Self::FlexboxItemStyle<'_> {
        &self.doc.layout[ix(id)].style
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
        &self.doc.layout[ix(id)].style
    }
    fn get_grid_child_style(&self, id: NodeId) -> Self::GridItemStyle<'_> {
        &self.doc.layout[ix(id)].style
    }
}

impl taffy::LayoutBlockContainer for Tree<'_> {
    type BlockContainerStyle<'b>
        = &'b Style
    where
        Self: 'b;
    type BlockItemStyle<'b>
        = &'b Style
    where
        Self: 'b;
    fn get_block_container_style(&self, id: NodeId) -> Self::BlockContainerStyle<'_> {
        &self.doc.layout[ix(id)].style
    }
    fn get_block_child_style(&self, id: NodeId) -> Self::BlockItemStyle<'_> {
        &self.doc.layout[ix(id)].style
    }
}

impl taffy::LayoutContainingBlock for Tree<'_> {
    type OofItemStyle<'b>
        = &'b Style
    where
        Self: 'b;
    fn get_oof_item_style(&self, id: NodeId) -> Self::OofItemStyle<'_> {
        &self.doc.layout[ix(id)].style
    }
    fn clear_hoisted_children(&mut self, id: NodeId) {
        self.doc.layout[ix(id)].hoisted.clear();
    }
    fn add_hoisted_children(&mut self, id: NodeId, hoisted: &[NodeId]) {
        self.doc.layout[ix(id)].hoisted.extend(hoisted.iter().map(|&h| PNode(u64::from(h))));
    }
}

impl CacheTree for Tree<'_> {
    fn cache_get(&mut self, id: NodeId, inputs: &LayoutInput) -> Option<LayoutOutput> {
        self.doc.layout[ix(id)].cache.get(inputs)
    }
    fn cache_store(&mut self, id: NodeId, inputs: &LayoutInput, out: LayoutOutput) {
        self.doc.layout[ix(id)].cache.store(inputs, out)
    }
    fn cache_clear(&mut self, id: NodeId) {
        self.doc.layout[ix(id)].cache.clear();
    }
}

impl taffy::RoundTree for Tree<'_> {
    fn get_unrounded_layout(&self, id: NodeId) -> taffy::Layout {
        self.doc.layout[ix(id)].unrounded
    }
    fn set_final_layout(&mut self, id: NodeId, layout: &taffy::Layout) {
        let i = ix(id);
        let old = self.doc.layout[i].layout;
        if old.location != layout.location {
            self.doc.core[i].dirty.insert(Dirty::PROPERTY);
        }
        if old.size != layout.size {
            self.doc.core[i].dirty.insert(Dirty::PAINT | Dirty::PROPERTY | Dirty::A11Y);
        }
        if old.scrollable_overflow_rect != layout.scrollable_overflow_rect {
            self.doc.core[i].dirty.insert(Dirty::PROPERTY);
        }
        if self.doc.core[i].dirty.intersects(Dirty::RECORD) {
            self.doc.mark_up(i, Dirty::DESC_RECORD);
        }
        self.doc.layout[i].layout = *layout;
    }
    fn is_out_of_flow(&self, id: NodeId) -> bool {
        out_of_flow(self.doc, ix(id))
    }
    fn hoisted_child_count(&self, id: NodeId) -> usize {
        self.doc.layout[ix(id)].hoisted.len()
    }
    fn get_hoisted_child_id(&self, id: NodeId, index: usize) -> NodeId {
        tid(self.doc.layout[ix(id)].hoisted[index])
    }
}

/// An out-of-flow box (`position: absolute`, shown).
pub(crate) fn out_of_flow(doc: &Document, i: usize) -> bool {
    let st = &doc.layout[i].style;
    st.position == taffy::Position::Absolute && st.display != Display::None
}

/// What node `i`'s layout location is relative to: its parent, or for an absolute box its containing
/// block (the nearest positioned ancestor, else the top of its tree).
pub(crate) fn positional_parent(doc: &Document, i: usize) -> Option<usize> {
    let p = doc.core[i].parent.map(|p| p.index() as usize)?;
    if !out_of_flow(doc, i) {
        return Some(p);
    }
    let mut a = p;
    loop {
        // taffy's containing block: the nearest positioned box (`relative` or `absolute`; boxes are
        // `static` unless CSS says otherwise), else the window root, which claims the rest
        if doc.layout[a].style.position != taffy::Position::Static {
            return Some(a);
        }
        match doc.core[a].parent {
            Some(x) => a = x.index() as usize,
            None => return Some(a),
        }
    }
}

impl Tree<'_> {
    /// The whole tree under `top` for a content area of `size`.
    fn whole(&mut self, top: PNode, size: PSize) {
        let i = top.index() as usize;
        let want = Size { width: taffy::Dimension::length(size.w), height: taffy::Dimension::length(size.h) };
        if self.doc.layout[i].style.size != want {
            std::sync::Arc::make_mut(&mut self.doc.layout[i].style).size = want;
            self.doc.layout[i].cache.clear();
        }
        let id = tid(top);
        compute_root_layout(self, id, Size { width: AvailableSpace::Definite(size.w), height: AvailableSpace::Definite(size.h) });
        round_layout(self, id);
    }

    /// Boundary `b`'s content at its current size. False if an absolute box inside it has its
    /// containing block outside it (the caller lays out the whole window instead).
    fn boundary(&mut self, b: PNode) -> bool {
        let i = b.index() as usize;
        let ul = self.doc.layout[i].unrounded;
        let (w, h) = (ul.size.width, ul.size.height);
        let parent_size = match self.doc.core[i].parent {
            Some(p) => {
                let pl = self.doc.layout[p.index() as usize].unrounded;
                Size { width: Some(pl.size.width), height: Some(pl.size.height) }
            }
            None => Size::NONE,
        };
        let out = self.compute_child_layout(
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
        out.oof_candidates.is_empty()
    }

    /// Rounds `b`'s subtree exactly as a whole-tree `round_layout` would: taffy rounds cumulative
    /// positions, so `b` is rounded from its absolute unrounded position (its own final layout,
    /// relative to its parent, does not change and is restored).
    fn round_from(&mut self, b: PNode) {
        let i = b.index() as usize;
        let (mut x, mut y) = (0.0, 0.0);
        let mut a = positional_parent(self.doc, i);
        while let Some(pi) = a {
            x += self.doc.layout[pi].unrounded.location.x;
            y += self.doc.layout[pi].unrounded.location.y;
            a = positional_parent(self.doc, pi);
        }
        let (saved_u, saved_f) = (self.doc.layout[i].unrounded, self.doc.layout[i].layout);
        self.doc.layout[i].unrounded.location.x += x;
        self.doc.layout[i].unrounded.location.y += y;
        round_layout(self, tid(b));
        self.doc.layout[i].unrounded = saved_u;
        self.doc.layout[i].layout = saved_f;
    }
}

impl Document {
    /// The queued layout work of `window`.
    pub(crate) fn run_layout(&mut self, window: WindowId) -> LayoutRun {
        let mut run = LayoutRun::default();
        let Some(w) = self.window_mut(window) else { return run };
        let (root, size) = (w.root, w.size);
        let mut full = std::mem::take(&mut w.dirty.full) || !w.laid_out;
        let remeasure = std::mem::take(&mut w.dirty.remeasure);
        // the measure guard: a leaf whose max-content size is unchanged needs no layout
        for n in remeasure {
            let Some(i) = self.idx(n) else { continue };
            self.core[i].flags.remove(NodeFlags::IN_REMEASURE);
            self.core[i].dirty.remove(Dirty::MEASURE);
            if self.core[i].window != Some(window) {
                continue;
            }
            run.remeasured += 1;
            let new = measure_leaf(self, i, Size::NONE, Size { width: AvailableSpace::MaxContent, height: AvailableSpace::MaxContent });
            let new = PSize::new(new.width, new.height);
            let old = self.layout[i].intrinsic.replace(new);
            // a wrapping leaf wider than its box may break differently with new text
            let width = self.layout[i].layout.size.width;
            let wraps = self.style.computed(i).wrap && new.w > width + 0.5;
            if old != Some(new) || wraps {
                run.resized += 1;
                self.invalidate_content(i);
            }
        }
        let relayout = std::mem::take(&mut self.window_mut(window).expect("window").dirty.relayout);
        let mut t = Tree { doc: self, leaf_layouts: 0 };
        if !full {
            for b in &relayout {
                let Some(i) = t.doc.idx(*b) else { continue };
                t.doc.core[i].flags.remove(NodeFlags::IN_RELAYOUT);
                if t.doc.core[i].window != Some(window) {
                    continue;
                }
                if *b == root {
                    full = true;
                    break;
                }
                if !t.boundary(*b) {
                    // an absolute box inside escapes to a containing block above: the blocks above
                    // lay out again (a cached one would keep its old hoisted list, without the box)
                    let mut a = t.doc.core[i].parent;
                    while let Some(p) = a {
                        let pi = p.index() as usize;
                        t.doc.layout[pi].cache.clear();
                        a = t.doc.core[pi].parent;
                    }
                    full = true;
                    break;
                }
                t.round_from(*b);
                run.boundaries += 1;
            }
        }
        for b in &relayout {
            if let Some(i) = t.doc.idx(*b) {
                t.doc.core[i].flags.remove(NodeFlags::IN_RELAYOUT);
            }
        }
        if full {
            t.whole(root, size);
            run.full = true;
        }
        run.leaf_layouts = t.leaf_layouts;
        if let Some(w) = self.window_mut(window) {
            w.laid_out = true;
            if run.full || run.boundaries > 0 {
                w.rebuild = true;
            }
        }
        run
    }
}
