//! Inline direction (POL4-DIR-CORE): `dir`, the inherited CSS `direction`, `:dir()`, and the
//! restyles a change of either needs.

use super::*;
use crate::{Direction, Value, common};
use skelter_base::color::Color;

fn dir(d: &mut Document, n: NodeId, v: &str) {
    d.set_prop(n, common::DIR, Value::from(v)).unwrap();
}

fn text(d: &mut Document, n: NodeId, v: &str) {
    d.set_prop(n, testkinds::TEXT, Value::from(v)).unwrap();
}

#[test]
fn direction_inherits_from_a_dir() {
    let (mut d, root) = doc();
    let a = child(&mut d, root, BOX, "");
    let b = child(&mut d, a, BOX, "");
    let t = child(&mut d, b, TEXT, "");
    frame(&mut d);
    assert_eq!(d.node_direction(t), Some(Direction::Ltr), "the document's default");
    dir(&mut d, a, "rtl");
    frame(&mut d);
    for n in [a, b, t] {
        assert_eq!(d.node_direction(n), Some(Direction::Rtl));
    }
    assert_eq!(d.node_direction(root), Some(Direction::Ltr));
    // an unknown value is no `dir`
    dir(&mut d, a, "sideways");
    frame(&mut d);
    assert_eq!(d.node_direction(t), Some(Direction::Ltr));
}

#[test]
fn a_dir_beats_the_inherited_direction_and_author_css_beats_a_dir() {
    let (mut d, root) = doc();
    let a = child(&mut d, root, BOX, "");
    let b = child(&mut d, a, BOX, "");
    let t = child(&mut d, b, TEXT, "");
    dir(&mut d, a, "rtl");
    dir(&mut d, b, "LTR");
    frame(&mut d);
    assert_eq!(d.node_direction(a), Some(Direction::Rtl));
    assert_eq!(d.node_direction(b), Some(Direction::Ltr));
    assert_eq!(d.node_direction(t), Some(Direction::Ltr));
    // `direction` in a rule wins over the node's `dir` (a UA-level hint), and inherits
    d.set_style(b, StyleInput { inline: Some("direction: rtl".into()), ..Default::default() });
    frame(&mut d);
    assert_eq!(d.node_direction(b), Some(Direction::Rtl));
    assert_eq!(d.node_direction(t), Some(Direction::Rtl));
}

#[test]
fn the_document_direction_is_the_roots() {
    let (mut d, root) = doc();
    let a = child(&mut d, root, BOX, "");
    let t = child(&mut d, a, TEXT, "");
    frame(&mut d);
    d.set_direction(Direction::Rtl);
    assert_eq!(d.direction(), Direction::Rtl);
    frame(&mut d);
    assert_eq!(d.node_direction(root), Some(Direction::Rtl));
    assert_eq!(d.node_direction(t), Some(Direction::Rtl));
    d.set_direction(Direction::Ltr);
    frame(&mut d);
    assert_eq!(d.node_direction(t), Some(Direction::Ltr));
}

#[test]
fn dir_auto_reads_the_first_strong_character() {
    let (mut d, root) = doc();
    let a = child(&mut d, root, BOX, "");
    let inner = child(&mut d, a, BOX, "");
    let skipped = child(&mut d, inner, TEXT, "");
    let t = child(&mut d, inner, TEXT, "");
    dir(&mut d, a, "auto");
    // digits and punctuation are not strong; a descendant with its own `dir` is skipped
    text(&mut d, skipped, "Hello");
    dir(&mut d, skipped, "ltr");
    text(&mut d, t, "123 - \u{645}\u{631}\u{62d}\u{628}\u{627} hello");
    frame(&mut d);
    assert_eq!(d.node_direction(a), Some(Direction::Rtl));
    assert_eq!(d.node_direction(t), Some(Direction::Rtl));
    assert_eq!(d.node_direction(skipped), Some(Direction::Ltr));
    // the text changes: the `auto` node and its subtree follow
    text(&mut d, t, "42 hello \u{645}\u{631}\u{62d}\u{628}\u{627}");
    frame(&mut d);
    assert_eq!(d.node_direction(a), Some(Direction::Ltr));
    assert_eq!(d.node_direction(t), Some(Direction::Ltr));
    // no strong character: ltr
    text(&mut d, t, "1234");
    frame(&mut d);
    assert_eq!(d.node_direction(a), Some(Direction::Ltr));
    // a child arrives with Hebrew text first
    let first = d.create(TEXT);
    text(&mut d, first, "\u{5e9}\u{5dc}\u{5d5}\u{5dd}");
    d.insert(inner, first, Some(skipped), None).unwrap();
    frame(&mut d);
    assert_eq!(d.node_direction(a), Some(Direction::Rtl));
    assert_eq!(d.node_direction(t), Some(Direction::Rtl));
}

#[test]
fn dir_pseudo_class_matches_the_directionality() {
    let (mut d, root) = doc();
    d.add_sheet(":dir(rtl) { color: #ff0000 } :dir(ltr) { color: #00ff00 } [dir=rtl] > box { font-size: 20px }");
    let a = child(&mut d, root, BOX, "");
    let b = child(&mut d, a, BOX, "");
    let t = child(&mut d, b, TEXT, "");
    frame(&mut d);
    let (red, green) = (Color::rgba(1.0, 0.0, 0.0, 1.0), Color::rgba(0.0, 1.0, 0.0, 1.0));
    assert_eq!(d.computed(t).unwrap().color, green);
    dir(&mut d, a, "rtl");
    frame(&mut d);
    assert_eq!(d.computed(a).unwrap().color, red);
    assert_eq!(d.computed(t).unwrap().color, red);
    assert_eq!(d.computed(b).unwrap().font_size, 20.0);
    // `:dir()` is HTML's directionality, not the CSS property: `direction: ltr` does not change it
    d.set_style(b, StyleInput { inline: Some("direction: ltr".into()), ..Default::default() });
    frame(&mut d);
    assert_eq!(d.node_direction(t), Some(Direction::Ltr));
    assert_eq!(d.computed(t).unwrap().color, red);
}

#[test]
fn a_dir_change_restyles_the_subtree_only() {
    // the SV65/SV69 path: a change at `a` reaches every descendant (inherited) and nothing beside it
    let (mut d, root) = doc();
    let a = child(&mut d, root, BOX, "");
    let deep: Vec<NodeId> = (0..5).scan(a, |p, _| {
        let n = child(&mut d, *p, BOX, "");
        *p = n;
        Some(n)
    }).collect();
    let others: Vec<NodeId> = (0..10).map(|_| child(&mut d, root, BOX, "")).collect();
    frame(&mut d);
    dir(&mut d, a, "rtl");
    let (_, stats) = d.frame_with_stats(W, Time(0));
    assert_eq!(stats.restyled, 6, "a and its five descendants");
    assert_eq!(d.node_direction(*deep.last().unwrap()), Some(Direction::Rtl));
    assert_eq!(d.node_direction(others[0]), Some(Direction::Ltr));
    // the same value again: nothing
    dir(&mut d, a, "rtl");
    let (_, stats) = d.frame_with_stats(W, Time(0));
    assert_eq!(stats.restyled, 0);
}

#[test]
fn a_row_lays_out_from_the_inline_start() {
    let (mut d, root) = doc();
    let row = child(&mut d, root, BOX, "flex-direction: row; width: 200px");
    let first = child(&mut d, row, BOX, "width: 30px; height: 10px");
    let second = child(&mut d, row, BOX, "width: 40px; height: 10px");
    frame(&mut d);
    assert_eq!(d.layout_rect(first).unwrap().min.x, 0.0);
    assert_eq!(d.layout_rect(second).unwrap().min.x, 30.0);
    dir(&mut d, row, "rtl");
    frame(&mut d);
    assert_eq!(d.layout_rect(first).unwrap().min.x, 170.0, "the first child at the right");
    assert_eq!(d.layout_rect(second).unwrap().min.x, 130.0);
}
