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
//! WG1: `grid` is a taffy CSS grid ([`parse_grid_template`] reads its columns); `collapsing-header`
//! is a flexbox whose top padding is its measured header and whose children get the hidden layout
//! while it is closed; `scroll-area` is a flexbox with `overflow: scroll` on its axes (content
//! overflows it, the walk scrolls it).
//! SG4: provider kinds lay out their (shadow) children: flexbox from their style, or the widget's
//! own [`Widget::layout`](crate::provider::Widget::layout) with `Caps::CUSTOM_LAYOUT` through a
//! [`LayoutCx`]. `position: absolute` boxes are hoisted to their containing block (the nearest
//! positioned ancestor, else the root's top) as taffy 0.15 records them (`NodeCold::hoisted`), and
//! their rects are placed relative to it. A boundary relayout whose subtree has an absolute box
//! with a containing block outside it falls back to a whole-root layout.
//!
//! Writer lanes: SCENE (the adapter), WIDGETS (per-kind measure), STYLE (computed style).

pub use crate::scene::dirty::invalidate;
use crate::scene::id::IDX_MASK;
use crate::scene::node::Payload;
use crate::scene::{Common, Dirty, NodeFlags, NodeId as SceneId, RootIx, Scene, Visual, W};
use egui::{Rect, Vec2, pos2, vec2};
use taffy::prelude::{AvailableSpace, Dimension, Display, FlexDirection, FlexWrap, Layout, LengthPercentage, LengthPercentageAuto, Position, Size, Style};
use taffy::style_helpers::{auto, fit_content, fr, length, max_content, min_content, minmax, percent, repeat};
use taffy::{
    AlignContent, AlignItems, CacheTree, GridTemplateComponent, LayoutInput, LayoutOutput, LayoutPartialTree, Line, MaxTrackSizingFunction, MinTrackSizingFunction, NodeId,
    RequestedAxis, RunMode, SizingMode, TrackSizingFunction, compute_cached_layout, compute_flexbox_layout, compute_grid_layout, compute_hidden_layout, compute_leaf_layout,
    compute_oof_layout, compute_root_layout, round_layout,
};

/// egui's default `Spacing::indent` and `interact_size.y`: a collapsing header's body offsets
/// before the first layout measures the real ones (`Tree::collapsing`).
const DEFAULT_INDENT: f32 = 18.0;
const DEFAULT_HEADER: f32 = 18.0;

/// Parses a `grid-template-columns` value (the CSS subset in the `grid` kind's doc). Empty: no
/// explicit columns (one implicit `auto` column).
pub fn parse_grid_template(src: &str) -> Result<Vec<GridTemplateComponent<String>>, String> {
    let mut out = Vec::new();
    for item in split_top(src, |c| c.is_ascii_whitespace())? {
        if let Some(inner) = call(&item, "repeat") {
            let parts = split_top(inner, |c| c == ',')?;
            let [count, tracks] = parts.as_slice() else { return Err(format!("`{item}`: repeat takes a count and tracks")) };
            let n: u16 = count.trim().parse().ok().filter(|n| *n > 0).ok_or_else(|| format!("`{item}`: the count must be a positive integer"))?;
            let tracks = split_top(tracks, |c| c.is_ascii_whitespace())?.iter().map(|t| track(t)).collect::<Result<Vec<_>, _>>()?;
            if tracks.is_empty() {
                return Err(format!("`{item}`: no tracks"));
            }
            out.push(repeat(n, tracks));
        } else {
            out.push(GridTemplateComponent::Single(track(&item)?));
        }
    }
    Ok(out)
}

/// `name(args)` → `args`.
fn call<'a>(s: &'a str, name: &str) -> Option<&'a str> {
    s.strip_prefix(name)?.trim_start().strip_prefix('(')?.strip_suffix(')')
}

/// Splits at top-level separators (outside parentheses), dropping empty pieces.
fn split_top(s: &str, sep: impl Fn(char) -> bool) -> Result<Vec<String>, String> {
    let (mut out, mut cur, mut depth) = (Vec::new(), String::new(), 0i32);
    for c in s.chars() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            _ => {}
        }
        if depth < 0 {
            return Err(format!("`{s}`: unbalanced parentheses"));
        }
        if depth == 0 && sep(c) {
            if !cur.trim().is_empty() {
                out.push(cur.trim().to_string());
            }
            cur.clear();
        } else {
            cur.push(c);
        }
    }
    if depth != 0 {
        return Err(format!("`{s}`: unbalanced parentheses"));
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    Ok(out)
}

enum Breadth {
    Points(f32),
    Percent(f32),
    Fr(f32),
}

fn breadth(t: &str) -> Option<Breadth> {
    let num = |s: &str| s.trim().parse::<f32>().ok().filter(|v| v.is_finite() && *v >= 0.0);
    if let Some(v) = t.strip_suffix("fr") {
        num(v).map(Breadth::Fr)
    } else if let Some(v) = t.strip_suffix('%') {
        num(v).map(Breadth::Percent)
    } else {
        num(t.strip_suffix("px").unwrap_or(t)).map(Breadth::Points)
    }
}

fn track(t: &str) -> Result<TrackSizingFunction, String> {
    let bad = || format!("`{t}` is not a grid track (auto, min-content, max-content, <n>fr, <n>px, <n>%, minmax(), fit-content())");
    Ok(match t {
        "auto" => auto(),
        "min-content" => min_content(),
        "max-content" => max_content(),
        _ => {
            if let Some(inner) = call(t, "minmax") {
                let parts = split_top(inner, |c| c == ',')?;
                let [a, b] = parts.as_slice() else { return Err(bad()) };
                minmax(min_track(a).ok_or_else(bad)?, max_track(b).ok_or_else(bad)?)
            } else if let Some(inner) = call(t, "fit-content") {
                match breadth(inner.trim()).ok_or_else(bad)? {
                    Breadth::Points(v) => fit_content(LengthPercentage::length(v)),
                    Breadth::Percent(v) => fit_content(LengthPercentage::percent(v / 100.0)),
                    Breadth::Fr(_) => return Err(bad()),
                }
            } else {
                match breadth(t).ok_or_else(bad)? {
                    Breadth::Points(v) => length(v),
                    Breadth::Percent(v) => percent(v / 100.0),
                    Breadth::Fr(v) => fr(v),
                }
            }
        }
    })
}

fn min_track(t: &str) -> Option<MinTrackSizingFunction> {
    Some(match t.trim() {
        "auto" => auto(),
        "min-content" => min_content(),
        "max-content" => max_content(),
        t => match breadth(t)? {
            Breadth::Points(v) => length(v),
            Breadth::Percent(v) => percent(v / 100.0),
            Breadth::Fr(_) => return None,
        },
    })
}

fn max_track(t: &str) -> Option<MaxTrackSizingFunction> {
    Some(match t.trim() {
        "auto" => auto(),
        "min-content" => min_content(),
        "max-content" => max_content(),
        t => match breadth(t)? {
            Breadth::Points(v) => length(v),
            Breadth::Percent(v) => percent(v / 100.0),
            Breadth::Fr(v) => fr(v),
        },
    })
}

/// A node's taffy style from its props.
pub fn style_of(c: &Common, w: &W, vis: &Visual) -> Style {
    let len = |v: Option<f64>| v.map(|x| Dimension::length(x as f32)).unwrap_or(Dimension::auto());
    let lpa = |v: Option<f64>| v.map(|x| LengthPercentageAuto::length(x as f32)).unwrap_or(LengthPercentageAuto::auto());
    // SG4: a provider kind declared a container (or laying out its own children) takes the box props
    let container = w.is_container()
        || matches!(w, W::Ext(x) if x.entry().caps.contains(crate::provider::Caps::CONTAINER) || x.entry().caps.contains(crate::provider::Caps::CUSTOM_LAYOUT));
    let mut s = Style {
        display: if !c.visible {
            Display::None
        } else if crate::overlays::out_of_flow(w) {
            // WG2a: overlays and menu entries take no place in their parent's flow
            Display::None
        } else if matches!(w, W::Grid { .. }) {
            Display::Grid
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
        // SG4: out-of-flow boxes (`position: absolute`), hoisted to their containing block
        position: match c.position {
            "absolute" => Position::Absolute,
            "relative" => Position::Relative,
            _ => Position::Static,
        },
        inset: taffy::Rect { left: lpa(c.left), right: lpa(c.right), top: lpa(c.top), bottom: lpa(c.bottom) },
        ..Style::default()
    };
    // OW2: an own widget's own part of its style (a scroll area's overflow, a panel's size)
    if let W::Ext(x) = w
        && let Ok(b) = x.0.try_borrow()
    {
        b.taffy_style(&mut s);
    }
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
    match w {
        W::Grid { columns, justify_items } => {
            s.grid_template_columns = parse_grid_template(columns).unwrap_or_default();
            s.justify_items = match *justify_items {
                "start" => AlignItems::START,
                "end" => AlignItems::END,
                "center" => AlignItems::CENTER,
                _ => AlignItems::STRETCH,
            };
        }
        W::CollapsingHeader { .. } => {
            // the header row above the body, the indent left of it (`Tree::collapsing` measures them)
            s.padding.top = LengthPercentage::length(DEFAULT_HEADER + c.padding as f32);
            s.padding.left = LengthPercentage::length(DEFAULT_INDENT + c.padding as f32);
        }
        // WG2b: `panel`, `sides`, `frame` (`crate::elements::panes`)
        w if crate::elements::is_container(w) => crate::elements::style(c, w, &mut s),
        _ => {}
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

    pub(crate) fn text_size(&self, text: &str, wrap: Option<f32>) -> Vec2 {
        self.text_size_in(text, self.font(), wrap)
    }

    /// The body font at `weight` (`crate::fonts`).
    fn weighted(&self, text: &str, weight: i64) -> egui::FontId {
        crate::fonts::text_font(&self.ctx, &self.style, egui::TextStyle::Body, weight, text)
    }

    fn text_size_in(&self, text: &str, font: egui::FontId, wrap: Option<f32>) -> Vec2 {
        self.ctx.fonts_mut(|f| match wrap {
            Some(w) => f.layout(text.to_string(), font, egui::Color32::WHITE, w.max(0.0)).size(),
            None => f.layout_no_wrap(text.to_string(), font, egui::Color32::WHITE).size(),
        })
    }

    pub(crate) fn row_height(&self) -> f32 {
        let font = self.font();
        self.ctx.fonts_mut(|f| f.row_height(&font))
    }

    /// The size the widget of `w` asks for with no wrapping (content + chrome).
    pub fn intrinsic(&self, w: &W) -> Vec2 {
        self.intrinsic_with(w, None, &[])
    }

    /// [`Self::intrinsic`] with the node's computed style (OW0: own widgets measure content +
    /// padding + border from it).
    pub(crate) fn intrinsic_with(&self, w: &W, css: Option<&svelters_style::ComputedStyle>, parts: &[(Box<str>, std::sync::Arc<svelters_style::ComputedStyle>)]) -> Vec2 {
        let sp = &self.style.spacing;
        let interact = sp.interact_size;
        match w {
            W::Label { rich: Some(r), .. } => crate::rich::measure(self, r, None), // RT2
            W::Run { .. } => Vec2::ZERO, // drawn by its label
            W::Label { text, weight, .. } => self.text_size_in(text, self.weighted(text, *weight), None),
            W::Button { text, small, weight, .. } => {
                let t = self.text_size_in(text, self.weighted(text, *weight), None);
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
            W::Box | W::Window { .. } | W::Grid { .. } | W::VirtualList(_) => Vec2::ZERO,
            // ---- WG2a ----
            W::Menu(x) => crate::menus::measure(self, x),
            W::Overlay(_) => Vec2::ZERO,
            W::Ext(x) => x.measure(self, crate::provider::AvailableSpace::MaxContent, css, parts),
            // ---- WG2b (`crate::elements`) ----
            W::Hyperlink { .. }
            | W::RadioButton { .. }
            | W::Switch { .. }
            | W::RangeSlider { .. }
            | W::DragValue { .. }
            | W::Sides
            | W::Frame { .. }
            | W::Stack { .. }
            | W::Image { .. }
            | W::ProgressBar { .. }
            | W::Spinner { .. }
            | W::Canvas(_) => crate::elements::intrinsic(self, w),
            // WG4a: the views' own size (`crate::views::intrinsic`)
            W::Table(_) | W::TableColumn { .. } | W::TabView { .. } | W::Tab { .. } | W::ToastHost(_) => crate::views::intrinsic(self, w),
            W::Picker(p) => p.measure(self), // WG4b
            // ---- WG1 ----
            W::ToggleButton { text, weight, .. } => {
                let t = self.text_size_in(text, self.weighted(text, *weight), None);
                vec2(t.x + 2.0 * sp.button_padding.x, (t.y + 2.0 * sp.button_padding.y).max(interact.y))
            }
            W::ComboBox { options, hint, .. } => {
                // the widest option (or the hint), the drop-down icon, the button padding
                let widest = options.iter().chain(std::iter::once(hint)).map(|o| self.text_size(o, None).x).fold(0.0, f32::max);
                let w = widest + sp.icon_spacing + sp.icon_width + 2.0 * sp.button_padding.x;
                vec2(w.max(sp.combo_width), (self.row_height() + 2.0 * sp.button_padding.y).max(interact.y))
            }
            // egui's `Separator::default()` spacing
            W::Separator { vertical: false } => vec2(0.0, 6.0),
            W::Separator { vertical: true } => vec2(6.0, 0.0),
            W::Icon { name, glyph, family, size, weight, frame, source, .. } => {
                let t = if !source.is_empty() {
                    // AS1: an image icon is a square of `size` (else the body size)
                    Vec2::splat(size.map_or_else(|| egui::TextStyle::Body.resolve(&self.style).size, |s| s as f32))
                } else {
                    match crate::fonts::icon_look(&self.ctx, &self.style, family, name, glyph, *weight, *size) {
                    crate::fonts::IconLook::Glyph(font, text) => self.text_size_in(&text, font, None),
                    crate::fonts::IconLook::Placeholder(side) => Vec2::splat(side),
                    }
                };
                if *frame { vec2(t.x + 2.0 * sp.button_padding.x, (t.y + 2.0 * sp.button_padding.y).max(interact.y)) } else { t }
            }
            // the header row (`CollapsingHeader::begin`): indent, caption, padding
            W::CollapsingHeader { text, .. } => {
                let t = self.text_size(text, None);
                vec2(sp.indent + t.x + sp.button_padding.x, t.y + 2.0 * sp.button_padding.y).max(interact)
            }
        }
    }

    /// Height-for-width of a leaf at `width` (labels that wrap).
    fn wrapped(&self, w: &W, width: f32) -> Option<Vec2> {
        match w {
            W::Label { rich: Some(r), wrap: "wrap", .. } => Some(crate::rich::measure(self, r, Some(width))), // RT2
            W::Label { text, wrap: "wrap", weight, .. } => Some(self.text_size_in(text, self.weighted(text, *weight), Some(width))),
            W::Ext(x) if x.entry().caps.contains(crate::provider::Caps::HEIGHT_FOR_WIDTH) => {
                Some(x.measure(self, crate::provider::AvailableSpace::Definite(width), None, &[]))
            }
            _ => None,
        }
    }
}

/// CS5f: the metrics leaf `i` measures with when its computed style changes its font or button
/// padding (`style::leaf_style`, which painting uses too).
fn styled_metrics(s: &Scene, m: &Metrics, i: usize) -> Option<Metrics> {
    crate::style::leaf_style(s, i, &m.ctx, &m.style).map(|st| Metrics { ctx: m.ctx.clone(), style: std::sync::Arc::new(st) })
}

// ---- the taffy adapter over the scene's columns ----

#[inline]
pub(crate) fn ix(id: NodeId) -> usize {
    (u64::from(id) & IDX_MASK) as usize
}

#[inline]
pub(crate) fn tid(id: SceneId) -> NodeId {
    NodeId::from(id.to_raw())
}

/// The scene seen as a taffy tree for one layout run (`pub(crate)` for the WG4a views' own
/// container layouts, `crate::views::layout`).
pub(crate) struct Tree<'a> {
    pub(crate) s: &'a mut Scene,
    pub(crate) m: &'a Metrics,
    /// Leaf computations (cache misses).
    pub(crate) leaf_layouts: u64,
    /// Leaves measured with egui's fonts.
    pub(crate) fonts: u64,
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
        self.s.layout[ix(id)].unrounded = crate::scene::compact::BoxRect::of(layout);
    }
    fn compute_child_layout(&mut self, id: NodeId, inputs: LayoutInput) -> LayoutOutput {
        // Hidden: `display: none`, or inside a hidden ancestor (taffy passes HIDDEN to children).
        if inputs.run_mode == RunMode::PerformHiddenLayout || self.s.layout[ix(id)].style.display == Display::None {
            return compute_hidden_layout(self, id);
        }
        compute_cached_layout(self, id, inputs, |tree, id, inputs| {
            let i = ix(id);
            // the WIDGETS containers with their own layout
            match tree.s.w(i) {
                // WG4a: table (columns hidden), tab-view (the selected page)
                W::Table(_) | W::TabView { .. } => return crate::views::layout(tree, id, inputs),
                &W::CollapsingHeader { open, .. } => return tree.collapsing(id, open, inputs),
                // WG2b: one child shown at a time (`crate::elements::panes`)
                W::Stack { .. } => {
                    let (pick, style) = (crate::elements::panes::stack_pick(tree.s, i), tree.s.layout[i].style.clone());
                    return crate::elements::panes::stack_layout(tree, id, inputs, &style, pick);
                }
                _ => {}
            }
            // SG4: a provider with children (shadow children) or its own layout is a container
            let custom = provider_caps(tree.s, i).is_some_and(|c| c.contains(crate::provider::Caps::CUSTOM_LAYOUT));
            // (OW0: an own widget replacing a built-in leaf stays a leaf: its children are attached
            // overlays, as on the built-in; OW2: one replacing a container, `Caps::CONTAINER`, is one)
            let provider_box = custom
                || (matches!(tree.s.w(i), W::Ext(x) if x.entry().replaces.is_none() || x.entry().caps.contains(crate::provider::Caps::CONTAINER))
                    && !tree.s.children[i].is_empty());
            if tree.s.w(i).is_container() || provider_box {
                let mut out = if custom {
                    tree.custom_layout(id, inputs)
                } else if matches!(tree.s.w(i), W::Grid { .. }) || tree.s.layout[i].style.display == Display::Grid {
                    // WG1's `grid`, and CS5e's `display: grid` from the computed style
                    compute_grid_layout(tree, id, inputs)
                } else {
                    compute_flexbox_layout(tree, id, inputs)
                };
                // the absolute boxes this node is the containing block of (measure runs must not)
                if inputs.run_mode == RunMode::PerformLayout {
                    compute_oof_layout(tree, id, &mut out);
                }
                return out;
            }
            if matches!(tree.s.w(i), W::VirtualList(_)) {
                return vlist_layout(tree, id, inputs);
            }
            tree.leaf_layouts += 1;
            // CS5f: the leaf's computed font and padding
            let styled = styled_metrics(tree.s, tree.m, i);
            let m = styled.as_ref().unwrap_or(tree.m);
            let s = &mut *tree.s;
            if s.layout[i].intrinsic.is_none() {
                crate::rich::refresh(s, i); // RT2: a rich label's runs, before it is measured
            }
            let w = match &s.payload[i] {
                Payload::Builtin(w) => w,
                Payload::Vacant => unreachable!("vacant payload in layout"),
            };
            let css = s.slots.id_at(i).and_then(|id| s.style.computed(id)).cloned();
            let parts = s.slots.id_at(i).map(|id| s.style.part_styles(id)).unwrap_or_default();
            let lb = &mut s.layout[i];
            let mut fonts = 0;
            let intrinsic = *lb.intrinsic.get_or_insert_with(|| {
                fonts += 1;
                m.intrinsic_with(w, css.as_deref(), &parts)
            });
            let out = compute_leaf_layout(inputs, &*lb.style, |_, _| 0.0, |known, avail| {
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

/// LS1: a `virtual-list` sizes itself as an empty leaf (its rows never reach its intrinsic size,
/// ARCH-SCENE §8.2 after slint §4) and, when it performs layout, lays out its bound rows only, at
/// its width: each row measured to its content height (the list's `row-height` is the minimum) or
/// at its row source's uniform height, then placed by the list's height model at
/// `offset(pos) - scroll` (`kinds/virtuallist.rs` `place_rows`). Unbound rows get the hidden
/// layout.
fn vlist_layout(tree: &mut Tree<'_>, id: NodeId, inputs: LayoutInput) -> LayoutOutput {
    use crate::kinds::virtuallist;
    let i = ix(id);
    let out = compute_leaf_layout(inputs, &*tree.s.layout[i].style, |_, _| 0.0, |known, _| Size { width: known.width.unwrap_or(0.0), height: known.height.unwrap_or(0.0) });
    if inputs.run_mode != RunMode::PerformLayout {
        return out;
    }
    let (w, h) = (out.size.width, out.size.height);
    let (placed, fixed, min_h) = {
        let W::VirtualList(v) = tree.s.w(i) else { unreachable!("a virtual-list") };
        let placed: Vec<_> = v.placed(tree.s).into_iter().map(|(n, p)| (n, p, v.row_inset(p).min(w))).collect();
        (placed, virtuallist::uniform_height(v), v.heights().estimate().min(v.row_height_points()))
    };
    for k in 0..tree.s.children[i].len() {
        let c = tree.s.children[i][k];
        if !placed.iter().any(|&(n, _, _)| n == c) {
            compute_hidden_layout(tree, tid(c));
        }
    }
    let mut measured = Vec::with_capacity(placed.len());
    for (c, pos, inset) in placed {
        let w = w - inset;
        let r = tree.compute_child_layout(
            tid(c),
            LayoutInput {
                run_mode: RunMode::PerformLayout,
                sizing_mode: SizingMode::InherentSize,
                axis: RequestedAxis::Both,
                known_dimensions: Size { width: Some(w), height: fixed },
                known_dimensions_are_definite: Size { width: true, height: fixed.is_some() },
                parent_size: Size { width: Some(w), height: Some(h) },
                available_space: Size { width: AvailableSpace::Definite(w), height: AvailableSpace::MaxContent },
                vertical_margins_are_collapsible: Line::FALSE,
            },
        );
        measured.push((c, pos, fixed.unwrap_or(r.size.height.max(min_h))));
    }
    let W::VirtualList(v) = tree.s.w_mut(i) else { unreachable!("a virtual-list") };
    let places = virtuallist::place_rows(v, vec2(w, h), &measured);
    for (k, (c, p, sz)) in places.into_iter().enumerate() {
        let mut l = Layout::with_order(k as u32);
        l.location = taffy::Point { x: p.x, y: p.y };
        l.size = Size { width: sz.x, height: sz.y };
        tree.set_unrounded_layout(tid(c), &l);
    }
    out
}

impl Tree<'_> {
    /// WG1: a collapsing header. Its top padding is the measured header row and its left padding
    /// the indent (both plus its own `padding`); open, it lays out its children as a flex box;
    /// closed, it is a leaf of the header's size and its children get the hidden layout.
    fn collapsing(&mut self, id: NodeId, open: bool, inputs: LayoutInput) -> LayoutOutput {
        let i = ix(id);
        let m = self.m;
        let head = match self.s.layout[i].intrinsic {
            Some(h) => h,
            None => {
                self.fonts += 1;
                let h = m.intrinsic(self.s.w(i));
                self.s.layout[i].intrinsic = Some(h);
                h
            }
        };
        let pad = self.s.common(i).padding as f32;
        let indent = m.style.spacing.indent;
        let (top, left) = (LengthPercentage::length(head.y + pad), LengthPercentage::length(indent + pad));
        let st = &mut self.s.layout[i].style;
        if st.padding.top != top || st.padding.left != left {
            // SG6: the header's own padding (unshares the interned style once)
            let st = std::sync::Arc::make_mut(st);
            st.padding.top = top;
            st.padding.left = left;
        }
        if open {
            return compute_flexbox_layout(self, id, inputs);
        }
        self.leaf_layouts += 1;
        let out = compute_leaf_layout(inputs, &*self.s.layout[i].style, |_, _| 0.0, |known, _| Size {
            width: known.width.unwrap_or((head.x - indent).max(0.0).ceil()),
            height: known.height.unwrap_or(0.0),
        });
        if inputs.run_mode == RunMode::PerformLayout {
            for k in 0..self.s.children[i].len() {
                let c = tid(self.s.children[i][k]);
                compute_hidden_layout(self, c);
            }
        }
        out
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

impl taffy::LayoutContainingBlock for Tree<'_> {
    type OofItemStyle<'b>
        = &'b Style
    where
        Self: 'b;
    fn get_oof_item_style(&self, id: NodeId) -> Self::OofItemStyle<'_> {
        &self.s.layout[ix(id)].style
    }
    // SG4: the out-of-flow boxes a containing block positions, in its cold column
    fn clear_hoisted_children(&mut self, id: NodeId) {
        if let Some(c) = self.s.cold[ix(id)].as_deref_mut() {
            c.hoisted.clear();
        }
    }
    fn add_hoisted_children(&mut self, id: NodeId, hoisted: &[NodeId]) {
        if hoisted.is_empty() {
            return;
        }
        let ids = hoisted.iter().filter_map(|&h| SceneId::from_raw(u64::from(h)));
        self.s.cold_mut(ix(id)).hoisted.extend(ids);
    }
}

/// SG6: true gives built-in leaves a layout cache too (the "before" of the milestone's layout
/// benchmark).
const CACHE_LEAVES: bool = false;

impl CacheTree for Tree<'_> {
    fn cache_get(&mut self, id: NodeId, inputs: &LayoutInput) -> Option<LayoutOutput> {
        self.s.layout[ix(id)].cache.get(inputs)
    }
    fn cache_store(&mut self, id: NodeId, inputs: &LayoutInput, out: LayoutOutput) {
        // SG6: only nodes whose layout is not a built-in leaf's cheap rerun keep a cache
        let i = ix(id);
        // (OW1a2: an own widget replacing a built-in leaf is such a leaf: its intrinsic size is
        // cached in `LayoutBox` like the egui wrapper's)
        let leaf_like = |w: &W| match w {
            W::Ext(x) => {
                let e = x.entry();
                e.replaces.is_some() && !e.caps.contains(crate::provider::Caps::CONTAINER) && !e.caps.contains(crate::provider::Caps::CUSTOM_LAYOUT)
            }
            w => !w.is_container(),
        };
        let cached = !self.s.children[i].is_empty() || !matches!(&self.s.payload[i], Payload::Builtin(w) if leaf_like(w));
        if cached || CACHE_LEAVES {
            self.s.layout[i].cache.store(inputs, out)
        }
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

impl taffy::RoundTree for Tree<'_> {
    fn get_unrounded_layout(&self, id: NodeId) -> Layout {
        self.s.layout[ix(id)].unrounded.to_layout()
    }
    fn set_final_layout(&mut self, id: NodeId, layout: &Layout) {
        self.s.layout[ix(id)].layout = crate::scene::compact::BoxRect::of(layout);
    }
    fn is_out_of_flow(&self, id: NodeId) -> bool {
        out_of_flow(self.s, ix(id))
    }
    fn hoisted_child_count(&self, id: NodeId) -> usize {
        self.s.cold(ix(id)).hoisted.len()
    }
    fn get_hoisted_child_id(&self, id: NodeId, index: usize) -> NodeId {
        tid(self.s.cold(ix(id)).hoisted[index])
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
        let want = Size { width: Dimension::length(size.x), height: Dimension::length(size.y) };
        let st = &mut self.s.layout[top.index() as usize].style;
        if st.size != want {
            std::sync::Arc::make_mut(st).size = want; // SG6: a root's own style (one per root)
        }
        let id = tid(top);
        compute_root_layout(self, id, Size { width: AvailableSpace::Definite(size.x), height: AvailableSpace::Definite(size.y) });
        round_layout(self, id);
    }

    /// Boundary `b`'s content at its current size (its own place and size do not change). False if
    /// an absolute box inside it has its containing block outside it (SG4: the caller lays out the
    /// whole root instead).
    fn boundary(&mut self, b: SceneId) -> bool {
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

    /// SG4: `Caps::CUSTOM_LAYOUT`: the provider's own `layout`, with its payload taken out.
    fn custom_layout(&mut self, id: NodeId, inputs: LayoutInput) -> LayoutOutput {
        let i = ix(id);
        let Some(node) = self.s.slots.id_at(i) else { return compute_flexbox_layout(self, id, inputs) };
        let m = self.m;
        let r = crate::provider::with_taken(self.s, i, |w, s| {
            let mut cx = LayoutCx { s, m, node, leaf_layouts: 0, fonts: 0 };
            let out = w.layout(&mut cx, inputs);
            (out, cx.leaf_layouts, cx.fonts)
        });
        match r {
            Some((out, leaf, fonts)) => {
                self.leaf_layouts += leaf;
                self.fonts += fonts;
                self.s.counters.custom_layouts += 1;
                out
            }
            None => compute_flexbox_layout(self, id, inputs),
        }
    }

    /// Rounds `b`'s subtree exactly as a whole-tree `round_layout` would: taffy rounds cumulative
    /// positions, so `b` is rounded from its absolute unrounded position (its own final layout,
    /// relative to its parent, does not change and is restored).
    fn round_from(&mut self, b: SceneId) {
        let i = b.index() as usize;
        let (mut x, mut y) = (0.0, 0.0);
        // positions are relative to the positional parent (SG4: an absolute box's containing block)
        let mut a = positional_parent(self.s, i);
        while let Some(pi) = a {
            x += self.s.layout[pi].unrounded.location.x;
            y += self.s.layout[pi].unrounded.location.y;
            a = positional_parent(self.s, pi);
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
    let mut order = Vec::new();
    while let Some((i, o)) = stack.pop() {
        order.push(i);
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
            let c = s.children[i][k].index() as usize;
            // SG4: an absolute box is placed from its containing block's hoisted list
            if !out_of_flow(s, c) {
                stack.push((c, min));
            }
        }
        let hoisted = s.cold(i).hoisted.len();
        for k in 0..hoisted {
            let h = s.cold(i).hoisted[k];
            if let Some(hi) = s.idx(h)
                && s.core[hi].root == s.core[i].root
            {
                stack.push((hi, min));
            }
        }
    }
    // SG6: ink bounds, children before parents, then up the ancestors while they change
    for &i in order.iter().rev() {
        ink(s, i);
    }
    let mut up = s.core[top.index() as usize].parent;
    while let Some(p) = up.and_then(|p| s.idx(p)) {
        if !ink(s, p) {
            break;
        }
        up = s.core[p].parent;
    }
    n
}

/// SG6 (ARCH-SCENE §7.3): node `i`'s ink rect, the root-relative rect its subtree paints in: its
/// paint bounds (CS5g shadows) joined with its children's ink, except where it clips them (a scroll
/// area, a virtual list, a `clip` channel). A TR1 transform is visual and changes without a
/// layout, so it is not in the ink (as before SG6, a transformed node is culled by its layout
/// bounds): an infinite ink would keep subtrees walked in clipped passes, where their widgets
/// register for focus (`tests/animator.rs` tab_skips_exiting_nodes). Stored only when larger than the node's own bounds
/// (`NodeFlags::INK_OVERFLOW`). Overlays and menus are out of flow and paint themselves. True if
/// it changed.
fn ink(s: &mut Scene, i: usize) -> bool {
    let own = crate::style::paint_bounds(s, i, s.core[i].rect);
    let mut ink = own;
    // a node that is not laid out (hidden, not shown) paints nothing, whatever its children's
    // last rects say
    // (OW2: the scroll area is svelters' own widget, `widgets::scroll_area`)
    let clips = own == Rect::NOTHING
        || matches!(s.w(i), W::VirtualList(_))
        || matches!(s.w(i), W::Ext(x) if x.entry().replaces == Some("scroll-area"))
        || s.vis(i).clip.is_some();
    if !clips {
        for &c in &s.children[i] {
            let ci = c.index() as usize;
            if s.core[ci].rect == Rect::NOTHING || !s.common(ci).visible || crate::overlays::out_of_flow(s.w(ci)) {
                continue;
            }
            ink = ink.union(ink_of(s, ci));
        }
    }
    let fits = ink == Rect::NOTHING || (own != Rect::NOTHING && own.contains_rect(ink));
    let key = i as u32;
    let before = s.ink.get(&key).copied();
    if fits {
        s.core[i].flags.remove(NodeFlags::INK_OVERFLOW);
        s.ink.remove(&key);
        before.is_some()
    } else {
        s.core[i].flags.insert(NodeFlags::INK_OVERFLOW);
        s.ink.insert(key, ink);
        before != Some(ink)
    }
}

/// SG6: node `i`'s ink rect, root-relative (its paint bounds unless its subtree overflows them).
pub(crate) fn ink_of(s: &Scene, i: usize) -> Rect {
    if s.core[i].flags.contains(NodeFlags::INK_OVERFLOW)
        && let Some(r) = s.ink.get(&(i as u32))
    {
        return *r;
    }
    crate::style::paint_bounds(s, i, s.core[i].rect)
}

/// SG4: an out-of-flow box (`position: absolute`, shown).
fn out_of_flow(s: &Scene, i: usize) -> bool {
    let st = &s.layout[i].style;
    st.position.is_out_of_flow() && st.display != Display::None
}

/// SG4: what node `i`'s layout location is relative to: its parent, or for an absolute box its
/// containing block (the nearest positioned ancestor, else the top of its tree).
fn positional_parent(s: &Scene, i: usize) -> Option<usize> {
    let p = s.core[i].parent.map(|p| p.index() as usize)?;
    if !out_of_flow(s, i) {
        return Some(p);
    }
    let mut a = p;
    loop {
        if s.layout[a].style.position.is_positioned() {
            return Some(a);
        }
        match s.core[a].parent {
            Some(x) => a = x.index() as usize,
            None => return Some(a),
        }
    }
}

/// The capabilities of node `i`'s provider kind (or of the own widget that replaces a built-in,
/// OW1a2: `frame`, `collapsing-header`), `None` for a built-in's egui wrapper.
fn provider_caps(s: &Scene, i: usize) -> Option<crate::provider::Caps> {
    // OW2: an own widget replacing a built-in has a provider entry too (`replaces`)
    if let Payload::Builtin(W::Ext(x)) = &s.payload[i] {
        return Some(x.entry().caps);
    }
    crate::kinds::KindRegistry::global().provider(s.core[i].kind as usize).map(|p| p.entry.caps)
}

/// What [`Widget::layout`](crate::provider::Widget::layout) gets (SG4, `Caps::CUSTOM_LAYOUT`,
/// ARCH-SCENE §7.3): the node's children and taffy for them. Positions are relative to this node's
/// border box. Unstable (§16): taffy's types.
pub struct LayoutCx<'a> {
    s: &'a mut Scene,
    m: &'a Metrics,
    node: SceneId,
    leaf_layouts: u64,
    fonts: u64,
}

impl LayoutCx<'_> {
    fn with_tree<R>(&mut self, f: impl FnOnce(&mut Tree<'_>) -> R) -> R {
        let mut t = Tree { s: &mut *self.s, m: self.m, leaf_layouts: 0, fonts: 0 };
        let r = f(&mut t);
        self.leaf_layouts += t.leaf_layouts;
        self.fonts += t.fonts;
        r
    }
    /// This node.
    pub fn node(&self) -> SceneId {
        self.node
    }
    /// OW2: this node's taffy style (its props, CSS and `Widget::taffy_style`).
    pub fn style(&self) -> &crate::taffy::Style {
        &self.s.layout[self.node.index() as usize].style
    }
    /// OW2: lays out `child` as hidden (`display: none`; not drawn).
    pub fn hide(&mut self, child: SceneId) {
        self.with_tree(|t| {
            compute_hidden_layout(t, tid(child));
        });
    }
    /// OW2: a child's `Common` props (its `flex-grow`, `min-width`, `visible`).
    pub fn child_common(&self, child: SceneId) -> Option<&crate::scene::Common> {
        self.s.idx(child).map(|i| self.s.common(i))
    }
    /// The children to lay out (the node's shadow children), in order.
    pub fn children(&self) -> &[SceneId] {
        &self.s.children[self.node.index() as usize]
    }
    /// The scene, read-only (this node's own payload is out while its `layout` runs).
    pub fn scene(&self) -> crate::scene::SceneRef<'_> {
        crate::scene::SceneRef::new(self.s)
    }
    /// taffy's `compute_child_layout` of `child` (measure or full layout, with caching).
    pub fn compute_child(&mut self, child: SceneId, inputs: LayoutInput) -> LayoutOutput {
        self.with_tree(|t| t.compute_child_layout(tid(child), inputs))
    }
    /// The size `child` wants within `available` (taffy's size-only run).
    pub fn measure(&mut self, child: SceneId, available: egui::Vec2) -> egui::Vec2 {
        let out = self.compute_child(
            child,
            LayoutInput {
                run_mode: RunMode::ComputeSize,
                sizing_mode: SizingMode::InherentSize,
                axis: RequestedAxis::Both,
                known_dimensions: Size::NONE,
                known_dimensions_are_definite: Size { width: true, height: true },
                parent_size: Size::NONE,
                available_space: Size { width: AvailableSpace::Definite(available.x), height: AvailableSpace::Definite(available.y) },
                vertical_margins_are_collapsible: Line::FALSE,
            },
        );
        vec2(out.size.width, out.size.height)
    }
    /// Lays out `child` at exactly `size` and puts it at `at` (relative to this node's border box).
    /// Only in a `RunMode::PerformLayout` run.
    pub fn place(&mut self, child: SceneId, at: egui::Pos2, size: egui::Vec2) {
        let order = self.children().iter().position(|&c| c == child).unwrap_or(0) as u32;
        let out = self.compute_child(
            child,
            LayoutInput {
                run_mode: RunMode::PerformLayout,
                sizing_mode: SizingMode::InherentSize,
                axis: RequestedAxis::Both,
                known_dimensions: Size { width: Some(size.x), height: Some(size.y) },
                known_dimensions_are_definite: Size { width: true, height: true },
                parent_size: Size::NONE,
                available_space: Size { width: AvailableSpace::Definite(size.x), height: AvailableSpace::Definite(size.y) },
                vertical_margins_are_collapsible: Line::FALSE,
            },
        );
        let mut l = Layout::with_order(order);
        l.size = out.size;
        l.location = taffy::Point { x: at.x, y: at.y };
        self.s.layout[child.index() as usize].unrounded = crate::scene::compact::BoxRect::of(&l);
    }
    /// What the node would get without the cap: flexbox over its children from its style.
    pub fn default_layout(&mut self, inputs: LayoutInput) -> LayoutOutput {
        let id = tid(self.node);
        self.with_tree(|t| compute_flexbox_layout(t, id, inputs))
    }

    /// OW1a2: [`Self::default_layout`] with `add` (left, right, top, bottom, in points) added to
    /// the node's padding: room for chrome the widget paints itself (a frame's caption and
    /// border, a collapsing header's row and indent).
    pub fn padded_layout(&mut self, inputs: LayoutInput, add: [f32; 4]) -> LayoutOutput {
        let i = self.node.index() as usize;
        let grow = |p: LengthPercentage, a: f32| match p.into_raw().tag() {
            taffy::style::CompactLength::LENGTH_TAG => LengthPercentage::length(p.into_raw().value() + a),
            _ => p,
        };
        let saved = self.s.layout[i].style.clone();
        let st = std::sync::Arc::make_mut(&mut self.s.layout[i].style); // SG6: a copy for this run only
        let p = st.padding;
        st.padding = taffy::Rect { left: grow(p.left, add[0]), right: grow(p.right, add[1]), top: grow(p.top, add[2]), bottom: grow(p.bottom, add[3]) };
        let out = self.default_layout(inputs);
        self.s.layout[i].style = saved;
        out
    }

    /// OW1a2: the node as a leaf of `size` (its known dimensions win), with every child given the
    /// hidden layout: a closed collapsing header.
    pub fn collapsed(&mut self, inputs: LayoutInput, size: egui::Vec2) -> LayoutOutput {
        let i = self.node.index() as usize;
        self.leaf_layouts += 1;
        let out = compute_leaf_layout(inputs, &*self.s.layout[i].style, |_, _| 0.0, |known, _| Size { width: known.width.unwrap_or(size.x.ceil()), height: known.height.unwrap_or(size.y.ceil()) });
        if inputs.run_mode == RunMode::PerformLayout {
            for k in 0..self.s.children[i].len() {
                let c = tid(self.s.children[i][k]);
                self.with_tree(|t| compute_hidden_layout(t, c));
            }
        }
        out
    }

    /// OW1a2: a [`MeasureCx`](crate::provider::MeasureCx) for this node (its computed style and
    /// part styles), to size chrome the widget paints.
    pub fn with_measure<R>(&self, f: impl FnOnce(&crate::provider::MeasureCx<'_>) -> R) -> R {
        let css = self.s.style.computed(self.node).cloned();
        let parts = self.s.style.part_styles(self.node);
        let cx = crate::provider::MeasureCx { ctx: &self.m.ctx, style: &self.m.style, css: css.as_deref(), parts: &parts };
        f(&cx)
    }
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
    let mut oof_escaped = false;
    if !full {
        for &b in &bs {
            if hidden(t.s, b) {
                // inside a hidden subtree: the hidden (zero) layout, as a whole-root run gives it
                compute_hidden_layout(&mut t, tid(b));
            } else {
                if !t.boundary(b) {
                    oof_escaped = true;
                }
                run.boundaries += 1;
            }
        }
    }
    if oof_escaped {
        // SG4: an absolute box inside a boundary has its containing block above it: only the
        // containing block's layout positions it, so lay out the whole root (caches above the
        // boundaries cleared, as for a full run)
        for &b in &bs {
            let mut a = Some(b);
            while let Some(x) = a {
                let i = x.index() as usize;
                t.s.layout[i].cache.clear();
                a = t.s.core[i].parent;
            }
        }
        full = true;
        run.full = true;
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
            let o = positional_parent(s, b.index() as usize).map(|p| s.core[p].rect.min).unwrap_or(pos2(0.0, 0.0));
            placed += place(s, b, o);
        }
    }
    // SG2: `Damage::relayout` for observers of this root's layout; SG4: for `ScenePass::after_layout`
    let laid: Vec<SceneId> = if full { vec![top] } else { bs.clone() };
    if s.journal.wants(crate::scene::ChangeMask::LAYOUT) {
        for &b in &laid {
            s.journal.laid_out(&s.core, b);
        }
    }
    if let Some(e) = s.roots.get_mut(r) {
        if full {
            e.laid.clear();
        }
        for b in laid {
            if !e.laid.contains(&b) {
                e.laid.push(b);
            }
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

/// WG2a: lays out overlay node `top` (`popup`, `modal`, `inner-window`: out of its parent's flow,
/// where it has the hidden layout) on its own, as the root of its subtree: the axes of `size` are
/// definite, the others max-content (its own `width`/`height` still apply). Places it with its
/// top-left at `at` (root-relative, so its children's rects stay root-relative) and returns its size
/// and the layout generation it saw. `seen` is the generation of its last run: when any root ran a
/// layout since (`Counters::nodes_placed` moved), a root run may have given the subtree the hidden
/// layout below a cache this run would reuse, so its caches are cleared first.
pub(crate) fn layout_overlay(s: &mut Scene, m: &Metrics, top: SceneId, size: [Option<f32>; 2], at: egui::Pos2, seen: u64) -> (Vec2, u64) {
    let i = top.index() as usize;
    let generation = s.counters.nodes_placed;
    if generation != seen {
        let mut stack = vec![i];
        while let Some(k) = stack.pop() {
            s.layout[k].cache.clear();
            stack.extend(s.children[k].iter().map(|c| c.index() as usize));
        }
    }
    let saved = s.layout[i].style.clone();
    let st = std::sync::Arc::make_mut(&mut s.layout[i].style); // SG6: a copy for this run only
    st.display = Display::Flex;
    if let Some(w) = size[0] {
        st.size.width = Dimension::length(w);
    }
    if let Some(h) = size[1] {
        st.size.height = Dimension::length(h);
    }
    let avail = |v: Option<f32>| v.map_or(AvailableSpace::MaxContent, AvailableSpace::Definite);
    let mut t = Tree { s, m, leaf_layouts: 0, fonts: 0 };
    compute_root_layout(&mut t, tid(top), Size { width: avail(size[0]), height: avail(size[1]) });
    round_layout(&mut t, tid(top));
    s.layout[i].style = saved;
    let l = s.layout[i].layout;
    place(s, top, at - vec2(l.location.x, l.location.y));
    (vec2(l.size.width, l.size.height), generation)
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
        crate::rich::refresh(s, i); // RT2
        // CS5f: the leaf's computed font and padding; OW0: the computed style for own widgets
        let styled = styled_metrics(s, m, i);
        let css = s.slots.id_at(i).and_then(|id| s.style.computed(id)).cloned();
        let parts = s.slots.id_at(i).map(|id| s.style.part_styles(id)).unwrap_or_default();
        let new = styled.as_ref().unwrap_or(m).intrinsic_with(s.w(i), css.as_deref(), &parts);
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

