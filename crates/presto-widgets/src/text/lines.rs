//! [`GeometryLines`]: quire-text's [`VisualLines`] (Up/Down, PageUp/PageDown, Home/End of a wrapped
//! row) over a field's layout. The layout is of the shown text ([`Shown`]); quire-text asks in
//! document bytes, so every answer goes through the shown mapping. The goal x is kept in 1/64 points
//! (fixed point, stable under zoom).

use super::shim::TextGeometry;
use super::shown::Shown;
use presto_base::geom::Point;
use quire_text::{Edge, Goal, Text, VisualLine, VisualLines};
use std::ops::Range;

pub struct GeometryLines<'a> {
    pub geom: &'a dyn TextGeometry,
    pub shown: &'a Shown,
    /// Rows a page holds (the field's visible height over its row height).
    pub page: usize,
}

impl GeometryLines<'_> {
    fn wraps_after(&self, row: usize) -> bool {
        row + 1 < self.geom.line_count() && self.geom.line(row + 1).range.start == self.geom.line(row).range.end
    }

    /// The row holding shown byte `s` (`upstream`: the row a wrap point ends).
    fn row_of(&self, s: usize, upstream: bool) -> usize {
        let n = self.geom.line_count();
        for i in 0..n {
            let r = self.geom.line(i).range;
            if s < r.end || (s == r.end && (!self.wraps_after(i) || upstream)) {
                return i;
            }
        }
        n.saturating_sub(1)
    }

    fn first_row(&self, text: &Text, line: usize) -> usize {
        self.row_of(self.shown.to_shown(text.line_to_byte(line)), false)
    }

    fn visual(&self, text: &Text, row: usize) -> VisualLine {
        let line = text.byte_to_line(self.shown.to_doc(self.geom.line(row).range.start));
        VisualLine { line, row: row - self.first_row(text, line).min(row) }
    }

    fn global(&self, text: &Text, v: VisualLine) -> usize {
        (self.first_row(text, v.line) + v.row).min(self.geom.line_count().saturating_sub(1))
    }
}

impl VisualLines for GeometryLines<'_> {
    fn locate(&self, text: &Text, byte: usize, upstream: bool) -> (VisualLine, Goal) {
        let s = self.shown.to_shown(byte);
        let row = self.row_of(s, upstream);
        let x = self.geom.caret_rect(s, upstream).min.x;
        (self.visual(text, row), Goal((x.max(0.0) * 64.0).round() as u32))
    }

    fn offset(&self, text: &Text, line: VisualLine, delta: isize) -> Result<VisualLine, Edge> {
        let j = self.global(text, line) as isize + delta;
        if j < 0 {
            Err(Edge::Start)
        } else if j as usize >= self.geom.line_count() {
            Err(Edge::End)
        } else {
            Ok(self.visual(text, j as usize))
        }
    }

    fn byte_at(&self, text: &Text, line: VisualLine, goal: Goal) -> (usize, bool) {
        let i = self.global(text, line);
        let info = self.geom.line(i);
        if goal == Goal::END {
            return (self.shown.to_doc(info.range.end), self.wraps_after(i));
        }
        let x = goal.0 as f32 / 64.0;
        let c = self.geom.hit_test(Point::new(x, info.top + info.height / 2.0));
        let s = c.byte.clamp(info.range.start, info.range.end);
        (self.shown.to_doc(s), c.upstream && s == info.range.end)
    }

    fn range(&self, text: &Text, line: VisualLine) -> Range<usize> {
        let r = self.geom.line(self.global(text, line)).range;
        self.shown.to_doc(r.start)..self.shown.to_doc(r.end)
    }

    fn lines_per_page(&self) -> usize {
        self.page.max(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::shim::MonoGeometry;
    use quire_text::NoWrap;

    #[test]
    fn vertical_motion_over_wrapped_rows() {
        // "abcdef\ngh" at 4 columns: abcd | ef | gh
        let text = Text::from("abcdef\ngh");
        let shown = Shown::build("abcdef\ngh", None, None);
        let g = MonoGeometry::new(&shown.text, 10.0, 20.0, Some(40.0));
        let l = GeometryLines { geom: &g, shown: &shown, page: 2 };
        let (v, goal) = l.locate(&text, 1, false);
        assert_eq!((v, goal), (VisualLine { line: 0, row: 0 }, Goal(640)));
        let down = l.offset(&text, v, 1).unwrap();
        assert_eq!(down, VisualLine { line: 0, row: 1 });
        assert_eq!(l.byte_at(&text, down, goal), (5, false));
        let next = l.offset(&text, down, 1).unwrap();
        assert_eq!(next, VisualLine { line: 1, row: 0 });
        assert_eq!(l.byte_at(&text, next, goal), (8, false));
        assert_eq!(l.offset(&text, next, 1), Err(Edge::End));
        // the end of a wrapped row is upstream
        assert_eq!(l.byte_at(&text, v, Goal::END), (4, true));
        assert_eq!(l.range(&text, down), 4..6);
        // without wrapping it agrees with quire-text's NoWrap
        let flat = MonoGeometry::new(&shown.text, 10.0, 20.0, None);
        let f = GeometryLines { geom: &flat, shown: &shown, page: 2 };
        let nw = NoWrap::default();
        let (fv, fg) = f.locate(&text, 8, false);
        let (nv, ng) = nw.locate(&text, 8, false);
        assert_eq!(fv, nv);
        assert_eq!(f.byte_at(&text, f.offset(&text, fv, -1).unwrap(), fg).0, nw.byte_at(&text, nw.offset(&text, nv, -1).unwrap(), ng).0);
    }
}
