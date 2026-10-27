//! Style on the tree: sheets, classes, state pseudo-classes and inheritance, with only the work each
//! computed change needs.

use super::*;
use crate::{Dirty, StateMask, StyleInput, Value};
use skelter_base::color::{Color, Fill};

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
fn a_custom_property_change_restyles_descendants() {
    // SV65: `background-color` does not inherit, so only the changed custom map reaches the leaf
    let (mut d, root) = doc();
    d.add_sheet(".leaf { background-color: var(--accent) }");
    let a = child(&mut d, root, BOX, "--accent: #00ff00");
    let b = child(&mut d, a, BOX, "");
    let leaf = child(&mut d, b, BOX, "");
    d.set_style(leaf, classes(&["leaf"]));
    frame(&mut d);
    assert_eq!(d.computed(leaf).unwrap().background, Fill::Solid(Color::rgba(0.0, 1.0, 0.0, 1.0)));
    d.set_style(a, StyleInput { inline: Some("--accent: #ff0000".into()), ..Default::default() });
    frame(&mut d);
    assert_eq!(d.computed(leaf).unwrap().background, Fill::Solid(Color::rgba(1.0, 0.0, 0.0, 1.0)));
}

#[test]
fn a_custom_property_change_restyles_only_its_readers() {
    // SV69: the root's `--accent` changes; of 20 plain boxes only the one that reads it through
    // `var()` and the one whose own custom property derives from it restyle; the rest only take
    // the new inherited map
    let (mut d, root) = doc();
    d.add_sheet(".reads { background-color: var(--accent) } .derives { --edge: var(--accent) } .edge { border-color: var(--edge) }");
    let a = child(&mut d, root, BOX, "--accent: #00ff00; --other: 1px");
    let plain: Vec<NodeId> = (0..20).map(|_| child(&mut d, a, BOX, "")).collect();
    let reads = child(&mut d, a, BOX, "");
    d.set_style(reads, classes(&["reads"]));
    let derives = child(&mut d, a, BOX, "");
    d.set_style(derives, classes(&["derives"]));
    let edge = child(&mut d, derives, BOX, "border-width: 1px");
    d.set_style(edge, StyleInput { classes: vec!["edge".into()], inline: Some("border-width: 1px".into()), ..Default::default() });
    frame(&mut d);
    let green = Color::rgba(0.0, 1.0, 0.0, 1.0);
    let red = Color::rgba(1.0, 0.0, 0.0, 1.0);
    assert_eq!(d.computed(reads).unwrap().background, Fill::Solid(green));
    assert_eq!(d.computed(edge).unwrap().border_color, green);
    d.set_style(a, StyleInput { inline: Some("--accent: #ff0000; --other: 1px".into()), ..Default::default() });
    let (_, stats) = d.frame_with_stats(W, Time(0));
    // `a`, `reads`, `derives` and `edge` (it references `--edge`, which changed under `derives`)
    assert_eq!(stats.restyled, 4);
    assert_eq!(d.computed(reads).unwrap().background, Fill::Solid(red));
    assert_eq!(d.computed(edge).unwrap().border_color, red);
    // the others still see the new value
    let custom = |d: &Document, n: NodeId, name: &str| d.view(n).unwrap().css().and_then(|c| c.custom(name)).map(str::to_owned);
    for &p in &plain {
        assert_eq!(custom(&d, p, "--accent").as_deref(), Some("#ff0000"));
    }
    // a change nobody reads restyles `a` and `derives` (a node that declares custom properties
    // rebuilds its map on any change), not `edge`
    d.set_style(a, StyleInput { inline: Some("--accent: #ff0000; --other: 2px".into()), ..Default::default() });
    let (_, stats) = d.frame_with_stats(W, Time(0));
    assert_eq!(stats.restyled, 2);
    assert_eq!(custom(&d, plain[3], "--other").as_deref(), Some("2px"));
}

#[test]
fn set_root_tokens_edits_a_sheet_in_place() {
    // SV69: no rule-index rebuild; the nodes the edited rule matches restyle, and the change reaches
    // the readers below them
    let (mut d, root) = doc();
    let tokens = d.add_sheet(":root { --accent: #00ff00 } .panel { --tint: #0000ff }");
    d.add_sheet(".reads { background-color: var(--accent) } .tinted { color: var(--tint) }");
    let r = child(&mut d, root, BOX, "");
    d.set_style(r, classes(&["reads"]));
    let p = child(&mut d, root, BOX, "");
    d.set_style(p, classes(&["panel"]));
    let t = child(&mut d, p, TEXT, "");
    d.set_style(t, classes(&["tinted"]));
    frame(&mut d);
    let rebuilds = d.style_rebuilds();
    d.set_root_tokens(tokens, &[("--accent", Some("#ff0000")), ("--tint", Some("#ff00ff"))]).unwrap();
    let (_, stats) = d.frame_with_stats(W, Time(0));
    assert_eq!(d.style_rebuilds(), rebuilds);
    assert_eq!(d.computed(r).unwrap().background, Fill::Solid(Color::rgba(1.0, 0.0, 0.0, 1.0)));
    assert_eq!(d.computed(t).unwrap().color, Color::rgba(1.0, 0.0, 1.0, 1.0));
    // the root, `r` (reads), `p` (matched by `.panel`) and `t` (reads `--tint`)
    assert_eq!(stats.restyled, 4);
    // a new token goes into the last rule that declares one; removing it unsets it
    d.set_root_tokens(tokens, &[("--fresh", Some("3px"))]).unwrap();
    frame(&mut d);
    assert_eq!(d.view(t).unwrap().css().and_then(|c| c.custom("--fresh")), Some("3px"));
    d.set_root_tokens(tokens, &[("--tint", None)]).unwrap();
    frame(&mut d);
    assert_eq!(d.view(t).unwrap().css().and_then(|c| c.custom("--tint")), None);
    // refused, changing nothing
    assert!(d.set_root_tokens(tokens, &[("--accent", Some("#000000")), ("color", Some("red"))]).is_err());
    frame(&mut d);
    assert_eq!(d.computed(r).unwrap().background, Fill::Solid(Color::rgba(1.0, 0.0, 0.0, 1.0)));
}

#[test]
fn user_sheets_beat_author_rules_but_not_inline() {
    // SV70
    let (mut d, root) = doc();
    d.add_sheet_with_origin("box { background-color: #00ff00 }", crate::Origin::User);
    d.add_sheet("#a { background-color: #0000ff } #b { background-color: #0000ff }");
    let a = child(&mut d, root, BOX, "");
    d.set_style(a, StyleInput { id: Some("a".into()), ..Default::default() });
    let b = child(&mut d, root, BOX, "background-color: #ff0000");
    d.set_style(b, StyleInput { id: Some("b".into()), inline: Some("background-color: #ff0000".into()), ..Default::default() });
    frame(&mut d);
    assert_eq!(d.computed(a).unwrap().background, Fill::Solid(Color::rgba(0.0, 1.0, 0.0, 1.0)));
    assert_eq!(d.computed(b).unwrap().background, Fill::Solid(Color::rgba(1.0, 0.0, 0.0, 1.0)));
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

#[test]
fn skelter_properties_reach_the_computed_box() {
    let (mut d, root) = doc();
    let a = child(&mut d, root, BOX, "z-index: 3; visibility: hidden; pointer-events: none; will-change: transform; transform: translate(4px, 0); transform-origin: left top");
    frame(&mut d);
    let c = d.computed(a).unwrap();
    assert_eq!(c.z_index, Some(3));
    assert!(c.visibility_hidden);
    assert!(c.pointer_events_none);
    assert!(c.will_change);
    assert_eq!(c.transform.apply(skelter_base::geom::Point::ZERO), skelter_base::geom::Point::new(4.0, 0.0));
    assert_eq!(c.transform_origin, skelter_base::geom::Point::new(0.0, 0.0));
}

fn ms(n: u64) -> Time {
    Time(n * 1_000_000)
}

#[test]
fn paint_transitions_sample_on_the_ui_thread() {
    use crate::{ListenFlags, common};
    let (mut d, root) = doc();
    let base = "width: 20px; height: 20px; transition: background-color 100ms linear";
    let a = child(&mut d, root, BOX, &format!("{base}; background-color: #000000"));
    d.frame(W, ms(0));
    d.listen(a, common::TRANSITIONEND, ListenFlags::EMPTY);
    d.set_style(a, StyleInput { inline: Some(format!("{base}; background-color: #ffffff").into()), ..Default::default() });
    let bg = |d: &Document| match &d.computed(a).unwrap().background {
        Fill::Solid(c) => c.r,
        _ => panic!("solid"),
    };
    // the transition starts at the next frame, from the old value
    assert!(d.frame(W, ms(1000)).is_some());
    assert!(bg(&d) < 0.01, "{}", bg(&d));
    assert!(d.busy().contains(crate::Busy::ANIMATING));
    let c = d.frame(W, ms(1050)).expect("a frame re-records the node");
    assert!(!c.lists.is_empty());
    assert!((bg(&d) - 0.5).abs() < 0.05, "{}", bg(&d));
    // a restyle to the same target keeps it running
    d.set_state(a, StateMask::HOVERED, true);
    d.frame(W, ms(1075));
    assert!((bg(&d) - 0.75).abs() < 0.05, "{}", bg(&d));
    d.frame(W, ms(1100));
    assert!((bg(&d) - 1.0).abs() < 1e-3);
    assert!(!d.busy().contains(crate::Busy::ANIMATING));
    let ev = d.take_events();
    let end = ev.iter().find(|e| e.signal == common::TRANSITIONEND).expect("transitionend");
    assert_eq!(end.arg.as_ref().and_then(|v| v.field("propertyName")).and_then(Value::as_str), Some("background-color"));
}

#[test]
fn layout_transitions_relayout_each_frame_and_reverse_from_the_current_value() {
    let (mut d, root) = doc();
    let base = "height: 10px; transition: width 100ms linear";
    let a = child(&mut d, root, BOX, &format!("{base}; width: 10px"));
    d.frame(W, ms(0));
    d.set_style(a, StyleInput { inline: Some(format!("{base}; width: 30px").into()), ..Default::default() });
    d.frame(W, ms(1000));
    assert_eq!(d.layout_rect(a).unwrap().width(), 10.0);
    d.frame(W, ms(1050));
    assert!((d.layout_rect(a).unwrap().width() - 20.0).abs() < 0.6);
    // back to 10px: it starts from about 20, not from 30
    d.set_style(a, StyleInput { inline: Some(format!("{base}; width: 10px").into()), ..Default::default() });
    d.frame(W, ms(1060));
    let w = d.layout_rect(a).unwrap().width();
    assert!((w - 20.0).abs() < 1.1, "{w}");
    d.frame(W, ms(1200));
    assert_eq!(d.layout_rect(a).unwrap().width(), 10.0);
}

#[test]
fn a_change_without_a_transition_stops_a_running_one() {
    let (mut d, root) = doc();
    let a = child(&mut d, root, BOX, "width: 10px; height: 10px; color: #000000; transition: color 100ms linear");
    d.frame(W, ms(0));
    d.set_style(a, StyleInput { inline: Some("width: 10px; height: 10px; color: #ffffff; transition: color 100ms linear".into()), ..Default::default() });
    d.frame(W, ms(1000));
    d.frame(W, ms(1050));
    d.set_style(a, StyleInput { inline: Some("width: 10px; height: 10px; color: #ff0000".into()), ..Default::default() });
    d.frame(W, ms(1060));
    assert_eq!(d.computed(a).unwrap().color, Color::rgba(1.0, 0.0, 0.0, 1.0));
    assert!(!d.busy().contains(crate::Busy::ANIMATING));
}

/// A27: the host's provider states resolve `:state(ns.name)`, and a rule matches a node with the bit.
#[test]
fn a_state_rule_matches_the_hosts_state_bit() {
    let (mut d, root) = doc();
    let pending = StateMask(1 << 40);
    d.set_state_names(Box::new([("cad.pending", pending)]));
    d.add_sheet("box:state(cad.pending) { width: 33px }");
    let n = child(&mut d, root, BOX, "height: 10px");
    frame(&mut d);
    assert_ne!(d.world_rect(n).unwrap().width(), 33.0);
    d.set_state(n, pending, true);
    frame(&mut d);
    assert_eq!(d.world_rect(n).unwrap().width(), 33.0, "the rule applies with the bit set");
}

/// SV66: host-set media preferences match `prefers-*` and, with reduced motion, values jump.
#[test]
fn media_preferences_match_queries_and_reduced_motion_jumps() {
    let (mut d, root) = doc();
    d.add_sheet("@media (prefers-contrast: more) { .c { border-width: 3px } }");
    let base = "width: 20px; height: 20px; transition: background-color 100ms linear";
    let a = child(&mut d, root, BOX, &format!("{base}; background-color: #000000"));
    d.set_style(a, StyleInput { classes: vec!["c".into()], inline: Some(format!("{base}; background-color: #000000").into()), ..Default::default() });
    d.frame(W, ms(0));
    assert_eq!(d.computed(a).unwrap().border_widths.left, 0.0);
    d.set_media_preferences(crate::MediaPreferences::new(true, crate::Contrast::More));
    assert_eq!(d.media_preferences(), crate::MediaPreferences::new(true, crate::Contrast::More));
    d.frame(W, ms(10));
    assert_eq!(d.computed(a).unwrap().border_widths.left, 3.0);
    d.set_style(a, StyleInput { classes: vec!["c".into()], inline: Some(format!("{base}; background-color: #ffffff").into()), ..Default::default() });
    d.frame(W, ms(1000));
    assert_eq!(d.computed(a).unwrap().background, Fill::Solid(Color::rgba(1.0, 1.0, 1.0, 1.0)), "no transition");
    assert!(!d.busy().contains(crate::Busy::ANIMATING));
}
