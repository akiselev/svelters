//! Hit testing, carets and selections: LTR, RTL, mixed direction, graphemes, wrapped lines.

use super::{lay, system};
use crate::{Affinity, Cursor};
use presto_base::geom::Point;

const HEBREW: &str = "\u{5e9}\u{5dc}\u{5d5}\u{5dd}"; // שלום, 8 bytes

#[test]
fn ltr_caret_and_hit_test_round_trip() {
    let mut ts = system();
    let l = lay(&mut ts, "abcd", 13.0, None);
    let y = l.lines()[0].baseline - 2.0;
    assert!(l.caret_rect(0).min.x.abs() < 0.01);
    assert!((l.caret_rect(4).min.x - l.size().w).abs() < 0.5);
    for byte in 0..=4 {
        let r = l.caret_rect(byte);
        let hit = l.hit_test(Point::new(r.min.x + 0.1, y));
        assert_eq!(hit.byte, byte, "caret {byte} at {r:?}");
    }
    assert_eq!(l.hit_test(Point::new(-50.0, y)).byte, 0);
    assert_eq!(l.hit_test(Point::new(500.0, y)).byte, 4);
    // Carets increase left to right.
    let xs: Vec<f32> = (0..=4).map(|b| l.caret_rect(b).min.x).collect();
    assert!(xs.windows(2).all(|w| w[1] > w[0]), "{xs:?}");
}

#[test]
fn rtl_carets_run_right_to_left() {
    let mut ts = system();
    let l = lay(&mut ts, HEBREW, 13.0, None);
    assert!(l.is_rtl());
    let y = l.lines()[0].baseline - 2.0;
    let line = &l.lines()[0];
    // Logical start is at the right edge, logical end at the left.
    assert!((l.caret_rect(0).min.x - line.right).abs() < 0.5, "{:?} {line:?}", l.caret_rect(0));
    assert!((l.caret_rect(8).min.x - line.left).abs() < 0.5, "{:?} {line:?}", l.caret_rect(8));
    let xs: Vec<f32> = [0, 2, 4, 6, 8].iter().map(|b| l.caret_rect(*b).min.x).collect();
    assert!(xs.windows(2).all(|w| w[1] < w[0]), "{xs:?}");
    assert_eq!(l.hit_test(Point::new(line.left - 20.0, y)).byte, 8);
    assert_eq!(l.hit_test(Point::new(line.right + 20.0, y)).byte, 0);
}

#[test]
fn rtl_selection_is_one_rect() {
    let mut ts = system();
    let l = lay(&mut ts, HEBREW, 13.0, None);
    let rects = l.selection_rects(0..8);
    assert_eq!(rects.len(), 1, "{rects:?}");
    let line = &l.lines()[0];
    assert!((rects[0].width() - (line.right - line.left)).abs() < 0.5);
    // The first letter (logical) is the rightmost.
    let first = l.selection_rects(0..2);
    assert_eq!(first.len(), 1);
    assert!((first[0].max.x - line.right).abs() < 0.5, "{first:?}");
}

#[test]
fn mixed_direction_selection_splits_visually() {
    let mut ts = system();
    // "ab " then Hebrew then " cd": selecting "b " + the first Hebrew letter crosses a direction change.
    let text = format!("ab {HEBREW} cd");
    let l = lay(&mut ts, &text, 13.0, None);
    assert!(!l.is_rtl());
    let rects = l.selection_rects(1..5);
    assert_eq!(rects.len(), 2, "{rects:?}");
    // The whole line selects as one piece.
    let all = l.selection_rects(0..text.len());
    assert_eq!(all.len(), 1, "{all:?}");
}

#[test]
fn carets_never_split_a_grapheme() {
    let mut ts = system();
    // e + combining acute, then x.
    let l = lay(&mut ts, "e\u{301}x", 13.0, None);
    assert_eq!(l.caret_rect(1), l.caret_rect(0));
    let y = l.lines()[0].baseline - 2.0;
    for i in 0..40 {
        let hit = l.hit_test(Point::new(i as f32 * 0.5, y));
        assert!(hit.byte == 0 || hit.byte == 3 || hit.byte == 4, "{hit:?}");
    }
}

#[test]
fn wrapped_line_end_keeps_the_caret_upstream() {
    let mut ts = system();
    let l = lay(&mut ts, "alpha beta gamma", 13.0, Some(45.0));
    assert!(l.lines().len() >= 2);
    let first = &l.lines()[0];
    let y = (first.top + first.bottom) / 2.0;
    let hit = l.hit_test(Point::new(first.right + 30.0, y));
    assert_eq!(hit.affinity, Affinity::Upstream, "{hit:?}");
    // Upstream draws on the first line, downstream on the second.
    let up = l.caret_rect_at(Cursor::new(hit.byte, Affinity::Upstream));
    assert!(up.min.y < first.bottom, "{up:?}");
    let second = &l.lines()[1];
    let down = l.caret_rect_at(Cursor::new(second.range.start, Affinity::Downstream));
    assert!(down.min.y >= second.top - 0.01, "{down:?}");
}

#[test]
fn hard_break_lines_and_empty_lines() {
    let mut ts = system();
    let l = lay(&mut ts, "ab\n\ncd", 13.0, None);
    assert_eq!(l.lines().len(), 3);
    // The caret on the empty middle line sits at its left edge, on that line.
    let mid = &l.lines()[1];
    let c = l.caret_rect(3);
    assert!(c.min.y >= mid.top - 0.01 && c.max.y <= mid.bottom + 0.01, "{c:?} {mid:?}");
    // A click right of "ab" lands before the newline.
    let first = &l.lines()[0];
    let hit = l.hit_test(Point::new(200.0, (first.top + first.bottom) / 2.0));
    assert_eq!(hit.byte, 2);
    // Selection spans lines.
    let rects = l.selection_rects(0..6);
    assert_eq!(rects.len(), 2, "two non-empty lines: {rects:?}");
}

#[test]
fn hit_test_below_and_above_clamps_to_lines() {
    let mut ts = system();
    let l = lay(&mut ts, "one two three", 13.0, Some(40.0));
    let n = l.lines().len();
    assert!(n >= 2);
    assert_eq!(l.hit_test(Point::new(-5.0, -100.0)).byte, 0);
    let end = l.hit_test(Point::new(1000.0, 1000.0));
    assert_eq!(end.byte, l.lines()[n - 1].range.end);
}
