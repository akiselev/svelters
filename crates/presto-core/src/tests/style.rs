//! Style on the tree: sheets, classes, state pseudo-classes and inheritance, with only the work each
//! computed change needs.

use super::*;
use crate::{Dirty, StateMask, StyleInput, Value};
use presto_base::color::{Color, Fill};

fn classes(names: &[&str]) -> StyleInput {
    StyleInput { classes: names.iter().map(|&c| c.into()).collect(), ..Default::default() }
}

#[test]
fn sheets_and_classes_compute() {
    let (mut d, root) = doc();
    d.add_sheet(".card { background-color: #0000ff; border-radius: 4px; padding: 8px } box text { font-size: 20px }");
    let a = child(&mut d, root, BOX, "");
    d.set_style(a, classes(&["card"]));
    let t = child(&mut d, a, TEXT, "");
    frame(&mut d);
    let c = d.computed(a).unwrap();
    assert_eq!(c.background, Fill::Solid(Color::rgba(0.0, 0.0, 1.0, 1.0)));
    assert_eq!(c.radii.tl, 4.0);
    assert_eq!(d.computed(t).unwrap().font_size, 20.0);
    // padding is layout: the text sits inside it
    assert_eq!(d.layout_rect(t).unwrap().min, Point::new(8.0, 8.0));
}

#[test]
fn inherited_values_reach_descendants() {
    let (mut d, root) = doc();
    let a = child(&mut d, root, BOX, "color: #00ff00; font-size: 18px");
    let b = child(&mut d, a, BOX, "");
    let t = child(&mut d, b, TEXT, "");
    frame(&mut d);
    assert_eq!(d.computed(t).unwrap().color, Color::rgba(0.0, 1.0, 0.0, 1.0));
    assert_eq!(d.computed(t).unwrap().font_size, 18.0);
    d.set_style(a, StyleInput { inline: Some("color: #ff0000; font-size: 18px".into()), ..Default::default() });
    frame(&mut d);
    assert_eq!(d.computed(t).unwrap().color, Color::rgba(1.0, 0.0, 0.0, 1.0));
}

#[test]
fn a_state_rule_restyles_with_paint_only() {
    let (mut d, root) = doc();
    d.add_sheet("toggle:hover { background-color: #ff0000 }");
    let g = child(&mut d, root, TOGGLE, "");
    frame(&mut d);
    d.set_state(g, StateMask::HOVERED, true);
    assert!(d.dirty(g).unwrap().contains(Dirty::STYLE), "the sheet tests :hover");
    let (c, stats) = d.frame_with_stats(W, Time(0));
    assert!(!stats.layout.full && stats.layout.boundaries == 0, "a colour change lays nothing out");
    assert_eq!(stats.recorded, 1);
    assert_eq!(c.unwrap().lists[0].0, g);
    assert_eq!(d.computed(g).unwrap().background, Fill::Solid(Color::rgba(1.0, 0.0, 0.0, 1.0)));
}

#[test]
fn a_state_no_rule_tests_restyles_nothing() {
    let (mut d, root) = doc();
    d.add_sheet("toggle:hover { background-color: #ff0000 }");
    let g = child(&mut d, root, TOGGLE, "");
    frame(&mut d);
    d.set_state(g, StateMask::SELECTED, true);
    assert!(!d.dirty(g).unwrap().contains(Dirty::STYLE));
}

#[test]
fn attribute_selectors_follow_props() {
    let (mut d, root) = doc();
    d.add_sheet("toggle[checked=\"true\"] { border-width: 2px; border-style: solid; border-color: #000000 }");
    let g = child(&mut d, root, TOGGLE, "");
    frame(&mut d);
    assert_eq!(d.computed(g).unwrap().border_widths.top, 0.0);
    d.set_prop(g, crate::testkinds::TOGGLE_CHECKED, Value::Bool(true)).unwrap();
    frame(&mut d);
    assert_eq!(d.computed(g).unwrap().border_widths.top, 2.0);
}

#[test]
fn removing_a_sheet_restyles() {
    let (mut d, root) = doc();
    let id = d.add_sheet("box { opacity: 0.5 }");
    let a = child(&mut d, root, BOX, "");
    frame(&mut d);
    assert_eq!(d.computed(a).unwrap().opacity, 0.5);
    d.remove_sheet(id);
    frame(&mut d);
    assert_eq!(d.computed(a).unwrap().opacity, 1.0);
}
