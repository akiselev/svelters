//! The text primitive: a galley laid out from computed style, painted in a content box.
//!
//! OW0a: one font (the proportional family at the style's size and weight), colour, alignment,
//! optional wrapping. Elision (`end`/`middle`/`start`), `letter-spacing`, `text-transform`,
//! decorations and the `::selection` highlight are OW0b/OW-CSS.

use super::{PaintCx, StyleView, TraceKind};
use egui::{Color32, FontFamily, FontId, Rect, Vec2, pos2};
use std::sync::Arc;

/// Horizontal placement of a galley in its box (`text-align`; `start` in LTR).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Align {
    #[default]
    Start,
    Center,
    End,
}

impl Align {
    /// `text-align` of `s`, else `default`.
    pub fn of(s: &StyleView, default: Align) -> Align {
        match s.keyword("text-align") {
            Some("center") => Align::Center,
            Some("end" | "right") => Align::End,
            Some("start" | "left") => Align::Start,
            _ => default,
        }
    }
}

/// The font of `s` for `text` (the weight's registered family when it has the glyphs).
fn font(ctx: &egui::Context, s: &StyleView, text: &str) -> FontId {
    let base = FontFamily::Proportional;
    let w = s.font_weight();
    let family = if w == 400 {
        base
    } else {
        let f = crate::fonts::family_for(ctx, &base, w);
        let ok = f == base || ctx.fonts_mut(|fs| text.chars().filter(|c| !c.is_control()).all(|c| fs.has_glyph(&FontId::new(14.0, f.clone()), c)));
        if ok { f } else { base }
    };
    FontId::new(s.font_size(), family)
}

/// `text` laid out under `s` (font, size, weight, colour), wrapped at `wrap` points if given.
/// egui's galley cache keeps it while it is used every frame.
pub fn text_galley(ctx: &egui::Context, s: &StyleView, text: &str, wrap: Option<f32>) -> Arc<egui::Galley> {
    let f = font(ctx, s, text);
    let color = s.color("color").unwrap_or(Color32::PLACEHOLDER);
    ctx.fonts_mut(|fs| match wrap {
        Some(w) => fs.layout(text.to_string(), f, color, w.max(0.0)),
        None => fs.layout_no_wrap(text.to_string(), f, color),
    })
}

/// The size of `text` under `s`; `avail` wraps it when definite.
pub fn measure_text(ctx: &egui::Context, s: &StyleView, text: &str, avail: crate::provider::AvailableSpace) -> Vec2 {
    let wrap = match avail {
        crate::provider::AvailableSpace::Definite(w) => Some(w),
        _ => None,
    };
    text_galley(ctx, s, text, wrap).size()
}

/// Paints `galley` in `content`: placed by `align`, vertically centred (single-line labels and
/// buttons, as VS Code), on the pixel grid. The galley's own colour is replaced by `color` of `s`.
pub fn paint_text(cx: &mut PaintCx, s: &StyleView, galley: &Arc<egui::Galley>, content: Rect, align: Align) {
    let size = galley.size();
    let x = match align {
        Align::Start => content.left(),
        Align::Center => content.center().x - size.x / 2.0,
        Align::End => content.right() - size.x,
    };
    let y = content.center().y - size.y / 2.0;
    let pos = pos2((x * cx.ppp).round() / cx.ppp, (y * cx.ppp).round() / cx.ppp);
    let color = s.color("color").unwrap_or(Color32::PLACEHOLDER);
    cx.painter.galley(pos, galley.clone(), color);
    cx.record(TraceKind::Text, Rect::from_min_size(pos, size), color, egui::Stroke::NONE, s.font_size());
}
