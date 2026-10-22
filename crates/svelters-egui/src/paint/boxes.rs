//! The box primitive: outer shadows, background, inset shadows, border; and the outline.
//!
//! OW0a paints a uniform border (one `RectShape`, `StrokeKind::Inside`, what VIS1 measures) with
//! one radius; per-side borders, per-corner radii and the 2-stop gradient are OW0b.

use super::{PaintCx, StyleView, TraceKind};
use egui::{Rect, Stroke, vec2};

/// The three boxes of a node: border box (the layout rect), padding box (inside the border), and
/// content box (inside the padding).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BoxGeom {
    pub border: Rect,
    pub padding: Rect,
    pub content: Rect,
}

/// The boxes of `border_box` under `s` (no painting).
pub fn box_geom(s: &StyleView, border_box: Rect) -> BoxGeom {
    let [bl, br, bt, bb] = s.sides("border-width");
    let [pl, pr, pt, pb] = s.sides("padding");
    let padding = Rect::from_min_max(border_box.min + vec2(bl, bt), border_box.max - vec2(br, bb));
    let content = Rect::from_min_max(padding.min + vec2(pl, pt), padding.max - vec2(pr, pb));
    BoxGeom { border: border_box, padding, content: if content.is_negative() { Rect::from_center_size(content.center(), egui::Vec2::ZERO) } else { content } }
}

/// Padding plus border around the content: what `measure` adds to the content size (width,
/// height). No egui `Spacing` involved.
pub fn box_extra(s: &StyleView) -> egui::Vec2 {
    let [bl, br, bt, bb] = s.sides("border-width");
    let [pl, pr, pt, pb] = s.sides("padding");
    vec2(bl + br + pl + pr, bt + bb + pt + pb)
}

/// Paints the box of `border_box` under `s`: outer shadows, background, inset shadows, border.
/// Returns its geometry for the content.
pub fn paint_box(cx: &mut PaintCx, s: &StyleView, border_box: Rect) -> BoxGeom {
    let g = box_geom(s, border_box);
    let r = cx.snap_rect(border_box);
    let radius = s.px("border-radius").unwrap_or(0.0).max(0.0);
    let shadows = s.shadows();
    let shadow_color = |sh: &svelters_style::Shadow| match sh.color {
        Some(k) => {
            let [r, g, b, a] = k.rgba8();
            egui::Color32::from_rgba_unmultiplied(r, g, b, a)
        }
        None => s.color("color").unwrap_or(egui::Color32::BLACK),
    };
    if let Some(list) = shadows {
        for sh in crate::style::shadow::shapes(list, r, radius, false, &shadow_color) {
            cx.painter.add(sh);
        }
    }
    let fill = s.color("background-color").unwrap_or(egui::Color32::TRANSPARENT);
    let [bl, br, bt, bb] = s.sides("border-width");
    // OW0a: a uniform border (the widest side; OW0b paints per-side borders)
    let bw = cx.snap_width(bl.max(br).max(bt).max(bb));
    let bc = s.color("border-top-color").or_else(|| s.color("border-color")).unwrap_or(egui::Color32::TRANSPARENT);
    let stroke = if bw > 0.0 && bc.a() > 0 { Stroke::new(bw, bc) } else { Stroke::NONE };
    if fill.a() > 0 || stroke.width > 0.0 {
        cx.painter.add(egui::epaint::RectShape::new(r, radius, fill, stroke, egui::StrokeKind::Inside));
    }
    cx.record(TraceKind::Box, r, fill, stroke, 0.0);
    if let Some(list) = shadows {
        let inset = crate::style::shadow::shapes(list, r, radius, true, &shadow_color);
        if !inset.is_empty() {
            let p = cx.painter.with_clip_rect(cx.painter.clip_rect().intersect(r));
            for sh in inset {
                p.add(sh);
            }
        }
    }
    g
}

/// Paints the outline of `border_box` under `s` (`outline-width`, `outline-color`,
/// `outline-style`, `outline-offset`): outside the border box by the offset, never affecting layout
/// or hit-testing. The focus ring is a `:focus-visible` outline (DESIGN R8).
pub fn paint_outline(cx: &mut PaintCx, s: &StyleView, border_box: Rect) {
    if matches!(s.keyword("outline-style"), None | Some("none" | "hidden")) {
        return;
    }
    let w = cx.snap_width(s.px("outline-width").unwrap_or(0.0));
    let Some(col) = s.color("outline-color") else { return };
    if w <= 0.0 || col.a() == 0 {
        return;
    }
    let off = s.px("outline-offset").unwrap_or(0.0);
    let r = cx.snap_rect(border_box.expand(off));
    let radius = (s.px("border-radius").unwrap_or(0.0) + off.max(0.0)).max(0.0);
    let stroke = Stroke::new(w, col);
    cx.painter.add(egui::epaint::RectShape::new(r, radius, egui::Color32::TRANSPARENT, stroke, egui::StrokeKind::Outside));
    cx.record(TraceKind::Outline, r, egui::Color32::TRANSPARENT, stroke, 0.0);
}
