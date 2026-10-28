//! Inline direction (POL4-DIR-CORE): `dir`, the inherited CSS `direction`, `:dir()`, and the
//! restyles a change of either needs.

use super::*;
use crate::{Direction, Value, common};
use skelter_base::color::Color;

fn dir(d: &mut Document, n: NodeId, v: &str) {
    d.set_prop(n, common::DIR, Value::from(v)).unwrap();
}

/// The node's taffy layout (padding and margin as laid out).
fn tl(d: &Document, n: NodeId) -> taffy::Layout {
    d.layout[d.idx(n).unwrap()].layout
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

#[test]
fn an_rtl_scroller_starts_at_its_inline_start() {
    // Chrome's model: the content overflows to the left and the view starts at the right end;
    // offsets stay left-based (0 = the content's left edge) for the compositor and scroll bars
    let (mut d, root) = doc();
    let s = child(&mut d, root, BOX, "flex-direction: row; width: 100px; height: 20px; overflow-x: scroll");
    let a = child(&mut d, s, BOX, "width: 80px; height: 10px; flex-shrink: 0");
    let b = child(&mut d, s, BOX, "width: 80px; height: 10px; flex-shrink: 0");
    frame(&mut d);
    assert_eq!(d.scroll_offset(s).x, 0.0);
    assert_eq!(d.world_rect(a).unwrap().min.x, 0.0, "ltr: the first child at the left");
    dir(&mut d, s, "rtl");
    frame(&mut d);
    let g = d.scroll_geometry(s).unwrap();
    assert_eq!((g.content.w, g.offset.x), (160.0, 60.0), "at the right end of 160 in 100");
    assert_eq!(d.world_rect(a).unwrap().min.x, 20.0, "rtl: the first child at the right, in view");
    assert_eq!(d.world_rect(b).unwrap().min.x, -60.0, "the second overflows to the left");
    // `:state(sv.overflow-left)`: content lies beyond the left edge, none beyond the right
    let st = d.state(s);
    assert!(st.contains(crate::OVERFLOW_LEFT) && !st.contains(crate::OVERFLOW_RIGHT), "{st:?}");
    // scrolling to the left end shows the second child
    d.set_scroll_offset(s, Vec2::new(0.0, 0.0));
    frame(&mut d);
    assert_eq!(d.world_rect(b).unwrap().min.x, 0.0);
    assert!(d.state(s).contains(crate::OVERFLOW_RIGHT));
    // content growing keeps the distance from the inline start (here: the left end, 60 from it)
    d.set_scroll_offset(s, Vec2::new(60.0, 0.0));
    let c = child(&mut d, s, BOX, "width: 40px; height: 10px; flex-shrink: 0");
    frame(&mut d);
    let g = d.scroll_geometry(s).unwrap();
    assert_eq!((g.content.w, g.offset.x), (200.0, 100.0), "still at the right end");
    assert_eq!(d.world_rect(a).unwrap().min.x, 20.0);
    assert_eq!(d.world_rect(c).unwrap().min.x, -100.0);
}

#[test]
fn logical_sides_resolve_against_the_boxs_direction() {
    let (mut d, root) = doc();
    let row = child(&mut d, root, BOX, "flex-direction: row; width: 200px");
    let a = child(&mut d, row, BOX, "width: 20px; height: 10px; margin-inline-start: 5px; padding-inline: 1px 3px; border-inline-start: 2px solid #ff0000");
    frame(&mut d);
    let r = d.layout_rect(a).unwrap();
    assert_eq!((r.min.x, r.width()), (5.0, 20.0));
    let c = d.computed(a).unwrap();
    assert_eq!((c.border_widths.left, c.border_widths.right), (2.0, 0.0));
    assert_eq!(c.border_color, Color::rgba(1.0, 0.0, 0.0, 1.0));
    assert_eq!(tl(&d, a).padding.left, 1.0);
    dir(&mut d, row, "rtl");
    frame(&mut d);
    // the start is the right: margin, border and the start padding move there
    let r = d.layout_rect(a).unwrap();
    assert_eq!((r.max.x, r.width()), (195.0, 20.0));
    let c = d.computed(a).unwrap();
    assert_eq!((c.border_widths.left, c.border_widths.right), (0.0, 2.0));
    let l = tl(&d, a);
    assert_eq!((l.padding.left, l.padding.right), (3.0, 1.0));
    // physical sides stay physical
    d.set_style(a, StyleInput { inline: Some("width: 20px; height: 10px; margin-left: 7px".into()), ..Default::default() });
    frame(&mut d);
    assert_eq!(tl(&d, a).margin.left, 7.0);
}

#[test]
fn start_and_end_alignment_follow_the_direction() {
    let (mut d, root) = doc();
    let row = child(&mut d, root, BOX, "flex-direction: row; width: 200px; justify-content: end");
    let a = child(&mut d, row, BOX, "width: 20px; height: 10px");
    let col = child(&mut d, root, BOX, "width: 200px; align-items: start");
    let b = child(&mut d, col, BOX, "width: 20px; height: 10px");
    frame(&mut d);
    assert_eq!(d.layout_rect(a).unwrap().min.x, 180.0);
    assert_eq!(d.layout_rect(b).unwrap().min.x, 0.0);
    d.set_direction(Direction::Rtl);
    frame(&mut d);
    assert_eq!(d.layout_rect(a).unwrap().min.x, 0.0, "end of an rtl row: the left");
    assert_eq!(d.layout_rect(b).unwrap().min.x, 180.0, "start on an rtl column's cross axis: the right");
    // (grid columns: taffy runs them from the right in `rtl`; core's CSS has no grid templates,
    // the grid kinds set them)
}

#[test]
fn text_align_start_is_the_inline_start() {
    use crate::style::TextAlign;
    let (mut d, root) = doc();
    let a = child(&mut d, root, BOX, "");
    let end = child(&mut d, a, BOX, "text-align: end");
    let left = child(&mut d, a, BOX, "text-align: left");
    frame(&mut d);
    assert_eq!(d.computed(a).unwrap().text_align, TextAlign::Start, "the default");
    assert_eq!(d.computed(a).unwrap().physical_text_align(), TextAlign::Left);
    assert_eq!(d.computed(end).unwrap().physical_text_align(), TextAlign::Right);
    dir(&mut d, a, "rtl");
    frame(&mut d);
    assert_eq!(d.computed(a).unwrap().physical_text_align(), TextAlign::Right);
    assert_eq!(d.computed(end).unwrap().physical_text_align(), TextAlign::Left);
    assert_eq!(d.computed(left).unwrap().physical_text_align(), TextAlign::Left, "physical stays");
    // the measure spec carries an rtl box's base direction
    assert_eq!(d.computed(a).unwrap().text_spec("x").direction, Some(Direction::Rtl));
}

/// The style pass's cost with direction (POL4-DIR-CORE; not a gate): a full restyle of 5,000 nodes,
/// plain and with `dir=rtl` on every tenth box. `cargo test -p skelter-core --release --lib -- --ignored
/// style_pass_cost --nocapture`.
#[test]
#[ignore]
fn style_pass_cost() {
    for with_dir in [false, true] {
        let (mut d, root) = doc();
        d.add_sheet(".a { color: #ff0000; padding: 2px } .a .b { font-size: 12px } .x .c { margin-left: 4px }");
        let mut parents = vec![root];
        for k in 0..5000 {
            let p = parents[k / 4];
            let n = child(&mut d, p, BOX, "");
            d.set_style(n, StyleInput { classes: vec![["a", "b", "c"][k % 3].into()], ..Default::default() });
            if with_dir && k % 10 == 0 {
                dir(&mut d, n, if k % 20 == 0 { "rtl" } else { "ltr" });
            }
            parents.push(n);
        }
        frame(&mut d);
        let mut best = f64::MAX;
        for _ in 0..30 {
            let t0 = std::time::Instant::now();
            d.restyle_all();
            d.frame_with_stats(W, Time(0));
            best = best.min(t0.elapsed().as_secs_f64());
        }
        eprintln!("dir {with_dir}: {:.2} ms (best of 30) per full restyle + layout of 5000 nodes", best * 1e3);
    }
}
