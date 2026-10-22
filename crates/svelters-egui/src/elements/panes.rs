//! The containers `panel`, `sides`, `frame` and `stack`: their taffy style ([`style`]), the stack's
//! layout ([`stack_layout`], `layout.rs` dispatches to it) and their chrome ([`paint`]).
//!
//! `frame`'s chrome is padding: the egui frame's margins (inner margin, stroke, outer margin) and a
//! caption row are added to its `padding`, taken from egui's default style so layout does not
//! depend on the live theme; paint draws the live theme's frame in the same rect.

use super::changed;
use crate::kinds::P;
use crate::scene::{Arena, Common, Scene, W};
use crate::walk::Out;
use egui::{Rect, Sense, Ui, Vec2, pos2, vec2};
use std::sync::OnceLock;
use svelters_abi::Value;
use taffy::prelude::{AvailableSpace, Dimension, FlexDirection, LengthPercentage, Size};
use taffy::{AlignContent, CacheTree, Layout, LayoutInput, LayoutOutput, LayoutPartialTree, Line, NodeId, Point, RequestedAxis, RunMode, SizingMode, Style};

fn default_style() -> &'static egui::Style {
    static S: OnceLock<egui::Style> = OnceLock::new();
    S.get_or_init(egui::Style::default)
}

/// The egui frame of a `frame-style` (`None`: no chrome).
pub(crate) fn frame_of(name: &str, style: &egui::Style) -> Option<egui::Frame> {
    Some(match name {
        "canvas" => egui::Frame::canvas(style),
        "window" => egui::Frame::window(style),
        "menu" => egui::Frame::menu(style),
        "popup" => egui::Frame::popup(style),
        "panel" => egui::Frame::side_top_panel(style),
        "none" => return None,
        _ => egui::Frame::group(style),
    })
}

/// The height of a frame's caption row (egui's default `interact_size.y`).
fn caption_row() -> f32 {
    default_style().spacing.interact_size.y
}

/// The kind-specific style of WG2b's containers.
pub(crate) fn style(c: &Common, w: &W, s: &mut Style) {
    match w {
        W::Panel { side, size: Some(size), .. } => {
            let d = Dimension::length(*size as f32);
            if matches!(*side, "top" | "bottom") {
                s.size.height = d;
            } else {
                s.size.width = d;
            }
            // a sized panel keeps its size: the rest of the row or column gives way
            s.flex_shrink = 0.0;
        }
        W::Sides => {
            s.flex_direction = FlexDirection::Row;
            s.justify_content = AlignContent::SPACE_BETWEEN;
        }
        W::Frame { label, style, .. } => {
            let m = frame_of(style, default_style()).map(|f| f.total_margin()).unwrap_or_default();
            let pad = c.padding as f32;
            let top = if label.is_empty() { 0.0 } else { caption_row() };
            s.padding = taffy::Rect {
                left: LengthPercentage::length(pad + m.left),
                right: LengthPercentage::length(pad + m.right),
                top: LengthPercentage::length(pad + m.top + top),
                bottom: LengthPercentage::length(pad + m.bottom),
            };
        }
        _ => {}
    }
}

/// What lays out `stack` node `i`: the child it shows (an index into its children), its
/// `homogeneous` and its padding.
pub(crate) struct StackPick {
    pub visible: Option<usize>,
    pub homogeneous: bool,
    pub padding: f32,
}

pub(crate) fn stack_pick(s: &Scene, i: usize) -> StackPick {
    let W::Stack { name, index, homogeneous } = s.w(i) else { return StackPick { visible: None, homogeneous: false, padding: 0.0 } };
    let kids = &s.children[i];
    let by_name = (!name.is_empty()).then(|| kids.iter().position(|&c| s.common(s.ix(c)).anchor == *name)).flatten();
    let visible = by_name.or_else(|| usize::try_from(*index).ok().filter(|k| *k < kids.len()));
    StackPick { visible, homogeneous: *homogeneous, padding: s.common(i).padding as f32 }
}

/// Lays out a `stack`: sized like a leaf whose content is its visible child (or, homogeneous, the
/// largest child), with the visible child filling its content box and the others hidden.
pub(crate) fn stack_layout<T: LayoutPartialTree + CacheTree>(tree: &mut T, id: NodeId, inputs: LayoutInput, style: &Style, pick: StackPick) -> LayoutOutput {
    let StackPick { visible, homogeneous, padding: pad } = pick;
    let n = tree.child_count(id);
    let measured: Vec<usize> = if homogeneous { (0..n).collect() } else { visible.into_iter().collect() };
    let out = taffy::compute_leaf_layout(inputs, style, |_, _| 0.0, |known, avail| {
        let mut size = Size { width: 0.0f32, height: 0.0f32 };
        for &k in &measured {
            let c = tree.get_child_id(id, k);
            let o = tree.compute_child_layout(
                c,
                LayoutInput {
                    run_mode: RunMode::ComputeSize,
                    sizing_mode: SizingMode::InherentSize,
                    axis: RequestedAxis::Both,
                    known_dimensions: known,
                    known_dimensions_are_definite: Size { width: known.width.is_some(), height: known.height.is_some() },
                    parent_size: known,
                    available_space: avail,
                    vertical_margins_are_collapsible: Line::FALSE,
                },
            );
            size.width = size.width.max(o.size.width);
            size.height = size.height.max(o.size.height);
        }
        Size { width: known.width.unwrap_or(size.width), height: known.height.unwrap_or(size.height) }
    });
    if inputs.run_mode == RunMode::PerformLayout {
        let inner = Size { width: (out.size.width - 2.0 * pad).max(0.0), height: (out.size.height - 2.0 * pad).max(0.0) };
        for k in 0..n {
            let c = tree.get_child_id(id, k);
            if Some(k) != visible {
                taffy::compute_hidden_layout(tree, c);
                continue;
            }
            let o = tree.compute_child_layout(
                c,
                LayoutInput {
                    run_mode: RunMode::PerformLayout,
                    sizing_mode: SizingMode::InherentSize,
                    axis: RequestedAxis::Both,
                    known_dimensions: Size { width: Some(inner.width), height: Some(inner.height) },
                    known_dimensions_are_definite: Size { width: true, height: true },
                    parent_size: Size { width: Some(inner.width), height: Some(inner.height) },
                    available_space: Size { width: AvailableSpace::Definite(inner.width), height: AvailableSpace::Definite(inner.height) },
                    vertical_margins_are_collapsible: Line::FALSE,
                },
            );
            tree.set_unrounded_layout(c, &Layout { location: Point { x: pad, y: pad }, size: o.size, ..Layout::with_order(k as u32) });
        }
    }
    out
}

/// Paints container `i` (`r`: its rect on screen): chrome, tooltip, `invalid` outline, children.
pub(crate) fn paint(arena: &mut Arena, i: usize, r: Rect, origin: Vec2, ui: &mut Ui, out: &mut Vec<Out>) {
    let c = arena.common(i);
    let (tip, invalid) = (c.tooltip.clone(), c.invalid);
    let id = arena.core[i].egui_id;
    match arena.w(i) {
        W::Panel { .. } => {
            ui.painter().rect_filled(r, 0.0, ui.visuals().panel_fill);
        }
        W::Frame { label, style } => {
            if let Some(f) = frame_of(style, ui.style()) {
                // the frame fills the rect less its outer margin (laid out with the default margins)
                let outer = r - frame_of(style, default_style()).map(|f| f.outer_margin).unwrap_or_default();
                let mut shape_frame = f;
                shape_frame.inner_margin = egui::Margin::ZERO;
                shape_frame.outer_margin = egui::Margin::ZERO;
                let content = outer.shrink(f.stroke.width);
                ui.painter().add(shape_frame.paint(content));
            }
            if !label.is_empty() {
                let m = frame_of(style, default_style()).map(|f| f.total_margin()).unwrap_or_default();
                let pad = arena.common(i).padding as f32;
                let galley = egui::WidgetText::from(egui::RichText::new(label.as_str()).strong()).into_galley(ui, Some(egui::TextWrapMode::Truncate), (r.width() - m.sum().x - 2.0 * pad).max(0.0), egui::TextStyle::Body);
                let top = r.top() + m.top + pad;
                let pos = pos2(r.left() + m.left + pad, top + (caption_row() - galley.size().y) / 2.0);
                ui.painter().galley(pos, galley, ui.visuals().strong_text_color());
            }
        }
        _ => {}
    }
    if !tip.is_empty() {
        ui.interact(r, id.with("tip"), Sense::hover()).on_hover_text(tip);
    }
    if let W::Stack { .. } = arena.w(i) {
        if let Some(k) = stack_pick(arena, i).visible {
            let c = arena.children[i][k].index() as usize;
            crate::walk::paint_at(arena, c, origin, ui, out);
        }
    } else {
        crate::walk::paint_children(arena, i, origin, ui, out);
    }
    if matches!(arena.w(i), W::Panel { .. }) {
        panel_edge(arena, i, r, ui, out);
    }
    if invalid {
        crate::leaf::invalid_outline(ui, r);
    }
    arena.core[i].drawn = r;
}

/// A panel's inner edge: the separator line and, when `resizable`, the grip that drags `size`.
fn panel_edge(arena: &mut Arena, i: usize, r: Rect, ui: &mut Ui, out: &mut Vec<Out>) {
    let W::Panel { side, size, resizable, separator } = arena.w(i) else { return };
    let (side, size, resizable, separator) = (*side, *size, *resizable, *separator);
    let horizontal = matches!(side, "left" | "right");
    // the inner edge: a vertical line for a side panel, a horizontal one for a top/bottom panel
    let (a, b) = match side {
        "right" => (r.left_top(), r.left_bottom()),
        "top" => (r.left_bottom(), r.right_bottom()),
        "bottom" => (r.left_top(), r.right_top()),
        _ => (r.right_top(), r.right_bottom()),
    };
    let mut active = false;
    if resizable && arena.common(i).enabled {
        let grab = ui.style().interaction.resize_grab_radius_side;
        let strip = Rect::from_two_pos(a, b).expand2(if horizontal { vec2(grab, 0.0) } else { vec2(0.0, grab) });
        let id = arena.core[i].egui_id.with("resize");
        let resp = ui.interact(strip, id, Sense::drag());
        active = resp.hovered() || resp.dragged();
        if active {
            ui.ctx().set_cursor_icon(if horizontal { egui::CursorIcon::ResizeHorizontal } else { egui::CursorIcon::ResizeVertical });
        }
        if resp.dragged()
            && let Some(p) = resp.interact_pointer_pos()
        {
            let c = arena.common(i);
            let (lo, hi) = if horizontal { (c.min_width, c.max_width) } else { (c.min_height, c.max_height) };
            let raw = match side {
                "right" => r.right() - p.x,
                "top" => p.y - r.top(),
                "bottom" => r.bottom() - p.y,
                _ => p.x - r.left(),
            } as f64;
            let new = raw.max(lo.unwrap_or(0.0)).min(hi.unwrap_or(f64::INFINITY)).max(0.0).round();
            let current = size.unwrap_or(if horizontal { r.width() } else { r.height() } as f64);
            if new != current {
                if let W::Panel { size, .. } = arena.w_mut(i) {
                    *size = Some(new);
                }
                // its own size changed: restyle, relayout from the parent next frame
                let node = arena.slots.id_at(i).expect("live node");
                arena.restyle(i);
                arena.invalidate_style(node);
                ui.ctx().request_repaint();
                changed(arena, i, &[P::Size], vec![Value::F64(new)], out);
            }
        }
    }
    if separator || active {
        let stroke = if active { ui.visuals().widgets.hovered.fg_stroke } else { ui.visuals().widgets.noninteractive.bg_stroke };
        ui.painter().line_segment([a, b], stroke);
    }
}
