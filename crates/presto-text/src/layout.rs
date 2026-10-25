//! A laid-out paragraph: placed glyph runs for painting, plus the line and cluster geometry that
//! hit testing, carets and selections read.
//!
//! Coordinates are points relative to the layout's top-left corner, y down. A [`TextLayout`] is
//! immutable and self-contained (it holds no parley state), so nodes keep it as `Arc<TextLayout>`.
//!
//! **Clusters** are grapheme clusters (parley 0.12 clusters never split one), stored per line in
//! visual order with their logical byte range and direction. All geometry is computed from them:
//! - [`TextLayout::hit_test`] picks the line by y, the cluster by x, and the cluster edge nearer
//!   the point; the left edge of an RTL cluster is its logical end.
//! - [`TextLayout::caret_rect`] places the caret on the cluster that starts at the byte
//!   (downstream) or, failing that, the one that ends there (upstream); [`Cursor`] chooses
//!   explicitly at bidi boundaries and soft line ends.
//! - [`TextLayout::selection_rects`] unions, line by line, the visual runs of clusters that overlap
//!   the range, so a range crossing a direction change yields one rect per visual piece.

use presto_base::geom::{Point, Rect, Size};
use presto_base::text::GlyphRun;
use std::ops::Range;
use std::sync::Arc;

/// Width of the caret rect, points.
pub const CARET_WIDTH: f32 = 1.0;

/// Which side of a byte offset a caret attaches to, where the two sides differ (a soft line break,
/// a bidi boundary).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Affinity {
    /// Attached to the character before the offset (the end of the previous line, at a wrap).
    Upstream,
    /// Attached to the character after the offset.
    #[default]
    Downstream,
}

/// A caret position: a byte offset into the text and its affinity.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Cursor {
    pub byte: usize,
    pub affinity: Affinity,
}

impl Cursor {
    pub const fn new(byte: usize, affinity: Affinity) -> Self {
        Cursor { byte, affinity }
    }
}

/// One shaped run placed in the layout. `origin` is the pen position of the run's first glyph on
/// its baseline; glyph positions inside [`GlyphRun`] are relative to it.
#[derive(Clone, Debug, PartialEq)]
pub struct PlacedRun {
    pub run: Arc<GlyphRun>,
    pub origin: Point,
    /// The paint style id ([`crate::TextStyle::paint`]) of the span the run belongs to.
    pub style: u32,
    /// The logical byte range of the text the run draws.
    pub range: Range<usize>,
}

/// A line of the layout.
#[derive(Clone, Debug, PartialEq)]
pub struct LineInfo {
    /// Logical byte range, including a trailing newline or hanging space.
    pub range: Range<usize>,
    /// Top and bottom of the line box.
    pub top: f32,
    pub bottom: f32,
    pub baseline: f32,
    /// Left and right edges of the line's content (after alignment).
    pub left: f32,
    pub right: f32,
    pub(crate) clusters: Range<u32>,
    /// The paragraph direction of this line (where an empty line puts its caret).
    pub(crate) rtl: bool,
}

/// One grapheme cluster of a line, as [`TextLayout::clusters`] yields it.
#[derive(Clone, Debug, PartialEq)]
pub struct ClusterInfo {
    /// Logical byte range in the text.
    pub range: Range<usize>,
    /// Left edge, points from the layout's left.
    pub x: f32,
    /// Horizontal advance (0 for a hard line break).
    pub advance: f32,
    pub rtl: bool,
    /// A hard line break (draws nothing).
    pub newline: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ClusterBox {
    pub range: Range<u32>,
    pub x0: f32,
    pub x1: f32,
    pub rtl: bool,
    /// A hard line break: it is never a hit-test target and never draws.
    pub newline: bool,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct TextLayout {
    pub(crate) size: Size,
    pub(crate) lines: Vec<LineInfo>,
    pub(crate) runs: Vec<PlacedRun>,
    pub(crate) clusters: Vec<ClusterBox>,
    pub(crate) text_len: usize,
    pub(crate) truncated: bool,
    pub(crate) rtl: bool,
}

impl TextLayout {
    pub fn size(&self) -> Size {
        self.size
    }

    pub fn runs(&self) -> &[PlacedRun] {
        &self.runs
    }

    pub fn lines(&self) -> &[LineInfo] {
        &self.lines
    }

    /// Whether `max_lines` or the width cut the text (it then ends in "…" if ellipsis was asked).
    pub fn truncated(&self) -> bool {
        self.truncated
    }

    /// Whether the paragraph's base direction is right-to-left.
    pub fn is_rtl(&self) -> bool {
        self.rtl
    }

    /// The baseline of the first line (for CSS baseline alignment), 0 for an empty layout.
    pub fn first_baseline(&self) -> f32 {
        self.lines.first().map(|l| l.baseline).unwrap_or(0.0)
    }

    /// The clusters of line `line` (an index into [`Self::lines`]) in visual order, left to right;
    /// empty for an index past the last line.
    pub fn clusters(&self, line: usize) -> impl Iterator<Item = ClusterInfo> + '_ {
        let boxes = self.lines.get(line).map(|l| self.line_clusters(l)).unwrap_or(&[]);
        boxes.iter().map(|c| ClusterInfo {
            range: c.range.start as usize..c.range.end as usize,
            x: c.x0,
            advance: c.x1 - c.x0,
            rtl: c.rtl,
            newline: c.newline,
        })
    }

    fn line_clusters(&self, line: &LineInfo) -> &[ClusterBox] {
        &self.clusters[line.clusters.start as usize..line.clusters.end as usize]
    }

    /// The caret position nearest to `p`.
    pub fn hit_test(&self, p: Point) -> Cursor {
        let Some(line_ix) = self.line_at_y(p.y) else { return Cursor::default() };
        let line = &self.lines[line_ix];
        let soft_end = line_ix + 1 < self.lines.len() && !self.ends_with_newline(line);
        let targets: Vec<&ClusterBox> = self.line_clusters(line).iter().filter(|c| !c.newline).collect();
        let Some(first) = targets.first() else {
            return Cursor::new(line.range.start, Affinity::Downstream);
        };
        let last = targets[targets.len() - 1];
        let edge = |c: &ClusterBox, right: bool| -> usize {
            if right != c.rtl { c.range.end as usize } else { c.range.start as usize }
        };
        let cursor_at = |byte: usize| {
            // At the end of a wrapped line the caret stays on this line.
            let affinity = if soft_end && byte == self.line_content_end(line) {
                Affinity::Upstream
            } else if targets.iter().any(|c| c.range.start as usize == byte) {
                Affinity::Downstream
            } else {
                Affinity::Upstream
            };
            Cursor::new(byte, affinity)
        };
        if p.x <= first.x0 {
            return cursor_at(edge(first, false));
        }
        if p.x >= last.x1 {
            return cursor_at(edge(last, true));
        }
        for c in &targets {
            if p.x < c.x1 {
                let right = p.x >= (c.x0 + c.x1) * 0.5;
                return cursor_at(edge(c, right));
            }
        }
        cursor_at(edge(last, true))
    }

    /// The caret rect at `byte` with downstream affinity (see [`Self::caret_rect_at`]).
    pub fn caret_rect(&self, byte: usize) -> Rect {
        self.caret_rect_at(Cursor::new(byte, Affinity::Downstream))
    }

    /// The caret rect for `cursor`: [`CARET_WIDTH`] wide, the height of its line box. A byte inside a
    /// cluster snaps to the cluster's start.
    pub fn caret_rect_at(&self, cursor: Cursor) -> Rect {
        let byte = self.snap_to_cluster(cursor.byte.min(self.text_len));
        let starting = self.clusters.iter().position(|c| !c.newline && c.range.start as usize == byte);
        let ending = self.clusters.iter().position(|c| !c.newline && c.range.end as usize == byte);
        let newline_at = self.clusters.iter().position(|c| c.newline && c.range.start as usize == byte);
        let pick = match cursor.affinity {
            Affinity::Downstream => starting.map(|i| (i, true)).or(ending.map(|i| (i, false))),
            Affinity::Upstream => ending.map(|i| (i, false)).or(starting.map(|i| (i, true))),
        };
        // A caret before a hard break, or on an empty line, sits at the line's content edge.
        let line_edge = |line: &LineInfo| {
            let x = if line.rtl { line.right - CARET_WIDTH } else { line.left };
            Rect::new(x, line.top, x + CARET_WIDTH, line.bottom)
        };
        match pick {
            Some((i, at_start)) => {
                let c = &self.clusters[i];
                let line = self.line_of_cluster(i);
                // LTR start and RTL end are the cluster's left edge.
                let x = if at_start != c.rtl { c.x0 } else { c.x1 };
                Rect::new(x, line.top, x + CARET_WIDTH, line.bottom)
            }
            None => {
                if let Some(n) = newline_at {
                    let line = self.line_of_cluster(n);
                    let content: Vec<&ClusterBox> =
                        self.line_clusters(line).iter().filter(|c| !c.newline).collect();
                    if content.is_empty() {
                        return line_edge(line);
                    }
                    let x = if line.rtl { line.left - CARET_WIDTH } else { line.right };
                    return Rect::new(x, line.top, x + CARET_WIDTH, line.bottom);
                }
                let line = self
                    .lines
                    .iter()
                    .find(|l| l.range.start <= byte && byte <= l.range.end && l.clusters.is_empty())
                    .or(self.lines.last());
                match line {
                    Some(line) => line_edge(line),
                    None => Rect::new(0.0, 0.0, CARET_WIDTH, self.size.h),
                }
            }
        }
    }

    /// One rect per visually contiguous piece of `range`, line by line, each the height of its line.
    pub fn selection_rects(&self, range: Range<usize>) -> Vec<Rect> {
        let mut out = Vec::new();
        if range.start >= range.end {
            return out;
        }
        for line in &self.lines {
            if line.range.end <= range.start || line.range.start >= range.end {
                continue;
            }
            let mut cur: Option<(f32, f32)> = None;
            for c in self.line_clusters(line) {
                let selected = (c.range.start as usize) < range.end && (c.range.end as usize) > range.start;
                if selected {
                    cur = Some(match cur {
                        Some((x0, x1)) if (c.x0 - x1).abs() < 0.01 => (x0, c.x1.max(x1)),
                        Some((x0, x1)) => {
                            out.push(Rect::new(x0, line.top, x1, line.bottom));
                            (c.x0, c.x1)
                        }
                        None => (c.x0, c.x1),
                    });
                } else if let Some((x0, x1)) = cur.take() {
                    out.push(Rect::new(x0, line.top, x1, line.bottom));
                }
            }
            if let Some((x0, x1)) = cur {
                out.push(Rect::new(x0, line.top, x1, line.bottom));
            }
        }
        out.retain(|r| r.width() > 0.0);
        out
    }

    fn line_at_y(&self, y: f32) -> Option<usize> {
        if self.lines.is_empty() {
            return None;
        }
        Some(self.lines.iter().position(|l| y < l.bottom).unwrap_or(self.lines.len() - 1))
    }

    fn line_of_cluster(&self, i: usize) -> &LineInfo {
        let i = i as u32;
        self.lines.iter().find(|l| l.clusters.contains(&i)).unwrap_or(&self.lines[self.lines.len() - 1])
    }

    fn ends_with_newline(&self, line: &LineInfo) -> bool {
        self.line_clusters(line).iter().any(|c| c.newline)
    }

    /// The logical end of the line's content, before hanging whitespace is counted: the largest
    /// cluster end of the line.
    fn line_content_end(&self, line: &LineInfo) -> usize {
        self.line_clusters(line).iter().filter(|c| !c.newline).map(|c| c.range.end as usize).max().unwrap_or(line.range.end)
    }

    /// The start of the cluster containing `byte` (or `byte` at a boundary or the end).
    fn snap_to_cluster(&self, byte: usize) -> usize {
        for c in &self.clusters {
            if (c.range.start as usize) < byte && byte < c.range.end as usize {
                return c.range.start as usize;
            }
        }
        byte
    }
}
