//! Span edits over source text (a small MagicString): Svelte's own transforms rewrite the AST and
//! reprint it; we instead keep the user's JavaScript text and splice runtime calls into it, which
//! preserves formatting, comments and evaluation order, and makes source maps exact.
//!
//! Ordering at one position follows pre-order visiting (outer node first):
//! - `open(pos, ..)` texts attach to what follows `pos`; earlier-added opens come first.
//! - `close(pos, ..)` texts attach to what precedes `pos`; later-added closes come first, so an
//!   inner node's close precedes its parent's.
//! - All closes at a position come before all opens there.
//!
//! `overwrite(start, end, ..)` replaces a range; edits strictly inside it are dropped.

use super::writer::Writer;

#[derive(Default)]
pub struct Edits {
    opens: Vec<(u32, usize, String)>,
    closes: Vec<(u32, usize, String)>,
    overwrites: Vec<(u32, u32, String)>,
    /// (start, end, open seq, close seq, prefix, suffix)
    wraps: Vec<(u32, u32, usize, usize, String, String)>,
    seq: usize,
}

impl Edits {
    pub fn new() -> Edits {
        Edits::default()
    }

    pub fn open(&mut self, pos: u32, text: impl Into<String>) {
        self.seq += 1;
        self.opens.push((pos, self.seq, text.into()));
    }

    pub fn close(&mut self, pos: u32, text: impl Into<String>) {
        self.seq += 1;
        self.closes.push((pos, self.seq, text.into()));
    }

    pub fn overwrite(&mut self, start: u32, end: u32, text: impl Into<String>) {
        self.overwrites.push((start, end, text.into()));
    }

    /// Wraps `[start, end)` as `prefix … suffix`.
    /// A wrap lying inside an overwritten range is dropped as a whole.
    pub fn wrap(&mut self, start: u32, end: u32, prefix: impl Into<String>, suffix: impl Into<String>) {
        self.seq += 2;
        self.wraps.push((start, end, self.seq - 1, self.seq, prefix.into(), suffix.into()));
    }

    pub fn is_empty(&self) -> bool {
        self.opens.is_empty() && self.closes.is_empty() && self.overwrites.is_empty() && self.wraps.is_empty()
    }

    /// Writes `source[start..end]` with the edits applied.
    pub fn emit(&self, w: &mut Writer, source: &str, start: u32, end: u32) {
        let mut ow: Vec<&(u32, u32, String)> = self.overwrites.iter().filter(|o| o.0 >= start && o.1 <= end).collect();
        ow.sort_by_key(|o| (o.0, std::cmp::Reverse(o.1)));
        // drop overwrites nested in earlier ones
        let mut kept: Vec<&(u32, u32, String)> = Vec::new();
        for o in ow {
            if kept.last().is_none_or(|k| o.0 >= k.1) {
                kept.push(o);
            }
        }
        let inside = |p: u32| kept.iter().any(|k| p > k.0 && p < k.1);
        let mut opens_all: Vec<(u32, usize, String)> = self.opens.clone();
        let mut closes_all: Vec<(u32, usize, String)> = self.closes.clone();
        for w in &self.wraps {
            let covered = kept.iter().any(|k| w.0 >= k.0 && w.1 <= k.1 && (w.0, w.1) != (k.0, k.1));
            if !covered {
                opens_all.push((w.0, w.2, w.4.clone()));
                closes_all.push((w.1, w.3, w.5.clone()));
            }
        }
        let mut points: Vec<u32> = opens_all
            .iter()
            .chain(closes_all.iter())
            .map(|e| e.0)
            .filter(|&p| p >= start && p <= end && !inside(p))
            .chain(kept.iter().flat_map(|k| [k.0, k.1]))
            .collect();
        points.sort_unstable();
        points.dedup();

        let mut cur = start;
        for p in points {
            if p > cur {
                w.copy(source, cur, p);
                cur = p;
            }
            let mut closes: Vec<&(u32, usize, String)> = closes_all.iter().filter(|e| e.0 == p).collect();
            closes.sort_by_key(|e| std::cmp::Reverse(e.1));
            for c in closes {
                w.write(&c.2);
            }
            if let Some(k) = kept.iter().find(|k| k.0 == p) {
                let mut opens: Vec<&(u32, usize, String)> = opens_all.iter().filter(|e| e.0 == p).collect();
                opens.sort_by_key(|e| e.1);
                for o in opens {
                    w.write_at(p, &o.2);
                }
                w.write_at(k.0, &k.2);
                cur = k.1;
                continue;
            }
            let mut opens: Vec<&(u32, usize, String)> = opens_all.iter().filter(|e| e.0 == p).collect();
            opens.sort_by_key(|e| e.1);
            for o in opens {
                w.write_at(p, &o.2);
            }
        }
        if end > cur {
            w.copy(source, cur, end);
        }
    }

    /// Convenience for tests and small expressions: the edited text without mappings.
    pub fn apply(&self, source: &str, start: u32, end: u32) -> String {
        let mut w = Writer::new(source);
        self.emit(&mut w, source, start, end);
        w.code().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_wraps_close_inner_first() {
        // `x = a` -> `$.set(x, $.get(a))`
        let src = "x = a";
        let mut e = Edits::new();
        e.overwrite(0, 4, "$.set(x, ");
        e.close(5, ")");
        e.wrap(4, 5, "$.get(", ")");
        assert_eq!(e.apply(src, 0, 5), "$.set(x, $.get(a))");
    }

    #[test]
    fn update_and_reads() {
        // `() => count++ + count` -> `() => $.update(count) + $.get(count)`
        let src = "() => count++ + count";
        let mut e = Edits::new();
        e.overwrite(6, 13, "$.update(count)");
        e.wrap(16, 21, "$.get(", ")");
        assert_eq!(e.apply(src, 0, src.len() as u32), "() => $.update(count) + $.get(count)");
    }

    #[test]
    fn edits_inside_overwrite_are_dropped_and_ranges_respected() {
        let src = "f(a, b)";
        let mut e = Edits::new();
        e.overwrite(2, 6, "z");
        e.wrap(2, 3, "$.get(", ")");
        assert_eq!(e.apply(src, 0, 7), "f(z)");
        assert_eq!(e.apply(src, 2, 6), "z");
    }

    #[test]
    fn adjacent_close_then_open() {
        let src = "a+b";
        let mut e = Edits::new();
        e.wrap(0, 1, "[", "]");
        e.wrap(1, 2, "<", ">");
        assert_eq!(e.apply(src, 0, 3), "[a]<+>b");
    }
}
