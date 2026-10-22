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

/// The size of `text` under `s`; `avail` wraps it when definite. A `line-height` in px sets the
/// height of each line (VS Code's text button: 16px lines in a 26px button).
pub fn measure_text(ctx: &egui::Context, s: &StyleView, text: &str, avail: crate::provider::AvailableSpace) -> Vec2 {
    let wrap = match avail {
        crate::provider::AvailableSpace::Definite(w) => Some(w),
        _ => None,
    };
    let g = text_galley(ctx, s, text, wrap);
    let mut size = g.size();
    if let Some(lh) = s.px("line-height")
        && lh > 0.0
    {
        size.y = lh * g.rows.len().max(1) as f32;
    }
    size
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

/// How [`elide`] cuts a line that is too wide (WG6 QW13, `elide` / CSS `text-overflow`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Elide {
    /// `…tail`.
    Start,
    /// `head…tail`, the middle cut (paths and names).
    Middle,
    /// `head…` (CSS `text-overflow: ellipsis`).
    End,
}

impl Elide {
    /// The `elide` prop's value (`none` and anything else: `None`).
    pub fn parse(v: &str) -> Option<Elide> {
        match v {
            "start" => Some(Elide::Start),
            "middle" => Some(Elide::Middle),
            "end" => Some(Elide::End),
            _ => None,
        }
    }
    /// The mode `s` asks for: `text-overflow: ellipsis` is an end cut.
    pub fn of_style(s: &StyleView) -> Option<Elide> {
        (s.keyword("text-overflow") == Some("ellipsis")).then_some(Elide::End)
    }
}

/// `text` cut with `…` to fit `width` on one line under `s` (the longest cut that fits, found by
/// a binary search over measured cuts); `text` itself when it fits.
pub fn elide(ctx: &egui::Context, s: &StyleView, text: &str, mode: Elide, width: f32) -> String {
    let f = font(ctx, s, text);
    elide_with(text, mode, width, |t| ctx.fonts_mut(|fs| fs.layout_no_wrap(t.to_owned(), f.clone(), Color32::WHITE).size().x))
}

/// [`elide`] over any measure.
pub fn elide_with(text: &str, mode: Elide, width: f32, measure: impl Fn(&str) -> f32) -> String {
    if measure(text) <= width {
        return text.to_owned();
    }
    let chars: Vec<char> = text.chars().collect();
    let cut = |n: usize| -> String {
        let (head, tail) = match mode {
            Elide::Start => (0, n),
            Elide::End => (n, 0),
            Elide::Middle => (n.div_ceil(2), n / 2),
        };
        let mut s: String = chars[..head].iter().collect();
        s.push('…');
        s.extend(&chars[chars.len() - tail..]);
        s
    };
    // the widest cut that fits: `fits(n)` is monotonic in n
    let (mut lo, mut hi) = (0usize, chars.len().saturating_sub(1));
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        if measure(&cut(mid)) <= width {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    cut(lo)
}

#[cfg(test)]
mod tests {
    use super::{Elide, elide_with};

    fn w(s: &str) -> f32 {
        s.chars().count() as f32
    }

    #[test]
    fn ow1a2_elide_modes() {
        let t = "/home/user/projects/part.step";
        assert_eq!(elide_with(t, Elide::Middle, 100.0, w), t);
        assert_eq!(elide_with(t, Elide::End, 10.0, w), "/home/use…");
        assert_eq!(elide_with(t, Elide::Start, 10.0, w), "…part.step");
        assert_eq!(elide_with(t, Elide::Middle, 11.0, w), "/home….step");
        assert_eq!(elide_with(t, Elide::Middle, 0.0, w), "…");
    }
}
