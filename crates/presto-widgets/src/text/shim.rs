//! Stand-ins for wave-1 APIs the text kinds need before those crates have them, and the small
//! interfaces the field records and lays out through. Each stand-in names the API it stands for; the
//! consolidation replaces it with the real one.
//!
//! - [`TextGeometry`]: the field's view of a layout. `presto_text::TextLayout` implements it
//!   ([`super::geometry`]); so does [`MonoGeometry`]. Not a stand-in: it lets tests lay out without
//!   fonts.
//! - [`Layouter`]: lays out a field's shown text in its style; [`super::geometry::SystemLayouter`]
//!   implements it over `presto_text::TextSystem`, [`MonoLayouter`] over [`MonoGeometry`].
//! - [`Clipboard`] stands for `presto_platform::Clipboard` (`get_text`/`set_text` by
//!   `ClipboardKind`), which presto-widgets does not depend on; the app adapts it (giving PRIMARY up is
//!   setting it empty there).
//! - [`RecordSink`] stands for the part of presto-core's `RecordCx` a field records through (`push`,
//!   `draw_run`); [`ListSink`] collects into a `DisplayList` for tests.
//! - [`TextHost`]: the document service (`Document::set_service`) the text kinds read the clipboard
//!   and the caret's fast-commit channel from; the app installs it beside the `TextSystem` service.
//! - [`MonoGeometry`] is a real layout of its own (fixed advance, fixed line height): what the tests
//!   and the conformance pins lay text out with, and a fallback for headless use. It has no font, so
//!   it records no glyphs.

use presto_base::color::Color;
use presto_base::commit::FastCommit;
use presto_base::display::{DisplayList, Prim};
use presto_base::geom::{Point, Rect, Size};
use presto_base::text::GlyphRun;
use std::ops::Range;
use std::sync::Arc;

/// A position in laid-out text (`presto_text::Cursor`): a byte of the laid-out string and its
/// affinity. `upstream`: drawn at the end of the row before a soft wrap rather than the start of the
/// next one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Cursor {
    pub byte: usize,
    pub upstream: bool,
}

/// One laid-out row.
#[derive(Clone, Debug, PartialEq)]
pub struct LineInfo {
    /// The row's bytes in the laid-out string, without its line feed.
    pub range: Range<usize>,
    /// Top and height in layout coordinates.
    pub top: f32,
    pub height: f32,
    pub baseline: f32,
}

/// Laid-out text, as the text kinds query it. Coordinates are the layout's own (origin at its top
/// left, points); offsets are bytes of the string that was laid out.
pub trait TextGeometry {
    fn size(&self) -> Size;
    fn hit_test(&self, p: Point) -> Cursor;
    fn caret_rect(&self, byte: usize, upstream: bool) -> Rect;
    fn selection_rects(&self, range: Range<usize>) -> Vec<Rect>;
    fn line_count(&self) -> usize;
    fn line(&self, i: usize) -> LineInfo;
    /// Records the glyph runs at `origin` in `color`.
    fn record_glyphs(&self, sink: &mut dyn RecordSink, origin: Point, color: Color);
}

/// Where a field records: display-list primitives and shaped runs.
pub trait RecordSink {
    fn push(&mut self, p: Prim, ink: Rect);
    /// A run with its baseline origin at `origin`.
    fn draw_run(&mut self, run: Arc<GlyphRun>, origin: Point, color: Color);
}

/// A [`RecordSink`] that keeps the list and the runs (tests, headless dumps).
#[derive(Clone, Debug, Default)]
pub struct ListSink {
    pub list: DisplayList,
    pub runs: Vec<(Arc<GlyphRun>, Point, Color)>,
}

impl RecordSink for ListSink {
    fn push(&mut self, p: Prim, ink: Rect) {
        self.list.push(p, ink);
    }
    fn draw_run(&mut self, run: Arc<GlyphRun>, origin: Point, color: Color) {
        self.runs.push((run, origin, color));
    }
}

/// What the text kinds need from the app besides the text system: the clipboard (PRIMARY included)
/// and the channel that sends the caret's fast commits to the window's compositor. Lent to kinds as a
/// presto-core document service (`Document::set_service(TextHost { .. })`).
pub struct TextHost {
    pub clipboard: Box<dyn Clipboard>,
    pub fast_commit: Box<dyn FnMut(FastCommit)>,
}

/// Lays out a field's shown text in its style; `max_width` wraps (`None`: rows end at line feeds).
pub trait Layouter {
    fn layout(&mut self, text: &str, max_width: Option<f32>) -> Arc<dyn TextGeometry>;
}

/// [`MonoGeometry`] as a [`Layouter`].
#[derive(Clone, Copy, Debug)]
pub struct MonoLayouter {
    pub advance: f32,
    pub line_height: f32,
}

impl Layouter for MonoLayouter {
    fn layout(&mut self, text: &str, max_width: Option<f32>) -> Arc<dyn TextGeometry> {
        Arc::new(MonoGeometry::new(text, self.advance, self.line_height, max_width))
    }
}

/// The system clipboard and, on X11 and Wayland, the PRIMARY selection.
pub trait Clipboard {
    fn get_text(&mut self) -> Option<String>;
    fn set_text(&mut self, text: &str);
    /// `None` where the platform has no PRIMARY selection.
    fn get_primary(&mut self) -> Option<String> {
        None
    }
    fn set_primary(&mut self, _text: &str) {}
    fn clear_primary(&mut self) {}
}

/// A fixed-advance layout: every char is `advance` wide, every row `line_height` tall; rows wrap
/// at `wrap_width` (whole chars), else only at line feeds.
#[derive(Clone, Debug)]
pub struct MonoGeometry {
    text: String,
    advance: f32,
    line_height: f32,
    rows: Vec<Range<usize>>,
    width: f32,
}

impl MonoGeometry {
    pub fn new(text: &str, advance: f32, line_height: f32, wrap_width: Option<f32>) -> Self {
        let cols = wrap_width.map(|w| ((w / advance).floor() as usize).max(1));
        let mut rows = Vec::new();
        let mut start = 0;
        for line in text.split('\n') {
            let end = start + line.len();
            match cols {
                Some(cols) if line.chars().count() > cols => {
                    let mut row_start = start;
                    let mut n = 0;
                    for (i, _) in line.char_indices() {
                        if n == cols {
                            rows.push(row_start..start + i);
                            row_start = start + i;
                            n = 0;
                        }
                        n += 1;
                    }
                    rows.push(row_start..end);
                }
                _ => rows.push(start..end),
            }
            start = end + 1;
        }
        let width = rows.iter().map(|r| text[r.clone()].chars().count()).max().unwrap_or(0) as f32 * advance;
        MonoGeometry { text: text.to_string(), advance, line_height, rows, width }
    }

    fn row_of(&self, byte: usize, upstream: bool) -> usize {
        let byte = byte.min(self.text.len());
        for (i, r) in self.rows.iter().enumerate() {
            let wraps = self.rows.get(i + 1).is_some_and(|n| n.start == r.end);
            if byte < r.end || (byte == r.end && (!wraps || upstream)) {
                return i;
            }
        }
        self.rows.len() - 1
    }

    fn x_of(&self, row: usize, byte: usize) -> f32 {
        let r = &self.rows[row];
        let byte = byte.clamp(r.start, r.end);
        self.text[r.start..byte].chars().count() as f32 * self.advance
    }
}

impl TextGeometry for MonoGeometry {
    fn size(&self) -> Size {
        Size::new(self.width, self.rows.len() as f32 * self.line_height)
    }

    fn hit_test(&self, p: Point) -> Cursor {
        let row = ((p.y / self.line_height).floor().max(0.0) as usize).min(self.rows.len() - 1);
        let r = &self.rows[row];
        let n = self.text[r.clone()].chars().count();
        let col = ((p.x / self.advance).round().max(0.0) as usize).min(n);
        let byte = self.text[r.clone()].char_indices().nth(col).map_or(r.end, |(i, _)| r.start + i);
        let wraps = self.rows.get(row + 1).is_some_and(|n| n.start == r.end);
        Cursor { byte, upstream: wraps && byte == r.end }
    }

    fn caret_rect(&self, byte: usize, upstream: bool) -> Rect {
        let row = self.row_of(byte, upstream);
        let x = self.x_of(row, byte);
        let top = row as f32 * self.line_height;
        Rect::new(x, top, x + 1.0, top + self.line_height)
    }

    fn selection_rects(&self, range: Range<usize>) -> Vec<Rect> {
        let mut out = Vec::new();
        for (i, r) in self.rows.iter().enumerate() {
            let (a, b) = (range.start.max(r.start), range.end.min(r.end));
            // a selected line feed shows as a little extra width at the row's end
            let feed = range.start <= r.end && range.end > r.end && self.text.as_bytes().get(r.end) == Some(&b'\n');
            if a > b || (a == b && !feed) {
                continue;
            }
            let top = i as f32 * self.line_height;
            let x1 = self.x_of(i, b) + if feed { self.advance / 2.0 } else { 0.0 };
            out.push(Rect::new(self.x_of(i, a), top, x1, top + self.line_height));
        }
        out
    }

    fn line_count(&self) -> usize {
        self.rows.len()
    }

    fn line(&self, i: usize) -> LineInfo {
        let top = i as f32 * self.line_height;
        LineInfo { range: self.rows[i].clone(), top, height: self.line_height, baseline: top + self.line_height * 0.8 }
    }

    fn record_glyphs(&self, _sink: &mut dyn RecordSink, _origin: Point, _color: Color) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mono_geometry_wraps_hits_and_places_carets() {
        let g = MonoGeometry::new("abcdef\ngh", 10.0, 20.0, Some(40.0));
        // rows: abcd | ef | gh
        assert_eq!(g.line_count(), 3);
        assert_eq!(g.line(0).range, 0..4);
        assert_eq!(g.line(1).range, 4..6);
        assert_eq!(g.line(2).range, 7..9);
        assert_eq!(g.hit_test(Point::new(14.0, 5.0)), Cursor { byte: 1, upstream: false });
        assert_eq!(g.hit_test(Point::new(99.0, 5.0)), Cursor { byte: 4, upstream: true });
        assert_eq!(g.hit_test(Point::new(10.0, 45.0)).byte, 8);
        // byte 4 is the wrap point: downstream starts row 1, upstream ends row 0
        assert_eq!(g.caret_rect(4, false).min, Point::new(0.0, 20.0));
        assert_eq!(g.caret_rect(4, true).min, Point::new(40.0, 0.0));
        let sel = g.selection_rects(2..8);
        assert_eq!(sel.len(), 3);
        assert_eq!(sel[1], Rect::new(0.0, 20.0, 25.0, 40.0), "the selected line feed adds half a cell");
        assert_eq!(g.size(), Size::new(40.0, 60.0));
    }
}
