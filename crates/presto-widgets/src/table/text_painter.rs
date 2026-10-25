//! The tier-1 record code's [`TextPainter`] over this crate's kind interface: text is laid out by the
//! thread's [`TextShaper`] (presto-text through `host.rs`) in the requested family, size, weight and
//! slant, and each placed run is registered through the recording's [`Resources`] and drawn in the
//! requested colour. An elided draw is one line cut with `…` by the shaper at the box's width.

use super::shim::{HAlign, TextPainter, TextStyle};
use crate::shim::core::Resources;
use crate::shim::text::{self, RichText, TextLayout, TextShaper};
use presto_base::display::{DisplayList, Glyphs, Prim};
use presto_base::geom::{Point, Rect, Size};
use std::sync::Arc;

/// A [`TextPainter`] over a shaper; without `res` (measuring) draws record nothing.
pub struct ShaperPainter<'a> {
    pub text: &'a mut dyn TextShaper,
    pub res: Option<&'a mut dyn Resources>,
}

impl ShaperPainter<'_> {
    fn layout(&mut self, s: &str, style: &TextStyle, elide_width: Option<f32>) -> Arc<TextLayout> {
        let base = text::TextStyle { family: Arc::from(style.family.as_str()), size: style.size, weight: style.weight, italic: style.italic, ..text::TextStyle::default() };
        let rich = RichText { text: s.to_owned(), base, ellipsis: elide_width.is_some(), max_lines: Some(1), ..RichText::default() };
        self.text.layout(&rich, elide_width)
    }
}

impl TextPainter for ShaperPainter<'_> {
    fn measure(&mut self, text: &str, style: &TextStyle) -> Size {
        self.layout(text, style, None).size()
    }

    fn draw(&mut self, list: &mut DisplayList, text: &str, style: &TextStyle, rect: Rect, align: HAlign, elide: bool) {
        let l = self.layout(text, style, elide.then_some(rect.width().max(0.0)));
        let Some(res) = self.res.as_deref_mut() else { return };
        let size = l.size();
        let x = match align {
            HAlign::Start => rect.min.x,
            HAlign::Center => rect.min.x + (rect.width() - size.w) / 2.0,
            HAlign::End => rect.max.x - size.w,
        };
        let top = Point::new(x, rect.min.y + (rect.height() - size.h) / 2.0);
        for pr in l.runs() {
            if pr.run.glyphs.is_empty() || style.color.a <= 0.0 {
                continue;
            }
            let origin = top + pr.origin.to_vec();
            let ink = Rect::new(origin.x, origin.y - pr.run.ascent, origin.x + pr.run.advance, origin.y + pr.run.descent);
            let id = res.run(&pr.run);
            list.push(Prim::Glyphs(Glyphs { run: id, origin, color: style.color }), ink);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shim::testing::MonoShaper;
    use presto_base::color::Color;

    #[test]
    fn draws_runs_in_the_requested_colour_and_measures() {
        let mut shaper = MonoShaper;
        let mut res = crate::shim::testing::MemResources::default();
        let style = TextStyle { color: Color::WHITE, ..TextStyle::default() };
        let mut p = ShaperPainter { text: &mut shaper, res: Some(&mut res) };
        let size = p.measure("Mon", &style);
        assert!(size.w > 0.0 && size.h > 0.0);
        let mut list = DisplayList::new();
        p.draw(&mut list, "Mon", &style, Rect::new(0.0, 0.0, 100.0, 20.0), HAlign::Center, false);
        assert!(list.items.iter().any(|i| matches!(i, Prim::Glyphs(g) if g.color == Color::WHITE)));
    }
}
