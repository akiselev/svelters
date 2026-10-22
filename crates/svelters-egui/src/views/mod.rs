//! WG4a: the Tier 1 views of EGUI.md §5 (plans/milestones/WG4a-tier1-views.md). Their kind tables
//! are `kinds/views.rs`; this module holds their state, layout, paint and methods, reached through
//! one dispatch arm each in `layout.rs`, `leaf.rs`, `scene/props.rs` and `ops.rs`:
//!
//! - [`table`]: `table` (egui_table) and its `table-column` children;
//! - [`tabs`]: `tab-view` and its `tab` pages;
//! - [`split`]: `split-view`;
//! - [`toast`]: `toast-host` and the Rust API [`toast::show`].
//!
//! Layout ([`layout`]): the three containers size themselves like a leaf (props, flex, or their
//! content's natural size) and place their children themselves: a table hides its columns, a
//! tab-view lays out only its selected page under the bar, a split-view its two panes.
//!
//! Writer lane: WIDGETS (WG4a).

pub(crate) mod split;
pub(crate) mod table;
pub(crate) mod tabs;
pub mod toast;

use crate::backend::EguiBackend;
use crate::layout::{Metrics, Tree, ix, tid};
use crate::scene::{Arena, Handle, W};
use crate::walk::Out;
use egui::{Rect, Ui, Vec2};
use svelters_abi::{StrResult, Value};
use taffy::prelude::{AvailableSpace, Layout, Size};
use taffy::{
    LayoutInput, LayoutOutput, LayoutPartialTree, Line, NodeId, Point, RequestedAxis, RunMode, SizingMode, compute_hidden_layout, compute_leaf_layout,
};

/// The views that have children (`W::is_container`).
pub(crate) fn is_container(w: &W) -> bool {
    matches!(w, W::Table(_) | W::TabView { .. } | W::Tab { .. } | W::SplitView { .. })
}

/// The no-wrap size of a view that measures itself (only `table`; the others are sized by their
/// children or take no space).
pub(crate) fn intrinsic(m: &Metrics, w: &W) -> Vec2 {
    match w {
        W::Table(t) => table::intrinsic(m, t),
        _ => Vec2::ZERO,
    }
}

/// Paints the view leaves (`toast-host`; a `table-column` draws nothing) and returns a response
/// at the node's rect (EV2b's host response). `None`: not a view.
pub(crate) fn leaf(arena: &mut Arena, i: usize, ui: &mut Ui, out: &mut Vec<Out>) -> Option<egui::Response> {
    match arena.w(i) {
        W::ToastHost(_) => toast::paint(arena, i, ui, out),
        W::TableColumn { .. } => {}
        _ => return None,
    }
    Some(ui.interact(ui.max_rect(), arena.core[i].egui_id.with("view"), egui::Sense::hover()))
}

/// Paints view container `i` at `r` (screen) with its children (`leaf::container`'s arm).
pub(crate) fn container(arena: &mut Arena, i: usize, r: Rect, origin: Vec2, ui: &mut Ui, out: &mut Vec<Out>) {
    match arena.w(i) {
        W::Table(_) => table::paint(arena, i, r, ui, out),
        W::TabView { .. } => tabs::paint(arena, i, r, origin, ui, out),
        W::SplitView { .. } => split::paint(arena, i, r, origin, ui, out),
        _ => unreachable!("not a view container"),
    }
    let c = arena.common(i);
    if c.invalid {
        crate::leaf::invalid_outline(ui, r);
    }
}

/// The methods of the views (`ops.rs` `call`); `None`: not one of theirs.
pub(crate) fn call(b: &EguiBackend, h: Handle, name: &str, args: &[Value]) -> Option<StrResult<Value>> {
    let (r, root) = {
        let mut scene = b.0.scene.borrow_mut();
        let id = match scene.live(h) {
            Ok(id) => id,
            Err(e) => return Some(Err(e)),
        };
        let i = scene.ix(id);
        let r = match (name, scene.w_mut(i)) {
            ("set-rows", W::Table(t)) => t.set_rows(args),
            ("clear-rows", W::Table(t)) => t.clear_rows(args),
            ("scroll-to-row", W::Table(t)) => t.scroll_to_row(args),
            ("show", W::ToastHost(t)) => t.show(args),
            ("dismiss", W::ToastHost(t)) => t.dismiss(args),
            ("clear", W::ToastHost(t)) => t.clear(args),
            _ => return None,
        };
        (r, scene.core[i].root)
    };
    b.touched(Some(root));
    Some(r)
}

/// A method's argument `k` (`Null` when missing).
pub(crate) fn arg(args: &[Value], k: usize) -> &Value {
    args.get(k).unwrap_or(&Value::Null)
}

/// Checks a method's argument count.
pub(crate) fn arity(name: &str, args: &[Value], min: usize, max: usize) -> StrResult<()> {
    if args.len() < min || args.len() > max {
        let want = if min == max { format!("{min}") } else { format!("{min} to {max}") };
        return Err(format!("{name} takes {want} argument(s), got {}", args.len()));
    }
    Ok(())
}

// ---- layout ----

/// Lays out view container `id`: its own size as a leaf (`natural` measures its content), then,
/// when performing layout, its children (`tabs`, `split` and `table` say where).
pub(crate) fn layout(tree: &mut Tree<'_>, id: NodeId, inputs: LayoutInput) -> LayoutOutput {
    let i = ix(id);
    tree.leaf_layouts += 1;
    let kids: Vec<NodeId> = tree.s.children[i].iter().map(|&c| tid(c)).collect();
    let style = tree.s.layout[i].style.clone();
    // what the view needs, without cloning its payload (a table's holds its data window)
    enum V {
        Table(Vec2),
        Tabs(i64),
        Split { vertical: bool, position: Option<f64>, min_size: f64 },
    }
    let v = match tree.s.w(i) {
        W::Table(t) => V::Table(table::intrinsic(tree.m, t)),
        &W::TabView { selected } => V::Tabs(selected),
        &W::SplitView { vertical, position, min_size, .. } => V::Split { vertical, position, min_size },
        _ => unreachable!("not a view container"),
    };
    let out = compute_leaf_layout(
        inputs,
        &style,
        |_, _| 0.0,
        |known, avail| {
            if let Size { width: Some(w), height: Some(h) } = known {
                return Size { width: w, height: h };
            }
            let natural = match v {
                V::Table(s) => Size { width: s.x, height: s.y },
                V::Tabs(selected) => tabs::natural(tree, &kids, selected, avail),
                V::Split { vertical, .. } => split::natural(tree, &kids, vertical, avail),
            };
            Size { width: known.width.unwrap_or(natural.width.ceil()), height: known.height.unwrap_or(natural.height.ceil()) }
        },
    );
    if inputs.run_mode == RunMode::PerformLayout {
        let size = out.size;
        match v {
            V::Table(_) => kids.iter().for_each(|&k| hide(tree, k)),
            V::Tabs(selected) => tabs::place(tree, &kids, selected, size),
            V::Split { vertical, position, min_size } => split::place(tree, &kids, vertical, position, min_size, size),
        }
    }
    out
}

/// The hidden (zero) layout for `child` and its subtree.
pub(crate) fn hide(tree: &mut Tree<'_>, child: NodeId) {
    compute_hidden_layout(tree, child);
}

/// Lays out `child` at exactly `size`, at `at` in its parent.
pub(crate) fn place_child(tree: &mut Tree<'_>, child: NodeId, order: u32, at: (f32, f32), size: Size<f32>) {
    let (w, h) = (size.width.max(0.0), size.height.max(0.0));
    tree.compute_child_layout(
        child,
        LayoutInput {
            run_mode: RunMode::PerformLayout,
            sizing_mode: SizingMode::InherentSize,
            axis: RequestedAxis::Both,
            known_dimensions: Size { width: Some(w), height: Some(h) },
            known_dimensions_are_definite: Size { width: true, height: true },
            parent_size: Size { width: Some(w), height: Some(h) },
            available_space: Size { width: AvailableSpace::Definite(w), height: AvailableSpace::Definite(h) },
            vertical_margins_are_collapsible: Line::FALSE,
        },
    );
    let l = Layout { order, location: Point { x: at.0, y: at.1 }, size: Size { width: w, height: h }, ..Layout::new() };
    tree.set_unrounded_layout(child, &l);
}

/// The size `child` asks for in `avail`.
pub(crate) fn measure_child(tree: &mut Tree<'_>, child: NodeId, avail: Size<AvailableSpace>) -> Size<f32> {
    tree.compute_child_layout(
        child,
        LayoutInput {
            run_mode: RunMode::ComputeSize,
            sizing_mode: SizingMode::InherentSize,
            axis: RequestedAxis::Both,
            known_dimensions: Size::NONE,
            known_dimensions_are_definite: Size { width: false, height: false },
            parent_size: avail.into_options(),
            available_space: avail,
            vertical_margins_are_collapsible: Line::FALSE,
        },
    )
    .size
}

/// `avail` less `w`, `h` points (a definite space only).
pub(crate) fn shrink(avail: Size<AvailableSpace>, w: f32, h: f32) -> Size<AvailableSpace> {
    let less = |a: AvailableSpace, d: f32| match a {
        AvailableSpace::Definite(x) => AvailableSpace::Definite((x - d).max(0.0)),
        other => other,
    };
    Size { width: less(avail.width, w), height: less(avail.height, h) }
}
