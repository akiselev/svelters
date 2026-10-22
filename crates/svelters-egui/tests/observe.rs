//! Observe on egui is exact for every G1 kind: a widget change of an observed prop is announced
//! once (after the pass), an unobserved one never, and host writes never echo. Headless (kittest).

mod common;

use common::Rig;
use egui_kittest::kittest::Queryable;
use svelters_abi::*;
use svelters_egui::kinds::{self, KINDS, Native};

/// Every prop of every element kind: observable, nested on/off counts, and `set_prop` of a new
/// value never calls `prop_changed`.
#[test]
fn every_prop_is_observable_and_host_writes_never_echo() {
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let mut checked = 0;
    for (ki, k) in KINDS.iter().enumerate() {
        let Some(tag) = k.tag else { continue };
        let h = if k.toplevel { rig.create(tag) } else { rig.leaf(col, tag, &[]) };
        let br = rig.br();
        for kk in kinds::chain(ki) {
            for p in KINDS[kk].props {
                let id = br.resolve(NameKind::Prop, kinds::kind_id(ki), p.name);
                assert_ne!(id, 0, "{tag}.{}", p.name);
                let d = br.describe(NameKind::Prop, kinds::kind_id(ki), id).expect("described");
                assert!(d.flags & SVL_M_OBSERVABLE != 0, "{tag}.{} not observable", p.name);
                br.observe(h, id, true).unwrap();
                br.observe(h, id, true).unwrap();
                br.observe(h, id, false).unwrap();
                let v = match p.ty {
                    kinds::Ty::Bool => Value::Bool(!matches!(rig.get(h, p.name), Value::Bool(true))),
                    kinds::Ty::F64 => Value::F64(3.0),
                    kinds::Ty::I64 => Value::I64(3),
                    // WG1: a grid template is validated on write
                    kinds::Ty::Str if p.name == "grid-template-columns" => Value::Str("1fr 2fr".into()),
                    // WG2b: a canvas display list is validated on write
                    kinds::Ty::Str if p.name == "ops" => Value::Str("[[\"fillRect\",0,0,1,1]]".into()),
                    // WG4b: the pickers' values are validated on write
                    kinds::Ty::Str if tag == "date-picker" && p.name == "value" => Value::Str("2026-10-22".into()),
                    kinds::Ty::Str if tag == "color-picker" && p.name == "value" => Value::Str("#123456".into()),
                    kinds::Ty::Str => Value::Str("x".into()),
                    kinds::Ty::Enum(vals) => Value::Str(vals[vals.len() - 1].into()),
                    kinds::Ty::StrList => Value::List(vec![Value::Str("x".into())]),
                    kinds::Ty::IdList => Value::List(vec![Value::I64(3)]),
                    kinds::Ty::Snippet => Value::Bool(true),
                    kinds::Ty::Provider => continue, // RA1: no provider kinds in this binary
                };
                br.set_prop(h, id, &v).unwrap();
                assert_eq!(br.get_prop(h, id).unwrap(), v, "{tag}.{} reads back", p.name);
                br.set_prop(h, id, &Value::Null).unwrap(); // back to the default
                checked += 1;
            }
        }
        if !k.toplevel {
            // kittest refuses inputs without an accessible name (reset to "" above)
            rig.set(h, "accessible-name", Value::Str(tag.into()));
        }
        rig.run();
        assert!(rig.take().changed.is_empty(), "<{tag}>: a host write was announced");
        let _ = Native::Never;
    }
    assert!(checked > 60, "{checked}");
}

/// The three props a widget changes, through real input: one announcement per change when
/// observed, none when not; the node holds the new value; `changed` carries it.
#[test]
fn widget_changes_are_announced_exactly_when_observed() {
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let te = rig.leaf(col, "text-edit", &[("hint-text", Value::Str("Name".into()))]);
    let cb = rig.leaf(col, "checkbox", &[("text", Value::Str("Agree".into()))]);
    let sl = rig.leaf(col, "slider", &[("accessible-name", Value::Str("Level".into())), ("step", Value::F64(1.0))]);
    rig.run();

    // unobserved: the value changes, nothing is announced
    rig.type_into(te, "hi");
    assert_eq!(rig.get(te, "text"), Value::Str("hi".into()));
    rig.click(cb);
    assert_eq!(rig.get(cb, "checked"), Value::Bool(true));
    let l = rig.take();
    assert!(l.changed.is_empty(), "{:?}", l.changed);

    for (h, p) in [(te, "text"), (cb, "checked"), (sl, "value")] {
        rig.observe(h, p, true);
        rig.listen(h, "changed");
    }
    rig.type_into(te, "Ann");
    rig.click(cb);
    rig.harness.get_by_label("Level").set_value_accesskit(42.0);
    rig.run();
    let l = rig.take();
    let text = rig.prop_id(te, "text");
    let checked = rig.prop_id(cb, "checked");
    let value = rig.prop_id(sl, "value");
    assert_eq!(l.changed, vec![(te, text), (cb, checked), (sl, value)], "one announcement each, in order");
    assert_eq!(rig.get(te, "text"), Value::Str("Ann".into()));
    assert_eq!(rig.get(cb, "checked"), Value::Bool(false));
    assert_eq!(rig.get(sl, "value"), Value::F64(42.0));
    let args: Vec<Vec<Value>> = l.events.iter().map(|e| e.2.clone()).collect();
    assert_eq!(args, vec![vec![Value::Str("Ann".into())], vec![Value::Bool(false)], vec![Value::F64(42.0)]]);
    // a checkpoint follows the announcements (the host reads and delivers them there)
    assert!(l.checkpoints >= 1);

    // a host write afterwards: no echo
    rig.set(te, "text", Value::Str("Bob".into()));
    rig.set(sl, "value", Value::F64(7.0));
    rig.run();
    assert!(rig.take().changed.is_empty());

    // nested observation: off once still observes, off twice stops
    rig.observe(te, "text", true);
    rig.observe(te, "text", false);
    rig.type_into(te, "Z");
    assert_eq!(rig.take().changed, vec![(te, text)]);
    rig.observe(te, "text", false);
    rig.type_into(te, "Y");
    assert!(rig.take().changed.is_empty());
}

/// A slider clamps a host-set value into its range when drawn: that is a native change, announced
/// (the binding's `natively_normalizes`).
#[test]
fn slider_clamp_is_announced() {
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let sl = rig.leaf(col, "slider", &[("accessible-name", Value::Str("S".into()))]);
    rig.observe(sl, "value", true);
    rig.run();
    rig.take();
    rig.set(sl, "value", Value::F64(500.0));
    rig.run();
    assert_eq!(rig.get(sl, "value"), Value::F64(100.0));
    assert_eq!(rig.take().changed.len(), 1);
}
