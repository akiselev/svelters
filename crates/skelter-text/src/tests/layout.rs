//! Shaping, breaking, ellipsis, tabs, measurement and the caches.

use super::{lay, plain, style, system};
use crate::{Align, RichText, Span, TextStyle};
use skelter_base::text::{TextMeasure, TextSpec};

#[test]
fn bundled_font_shapes_latin_in_one_run() {
    let mut ts = system();
    let l = lay(&mut ts, "Hello", 13.0, None);
    assert_eq!(l.runs().len(), 1);
    let run = &l.runs()[0];
    assert_eq!(run.run.glyphs.len(), 5);
    assert_eq!(run.range, 0..5);
    assert!(run.run.advance > 20.0 && run.run.advance < 50.0, "advance {}", run.run.advance);
    assert!(run.run.glyphs.windows(2).all(|g| g[1].x > g[0].x));
    assert!(l.size().h > 13.0);
    assert_eq!(l.lines().len(), 1);
}

#[test]
fn hebrew_falls_back_to_the_bundled_hebrew_face() {
    let mut ts = system();
    let l = lay(&mut ts, "abc \u{5e9}\u{5dc}\u{5d5}\u{5dd}", 13.0, None);
    let fonts: std::collections::HashSet<_> = l.runs().iter().map(|r| r.run.font).collect();
    assert_eq!(fonts.len(), 2, "Latin and Hebrew faces: {:?}", l.runs());
    // No .notdef glyphs.
    assert!(l.runs().iter().all(|r| r.run.glyphs.iter().all(|g| g.id != 0)));
}

#[test]
fn content_widths_bound_the_layout() {
    let mut ts = system();
    let rt = plain("alpha beta gamma", 13.0);
    let (min, max) = ts.content_widths(&rt);
    assert!(min > 0.0 && min < max, "min {min} max {max}");
    let one_line = ts.layout(&rt, None).size().w;
    assert!((one_line - max).abs() < 0.5, "max-content {max} vs unwrapped width {one_line}");
    let narrow = ts.layout(&rt, Some(min));
    assert_eq!(narrow.lines().len(), 3);
    assert!(narrow.size().w <= min + 0.5);
}

#[test]
fn text_measure_matches_layout() {
    let mut ts = system();
    let spec = TextSpec {
        text: "measure me",
        family: "",
        size: 13.0,
        weight: 400,
        italic: false,
        line_height: None,
        letter_spacing: 0.0,
        wrap: true,
        direction: None,
    };
    let s = TextMeasure::measure(&mut ts, &spec, None);
    assert_eq!(s, lay(&mut ts, "measure me", 13.0, None).size());
    let (min, max) = TextMeasure::content_widths(&mut ts, &spec);
    assert!(min <= max);
}

#[test]
fn wrapping_breaks_at_spaces() {
    let mut ts = system();
    let l = lay(&mut ts, "one two three four five six", 13.0, Some(60.0));
    assert!(l.lines().len() >= 3);
    for line in l.lines() {
        assert!(line.right - line.left <= 60.0 + 4.0, "{line:?}");
    }
    // Lines cover the text without gaps.
    for w in l.lines().windows(2) {
        assert_eq!(w[0].range.end, w[1].range.start);
    }
}

#[test]
fn ellipsis_cuts_a_nowrap_line_to_the_width() {
    let mut ts = system();
    let mut rt = RichText::plain("A long label that does not fit", TextStyle { wrap: false, ..style(13.0) });
    rt.ellipsis = true;
    let l = ts.layout(&rt, Some(80.0));
    assert!(l.truncated());
    assert_eq!(l.lines().len(), 1);
    assert!(l.size().w <= 80.0 + 0.01, "{}", l.size().w);
    let last = l.runs().last().unwrap();
    assert_eq!(last.range, rt.text.len()..rt.text.len(), "the ellipsis run comes last");
    assert_eq!(last.run.glyphs.len(), 1);
    // Without ellipsis the line overflows instead.
    rt.ellipsis = false;
    let l = ts.layout(&rt, Some(80.0));
    assert!(!l.truncated());
    assert!(l.size().w > 80.0);
}

/// POL2-REGISTER R-14: one wrapping line cut with "…" is cut where the width runs out, not at the
/// last break opportunity (a hyphen) before it; a short text keeps its alignment in the width.
#[test]
fn a_one_line_ellipsis_cuts_at_the_width_not_at_a_break() {
    let mut ts = system();
    let mut rt = plain("usb-cpowerdeliverycontroller.kicad_sch", 13.0);
    rt.max_lines = Some(1);
    rt.ellipsis = true;
    let l = ts.layout(&rt, Some(150.0));
    assert_eq!(l.lines().len(), 1);
    assert!(l.truncated());
    let w = l.size().w;
    assert!(w <= 150.0 + 0.01 && w > 150.0 - 13.0, "the cut fills the width: {w}");
    let mut short = plain("ok", 13.0);
    short.max_lines = Some(1);
    short.ellipsis = true;
    short.align = crate::Align::Center;
    let l = ts.layout(&short, Some(100.0));
    let r = l.runs().first().unwrap();
    assert!(r.origin.x > 40.0, "centred in the width: {}", r.origin.x);
}

#[test]
fn max_lines_drops_lines_and_ends_in_an_ellipsis() {
    let mut ts = system();
    let mut rt = plain("one two three four five six seven eight", 13.0);
    rt.max_lines = Some(2);
    rt.ellipsis = true;
    let l = ts.layout(&rt, Some(70.0));
    assert_eq!(l.lines().len(), 2);
    assert!(l.truncated());
    assert_eq!(l.runs().last().unwrap().range.start, rt.text.len());
    rt.ellipsis = false;
    let l = ts.layout(&rt, Some(70.0));
    assert_eq!(l.lines().len(), 2);
    assert!(l.truncated());
}

#[test]
fn tabs_advance_to_tab_stops() {
    let mut ts = system();
    let mut rt = plain("a\tb\tc", 13.0);
    rt.tab_size = Some(4.0);
    let l = ts.layout(&rt, None);
    let space = lay(&mut ts, " ", 13.0, None).size().w;
    let stop = space * 4.0;
    let x_of = |byte: u32| l.clusters.iter().find(|c| c.range.start == byte).unwrap().x0;
    let (b, c) = (x_of(2), x_of(4));
    assert!((b - stop).abs() < 0.01, "b at {b}, stop {stop}");
    assert!((c - 2.0 * stop).abs() < 0.01, "c at {c}, stop {stop}");
    // Tabs draw nothing.
    let glyphs: usize = l.runs().iter().map(|r| r.run.glyphs.len()).sum();
    assert_eq!(glyphs, 3);
}

#[test]
fn paint_ids_change_without_reshaping() {
    let mut ts = system();
    let mut rt = plain("hover me", 13.0);
    rt.spans.push(Span { range: 0..5, style: TextStyle { paint: 7, ..style(13.0) } });
    let a = ts.layout(&rt, None);
    rt.spans[0].style.paint = 8;
    rt.base.paint = 3;
    let b = ts.layout(&rt, None);
    let st = ts.stats();
    assert_eq!(st.shaped, 1, "{st:?}");
    assert_eq!(st.placed, 2);
    assert_eq!(a.runs()[0].style, 7);
    assert_eq!(b.runs()[0].style, 8);
    assert_eq!(b.runs().last().unwrap().style, 3);
    assert_eq!(a.runs()[0].run.glyphs, b.runs()[0].run.glyphs);
    // The same request again is a placed-layout hit.
    let c = ts.layout(&rt, None);
    assert!(std::sync::Arc::ptr_eq(&b, &c));
    assert_eq!(ts.stats().place_hits, 1);
}

#[test]
fn rewrapping_reuses_the_shaped_paragraph() {
    let mut ts = system();
    let rt = plain("several words to wrap around", 13.0);
    for w in [40.0, 80.0, 120.0, 40.0] {
        ts.layout(&rt, Some(w));
    }
    let st = ts.stats();
    assert_eq!(st.shaped, 1, "{st:?}");
}

#[test]
fn old_generations_are_evicted_over_budget() {
    let mut ts = system();
    ts.set_cache_budget(0);
    lay(&mut ts, "first", 13.0, None);
    ts.end_frame();
    lay(&mut ts, "second", 13.0, None);
    ts.end_frame();
    // "first" was last used two generations ago; "second" one generation ago.
    ts.end_frame();
    let st = ts.stats();
    assert!(st.evicted >= 1, "{st:?}");
    assert!(st.entries <= 1, "{st:?}");
}

#[test]
fn bold_span_uses_the_bold_face() {
    let mut ts = system();
    let mut rt = plain("normal bold", 13.0);
    rt.spans.push(Span { range: 7..11, style: TextStyle { weight: 700, ..style(13.0) } });
    let l = ts.layout(&rt, None);
    let fonts: std::collections::HashSet<_> = l.runs().iter().map(|r| r.run.font).collect();
    assert_eq!(fonts.len(), 2, "{:?}", l.runs());
    assert!(l.runs().iter().all(|r| r.run.embolden == 0.0), "a real bold face needs no synthesis");
}

#[test]
fn center_alignment_offsets_lines() {
    let mut ts = system();
    let mut rt = plain("mid", 13.0);
    rt.align = Align::Center;
    let l = ts.layout(&rt, Some(200.0));
    let line = &l.lines()[0];
    let w = line.right - line.left;
    assert!((line.left - (200.0 - w) / 2.0).abs() < 0.5, "{line:?}");
}

#[test]
fn empty_text_has_one_line_of_height() {
    let mut ts = system();
    let l = lay(&mut ts, "", 13.0, None);
    assert!(l.runs().is_empty());
    assert!(l.size().h > 0.0);
    let caret = l.caret_rect(0);
    assert!(caret.height() > 0.0);
}

#[cfg(feature = "core")]
#[test]
fn skelter_core_shaper_matches_layout() {
    use skelter_core::TextShaper;
    let mut ts = system();
    let spec = TextSpec {
        text: "Tab title",
        family: "",
        size: 13.0,
        weight: 400,
        italic: false,
        line_height: None,
        letter_spacing: 0.0,
        wrap: true,
        direction: None,
    };
    let shaped = TextShaper::shape(&mut ts, &spec, None);
    let l = lay(&mut ts, "Tab title", 13.0, None);
    assert_eq!(shaped.size, l.size());
    assert_eq!(shaped.runs.len(), l.runs().len());
    for ((run, origin), placed) in shaped.runs.iter().zip(l.runs()) {
        assert_eq!(**run, *placed.run);
        assert_eq!(*origin, placed.origin);
    }
    assert!(!shaped.runs.is_empty() && !shaped.runs[0].0.glyphs.is_empty());
    // A document can own it as its shaper.
    let _boxed: Box<dyn TextShaper> = Box::new(system());
}
