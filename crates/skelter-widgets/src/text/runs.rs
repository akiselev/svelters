//! [`TextRanges`]: a field's shown text cut into the runs an accessibility tree carries (AccessKit's
//! `Role::TextRun` children, plan `qc-a11y` §3.1), as plain data a semantic model can carry too.
//!
//! One run per laid-out row; a row longer than [`MAX_RUN_CHARS`] characters is cut into chunks
//! linked on the line (AccessKit's `word_starts` are `u8`), as egui 0.37 does
//! (`text_selection/accesskit_text.rs`). A row ending in a line feed carries it as a last character
//! of width 0, and an empty text is one empty run, so a consumer still finds text ranges on an empty
//! field. A character is a grapheme cluster (one longer than 255 bytes is cut at its chars). Positions
//! and widths come from the layout's carets at character boundaries, relative to the run's left edge;
//! runs are left to right until skelter-text reports bidi levels.

use super::shim::TextGeometry;
use skelter_base::geom::Rect;
use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;

/// The most characters in one run: AccessKit's `word_starts` index characters with a `u8`.
pub const MAX_RUN_CHARS: usize = 255;

/// One run: a row of the shown text, or a chunk of one.
#[derive(Clone, Debug, PartialEq)]
pub struct TextRun {
    /// The run's text, with the row's line feed when it is the row's last chunk.
    pub text: String,
    /// The shown bytes it covers, without the line feed.
    pub range: Range<usize>,
    /// The run ends in the row's line feed.
    pub feed: bool,
    /// UTF-8 length of each character.
    pub lengths: Vec<u8>,
    /// Each character's left edge from the run's, and its advance.
    pub positions: Vec<f32>,
    pub widths: Vec<f32>,
    /// The characters that start a word.
    pub word_starts: Vec<u8>,
    /// The run's box in layout coordinates.
    pub rect: Rect,
    /// The laid-out row, and the chunk of it with the row's chunk count.
    pub row: usize,
    pub chunk: usize,
    pub chunks: usize,
}

impl TextRun {
    /// The shown byte after the run, its line feed included.
    pub fn end(&self) -> usize {
        self.range.end + self.feed as usize
    }
}

/// A field's runs, in text order. See the module docs.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TextRanges {
    pub runs: Vec<TextRun>,
}

/// The characters of `s` (grapheme clusters; one over 255 bytes is cut at its chars), as
/// `(start, len)` from `s`'s start.
fn characters(s: &str) -> Vec<(usize, u8)> {
    let mut out = Vec::new();
    for (b, g) in s.grapheme_indices(true) {
        if g.len() <= u8::MAX as usize {
            out.push((b, g.len() as u8));
        } else {
            out.extend(g.char_indices().map(|(i, c)| (b + i, c.len_utf8() as u8)));
        }
    }
    out
}

impl TextRanges {
    /// The runs of `text` (the shown text) as `geom` laid it out (`None`: one unplaced row per line
    /// feed); `word_starts` are the shown bytes that start words, ascending.
    pub fn build(text: &str, geom: Option<&dyn TextGeometry>, word_starts: &[usize]) -> TextRanges {
        // the rows: their bytes without the line feed, and their tops and heights
        let rows: Vec<(Range<usize>, f32, f32)> = match geom {
            Some(g) if g.line_count() > 0 => (0..g.line_count()).map(|i| g.line(i)).map(|l| (l.range, l.top, l.height)).collect(),
            _ => {
                let mut start = 0;
                text.split('\n')
                    .map(|l| {
                        let r = start..start + l.len();
                        start = r.end + 1;
                        (r, 0.0, 0.0)
                    })
                    .collect()
            }
        };
        let x = |b: usize, row_end: usize| geom.map_or(0.0, |g| g.caret_rect(b, b == row_end).min.x);
        let mut runs = Vec::new();
        let mut words = word_starts.iter().copied().peekable();
        for (row, (range, top, height)) in rows.into_iter().enumerate() {
            let range = range.start.min(text.len())..range.end.min(text.len());
            let feed = text.as_bytes().get(range.end) == Some(&b'\n');
            let mut chars: Vec<(usize, u8)> = characters(&text[range.clone()]).into_iter().map(|(b, l)| (range.start + b, l)).collect();
            let mut xs: Vec<f32> = chars.iter().map(|&(b, _)| x(b, range.end)).collect();
            let row_x1 = x(range.end, range.end);
            let mut ws: Vec<f32> = (0..chars.len()).map(|k| (xs.get(k + 1).copied().unwrap_or(row_x1) - xs[k]).max(0.0)).collect();
            if feed {
                chars.push((range.end, 1));
                xs.push(row_x1);
                ws.push(0.0);
            }
            let x0 = xs.first().copied().unwrap_or(row_x1);
            // the row's word starts, as character indices
            while words.peek().is_some_and(|&w| w < range.start) {
                words.next();
            }
            let mut starts = Vec::new();
            while let Some(&w) = words.peek().filter(|&&w| w < range.end) {
                if let Ok(k) = chars.binary_search_by_key(&w, |c| c.0) {
                    starts.push(k);
                }
                words.next();
            }
            let n = chars.len();
            let chunks = n.div_ceil(MAX_RUN_CHARS).max(1);
            for chunk in 0..chunks {
                let (a, b) = (chunk * MAX_RUN_CHARS, ((chunk + 1) * MAX_RUN_CHARS).min(n));
                let last = chunk + 1 == chunks;
                let start = chars.get(a).map_or(range.end, |c| c.0);
                let end = if last { range.end } else { chars[b].0 };
                let left = if a < n { xs[a] } else { x0 };
                let right = if b > a { xs[b - 1] + ws[b - 1] } else { left };
                let mut t = text[start..end].to_string();
                if last && feed {
                    t.push('\n');
                }
                runs.push(TextRun {
                    text: t,
                    range: start..end,
                    feed: last && feed,
                    lengths: chars[a..b].iter().map(|c| c.1).collect(),
                    positions: xs[a..b].iter().map(|p| p - left).collect(),
                    widths: ws[a..b].to_vec(),
                    word_starts: starts.iter().filter(|&&k| k >= a && k < b).map(|&k| (k - a) as u8).collect(),
                    rect: Rect::new(left, top, right, top + height),
                    row,
                    chunk,
                    chunks,
                });
            }
        }
        if runs.is_empty() {
            runs.push(TextRun {
                text: String::new(),
                range: 0..0,
                feed: false,
                lengths: Vec::new(),
                positions: Vec::new(),
                widths: Vec::new(),
                word_starts: Vec::new(),
                rect: Rect::ZERO,
                row: 0,
                chunk: 0,
                chunks: 1,
            });
        }
        TextRanges { runs }
    }

    /// Shown byte `s` as `(run, character)`. A position between two chunks of a row ends the earlier
    /// chunk (egui's rule); one at a soft wrap starts the next row; the end of the text ends the last
    /// run.
    pub fn position(&self, s: usize) -> (usize, usize) {
        let n = self.runs.len();
        for (k, r) in self.runs.iter().enumerate() {
            let next_chunk = self.runs.get(k + 1).is_some_and(|q| q.row == r.row);
            if s < r.end() || (s == r.end() && (k + 1 == n || next_chunk)) {
                let s = s.max(r.range.start);
                let mut at = r.range.start;
                let mut c = 0;
                for &l in &r.lengths {
                    if at + l as usize > s {
                        break;
                    }
                    at += l as usize;
                    c += 1;
                }
                return (k, c);
            }
        }
        (n - 1, self.runs[n - 1].lengths.len())
    }

    /// The shown byte of character `c` of run `k` (clamped to the run's end).
    pub fn byte(&self, k: usize, c: usize) -> usize {
        let Some(r) = self.runs.get(k) else { return self.runs.last().map_or(0, TextRun::end) };
        let c = c.min(r.lengths.len());
        r.range.start + r.lengths[..c].iter().map(|&l| l as usize).sum::<usize>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::shim::MonoGeometry;

    #[test]
    fn rows_chunks_feeds_and_the_empty_text() {
        let g = MonoGeometry::new("ab\ncd", 10.0, 20.0, None);
        let r = TextRanges::build("ab\ncd", Some(&g), &[0, 3]);
        assert_eq!(r.runs.len(), 2);
        assert_eq!(r.runs[0].text, "ab\n");
        assert_eq!(r.runs[0].lengths, vec![1, 1, 1]);
        assert_eq!(r.runs[0].positions, vec![0.0, 10.0, 20.0]);
        assert_eq!(r.runs[0].widths, vec![10.0, 10.0, 0.0], "the line feed has no width");
        assert_eq!(r.runs[1].rect, Rect::new(0.0, 20.0, 20.0, 40.0));
        assert_eq!(r.runs[1].word_starts, vec![0]);
        assert_eq!(r.position(2), (0, 2), "before the feed");
        assert_eq!(r.position(3), (1, 0), "after it");
        assert_eq!(r.position(5), (1, 2));
        assert_eq!(r.byte(1, 1), 4);

        let e = TextRanges::build("", Some(&MonoGeometry::new("", 10.0, 20.0, None)), &[]);
        assert_eq!(e.runs.len(), 1);
        assert_eq!(e.runs[0].text, "");
        assert_eq!(e.position(0), (0, 0));

        let long = "x".repeat(600);
        let r = TextRanges::build(&long, Some(&MonoGeometry::new(&long, 1.0, 20.0, None)), &[]);
        assert_eq!(r.runs.iter().map(|r| r.lengths.len()).collect::<Vec<_>>(), vec![255, 255, 90]);
        assert_eq!(r.runs[1].rect.min.x, 255.0);
        assert_eq!(r.runs[1].positions[0], 0.0);
        assert_eq!(r.position(255), (0, 255), "a chunk boundary ends the earlier chunk");
        assert_eq!(r.position(256), (1, 1));
        assert_eq!(r.position(600), (2, 90));
        assert_eq!(r.byte(2, 0), 510);
    }

    #[test]
    fn graphemes_are_characters_and_soft_wraps_start_the_next_row() {
        let t = "e\u{301}fgh";
        let g = MonoGeometry::new(t, 10.0, 20.0, Some(20.0));
        let r = TextRanges::build(t, Some(&g), &[]);
        // two columns a row (the mono layout counts chars): "é" | "fg" | "h"
        assert_eq!(r.runs[0].lengths, vec![3], "é as e + combining acute is one character");
        // the soft wrap after "é": the boundary byte starts the next row
        let wrap = r.runs[0].range.end;
        assert_eq!(r.position(wrap), (1, 0));
        assert!(!r.runs[0].feed);
    }
}
