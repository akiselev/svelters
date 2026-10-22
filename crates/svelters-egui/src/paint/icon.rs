//! The icon primitive. OW0a: a glyph of a registered icon font (`fonts::register_icons`, the
//! codicon path `icon` uses today), tinted by `color`, sized by `font-size` (or the box). ICR1
//! swaps the source for `IconRegistry::resolve(key, physical_px)` (glyph or texture).

use super::{PaintCx, StyleView, TraceKind};
use egui::{FontFamily, FontId, Rect};

/// An icon by family and name (`codicons`, `check`): what `IconRegistry` keys will replace.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct IconKey {
    pub family: String,
    pub name: String,
}

impl IconKey {
    pub fn new(family: impl Into<String>, name: impl Into<String>) -> IconKey {
        IconKey { family: family.into(), name: name.into() }
    }
}

fn glyph(ctx: &egui::Context, s: &StyleView, key: &IconKey) -> Option<(FontId, String)> {
    let text = crate::fonts::icon_text(&key.family, &key.name, "")?;
    let family = if key.family.is_empty() { FontFamily::Proportional } else { FontFamily::Name(key.family.as_str().into()) };
    if !crate::fonts::has_family(ctx, &family) {
        return None;
    }
    Some((FontId::new(s.font_size(), family), text))
}

/// The icon's box: its glyph's size at `font-size`, or a square of that side when it cannot be
/// drawn.
pub fn icon_size(ctx: &egui::Context, s: &StyleView, key: &IconKey) -> egui::Vec2 {
    match glyph(ctx, s, key) {
        Some((f, t)) => ctx.fonts_mut(|fs| fs.layout_no_wrap(t, f, egui::Color32::WHITE).size()),
        None => egui::Vec2::splat(s.font_size()),
    }
}

/// The icon a glyph part shows: `content: icon(<key>)` (WIDGET-PARITY P2, `family:name` or a
/// name in `default`'s family) when its style sets one, else `default`. `None` for
/// `content: none`.
pub fn content_icon(s: &StyleView, default: &IconKey) -> Option<IconKey> {
    match s.keyword("content") {
        Some("none") => None,
        Some(v) => match super::painters::css_call(v, "icon").and_then(|a| a.first().map(|k| k.trim_matches(['"', '\'']).to_string())) {
            Some(k) => Some(match k.split_once(':') {
                Some((f, n)) => IconKey::new(f, n),
                None => IconKey::new(default.family.clone(), k),
            }),
            None => Some(default.clone()),
        },
        None => Some(default.clone()),
    }
}

/// Paints icon `key` centred in `rect`, tinted by `color`; an outlined square when it cannot be
/// drawn (never egui's missing-glyph box). A glyph part's `content` overrides `key` (P2:
/// `content: icon(check)`, `content: none`, and `content: paint(name)` for a part painter).
pub fn paint_icon(cx: &mut PaintCx, s: &StyleView, key: &IconKey, rect: Rect) {
    if let Some((_, p)) = super::painters::selected(s, "content") {
        cx.run_painter(&*p, s, rect);
        return;
    }
    let Some(key) = content_icon(s, key) else { return };
    let key = &key;
    let color = s.color("color").unwrap_or(egui::Color32::PLACEHOLDER);
    let ctx = cx.painter.ctx().clone();
    match glyph(&ctx, s, key) {
        Some((f, t)) => {
            let g = ctx.fonts_mut(|fs| fs.layout_no_wrap(t, f, color));
            let pos = rect.center() - g.size() / 2.0;
            let r = Rect::from_min_size(pos, g.size());
            cx.painter.galley(pos, g, color);
            cx.record(TraceKind::Icon, r, color, egui::Stroke::NONE, s.font_size());
        }
        None => {
            let side = s.font_size().min(rect.width()).min(rect.height());
            let r = cx.snap_rect(Rect::from_center_size(rect.center(), egui::Vec2::splat(side)));
            let stroke = egui::Stroke::new(cx.snap_width(1.0), color);
            cx.painter.rect_stroke(r, 1.0, stroke, egui::StrokeKind::Inside);
            cx.record(TraceKind::Icon, r, egui::Color32::TRANSPARENT, stroke, s.font_size());
        }
    }
}
