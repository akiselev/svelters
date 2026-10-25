//! The kind services the built-in widgets need: timers, AccessKit actions, methods, blur, typed
//! services, the engine's computed style, attaching a JS-created window node, id collisions.

use super::*;
use crate::testkinds::{HOLD, TOGGLE_CHECKED, TOGGLE_FLIP, TOGGLED};
use crate::{FocusReason, KindRegistry, KindSpec, ListenFlags, PropSpec, RegistryError, Value, ValueType};
use presto_base::change::ChangeClass;
use presto_base::event::PointerPhase;

#[test]
fn a_kind_timer_fires_at_its_deadline() {
    let (mut d, root) = doc();
    let t = child(&mut d, root, TOGGLE, "");
    frame(&mut d);
    d.dispatch(&pointer(PointerPhase::Down, 5.0, 5.0, 1_000));
    let due = d.next_timer().expect("the press asked for a timer");
    assert_eq!(due, Time(1_000) + HOLD);
    assert!(d.run_timers(Time(1_000)).is_empty(), "not due yet");
    assert_eq!(d.get_prop(t, TOGGLE_CHECKED), Some(Value::Bool(false)));
    d.listen(root, TOGGLED, ListenFlags::EMPTY);
    let evs = d.run_timers(due);
    assert_eq!(evs.len(), 1);
    assert_eq!(d.get_prop(t, TOGGLE_CHECKED), Some(Value::Bool(true)));
    assert_eq!(d.next_timer(), None, "one-shot");
}

#[test]
fn accesskit_actions_reach_the_kind_then_the_defaults() {
    let (mut d, root) = doc();
    let t = child(&mut d, root, TOGGLE, "");
    frame(&mut d);
    let req = |action| accesskit::ActionRequest { action, target_tree: accesskit::TreeId::ROOT, target_node: accesskit::NodeId(t.0), data: None };
    let out = d.a11y_action(W, &req(accesskit::Action::Click));
    assert!(out.handled);
    assert_eq!(d.get_prop(t, TOGGLE_CHECKED), Some(Value::Bool(true)));
    let out = d.a11y_action(W, &req(accesskit::Action::Focus));
    assert!(out.handled);
    assert_eq!(d.focused(W), Some(t));
}

#[test]
fn methods_are_checked_and_called() {
    let (mut d, root) = doc();
    let t = child(&mut d, root, TOGGLE, "");
    let (r, _) = d.call(t, TOGGLE_FLIP, &[]).unwrap();
    assert_eq!(r, Some(Value::Bool(true)));
    assert!(d.call(t, TOGGLE_FLIP, &[Value::I64(1)]).is_err(), "too many arguments");
    assert!(d.call(t, crate::MethodId::of("toggle", "nope", "void"), &[]).is_err());
}

#[test]
fn blur_gives_up_focus() {
    let (mut d, root) = doc();
    let t = child(&mut d, root, TOGGLE, "");
    d.focus(t, FocusReason::Keyboard);
    assert_eq!(d.focused(W), Some(t));
    d.blur(W);
    assert_eq!(d.focused(W), None);
}

#[test]
fn services_are_lent_by_type() {
    let (mut d, _root) = doc();
    assert!(d.service::<String>().is_none());
    d.set_service(String::from("shaper"));
    d.service::<String>().unwrap().push('!');
    assert_eq!(d.service::<String>().map(|s| s.as_str()), Some("shaper!"));
}

#[test]
fn views_carry_the_engine_style() {
    let (mut d, root) = doc();
    let a = child(&mut d, root, BOX, "letter-spacing: 2px");
    assert!(d.view(a).unwrap().css().is_none(), "not styled yet");
    frame(&mut d);
    let v = d.view(a).unwrap();
    assert!(v.css().unwrap().get("letter-spacing").is_some());
}

#[test]
fn a_detached_node_can_become_a_window() {
    let (mut d, _root) = doc();
    let win = d.create(BOX);
    let inner = d.create(BOX);
    d.set_style(inner, crate::StyleInput { inline: Some("width: 10px; height: 10px; background-color: #ff0000".into()), ..Default::default() });
    d.insert(win, inner, None, None).unwrap();
    let w2 = presto_base::ids::WindowId(2);
    d.attach_window(w2, win, Size::new(50.0, 50.0), Scale(2.0)).unwrap();
    assert_eq!(d.window_root(w2), Some(win));
    assert_eq!(d.window_of(inner), Some(w2));
    let c = d.frame(w2, Time(0)).unwrap();
    assert_eq!(c.window, w2);
    assert_eq!(c.lists.len(), 1);
    // a node with a parent cannot be a window
    assert!(d.attach_window(presto_base::ids::WindowId(3), inner, Size::new(1.0, 1.0), Scale(1.0)).is_err());
}

#[test]
fn ids_are_unique_across_kinds() {
    static A: KindSpec = KindSpec { props: &[PropSpec::new("x", ValueType::Bool, ChangeClass::Paint)], ..KindSpec::new("a") };
    static B: KindSpec = KindSpec { props: &[PropSpec::new("x", ValueType::Bool, ChangeClass::Paint)], ..KindSpec::new("b") };
    static DUP: KindSpec = KindSpec {
        props: &[PropSpec::new("x", ValueType::Bool, ChangeClass::Paint), PropSpec::new("x", ValueType::Bool, ChangeClass::Paint)],
        ..KindSpec::new("dup")
    };
    let mut r = KindRegistry::new();
    let f: crate::kind::KindFactory = || Box::new(crate::testkinds::BoxKind);
    r.register_with(&A, f).unwrap();
    r.register_with(&B, f).unwrap();
    assert!(matches!(r.register_with(&A, f), Err(RegistryError::DuplicateKind { .. })));
    assert!(matches!(r.register_with(&DUP, f), Err(RegistryError::MemberCollision { .. })));
    // two different names hashing alike are found the same way (the error names both)
    let e = RegistryError::CrossKindCollision { kind: "a", member: "x", other_kind: "b", other: "y", id: 7 };
    assert!(e.to_string().contains("a.x") && e.to_string().contains("b.y"));
}
