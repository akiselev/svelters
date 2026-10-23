//! OW6: the picker glyphs (`prev`/`next` chevrons, the drop button's chevron, R14):
//! `content: icon(<key>)` when CSS sets one (WIDGET-PARITY P2), else a path sized from the
//! part's box, as OW3's `glyph.rs` (no icon font until ICR1). Writer lane: OW6.

use crate::paint::{self, IconKey, PaintCx, StyleView, TraceKind};
use egui::{Rect, Stroke, pos2};

/// Which way a chevron points.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Dir {
    Left,
    Right,
    Down,
}

/// Paints a chevron centred in `r` in the part's `color`, at its `font-size`.
pub(crate) fn chevron(cx: &mut PaintCx<'_>, s: &StyleView<'_>, d: Dir, r: Rect) {
    let key = match d {
        Dir::Left => "chevron-left",
        Dir::Right => "chevron-right",
        Dir::Down => "chevron-down",
    };
    match s.keyword("content") {
        Some("none") => return,
        Some(_) => return paint::paint_icon(cx, s, &IconKey::new("icons", key), r),
        None => {}
    }
    let color = s.color("color").unwrap_or(egui::Color32::PLACEHOLDER);
    let side = s.font_size().min(r.width()).min(r.height());
    let b = Rect::from_center_size(r.center(), egui::Vec2::splat(side)).shrink(side * 0.2);
    let stroke = Stroke::new(cx.snap_width((side / 12.0).max(1.0)), color);
    let (c, w) = (b.center(), b.width() * 0.25);
    let pts = match d {
        Dir::Left => vec![pos2(c.x + w, b.top()), pos2(c.x - w, c.y), pos2(c.x + w, b.bottom())],
        Dir::Right => vec![pos2(c.x - w, b.top()), pos2(c.x + w, c.y), pos2(c.x - w, b.bottom())],
        Dir::Down => vec![pos2(b.left(), c.y - w), pos2(c.x, c.y + w), pos2(b.right(), c.y - w)],
    };
    cx.painter.add(egui::Shape::line(pts, stroke));
    cx.record(TraceKind::Path, b, color, stroke, side);
}

/// A part's `gap` (between the pickers' rows and items), else `default`.
pub(crate) fn gap(s: &StyleView<'_>, default: f32) -> f32 {
    s.px("gap").or_else(|| s.px("column-gap")).unwrap_or(default)
}
