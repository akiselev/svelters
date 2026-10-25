//! The tier-1 record code's [`TextPainter`] over this crate's kind interface: text is laid out by the
//! thread's [`TextShaper`] (skelter-text through `host.rs`) in the requested family, size, weight and
//! slant, and each placed run is registered through the recording's [`Resources`] and drawn in the
//! requested colour. An elided draw is one line cut with `…` by the shaper at the box's width.
//!
//! Kinds that implement skelter-core's `Kind` directly (the containers `tab-view` and `table`, which
//! read their children) shape with the document's skelter-text service through [`with_text`] and
//! record through [`record_core`].

use super::shim::{HAlign, TextPainter, TextStyle};
use crate::shim::core::Resources;
use crate::shim::text::{self, RichText, TextLayout, TextShaper};
use skelter_base::commit::ImageData;
use skelter_base::display::{DisplayList, Glyphs, Prim};
use skelter_base::geom::{Point, Rect, Size};
use skelter_base::ids::{ImageId, RunId};
use skelter_base::text::GlyphRun;
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

/// The runs a recording made, before skelter-core assigns their ids: a run's provisional id is its
/// position in `runs` plus one.
#[derive(Default)]
pub struct CollectRes {
    pub runs: Vec<Arc<GlyphRun>>,
}

impl Resources for CollectRes {
    fn run(&mut self, run: &Arc<GlyphRun>) -> RunId {
        self.runs.push(run.clone());
        RunId(self.runs.len() as u64)
    }
    fn image(&mut self, _key: &str, _make: &mut dyn FnMut() -> Option<ImageData>) -> Option<ImageId> {
        None
    }
}

/// The shaper of a document without a text system: no text is measured or drawn.
pub struct NoShaper;

impl TextShaper for NoShaper {
    fn layout(&mut self, _text: &RichText, _max_width: Option<f32>) -> Arc<TextLayout> {
        Arc::new(TextLayout::default())
    }
}

/// Runs `f` with a measuring painter over `ts` (the document's text system), or over [`NoShaper`].
pub fn with_text<R>(ts: Option<&mut skelter_text::TextSystem>, f: impl FnOnce(&mut dyn TextPainter) -> R) -> R {
    let mut none = NoShaper;
    let text: &mut dyn TextShaper = match ts {
        Some(t) => t,
        None => &mut none,
    };
    f(&mut ShaperPainter { text, res: None })
}

/// Runs `f` (record code) with a painter over the document's text system and moves what it recorded
/// into `cx`, registering its runs with skelter-core.
pub fn record_core(cx: &mut skelter_core::RecordCx<'_>, f: impl FnOnce(&mut dyn TextPainter, &mut DisplayList)) {
    let mut list = DisplayList::new();
    let mut res = CollectRes::default();
    {
        let mut none = NoShaper;
        let text: &mut dyn TextShaper = match cx.service::<skelter_text::TextSystem>() {
            Some(t) => t,
            None => &mut none,
        };
        let mut p = ShaperPainter { text, res: Some(&mut res) };
        f(&mut p, &mut list);
    }
    let ids: Vec<RunId> = res.runs.into_iter().map(|r| cx.add_run(r)).collect();
    let bounds = list.bounds;
    for mut item in list.items {
        if let Prim::Glyphs(g) = &mut item
            && let Some(id) = (g.run.0 as usize).checked_sub(1).and_then(|k| ids.get(k))
        {
            g.run = *id;
        }
        cx.push(item, bounds);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shim::testing::MonoShaper;
    use skelter_base::color::Color;

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
