//! Dispatch (svelters EV1): hit testing on the last frame, capture/target/bubble order, passive
//! listeners, click semantics, hover, pointer capture and kinds' signals.

use super::*;
use crate::testkinds::{TOGGLE_CHECKED, TOGGLED};
use crate::{ListenFlags, Phase, Value, common};
use skelter_base::event::PointerPhase;

#[test]
fn listeners_run_capture_target_bubble() {
    let (mut d, root) = doc();
    let a = child(&mut d, root, BOX, "width: 100px; height: 50px");
    let b = child(&mut d, a, BOX, "width: 20px; height: 20px");
    frame(&mut d);
    d.listen(root, common::CLICK, ListenFlags::CAPTURE);
    d.listen(a, common::CLICK, ListenFlags::CAPTURE);
    d.listen(b, common::CLICK, ListenFlags::EMPTY);
    d.listen(a, common::CLICK, ListenFlags::EMPTY);
    d.listen(root, common::CLICK, ListenFlags::PASSIVE);
    let out = click(&mut d, 5.0, 5.0);
    let ev = out[1].events.iter().find(|e| e.signal == common::CLICK).expect("click");
    assert_eq!(ev.target, b);
    let order: Vec<(NodeId, Phase)> = ev.path.iter().map(|p| (p.node, p.phase)).collect();
    assert_eq!(order, [(root, Phase::Capture), (a, Phase::Capture), (b, Phase::Target), (a, Phase::Bubble), (root, Phase::Bubble)]);
    assert!(ev.path[4].flags.contains(ListenFlags::PASSIVE));
    assert!(ev.cancelable, "some listener is not passive");
    assert!(ev.sync, "click is delivered at event time");
    // the record carries the DOM fields
    let arg = ev.arg.as_ref().unwrap();
    assert_eq!(arg.field("clientX"), Some(&Value::F64(5.0)));
    assert_eq!(arg.field("detail"), Some(&Value::I64(1)));
}

#[test]
fn events_without_listeners_are_not_listed() {
    let (mut d, root) = doc();
    child(&mut d, root, BOX, "width: 20px; height: 20px");
    frame(&mut d);
    let out = click(&mut d, 5.0, 5.0);
    assert!(out.iter().all(|o| o.events.is_empty()));
}

#[test]
fn a_passive_only_path_is_not_cancelable() {
    let (mut d, root) = doc();
    let a = child(&mut d, root, BOX, "width: 20px; height: 20px");
    frame(&mut d);
    d.listen(a, common::CLICK, ListenFlags::PASSIVE);
    let out = click(&mut d, 5.0, 5.0);
    let ev = out[1].events.iter().find(|e| e.signal == common::CLICK).unwrap();
    assert!(!ev.cancelable);
}

#[test]
fn non_bubbling_signals_reach_only_the_target() {
    let (mut d, root) = doc();
    let a = child(&mut d, root, BOX, "width: 20px; height: 20px");
    frame(&mut d);
    d.listen(root, common::POINTERENTER, ListenFlags::EMPTY);
    d.listen(a, common::POINTERENTER, ListenFlags::EMPTY);
    let out = d.dispatch(&pointer(PointerPhase::Move, 5.0, 5.0, 0));
    let enters: Vec<NodeId> = out.events.iter().filter(|e| e.signal == common::POINTERENTER).map(|e| e.target).collect();
    // entered outermost first, each at its own node only
    assert_eq!(enters, [root, a]);
    assert!(out.events.iter().filter(|e| e.signal == common::POINTERENTER).all(|e| e.path.len() == 1));
    assert!(d.state(a).contains(crate::StateMask::HOVERED));
    let out = d.dispatch(&pointer(PointerPhase::Move, 150.0, 80.0, 0));
    assert!(out.events.is_empty(), "nobody listens to pointerleave");
    assert!(!d.state(a).contains(crate::StateMask::HOVERED));
    assert!(d.state(root).contains(crate::StateMask::HOVERED));
}

#[test]
fn a_click_is_a_release_over_the_pressed_node() {
    let (mut d, root) = doc();
    let a = child(&mut d, root, BOX, "width: 20px; height: 20px");
    let b = child(&mut d, root, BOX, "width: 20px; height: 20px");
    frame(&mut d);
    d.listen(root, common::CLICK, ListenFlags::EMPTY);
    // press on a, release on b: no click
    d.dispatch(&pointer(PointerPhase::Down, 5.0, 5.0, 0));
    assert!(d.state(a).contains(crate::StateMask::PRESSED));
    let up = d.dispatch(&pointer(PointerPhase::Up, 5.0, 25.0, 0));
    assert_eq!(up.target, Some(b));
    assert!(up.events.iter().all(|e| e.signal != common::CLICK));
    assert!(!d.state(a).contains(crate::StateMask::PRESSED));
}

#[test]
fn two_quick_clicks_are_a_double_click() {
    let (mut d, root) = doc();
    let a = child(&mut d, root, BOX, "width: 20px; height: 20px");
    frame(&mut d);
    d.listen(a, common::DBLCLICK, ListenFlags::EMPTY);
    click(&mut d, 5.0, 5.0);
    let out = click(&mut d, 6.0, 5.0);
    assert!(out[1].events.iter().any(|e| e.signal == common::DBLCLICK));
}

#[test]
fn a_kind_handles_its_default_and_emits_its_signal() {
    let (mut d, root) = doc();
    let t = child(&mut d, root, TOGGLE, "");
    frame(&mut d);
    d.listen(root, TOGGLED, ListenFlags::EMPTY);
    let out = click(&mut d, 5.0, 5.0);
    assert!(out[1].handled);
    assert!(out[1].changed);
    let ev = out[1].events.iter().find(|e| e.signal == TOGGLED).expect("toggled bubbles to the root's listener");
    assert_eq!(ev.target, t);
    assert_eq!(ev.arg, Some(Value::Bool(true)));
    assert_eq!(ev.path[0].node, root);
    assert_eq!(d.get_prop(t, TOGGLE_CHECKED), Some(Value::Bool(true)));
    assert!(d.state(t).contains(crate::StateMask::CHECKED));
    assert_eq!(d.focused(W), Some(t), "a press focuses the focusable target");
    // Space toggles it back through the focused node
    let out = d.dispatch(&key(Key::Space, Modifiers::default()));
    assert!(out.handled);
    assert_eq!(d.get_prop(t, TOGGLE_CHECKED), Some(Value::Bool(false)));
}

#[test]
fn a_disabled_node_gets_no_native_default() {
    let (mut d, root) = doc();
    let t = child(&mut d, root, TOGGLE, "");
    d.set_prop(t, common::DISABLED, Value::Bool(true)).unwrap();
    frame(&mut d);
    let out = click(&mut d, 5.0, 5.0);
    assert!(!out[1].handled);
    assert_eq!(d.get_prop(t, TOGGLE_CHECKED), Some(Value::Bool(false)));
    assert_eq!(d.focused(W), None, "disabled nodes don't take focus");
}

#[test]
fn hit_testing_uses_the_last_frame() {
    let (mut d, root) = doc();
    let a = child(&mut d, root, BOX, "width: 20px; height: 20px");
    frame(&mut d);
    // moved, not yet drawn: input still hits where the user saw it
    d.set_style(a, crate::StyleInput { inline: Some("width: 20px; height: 20px; margin-left: 100px".into()), ..Default::default() });
    assert_eq!(d.hit_test(W, Point::new(5.0, 5.0)), Some(a));
    frame(&mut d);
    assert_eq!(d.hit_test(W, Point::new(5.0, 5.0)), Some(root));
    assert_eq!(d.hit_test(W, Point::new(105.0, 5.0)), Some(a));
    // a node that is not displayed is not hit
    d.set_style(a, crate::StyleInput { inline: Some("width: 20px; height: 20px; margin-left: 100px; display: none".into()), ..Default::default() });
    frame(&mut d);
    assert_eq!(d.hit_test(W, Point::new(105.0, 5.0)), Some(root));
}

#[test]
fn a_destroyed_target_drops_its_capture_and_hover() {
    let (mut d, root) = doc();
    let a = child(&mut d, root, BOX, "width: 20px; height: 20px");
    frame(&mut d);
    d.dispatch(&pointer(PointerPhase::Down, 5.0, 5.0, 0));
    d.destroy(a);
    let up = d.dispatch(&pointer(PointerPhase::Up, 5.0, 5.0, 0));
    assert_ne!(up.target, Some(a));
    assert!(!up.events.iter().any(|e| e.target == a));
}

#[test]
fn keys_go_to_the_ime_during_a_composition() {
    let (mut d, root) = doc();
    let t = child(&mut d, root, TOGGLE, "");
    frame(&mut d);
    d.focus(t, crate::FocusReason::Programmatic);
    let ime = skelter_base::event::InputEvent::Ime(skelter_base::event::ImeEvent::Preedit { text: "に".into(), cursor: None });
    d.dispatch(&skelter_base::event::InputEnvelope { seq: skelter_base::ids::InputSeq(1), window: W, os_time: Time(0), recv_time: Time(0), event: ime });
    let mut k = key(Key::J, Modifiers::default());
    if let skelter_base::event::InputEvent::Key(e) = &mut k.event {
        e.text = Some("j".into());
    }
    let out = d.dispatch(&k);
    assert!(out.to_ime);
    assert!(out.events.is_empty());
}

#[test]
fn an_ime_source_takes_printable_keys_before_the_keymap() {
    let (mut d, root) = doc();
    let t = child(&mut d, root, TOGGLE, "");
    frame(&mut d);
    d.focus(t, crate::FocusReason::Programmatic);
    let hooked = std::rc::Rc::new(std::cell::Cell::new(0));
    let h = hooked.clone();
    d.set_key_hook(Box::new(move |_, _| {
        h.set(h.get() + 1);
        false
    }));
    d.set_ime_source(W, true);
    let typed = |mods| {
        let mut k = key(Key::J, mods);
        if let skelter_base::event::InputEvent::Key(e) = &mut k.event {
            e.text = Some("j".into());
        }
        k
    };
    assert!(d.dispatch(&typed(Modifiers::default())).to_ime, "a printable key goes to the IME");
    assert_eq!(hooked.get(), 0, "before the command keymap");
    assert!(!d.dispatch(&typed(Modifiers::CTRL)).to_ime, "a chord goes through the keymaps");
    assert_eq!(hooked.get(), 1);
    d.set_ime_source(W, false);
    assert!(!d.dispatch(&typed(Modifiers::default())).to_ime);
}
