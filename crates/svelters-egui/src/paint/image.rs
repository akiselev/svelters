//! The image primitive: a texture fitted into a rect (`object-fit`: `contain`, `cover`, `fill`,
//! `none`), with `opacity`. Loading stays in `elements/media.rs`; this only paints.

use super::{PaintCx, StyleView, TraceKind};
use egui::{Rect, TextureId, Vec2, pos2};

/// Paints texture `tex` of natural `size` in `rect` under `s`.
pub fn paint_image(cx: &mut PaintCx, s: &StyleView, tex: TextureId, size: Vec2, rect: Rect) {
    if size.x <= 0.0 || size.y <= 0.0 || !rect.is_positive() {
        return;
    }
    let fit = s.keyword("object-fit").unwrap_or("contain");
    let (sx, sy) = (rect.width() / size.x, rect.height() / size.y);
    let shown = match fit {
        "fill" => rect.size(),
        "none" => size,
        "cover" => size * sx.max(sy),
        _ => size * sx.min(sy),
    };
    let r = Rect::from_center_size(rect.center(), shown);
    let alpha = s.number("opacity").unwrap_or(1.0).clamp(0.0, 1.0);
    let tint = egui::Color32::WHITE.gamma_multiply(alpha);
    let p = cx.painter.with_clip_rect(cx.painter.clip_rect().intersect(rect));
    p.image(tex, r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), tint);
    cx.record(TraceKind::Image, r, tint, egui::Stroke::NONE, 0.0);
}
