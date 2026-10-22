//! The icon primitive. ICR1: a key of the default family (`icons`) that the context's
//! [`IconRegistry`](crate::icons::IconRegistry) has draws from it (`crate::icons::paint`: whole-pixel
//! box, optical-size master, `color` tint, the `--icon-subject/-accent/-badge` slots); anything else
//! is a glyph of a registered icon font (`fonts::register_icons`, OW0a's codicon path), tinted by
//! `color`, sized by `font-size` (or the box).

use super::{PaintCx, StyleView, TraceKind};
use egui::{FontFamily, FontId, Rect};

/// An icon by family and name. In the default family ([`IconKey::REGISTRY_FAMILY`], `icons`) the
/// name is first looked up as an [`IconRegistry`](crate::icons::IconRegistry) key (`file.save`,
/// `check`), then as a glyph name of the `icons` font; another family (`codicons:check`) is a font
/// glyph only.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct IconKey {
    pub family: String,
    pub name: String,
}

impl IconKey {
    /// The family whose names are registry keys first.
    pub const REGISTRY_FAMILY: &'static str = "icons";

    /// A registry key (`IconKey::key("file.save")`), in the default family.
    pub fn key(name: impl Into<String>) -> IconKey {
        IconKey::new(Self::REGISTRY_FAMILY, name)
    }

    /// The registry key this names, if it is in the default family.
    pub fn registry_key(&self) -> Option<&str> {
        (self.family == Self::REGISTRY_FAMILY).then_some(self.name.as_str())
    }

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

/// The registry entry `key` names in `ctx`'s registry, if any.
fn registered(ctx: &egui::Context, key: &IconKey) -> Option<std::sync::Arc<crate::icons::IconRegistry>> {
    let k = key.registry_key()?;
    let reg = crate::icons::registry(ctx);
    reg.contains(k).then_some(reg)
}

/// The box side of a registry icon: `width`/`height` when the part sets them, else `font-size`.
fn icon_side(s: &StyleView) -> f32 {
    match (s.px("width"), s.px("height")) {
        (Some(w), Some(h)) => w.min(h),
        (Some(v), None) | (None, Some(v)) => v,
        (None, None) => s.font_size(),
    }
}

/// The colour slots a style sets (`--icon-subject`, `--icon-accent`, `--icon-badge`; inherited
/// custom properties).
pub fn icon_slots(s: &StyleView) -> crate::icons::Slots {
    let c = s.computed();
    let get = |n: &str| c.and_then(|c| c.custom(n));
    crate::icons::Slots::from_css(get("--icon-subject"), get("--icon-accent"), get("--icon-badge"))
}

/// The icon's box: a registry icon's square (or its glyph), else its font glyph's size at
/// `font-size`, else a square of that side when it cannot be drawn.
pub fn icon_size(ctx: &egui::Context, s: &StyleView, key: &IconKey) -> egui::Vec2 {
    if let Some(reg) = registered(ctx, key)
        && let Some(v) = crate::icons::size(ctx, &reg, &key.name, icon_side(s))
    {
        return v;
    }
    match glyph(ctx, s, key) {
        Some((f, t)) => ctx.fonts_mut(|fs| fs.layout_no_wrap(t, f, egui::Color32::WHITE).size()),
        None => egui::Vec2::splat(s.font_size()),
    }
}

/// The icon a glyph part shows: `content: icon(<key>)` (WIDGET-PARITY P2, `family:name` or a
/// name in `default`'s family) when its style sets one, else `default`. `None` for
/// `content: none`.
pub fn content_icon(s: &StyleView, default: &IconKey) -> Option<IconKey> {
    let key = |k: &str| match k.split_once(':') {
        Some((f, n)) => IconKey::new(f, n),
        None => IconKey::new(default.family.clone(), k),
    };
    // the engine's typed value (OW-CSS P2); a UA table's keyword text below
    match s.css("content") {
        Some(svelters_style::ComputedValue::Content(c)) => {
            return match c {
                svelters_style::Content::None => None,
                svelters_style::Content::Icon(k) => Some(key(k)),
                // url(): images are the image primitive's; paint(): the painter ran instead
                _ => Some(default.clone()),
            };
        }
        Some(_) => return Some(default.clone()),
        None => {}
    }
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
    if let Some(reg) = registered(&ctx, key) {
        let style = crate::icons::IconStyle { size: icon_side(s), color, slots: icon_slots(s), high_contrast: false };
        if let Some(p) = crate::icons::paint(cx.painter, &reg, &key.name, rect, &style) {
            cx.record(TraceKind::Icon, p.rect, p.tint, egui::Stroke::NONE, style.size);
            return;
        }
    }
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
