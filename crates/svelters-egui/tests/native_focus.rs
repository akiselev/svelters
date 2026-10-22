//! FX1: the native focus policy and word rules (`svelters_egui::native`) on the built-in leaves,
//! through real input. The full comparison with the GTK and Qt references is
//! `svelters-conformance`'s `egui_measure`.

mod common;

use common::Rig;
use egui::text_edit::TextEditState;
use egui_kittest::kittest::Queryable;
use svelters_abi::*;
use svelters_egui::native::{self, Native};

const CTRL: egui::Modifiers = egui::Modifiers { alt: false, ctrl: true, shift: false, mac_cmd: false, command: true };

fn sel(rig: &Rig, h: svl_handle) -> Option<[usize; 2]> {
    let id = rig.backend.with_scene(|s| s.n(h).id);
    TextEditState::load(&rig.harness.ctx, id).and_then(|s| s.cursor.char_range()).map(|r| [r.secondary.index.0, r.primary.index.0])
}

fn focused(rig: &Rig, h: svl_handle) -> bool {
    let id = rig.backend.with_scene(|s| s.n(h).id);
    rig.harness.ctx.memory(|m| m.has_focus(id))
}

fn button_focused(rig: &Rig, label: &str) -> bool {
    rig.harness.query_by_label(label).is_some_and(|n| n.is_focused())
}

fn key(rig: &mut Rig, m: egui::Modifiers, k: egui::Key) {
    rig.harness.key_press_modifiers(m, k);
    rig.run();
}

#[test]
fn fx1_gtk_focus_policy_and_words() {
    let mut rig = Rig::new();
    native::set(&rig.harness.ctx, Native::Gtk);
    let (_w, col) = rig.window();
    let a = rig.leaf(col, "text-edit", &[("text", Value::Str("first".into())), ("accessible-name", Value::Str("a".into()))]);
    let b = rig.leaf(col, "text-edit", &[("text", Value::Str("snake_case".into())), ("accessible-name", Value::Str("b".into()))]);
    let _c = rig.leaf(col, "button", &[("text", Value::Str("Done".into()))]);
    rig.observe(b, "text", true);
    rig.listen(b, "changed");
    rig.run();

    // a click focuses without selecting; Tab into the next field selects all of it
    rig.click(a);
    assert!(focused(&rig, a));
    assert!(sel(&rig, a).is_some_and(|[x, y]| x == y), "{:?}", sel(&rig, a));
    key(&mut rig, egui::Modifiers::NONE, egui::Key::Tab);
    assert!(focused(&rig, b));
    assert_eq!(sel(&rig, b), Some([0, 10]));

    // Pango's word rules: `_` ends a word; Ctrl+BackSpace deletes back to the word start
    key(&mut rig, egui::Modifiers::NONE, egui::Key::End);
    rig.take();
    key(&mut rig, CTRL, egui::Key::Backspace);
    assert_eq!(rig.get(b, "text"), Value::Str("snake_".into()));
    let l = rig.take();
    assert_eq!(l.changed.len(), 1, "{:?}", l.changed);
    assert!(l.events.iter().any(|e| e.0 == b && e.2 == [Value::Str("snake_".into())]), "{:?}", l.events);
    key(&mut rig, CTRL, egui::Key::ArrowLeft);
    assert_eq!(sel(&rig, b), Some([0, 0]));

    // Escape keeps focus
    key(&mut rig, egui::Modifiers::NONE, egui::Key::Escape);
    assert!(focused(&rig, b));

    // a press focuses the button; Escape keeps it there too
    let c = rig.harness.query_by_label("Done").unwrap().rect().center();
    rig.harness.hover_at(c);
    for pressed in [true, false] {
        rig.harness.event(egui::Event::PointerButton { pos: c, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
    }
    rig.run();
    assert!(button_focused(&rig, "Done"));
    key(&mut rig, egui::Modifiers::NONE, egui::Key::Escape);
    assert!(button_focused(&rig, "Done"));
}

#[test]
fn fx1_egui_policy_is_egui() {
    let mut rig = Rig::new();
    native::set(&rig.harness.ctx, Native::Egui);
    let (_w, col) = rig.window();
    let a = rig.leaf(col, "text-edit", &[("text", Value::Str("first".into())), ("accessible-name", Value::Str("a".into()))]);
    let b = rig.leaf(col, "text-edit", &[("text", Value::Str("snake_case".into())), ("accessible-name", Value::Str("b".into()))]);
    let _c = rig.leaf(col, "button", &[("text", Value::Str("Done".into()))]);
    rig.run();
    rig.click(a);
    key(&mut rig, egui::Modifiers::NONE, egui::Key::Tab);
    assert!(focused(&rig, b));
    assert_ne!(sel(&rig, b), Some([0, 10]));
    key(&mut rig, egui::Modifiers::NONE, egui::Key::Escape);
    assert!(!focused(&rig, b));
    let c = rig.harness.query_by_label("Done").unwrap().rect().center();
    rig.harness.hover_at(c);
    for pressed in [true, false] {
        rig.harness.event(egui::Event::PointerButton { pos: c, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
    }
    rig.run();
    assert!(!button_focused(&rig, "Done"));
}
