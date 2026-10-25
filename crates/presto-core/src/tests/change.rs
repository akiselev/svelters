//! Change classes and transactions (architecture §4.3–§4.5).

use super::*;
use crate::testkinds::{BOX_EXTENT, BOX_RAISE, BOX_TINT, TOGGLE_CHECKED};
use crate::{Dirty, PropError, Value, ValueType};
use presto_base::change::ChangeClass;
use presto_base::color::Color;

#[test]
fn set_prop_returns_and_marks_its_class() {
    let (mut d, root) = doc();
    let a = child(&mut d, root, BOX, "width: 20px; height: 20px");
    frame(&mut d);
    assert_eq!(d.dirty(a).unwrap(), Dirty::EMPTY, "a frame serves every dirty bit");

    assert_eq!(d.set_prop(a, BOX_RAISE, Value::I64(1)), Ok(ChangeClass::Property));
    let dirty = d.dirty(a).unwrap();
    assert!(dirty.contains(Dirty::PROPERTY));
    assert!(!dirty.intersects(Dirty::PAINT | Dirty::LAYOUT | Dirty::MEASURE | Dirty::STYLE));
    frame(&mut d);

    assert_eq!(d.set_prop(a, BOX_TINT, Value::Color(Color::WHITE)), Ok(ChangeClass::Paint));
    let dirty = d.dirty(a).unwrap();
    assert!(dirty.contains(Dirty::PAINT));
    assert!(!dirty.intersects(Dirty::LAYOUT | Dirty::MEASURE | Dirty::STYLE));
    frame(&mut d);

    assert_eq!(d.set_prop(a, BOX_EXTENT, Value::F64(4.0)), Ok(ChangeClass::Layout));
    assert!(d.dirty(a).unwrap().contains(Dirty::MEASURE), "a leaf's layout prop remeasures it");
}

#[test]
fn prop_types_are_checked_and_coerced() {
    let (mut d, root) = doc();
    let a = child(&mut d, root, BOX, "");
    assert_eq!(d.set_prop(a, BOX_EXTENT, Value::I64(3)), Ok(ChangeClass::Layout));
    assert_eq!(d.get_prop(a, BOX_EXTENT), Some(Value::F64(3.0)));
    assert_eq!(
        d.set_prop(a, BOX_EXTENT, Value::from("x")),
        Err(PropError::WrongType { prop: BOX_EXTENT, expected: ValueType::F64, got: ValueType::Str })
    );
    let unknown = crate::PropId::of("box", "nope", "int");
    assert_eq!(d.set_prop(a, unknown, Value::I64(1)), Err(PropError::Unknown(unknown)));
    // common props work on every kind
    assert_eq!(d.set_prop(a, crate::common::DISABLED, Value::Bool(true)), Ok(ChangeClass::Paint));
    assert!(d.state(a).contains(crate::StateMask::DISABLED));
    // reset reads the default again
    d.reset_prop(a, crate::common::DISABLED).unwrap();
    assert_eq!(d.get_prop(a, crate::common::DISABLED), Some(Value::Bool(false)));
}

#[test]
fn a_transaction_is_one_revision() {
    let (mut d, root) = doc();
    let a = child(&mut d, root, BOX, "");
    let r0 = d.revision();
    d.transaction(|d| {
        d.set_prop(a, BOX_RAISE, Value::I64(1)).unwrap();
        d.set_prop(a, BOX_RAISE, Value::I64(2)).unwrap();
        let b = d.create(BOX);
        d.insert(root, b, None, None).unwrap();
    });
    assert_eq!(d.revision().0, r0.0 + 1);
    d.set_prop(a, BOX_RAISE, Value::I64(3)).unwrap();
    d.set_prop(a, BOX_RAISE, Value::I64(4)).unwrap();
    assert_eq!(d.revision().0, r0.0 + 3, "edits outside a transaction are one each");
}

#[test]
fn a_stale_js_write_of_native_state_is_rejected() {
    let (mut d, root) = doc();
    let t = child(&mut d, root, TOGGLE, "");
    frame(&mut d);
    let seen = d.revision();
    click(&mut d, 5.0, 5.0);
    assert_eq!(d.get_prop(t, TOGGLE_CHECKED), Some(Value::Bool(true)), "the click toggled it natively");
    let changed = d.revision();
    assert!(changed > seen);
    // JS that saw the tree before the click must not paint its stale value over it
    let r = d.js_transaction(seen, |d| d.set_prop(t, TOGGLE_CHECKED, Value::Bool(false)));
    assert!(matches!(r, Err(PropError::Superseded { prop, .. }) if prop == TOGGLE_CHECKED));
    assert_eq!(d.get_prop(t, TOGGLE_CHECKED), Some(Value::Bool(true)));
    // JS that saw the click may write
    let r = d.js_transaction(d.revision(), |d| d.set_prop(t, TOGGLE_CHECKED, Value::Bool(false)));
    assert_eq!(r, Ok(ChangeClass::Paint));
    // native code always may
    assert!(d.set_prop(t, TOGGLE_CHECKED, Value::Bool(true)).is_ok());
}

#[test]
fn a_write_of_the_same_value_marks_nothing() {
    let (mut d, root) = doc();
    let a = child(&mut d, root, BOX, "");
    d.set_prop(a, BOX_RAISE, Value::I64(1)).unwrap();
    frame(&mut d);
    let r = d.revision();
    d.set_prop(a, BOX_RAISE, Value::I64(1)).unwrap();
    assert_eq!(d.revision(), r);
    assert_eq!(d.dirty(a).unwrap(), Dirty::EMPTY);
}

#[test]
fn the_measure_guard_skips_layout_for_same_size_text() {
    let (mut d, root) = doc();
    let t = child(&mut d, root, TEXT, "");
    d.set_prop(t, crate::testkinds::TEXT, Value::from("abc")).unwrap();
    frame(&mut d);
    d.set_prop(t, crate::testkinds::TEXT, Value::from("xyz")).unwrap();
    let (c, stats) = d.frame_with_stats(W, Time(0));
    assert_eq!(stats.layout.remeasured, 1);
    assert_eq!(stats.layout.resized, 0);
    assert!(!stats.layout.full && stats.layout.boundaries == 0, "{:?}", stats.layout);
    assert_eq!(stats.recorded, 1);
    let c = c.unwrap();
    assert_eq!(c.lists.len(), 1);
    assert_eq!(c.lists[0].0, t);
    assert!(!c.dropped_runs.is_empty(), "the old text's run is dropped");
    d.set_prop(t, crate::testkinds::TEXT, Value::from("longer text")).unwrap();
    let (_, stats) = d.frame_with_stats(W, Time(0));
    assert_eq!(stats.layout.resized, 1);
}
