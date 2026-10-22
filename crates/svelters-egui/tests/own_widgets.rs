//! OW0 (ARCH-WIDGETS §6.2, plans/milestones/OW0-widget-foundation.md): svelters' own widgets
//! behind the per-kind switch. Every behaviour test runs on both implementations (the egui
//! wrapper and the own widget) until the kind's default flips; the registry's routing of a
//! replaced kind's row is checked once.

mod common;

use common::Rig;
use egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use svelters_abi::*;
use svelters_egui::kinds::KindRegistry;
use svelters_egui::native::{self, Native};
use svelters_egui::provider::StateBits;
use svelters_egui::scene::W;
use svelters_egui::widgets::KindSet;

fn s(v: &str) -> Value {
    Value::Str(v.into())
}

/// The two implementations of `button`: (name, switch).
fn impls() -> [(&'static str, KindSet); 2] {
    [("egui", KindSet::None), ("own", KindSet::tags(["button"]))]
}

fn rig(set: &KindSet) -> Rig {
    let rig = Rig::new();
    native::set(&rig.harness.ctx, Native::Gtk); // a press focuses a button (FX1)
    rig.backend.set_own_widgets(set.clone());
    rig
}

fn own(rig: &Rig, h: svl_handle) -> bool {
    rig.backend.with_scene(|s| matches!(s.n(h).w, W::Ext(_)))
}

fn state(rig: &Rig, h: svl_handle) -> StateBits {
    rig.backend.with_scene(|s| s.n(h).state)
}

fn events(rig: &Rig, h: svl_handle, name: &str) -> Vec<Vec<Value>> {
    let sig = rig.signal_id(h, name);
    rig.log.borrow().events.iter().filter(|e| e.0 == h && e.1 == sig).map(|e| e.2.clone()).collect()
}

fn key(rig: &mut Rig, k: egui::Key) {
    rig.harness.key_press(k);
    rig.run();
}

#[test]
fn ow0_button_replaces_the_builtin_row() {
    let reg = KindRegistry::global();
    let k = reg.tag_index("button").expect("button");
    let r = reg.replacement(k).expect("the own button replaces <button>");
    assert_eq!(r.entry.id, "sv.Button");
    assert_eq!(r.entry.replaces, Some("button"));
    assert!(reg.provider_index("sv.Button").is_none(), "an own widget is not an element of its own");
    // every own prop and signal of the row routes to the built-in id
    for (i, p) in r.entry.props.iter().enumerate() {
        assert_eq!(reg.ext_prop_id(k, i), reg.find_prop(k, p.name).unwrap().0, "{}", p.name);
    }
    for (i, e) in r.entry.events.iter().enumerate() {
        assert_eq!(reg.ext_signal_id(k, i), reg.find_signal(k, e.name).unwrap().0, "{}", e.name);
    }
}

#[test]
fn ow0_switch_picks_the_implementation_and_props_round_trip() {
    for (name, set) in impls() {
        let mut rig = rig(&set);
        let (_w, col) = rig.window();
        let b = rig.leaf(col, "button", &[]);
        assert_eq!(own(&rig, b), name == "own", "{name}");
        // defaults are the row's
        assert_eq!(rig.get(b, "text"), s(""), "{name}");
        assert_eq!(rig.get(b, "frame"), Value::Bool(true), "{name}");
        assert_eq!(rig.get(b, "font-weight"), Value::I64(400), "{name}");
        let props = [
            ("text", s("Save")),
            ("selected", Value::Bool(true)),
            ("frame", Value::Bool(false)),
            ("small", Value::Bool(true)),
            ("font-weight", Value::I64(600)),
            ("default", Value::Bool(true)),
            ("cancel", Value::Bool(true)),
            ("tooltip", s("Saves")),
        ];
        for (p, v) in &props {
            rig.set(b, p, v.clone());
        }
        for (p, v) in &props {
            assert_eq!(&rig.get(b, p), v, "{name}: {p}");
        }
        // null resets to the row's default
        rig.br().set_prop(b, rig.prop_id(b, "text"), &Value::Null).unwrap();
        assert_eq!(rig.get(b, "text"), s(""), "{name}");
        rig.set(b, "text", s("Save"));
        rig.run();
        assert_eq!(rig.backend.with_scene(|s| s.n(b).text().to_string()), "Save", "{name}: Node::text");
        assert!(rig.drawn(b).width() > 0.0, "{name}: drawn");
    }
}

#[test]
fn ow0_button_clicks_keys_and_accesskit_on_both() {
    for (name, set) in impls() {
        let mut rig = rig(&set);
        let (_w, col) = rig.window();
        let b = rig.leaf(col, "button", &[("text", s("Apply"))]);
        let off = rig.leaf(col, "button", &[("text", s("Off")), ("enabled", Value::Bool(false))]);
        for h in [b, off] {
            rig.listen(h, "clicked");
        }
        rig.run();
        let node = rig.harness.query_by_role_and_label(Role::Button, "Apply");
        assert!(node.is_some(), "{name}: a Button named by its text");

        // a click: one `clicked`, with no arguments (the row's arity)
        rig.click(off);
        assert!(events(&rig, off, "clicked").is_empty(), "{name}: disabled");
        rig.click(b);
        assert_eq!(events(&rig, b, "clicked"), vec![Vec::<Value>::new()], "{name}");

        // Space and Enter on the focused button
        assert!(rig.harness.query_by_label("Apply").unwrap().is_focused(), "{name}: a press focuses it (GTK)");
        rig.take();
        key(&mut rig, egui::Key::Space);
        key(&mut rig, egui::Key::Enter);
        assert_eq!(events(&rig, b, "clicked").len(), 2, "{name}: Space and Enter");

        // AccessKit Click goes through the same path
        rig.take();
        rig.harness.get_by_label("Apply").click_accesskit();
        rig.run();
        assert_eq!(events(&rig, b, "clicked").len(), 1, "{name}: AccessKit click");
    }
}

#[test]
fn ow0_default_and_cancel_buttons_on_both() {
    for (name, set) in impls() {
        let mut rig = rig(&set);
        let (_w, col) = rig.window();
        let field = rig.leaf(col, "text-edit", &[("accessible-name", s("name"))]);
        let ok = rig.leaf(col, "button", &[("text", s("OK")), ("default", Value::Bool(true))]);
        let cancel = rig.leaf(col, "button", &[("text", s("Cancel")), ("cancel", Value::Bool(true))]);
        for h in [ok, cancel] {
            rig.listen(h, "clicked");
        }
        rig.run();
        rig.click(field);
        rig.take();
        key(&mut rig, egui::Key::Enter);
        assert_eq!((events(&rig, ok, "clicked").len(), events(&rig, cancel, "clicked").len()), (1, 0), "{name}: Enter");
        rig.take();
        key(&mut rig, egui::Key::Escape);
        assert_eq!((events(&rig, ok, "clicked").len(), events(&rig, cancel, "clicked").len()), (0, 1), "{name}: Escape");
    }
}

#[test]
fn ow0_own_button_states_hover_press_focus_visible() {
    let mut rig = rig(&KindSet::tags(["button"]));
    let (_w, col) = rig.window();
    let a = rig.leaf(col, "button", &[("text", s("A"))]);
    let b = rig.leaf(col, "button", &[("text", s("B")), ("selected", Value::Bool(true))]);
    rig.run();
    assert!(state(&rig, b).contains(StateBits::SELECTED), "the prop mirror");

    rig.harness.hover_at(rig.drawn(a).center());
    rig.run();
    assert!(state(&rig, a).contains(StateBits::HOVERED));
    assert!(!state(&rig, b).contains(StateBits::HOVERED));

    // held: pressed; released: not
    let pos = rig.drawn(a).center();
    rig.harness.event(egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: true, modifiers: egui::Modifiers::NONE });
    rig.run();
    assert!(state(&rig, a).contains(StateBits::PRESSED));
    rig.harness.event(egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed: false, modifiers: egui::Modifiers::NONE });
    rig.run();
    rig.run();
    let st = state(&rig, a);
    assert!(!st.contains(StateBits::PRESSED));
    // focused by the press (GTK focus-on-click) but no focus ring (FX1, `:focus-visible`)
    assert!(st.contains(StateBits::FOCUSED), "{st:?}");
    assert!(!st.contains(StateBits::FOCUS_VISIBLE), "{st:?}");

    // Tab: focus moves by keyboard, the ring shows
    key(&mut rig, egui::Key::Tab);
    rig.run();
    let st = state(&rig, b);
    assert!(st.contains(StateBits::FOCUSED) && st.contains(StateBits::FOCUS_VISIBLE), "{st:?}");
    assert!(!state(&rig, a).contains(StateBits::FOCUSED));
}

#[test]
fn ow0_own_button_is_sized_by_its_style() {
    let mut rig = rig(&KindSet::tags(["button"]));
    let (_w, col) = rig.window();
    let b = rig.leaf(col, "button", &[("text", s("Width"))]);
    let small = rig.leaf(col, "button", &[("text", s("Width")), ("small", Value::Bool(true))]);
    rig.run();
    rig.run();
    let (r, rs) = (rig.drawn(b), rig.drawn(small));
    // content + padding (button-pad-x/y) + border (stroke) from the UA table: VS Code's 13px text
    assert!(r.height() >= 20.0 && r.height() <= 30.0, "{r:?}");
    assert!(rs.height() < r.height(), "small drops the vertical padding: {rs:?} vs {r:?}");
}
