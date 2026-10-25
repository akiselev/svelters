//! Text for tier-1 kinds through presto-core: a kind's record code draws through
//! [`TextPainter`], and presto-core shapes only in `prepare` ([`PrepareCx::shape_text`]) while
//! `record` turns runs into display-list glyphs ([`RecordCx::add_run`]). So a kind runs its record
//! code twice: in `prepare` with [`PrepareText`], which shapes every text it is asked to draw (an
//! elided text shaped at its elided length) into a [`TextCache`], and in `record` with
//! [`record_with`], which draws the cached runs in the requested colour.
//!
//! presto-core shapes in the node's computed font: [`TextStyle`]'s family, size and weight are not
//! applied yet (a header cell's bold), only its colour. A `RecordCx`/`PrepareCx` call that shapes a
//! given `TextSpec` replaces that limit (listed in `plans/milestones/PRESTO-widgets-tier1.md`).

use super::shim::{HAlign, TextPainter, TextStyle};
use presto_base::display::{DisplayList, Glyphs, Prim};
use presto_base::geom::{Point, Rect, Size};
use presto_core::{PrepareCx, RecordCx, ShapedText};
use std::collections::HashMap;

/// Shaped texts by (text, elide width): what `prepare` shaped for `record`.
#[derive(Clone, Debug, Default)]
pub struct TextCache {
    /// Key: the source text and, for an elided draw, the box width's bits (0 otherwise).
    shaped: HashMap<(String, u32), ShapedText>,
}

impl TextCache {
    fn key(text: &str, elide_width: Option<f32>) -> (String, u32) {
        (text.to_string(), elide_width.map_or(0, f32::to_bits))
    }

    pub fn clear(&mut self) {
        self.shaped.clear();
    }

    fn get(&self, text: &str, elide_width: Option<f32>) -> Option<&ShapedText> {
        self.shaped.get(&Self::key(text, elide_width)).or_else(|| self.shaped.get(&Self::key(text, None)))
    }
}

/// The `prepare` pass: shapes what record code asks for into a [`TextCache`]; records nothing.
pub struct PrepareText<'c, 'a> {
    pub cx: &'c mut PrepareCx<'a>,
    pub cache: &'c mut TextCache,
}

impl PrepareText<'_, '_> {
    fn shape(&mut self, text: &str) -> ShapedText {
        if let Some(s) = self.cache.shaped.get(&TextCache::key(text, None)) {
            return s.clone();
        }
        let s = self.cx.shape_text(text, None).unwrap_or_default();
        self.cache.shaped.insert(TextCache::key(text, None), s.clone());
        s
    }
}

impl TextPainter for PrepareText<'_, '_> {
    fn measure(&mut self, text: &str, _style: &TextStyle) -> Size {
        self.shape(text).size
    }

    fn draw(&mut self, _list: &mut DisplayList, text: &str, _style: &TextStyle, rect: Rect, _align: HAlign, elide: bool) {
        let full = self.shape(text);
        if !elide || full.size.w <= rect.width() {
            return;
        }
        // the longest prefix that fits with an ellipsis (binary search over char boundaries)
        let bounds: Vec<usize> = text.char_indices().map(|(i, _)| i).collect();
        let (mut lo, mut hi) = (0usize, bounds.len());
        let mut best = self.shape("…");
        while lo < hi {
            let mid = (lo + hi).div_ceil(2);
            let cand = format!("{}…", &text[..bounds.get(mid).copied().unwrap_or(text.len())]);
            let s = self.shape(&cand);
            if s.size.w <= rect.width() {
                best = s;
                lo = mid;
            } else {
                hi = mid - 1;
            }
        }
        self.cache.shaped.insert(TextCache::key(text, Some(rect.width())), best);
    }
}

/// The `record` pass's painter: draws cached runs; a text `prepare` did not shape draws nothing.
struct RecordText<'c, 'a> {
    cx: &'c mut RecordCx<'a>,
    cache: &'c TextCache,
}

impl TextPainter for RecordText<'_, '_> {
    fn measure(&mut self, text: &str, _style: &TextStyle) -> Size {
        self.cache.get(text, None).map_or(Size::ZERO, |s| s.size)
    }

    fn draw(&mut self, list: &mut DisplayList, text: &str, style: &TextStyle, rect: Rect, align: HAlign, elide: bool) {
        let Some(s) = self.cache.get(text, elide.then_some(rect.width())) else { return };
        let x = match align {
            HAlign::Start => rect.min.x,
            HAlign::Center => rect.min.x + (rect.width() - s.size.w) / 2.0,
            HAlign::End => rect.max.x - s.size.w,
        };
        let top = Point::new(x, rect.min.y + (rect.height() - s.size.h) / 2.0);
        for (run, o) in &s.runs {
            let origin = Point::new(top.x + o.x, top.y + o.y);
            let ink = Rect::new(origin.x, origin.y - run.ascent, origin.x + run.advance, origin.y + run.descent).intersect(&rect);
            let id = self.cx.add_run(run.clone());
            list.push(Prim::Glyphs(Glyphs { run: id, origin, color: style.color }), ink);
        }
    }
}

/// Runs `f` (record code) with a painter over `cache` and moves what it recorded into `cx`.
pub fn record_with(cx: &mut RecordCx<'_>, cache: &TextCache, f: impl FnOnce(&mut dyn TextPainter, &mut DisplayList)) {
    let mut list = DisplayList::new();
    {
        let mut painter = RecordText { cx: &mut *cx, cache };
        f(&mut painter, &mut list);
    }
    let bounds = list.bounds;
    for p in list.items {
        cx.push(p, bounds);
    }
}
