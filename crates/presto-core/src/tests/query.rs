//! Queries: kind sets, predicates, scopes, anchors, `changed_since`, CSS selectors.

use super::*;
use crate::testkinds::TOGGLE_CHECKED;
use crate::{StateMask, Value, common};

#[test]
fn kind_sets_follow_create_and_destroy() {
    let (mut d, root) = doc();
    let a = child(&mut d, root, TOGGLE, "");
    let b = child(&mut d, root, TOGGLE, "");
    assert_eq!(d.kinds_of(TOGGLE).len(), 2);
    d.destroy(a);
    assert_eq!(d.kinds_of(TOGGLE), &[b]);
    assert_eq!(d.kinds_of(BOX), &[root]);
}

#[test]
fn predicates_chain() {
    let (mut d, root) = doc();
    let group = child(&mut d, root, BOX, "");
    let a = child(&mut d, group, TOGGLE, "");
    let b = child(&mut d, group, TOGGLE, "");
    let outside = child(&mut d, root, TOGGLE, "");
    d.set_prop(b, TOGGLE_CHECKED, Value::Bool(true)).unwrap();
    d.set_prop(a, common::ANCHOR, Value::from("first")).unwrap();
    assert_eq!(d.query().kind(TOGGLE).within(group).count(), 2);
    assert_eq!(d.query().within(group).collect(), [group, a, b], "a scope drives in pre-order");
    assert_eq!(d.query().kind(TOGGLE).prop_eq(TOGGLE_CHECKED, Value::Bool(true)).collect(), [b]);
    assert_eq!(d.query().anchor("first").first(), Some(a));
    let rev = d.revision();
    d.set_prop(outside, common::TITLE, Value::from("t")).unwrap();
    assert_eq!(d.query().kind(TOGGLE).changed_since(rev).collect(), [outside]);
    d.set_state(a, StateMask::SELECTED, true);
    assert_eq!(d.query().state(StateMask::SELECTED).in_window(W).collect(), [a]);
}

#[test]
fn css_selectors_select() {
    let (mut d, root) = doc();
    let a = child(&mut d, root, TOGGLE, "");
    let b = child(&mut d, root, BOX, "");
    d.set_style(b, crate::StyleInput { classes: vec!["card".into()], ..Default::default() });
    assert_eq!(d.select(root, "toggle").unwrap(), [a]);
    assert_eq!(d.select(root, "box.card").unwrap(), [b]);
    assert!(d.matches(b, ".card").unwrap());
    assert!(d.select(root, "::(").is_err());
}
