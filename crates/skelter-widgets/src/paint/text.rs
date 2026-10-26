//! The text primitive: a layout from computed style (skelter-text through [`TextShaper`]), painted in a
//! content box as glyph runs. Colour is applied when recording, so a colour change never reshapes.
//!
//! One style per call (font family, size, weight, style, line height, letter spacing, underline);
//! rich runs are the `label` kind's (`controls::label`). Elision (`end`/`middle`/`start`) cuts by
//! measuring.

use super::{PaintCx, StyleView};
use crate::shim::text::{RichText, TextLayout, TextShaper, TextStyle};
use skelter_base::color::Color;
use skelter_base::display::{Glyphs, Prim};
use skelter_base::geom::{Point, Rect, Size};
use std::sync::Arc;

/// Horizontal placement of a layout in its box (`text-align`; `start` in LTR).
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

/// The text style `s` gives: `font-family` (else the theme's), `font-size`, `font-weight`,
/// `font-style`, `line-height` (px), `letter-spacing`, `text-decoration-line`.
pub fn text_style(s: &StyleView) -> TextStyle {
    let family = match s.keyword("font-family") {
        Some(f) if !f.is_empty() => Arc::from(f),
        _ => Arc::from(super::ua_theme(s.is_dark()).fonts().family.as_str()),
    };
    let deco = s.keyword("text-decoration-line").or(s.keyword("text-decoration")).unwrap_or("");
    TextStyle {
        family,
        size: s.font_size(),
        weight: s.font_weight().clamp(1, 1000) as u16,
        italic: matches!(s.keyword("font-style"), Some("italic" | "oblique")),
        line_height: s.px("line-height").filter(|l| *l > 0.0),
        letter_spacing: s.px("letter-spacing").unwrap_or(0.0),
        underline: deco.contains("underline"),
        strikethrough: deco.contains("line-through"),
    }
}

/// `text` laid out under `s`, wrapped at `wrap` points if given.
pub fn text_layout(shaper: &mut dyn TextShaper, s: &StyleView, text: &str, wrap: Option<f32>) -> Arc<TextLayout> {
    shaper.layout(&RichText::plain(text, text_style(s)), wrap.map(|w| w.max(0.0)))
}

/// The size of `text` under `s`; `wrap` wraps it when given. A `line-height` in px sets the height of
/// each line (VS Code's text button: 16px lines in a 26px button).
pub fn measure_text(shaper: &mut dyn TextShaper, s: &StyleView, text: &str, wrap: Option<f32>) -> Size {
    let l = text_layout(shaper, s, text, wrap);
    let mut size = l.size();
    if let Some(lh) = s.px("line-height")
        && lh > 0.0
    {
        size.h = lh * l.lines.max(1) as f32;
    }
    size
}

/// Paints `layout` in `content`: placed by `align`, vertically centred (single-line labels and
/// buttons, as VS Code), on the pixel grid, in `color` of `s`. Returns the rect it covers.
pub fn paint_text(cx: &mut PaintCx, s: &StyleView, layout: &TextLayout, content: Rect, align: Align) -> Rect {
    let size = layout.size();
    let x = match align {
        Align::Start => content.min.x,
        Align::Center => (content.min.x + content.max.x) / 2.0 - size.w / 2.0,
        Align::End => content.max.x - size.w,
    };
    let y = (content.min.y + content.max.y) / 2.0 - size.h / 2.0;
    let pos = cx.snap_point(Point::new(x, y));
    let color = s.color("color").unwrap_or(Color::BLACK);
    paint_runs(cx, layout, pos, &|_| color);
    Rect::from_origin_size(pos, size)
}

/// Pushes the runs of `layout` with its top-left at `pos`, each in `color(style index)`.
pub fn paint_runs(cx: &mut PaintCx, layout: &TextLayout, pos: Point, color: &dyn Fn(u32) -> Color) {
    for pr in layout.runs() {
        let c = color(pr.style);
        if c.a <= 0.0 || pr.run.glyphs.is_empty() {
            continue;
        }
        let origin = pos + pr.origin.to_vec();
        let id = cx.res.run(&pr.run);
        let ink = Rect::new(origin.x, origin.y - pr.run.ascent, origin.x + pr.run.advance, origin.y + pr.run.descent);
        cx.push(Prim::Glyphs(Glyphs { run: id, origin, color: c }), ink);
    }
}

/// How [`elide`] cuts a line that is too wide (`elide` / CSS `text-overflow`).
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
pub fn elide(shaper: &mut dyn TextShaper, s: &StyleView, text: &str, mode: Elide, width: f32) -> String {
    let base = text_style(s);
    let shaper = std::cell::RefCell::new(shaper);
    elide_with(text, mode, width, |t| shaper.borrow_mut().layout(&RichText::plain(t, base.clone()), None).size().w)
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
