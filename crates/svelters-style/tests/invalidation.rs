//! Dependency maps: what a change reaches, checked against a full re-match.

mod common;

use common::Tree;
use std::collections::BTreeSet;
use svelters_style::*;

fn inv(css: &str) -> Invalidation {
    let mut stylist = Stylist::new(&tables::EGUI);
    stylist.add_sheet(&Stylesheet::parse(css, &ParseOptions::default()), Origin::Author);
    stylist.invalidation().clone()
}

#[test]
fn reach_by_position() {
    let i = inv(".a { gap: 1px } .b .c { gap: 1px } .d + .e { gap: 1px } .f:has(.g) { gap: 1px } .h:has(.i) .j { gap: 1px } :is(.k .l) { gap: 1px }");
    assert_eq!(i.class_changed("a"), Reach::SELF);
    assert_eq!(i.class_changed("c"), Reach::SELF);
    assert_eq!(i.class_changed("b"), Reach::DESCENDANTS);
    assert_eq!(i.class_changed("d"), Reach::LATER_SIBLINGS);
    assert_eq!(i.class_changed("f"), Reach::SELF);
    assert_eq!(i.class_changed("g"), Reach::ANCESTORS);
    assert_eq!(i.class_changed("i"), Reach::TREE);
    assert_eq!(i.class_changed("h"), Reach::DESCENDANTS);
    assert_eq!(i.class_changed("k"), Reach::DESCENDANTS);
    assert_eq!(i.class_changed("l"), Reach::SELF);
    assert_eq!(i.class_changed("zzz"), Reach::NONE);
    assert!(i.structure_changed().contains(Reach::TREE));
}

#[test]
fn states_attrs_ids() {
    let i = inv("button:hover { gap: 1px } .row:focus-within label { gap: 1px } [value=\"3\"] { gap: 1px } #main > * { gap: 1px }");
    assert_eq!(i.state_changed(StateMask::NONE, StateMask::HOVERED), Reach::SELF);
    assert_eq!(i.state_changed(StateMask::FOCUS_WITHIN, StateMask::NONE), Reach::DESCENDANTS);
    assert_eq!(i.state_changed(StateMask::NONE, StateMask::CHECKED), Reach::NONE);
    assert_eq!(i.states_used(), StateMask::HOVERED | StateMask::FOCUS_WITHIN);
    assert!(i.uses_attr("value") && !i.uses_attr("text"));
    assert_eq!(i.attr_changed("value"), Reach::SELF);
    assert_eq!(i.id_changed("main"), Reach::DESCENDANTS);
    assert_eq!(i.structure_changed(), Reach::NONE);
    assert_eq!(inv("li:nth-child(2) { gap: 1px }").structure_changed(), Reach::DESCENDANTS);
}

const TREE: &str = "
window#root
  box#a
    label#a1
    label#a2
      label#a2x
    label#a3
  box#b
    label#b1
";

fn reached(reach: Reach, id: &str) -> BTreeSet<String> {
    let tree = Tree::parse(TREE);
    let mut out = BTreeSet::new();
    for_each_reached(tree.by_id(id), reach, &mut |e| {
        out.insert(e.tree.nodes[e.ix].id.clone().unwrap());
    });
    out
}

fn set(ids: &[&str]) -> BTreeSet<String> {
    ids.iter().map(|s| s.to_string()).collect()
}

#[test]
fn walks() {
    assert_eq!(reached(Reach::SELF, "a2"), set(&["a2"]));
    assert_eq!(reached(Reach::DESCENDANTS, "a"), set(&["a1", "a2", "a2x", "a3"]));
    assert_eq!(reached(Reach::LATER_SIBLINGS, "a1"), set(&["a2", "a2x", "a3"]));
    assert_eq!(reached(Reach::ANCESTORS, "a2x"), set(&["a2", "a1", "a", "root"]));
    assert_eq!(reached(Reach::TREE, "b1").len(), 8);
}

/// Every change the map calls NONE leaves every node's matched rules unchanged.
#[test]
fn none_means_no_rematch() {
    let css = ".x .y { gap: 1px } .p + .q { gap: 1px } .h:has(> .k) { gap: 1px } .s:hover { gap: 1px }";
    let mut stylist = Stylist::new(&tables::EGUI);
    stylist.add_sheet(&Stylesheet::parse(css, &ParseOptions::default()), Origin::Author);
    let i = stylist.invalidation().clone();
    for class in ["x", "y", "p", "q", "h", "k", "s", "unrelated"] {
        let mut tree = Tree::parse(TREE);
        let before: Vec<_> = tree.all().map(|e| stylist.matched_rules(e, &mut MatchCaches::new())).collect();
        tree.nodes[2].classes.push(class.into()); // a1
        let reach = i.class_changed(class);
        let mut hit = BTreeSet::new();
        for_each_reached(tree.el(2), reach, &mut |e| {
            hit.insert(e.ix);
        });
        for (ix, b) in before.iter().enumerate() {
            let after = stylist.matched_rules(tree.el(ix), &mut MatchCaches::new());
            if &after != b {
                assert!(hit.contains(&ix), "adding .{class} to a1 changed node {ix} outside the reach {reach:?}");
            }
        }
    }
}
