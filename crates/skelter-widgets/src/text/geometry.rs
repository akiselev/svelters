//! skelter-text's [`TextLayout`] as a field's [`TextGeometry`], and [`SystemLayouter`]: the field's
//! [`Layouter`] over a [`TextSystem`] in the node's text style.
//!
//! skelter-text's lines include their trailing line feed; [`TextGeometry::line`] ranges end before it
//! (what quire-text's `VisualLines` wants), so the adapter keeps which lines end in one.

use super::shim::{Cursor, Layouter, LineInfo, RecordSink, TextGeometry};
use skelter_base::color::Color;
use skelter_base::geom::{Point, Rect, Size};
use skelter_text::{Affinity, RichText, TextLayout, TextStyle, TextSystem};
use std::ops::Range;
use std::sync::Arc;

/// A laid-out string and which of its lines end in a line feed.
pub struct LaidOut {
    pub layout: Arc<TextLayout>,
    feeds: Vec<bool>,
}

impl LaidOut {
    pub fn new(text: &str, layout: Arc<TextLayout>) -> Self {
        let bytes = text.as_bytes();
        let feeds = layout.lines().iter().map(|l| l.range.end > l.range.start && bytes.get(l.range.end - 1) == Some(&b'\n')).collect();
        LaidOut { layout, feeds }
    }
}

fn affinity(upstream: bool) -> Affinity {
    if upstream { Affinity::Upstream } else { Affinity::Downstream }
}

impl TextGeometry for LaidOut {
    fn size(&self) -> Size {
        self.layout.size()
    }

    fn hit_test(&self, p: Point) -> Cursor {
        let c = self.layout.hit_test(p);
        Cursor { byte: c.byte, upstream: c.affinity == Affinity::Upstream }
    }

    fn caret_rect(&self, byte: usize, upstream: bool) -> Rect {
        self.layout.caret_rect_at(skelter_text::Cursor::new(byte, affinity(upstream)))
    }

    fn selection_rects(&self, range: Range<usize>) -> Vec<Rect> {
        self.layout.selection_rects(range)
    }

    fn line_count(&self) -> usize {
        self.layout.lines().len()
    }

    fn line(&self, i: usize) -> LineInfo {
        let l = &self.layout.lines()[i];
        let end = if self.feeds[i] { l.range.end - 1 } else { l.range.end };
        LineInfo { range: l.range.start..end, top: l.top, height: l.bottom - l.top, baseline: l.baseline }
    }

    fn record_glyphs(&self, sink: &mut dyn RecordSink, origin: Point, color: Color) {
        for r in self.layout.runs() {
            sink.draw_run(r.run.clone(), origin + r.origin.to_vec(), color);
        }
    }
}

/// Lays out in one style with a [`TextSystem`] (the UI thread's).
pub struct SystemLayouter<'a> {
    pub system: &'a mut TextSystem,
    pub style: TextStyle,
}

impl Layouter for SystemLayouter<'_> {
    fn layout(&mut self, text: &str, max_width: Option<f32>) -> Arc<dyn TextGeometry> {
        let mut style = self.style.clone();
        style.wrap = max_width.is_some();
        let layout = self.system.layout(&RichText::plain(text, style), max_width);
        Arc::new(LaidOut::new(text, layout))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::editor::{EditorConfig, FocusReason};
    use crate::text::field::{FieldColors, TextField};
    use crate::text::policy::{Platform, TextPolicy};
    use crate::text::shim::ListSink;
    use skelter_text::{FontConfig, FontStore};

    fn system() -> TextSystem {
        TextSystem::new(FontStore::new(FontConfig::testing()))
    }

    #[test]
    fn text_layout_lines_end_before_their_feed() {
        let mut sys = system();
        let mut lay = SystemLayouter { system: &mut sys, style: TextStyle::default() };
        let g = lay.layout("ab\ncd", None);
        assert_eq!(g.line_count(), 2);
        assert_eq!(g.line(0).range, 0..2);
        assert_eq!(g.line(1).range, 3..5);
        assert!(g.caret_rect(1, false).min.x > g.caret_rect(0, false).min.x);
        let r = g.caret_rect(4, false);
        assert_eq!(g.hit_test(Point::new(r.min.x, (r.min.y + r.max.y) / 2.0)).byte, 4);
        let mut sink = ListSink::default();
        g.record_glyphs(&mut sink, Point::ZERO, Color::BLACK);
        assert!(!sink.runs.is_empty());
    }

    #[test]
    fn a_field_on_real_fonts_records_runs_and_moves_its_caret() {
        let mut sys = system();
        let mut lay = SystemLayouter { system: &mut sys, style: TextStyle::default() };
        let mut f = TextField::new("Hello", EditorConfig::default(), TextPolicy::of(Platform::Windows));
        f.prepare(&mut lay, Rect::new(0.0, 0.0, 300.0, 20.0), skelter_base::geom::Affine::IDENTITY);
        f.focus_in(FocusReason::Pointer);
        f.editor.set_selection(5, 5);
        f.prepare(&mut lay, Rect::new(0.0, 0.0, 300.0, 20.0), skelter_base::geom::Affine::IDENTITY);
        let end = f.caret_rect().unwrap();
        f.editor.set_selection(0, 0);
        f.prepare(&mut lay, Rect::new(0.0, 0.0, 300.0, 20.0), skelter_base::geom::Affine::IDENTITY);
        assert!(f.caret_rect().unwrap().min.x < end.min.x);
        let colors = FieldColors { text: Color::BLACK, placeholder: Color::BLACK, selection: Color::WHITE, selection_inactive: Color::WHITE, preedit_underline: Color::BLACK };
        let mut sink = ListSink::default();
        f.record(&mut sink, &colors);
        assert!(!sink.runs.is_empty());
    }
}
