//! OW3: the glyph parts of menus and selects (`check`, `chevron`): `content: icon(<key>)` when CSS
//! sets one (WIDGET-PARITY P2), else a path sized from the part's box, because svelters-egui
//! bundles no icon font until ICR1 (as OW1a's check mark). Writer lane: OW3.

use crate::paint::{self, IconKey, PaintCx, StyleView, TraceKind};
use egui::{Rect, Stroke, pos2};

/// Which glyph.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Glyph {
    Check,
    /// A dot (a checked radio action when CSS asks for it; unused by default: VS Code shows a
    /// check for both).
    Dot,
    ChevronRight,
    ChevronDown,
}

impl Glyph {
    fn codicon(self) -> &'static str {
        match self {
            Glyph::Check => "check",
            Glyph::Dot => "circle-small-filled",
            Glyph::ChevronRight => "chevron-right",
            Glyph::ChevronDown => "chevron-down",
        }
    }
}

/// Paints `g` centred in `r` in the part's `color`, at `font-size` (the glyph box): CSS
/// `content: icon(…)`/`paint(…)`/`none` first.
pub(crate) fn paint(cx: &mut PaintCx<'_>, s: &StyleView<'_>, g: Glyph, r: Rect) {
    match s.keyword("content") {
        Some("none") => return,
        Some(_) => return paint::paint_icon(cx, s, &IconKey::new("icons", g.codicon()), r),
        None => {}
    }
    let color = s.color("color").unwrap_or(egui::Color32::PLACEHOLDER);
    let side = s.font_size().min(r.width()).min(r.height());
    let b = Rect::from_center_size(r.center(), egui::Vec2::splat(side)).shrink(side * 0.2);
    let stroke = Stroke::new(cx.snap_width((side / 12.0).max(1.0)), color);
    match g {
        Glyph::Check => {
            let pts = vec![pos2(b.left(), b.center().y), pos2(b.left() + b.width() * 0.38, b.bottom() - b.height() * 0.1), pos2(b.right(), b.top() + b.height() * 0.1)];
            cx.painter.add(egui::Shape::line(pts, stroke));
        }
        Glyph::Dot => {
            cx.painter.circle_filled(b.center(), b.width() * 0.25, color);
        }
        Glyph::ChevronRight => {
            let w = b.width() * 0.25;
            let pts = vec![pos2(b.center().x - w, b.top()), pos2(b.center().x + w, b.center().y), pos2(b.center().x - w, b.bottom())];
            cx.painter.add(egui::Shape::line(pts, stroke));
        }
        Glyph::ChevronDown => {
            let h = b.height() * 0.25;
            let pts = vec![pos2(b.left(), b.center().y - h), pos2(b.center().x, b.center().y + h), pos2(b.right(), b.center().y - h)];
            cx.painter.add(egui::Shape::line(pts, stroke));
        }
    }
    cx.record(TraceKind::Path, b, color, stroke, side);
}
