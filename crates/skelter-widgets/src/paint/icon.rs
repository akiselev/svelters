//! The icon primitive. A key of the default family (`icons`) that the icon registry has draws from it
//! (an SVG rasterised for its optical size, tinted by `color`); anything else is a glyph of an icon
//! font (the family named in the key), tinted by `color`, sized by `font-size`; an icon that cannot be
//! drawn is an outlined square (never a missing-glyph box). The registry, its rasters and the icon
//! font families are [`registry`]'s.

use super::{PaintCx, StyleView, text_style};
use crate::shim::text::{RichText, TextShaper};
use skelter_base::color::{Color, Fill};
use skelter_base::display::{BorderStyle, Prim, Quad};
use skelter_base::geom::{Corners, Insets, Point, Rect, Size, Vec2};
use std::sync::Arc;

/// An icon by family and name. In the default family ([`IconKey::REGISTRY_FAMILY`], `icons`) the name
/// is first looked up as an icon registry key (`file.save`, `check`), then as a glyph name of the
/// `icons` font; another family (`codicons:check`) is a font glyph only.
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

    /// A kind's `icon` prop: `name` in the `icons` family, or `family:name`; `None` when empty.
    pub fn parse(prop: &str) -> Option<IconKey> {
        (!prop.is_empty()).then(|| match prop.split_once(':') {
            Some((f, n)) => IconKey::new(f, n),
            None => IconKey::key(prop),
        })
    }
}

/// The argument of `<name>(<arg>)` in a CSS value (`icon(check)` → `check`).
fn css_call<'a>(v: &'a str, name: &str) -> Option<&'a str> {
    let inner = v.trim().strip_prefix(name)?.trim_start().strip_prefix('(')?.strip_suffix(')')?;
    Some(inner.trim().trim_matches(['"', '\'']))
}

/// The icon a glyph part shows: `content: icon(<key>)` (`family:name` or a name in `default`'s
/// family) when its style sets one, else `default`. `None` for `content: none`.
pub fn content_icon(s: &StyleView, default: &IconKey) -> Option<IconKey> {
    let key = |k: &str| match k.split_once(':') {
        Some((f, n)) => IconKey::new(f, n),
        None => IconKey::new(default.family.clone(), k),
    };
    if let Some(e) = s.engine() {
        match e.get("content") {
            Some(svelters_style::ComputedValue::Content(c)) => {
                return match c {
                    svelters_style::Content::None => None,
                    svelters_style::Content::Icon(k) => Some(key(k)),
                    // url(): images are the image primitive's
                    _ => Some(default.clone()),
                };
            }
            Some(_) => return Some(default.clone()),
            None => {}
        }
    }
    match s.keyword("content") {
        Some("none") => None,
        Some(v) => Some(css_call(v, "icon").map(key).unwrap_or_else(|| default.clone())),
        None => Some(default.clone()),
    }
}

/// The side of a registry icon's box: `width`/`height` when the part sets them, else `font-size`.
pub fn icon_side(s: &StyleView) -> f32 {
    match (s.px("width"), s.px("height")) {
        (Some(w), Some(h)) => w.min(h),
        (Some(v), None) | (None, Some(v)) => v,
        (None, None) => s.font_size(),
    }
}

/// The glyph text of an icon-font icon: the name from the family's table, else the name itself
/// (icon fonts without a table map names to glyphs through ligatures; skelter-text resolves the
/// family). `None` when the family is not a registered icon font or the name is not in its table.
fn glyph_text(key: &IconKey) -> Option<RichText> {
    if key.family.is_empty() || key.name.is_empty() || !registry::has_font(&key.family) {
        return None;
    }
    let text = registry::icon_text(&key.family, &key.name, "")?;
    Some(RichText::plain(&text, Default::default()))
}

/// The icon's box: a registry icon's square (or its glyph), else its font glyph's size at
/// `font-size`, else a square of that side when it cannot be drawn.
pub fn icon_size(shaper: &mut dyn TextShaper, s: &StyleView, key: &IconKey) -> Vec2 {
    icon_size_sized(shaper, s, key, None)
}

/// [`icon_size`] with a registry icon's side given (a kind's `size` prop) instead of the style's
/// `width`/`height`/`font-size`.
pub fn icon_size_sized(shaper: &mut dyn TextShaper, s: &StyleView, key: &IconKey, side: Option<f32>) -> Vec2 {
    if let Some(reg) = registry::registered(key)
        && let Some(v) = registry::size(shaper, &reg, &key.name, side.unwrap_or_else(|| icon_side(s)))
    {
        return v;
    }
    if let Some(mut t) = glyph_text(key) {
        t.base = text_style(s);
        t.base.family = Arc::from(key.family.as_str());
        let l = shaper.layout(&t, None);
        return Vec2::new(l.size().w, l.size().h);
    }
    Vec2::new(s.font_size(), s.font_size())
}

/// Whether `key` draws from the global icon registry (the default family, a registered key).
pub fn is_registered(key: &IconKey) -> bool {
    registry::registered(key).is_some()
}

/// Paints icon `key` centred in `rect`, tinted by `color`; an outlined square when it cannot be
/// drawn. A glyph part's `content` overrides `key` (`content: icon(check)`, `content: none`).
pub fn paint_icon(cx: &mut PaintCx, s: &StyleView, key: &IconKey, rect: Rect) {
    paint_icon_sized(cx, s, key, rect, None);
}

/// [`paint_icon`] with a registry icon's side given (a kind's `size` prop) instead of the style's
/// `width`/`height`/`font-size`.
pub fn paint_icon_sized(cx: &mut PaintCx, s: &StyleView, key: &IconKey, rect: Rect, side: Option<f32>) {
    let Some(key) = content_icon(s, key) else { return };
    let color = s.color("color").unwrap_or(Color::BLACK);
    let center = Point::new((rect.min.x + rect.max.x) / 2.0, (rect.min.y + rect.max.y) / 2.0);
    if let Some(reg) = registry::registered(&key) {
        let style = registry::IconStyle { size: side.unwrap_or_else(|| icon_side(s)), color, slots: registry::Slots::of(s), high_contrast: false };
        if registry::paint(cx, &reg, &key.name, rect, &style).is_some() {
            return;
        }
    }
    if let Some(mut t) = glyph_text(&key) {
        t.base = text_style(s);
        t.base.family = Arc::from(key.family.as_str());
        let l = cx.text.layout(&t, None);
        let pos = cx.snap_point(Point::new(center.x - l.size().w / 2.0, center.y - l.size().h / 2.0));
        super::paint_runs(cx, &l, pos, &|_| color);
        return;
    }
    paint_missing(cx, color, s.font_size().min(rect.width()).min(rect.height()), center);
}

/// The outlined square of side `side` centred at `center` that stands for an icon that cannot be
/// drawn (never a missing-glyph box).
pub fn paint_missing(cx: &mut PaintCx, color: Color, side: f32, center: Point) {
    let r = cx.snap_rect(Rect::from_origin_size(Point::new(center.x - side / 2.0, center.y - side / 2.0), Size::new(side, side)));
    let w = cx.snap_width(1.0);
    cx.push(Prim::Quad(Quad { rect: r, radii: Corners::all(1.0), fill: Fill::Solid(Color::TRANSPARENT), border_widths: Insets::all(w), border_color: color, border_style: BorderStyle::Solid }), r);
}

pub mod registry;
pub mod vector;
