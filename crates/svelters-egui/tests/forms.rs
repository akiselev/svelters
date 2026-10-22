//! FM1 (plans/milestones/FM1-form-widgets.md): the form widget gaps of QT-SURVEY §1 through the
//! backend's vtable, as the JS runtime drives it: `drag-value` steps and commits (QW7), `text-edit`
//! `submit`/`commit`/`pattern`/`read-only` (QW8), the `table` header's sort, column menu and saved
//! state (QW4), a box's platform `button-order` (QW11), and the keyboard context menu (QA8).
//! Headless.

mod common;

use common::Rig;
use egui::accesskit::Role;
use egui::{Key, Modifiers, PointerButton, pos2};
use egui_kittest::kittest::{NodeT, Queryable};
use svelters_abi::*;

fn s(v: &str) -> Value {
    Value::Str(v.into())
}

fn f(v: f64) -> Value {
    Value::F64(v)
}

/// The events of signal `name` on `h` in a log.
fn events(rig: &Rig, log: &common::Log, h: svl_handle, name: &str) -> Vec<Vec<Value>> {
    let sig = rig.signal_id(h, name);
    log.events.iter().filter(|(eh, es, _)| *eh == h && *es == sig).map(|(_, _, a)| a.clone()).collect()
}

fn click_at(rig: &mut Rig, pos: egui::Pos2, button: PointerButton) {
    rig.harness.hover_at(pos);
    for pressed in [true, false] {
        rig.harness.event(egui::Event::PointerButton { pos, button, pressed, modifiers: Modifiers::NONE });
    }
    rig.harness.run();
}

fn key(rig: &mut Rig, m: Modifiers, k: Key) {
    rig.harness.key_press_modifiers(m, k);
    rig.harness.run();
}

#[test]
fn fm1_drag_value_steps_by_buttons_keys_and_wheel_and_commits() {
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let d = rig.leaf(
        col,
        "drag-value",
        &[("value", f(3.0)), ("step", f(0.5)), ("min", f(0.0)), ("max", f(10.0)), ("buttons", Value::Bool(true)), ("accessible-name", s("Width")), ("align-self", s("start"))],
    );
    for e in ["changed", "commit"] {
        rig.listen(d, e);
    }
    rig.run();
    rig.take();
    // the + button is the last square of the node
    let r = rig.drawn(d);
    let side = r.height();
    click_at(&mut rig, pos2(r.max.x - side / 2.0, r.center().y), PointerButton::Primary);
    let log = rig.take();
    assert_eq!(rig.get(d, "value"), f(3.5));
    assert_eq!((events(&rig, &log, d, "changed"), events(&rig, &log, d, "commit")), (vec![vec![f(3.5)]], vec![vec![f(3.5)]]));
    // the − button
    click_at(&mut rig, pos2(r.max.x - side * 1.5, r.center().y), PointerButton::Primary);
    assert_eq!(rig.get(d, "value"), f(3.0));
    rig.take();
    // focused (a click on the number edits it): PageUp steps ten times, Down once, each a commit
    let field = pos2(r.min.x + 4.0, r.center().y);
    click_at(&mut rig, field, PointerButton::Primary);
    rig.run();
    key(&mut rig, Modifiers::NONE, Key::PageUp);
    assert_eq!(rig.get(d, "value"), f(8.0));
    key(&mut rig, Modifiers::NONE, Key::ArrowDown);
    assert_eq!(rig.get(d, "value"), f(7.5));
    let log = rig.take();
    assert_eq!(events(&rig, &log, d, "commit"), vec![vec![f(8.0)], vec![f(7.5)]]);
    // the wheel over the focused field: one notch up, one step; PageUp stops at max
    rig.harness.hover_at(field);
    rig.harness.event(egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Line, delta: egui::vec2(0.0, 1.0), phase: egui::TouchPhase::Move, modifiers: Modifiers::NONE, source: egui::MouseWheelSource::Wheel });
    rig.run();
    assert_eq!(rig.get(d, "value"), f(8.0));
    key(&mut rig, Modifiers::NONE, Key::PageUp);
    assert_eq!(rig.get(d, "value"), f(10.0), "clamped to max");
}

#[test]
fn fm1_drag_value_commit_mode_holds_the_value_until_enter() {
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let d = rig.leaf(col, "drag-value", &[("value", f(1.0)), ("commit", Value::Bool(true)), ("accessible-name", s("Count")), ("align-self", s("start"))]);
    let live = rig.leaf(col, "drag-value", &[("value", f(1.0)), ("accessible-name", s("Live")), ("align-self", s("start"))]);
    for h in [d, live] {
        rig.listen(h, "changed");
        rig.listen(h, "commit");
    }
    rig.run();
    rig.take();
    // typing into the commit-only field changes nothing until Enter
    rig.type_into(d, "5");
    rig.run();
    let log = rig.take();
    assert_eq!(rig.get(d, "value"), f(1.0));
    assert!(events(&rig, &log, d, "changed").is_empty() && events(&rig, &log, d, "commit").is_empty());
    key(&mut rig, Modifiers::NONE, Key::Enter);
    let log = rig.take();
    assert_eq!(rig.get(d, "value"), f(5.0));
    assert_eq!((events(&rig, &log, d, "changed"), events(&rig, &log, d, "commit")), (vec![vec![f(5.0)]], vec![vec![f(5.0)]]));
    // the live field follows each keystroke and commits once at the end
    rig.type_into(live, "4");
    rig.run();
    let log = rig.take();
    assert_eq!(rig.get(live, "value"), f(4.0));
    assert_eq!(events(&rig, &log, live, "changed"), vec![vec![f(4.0)]]);
    assert!(events(&rig, &log, live, "commit").is_empty());
    key(&mut rig, Modifiers::NONE, Key::Enter);
    assert_eq!(events(&rig, &rig.take(), live, "commit"), vec![vec![f(4.0)]]);
}

#[test]
fn fm1_text_edit_submit_commit_pattern_read_only() {
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let num = rig.leaf(col, "text-edit", &[("pattern", s("[0-9]+")), ("accessible-name", s("Num"))]);
    let ro = rig.leaf(col, "text-edit", &[("text", s("fixed")), ("read-only", Value::Bool(true)), ("accessible-name", s("Ro"))]);
    let free = rig.leaf(col, "text-edit", &[("accessible-name", s("Free"))]);
    for h in [num, ro, free] {
        for e in ["changed", "submit", "commit"] {
            rig.listen(h, e);
        }
    }
    rig.run();
    rig.take();
    // Enter in a one-line field: submit and commit
    rig.type_into(num, "12");
    key(&mut rig, Modifiers::NONE, Key::Enter);
    let log = rig.take();
    assert_eq!((events(&rig, &log, num, "submit"), events(&rig, &log, num, "commit")), (vec![vec![s("12")]], vec![vec![s("12")]]));
    // text that fails the pattern is invalid and finishes nothing
    rig.type_into(num, "1a");
    let node = rig.harness.get_by_label("Num");
    assert_eq!(node.accesskit_node().data().invalid(), Some(egui::accesskit::Invalid::True), "AccessKit invalid");
    key(&mut rig, Modifiers::NONE, Key::Enter);
    let log = rig.take();
    assert_eq!(events(&rig, &log, num, "changed"), vec![vec![s("1a")]]);
    assert!(events(&rig, &log, num, "submit").is_empty() && events(&rig, &log, num, "commit").is_empty());
    // a pattern that does not compile is refused
    let pid = rig.prop_id(num, "pattern");
    assert!(rig.br().set_prop(num, pid, &s("(")).is_err());
    // read-only: focusable and selectable, typing changes nothing
    rig.type_into(ro, "x");
    let log = rig.take();
    assert_eq!(rig.get(ro, "text"), s("fixed"));
    assert!(events(&rig, &log, ro, "changed").is_empty());
    // losing focus after an edit commits (no submit)
    rig.type_into(free, "abc");
    rig.click(ro);
    let log = rig.take();
    assert_eq!(events(&rig, &log, free, "commit"), vec![vec![s("abc")]]);
    assert!(events(&rig, &log, free, "submit").is_empty());
    // losing focus with no edit commits nothing
    rig.click(free);
    rig.click(ro);
    assert!(events(&rig, &rig.take(), free, "commit").is_empty());
}

#[test]
fn fm1_text_edit_submits_when_enter_goes_to_the_default_button() {
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let t = rig.leaf(col, "text-edit", &[("accessible-name", s("Name"))]);
    let ok = rig.leaf(col, "button", &[("text", s("OK")), ("default", Value::Bool(true))]);
    rig.listen(t, "submit");
    rig.listen(ok, "clicked");
    rig.run();
    rig.type_into(t, "q");
    key(&mut rig, Modifiers::NONE, Key::Enter);
    rig.run();
    let log = rig.take();
    assert_eq!(events(&rig, &log, t, "submit"), vec![vec![s("q")]]);
    assert_eq!(events(&rig, &log, ok, "clicked").len(), 1);
}

#[test]
fn fm1_box_button_order_follows_the_platform() {
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let row = rig.leaf(col, "box", &[("flex-direction", s("row")), ("gap", f(4.0)), ("button-order", s("windows"))]);
    let ok = rig.leaf(row, "button", &[("text", s("OK")), ("default", Value::Bool(true))]);
    let cancel = rig.leaf(row, "button", &[("text", s("Cancel")), ("cancel", Value::Bool(true))]);
    let help = rig.leaf(row, "button", &[("text", s("Help")), ("dialog-role", s("help"))]);
    rig.run();
    let xs = |rig: &Rig| {
        let mut v = [(rig.drawn(help).min.x, "help"), (rig.drawn(ok).min.x, "ok"), (rig.drawn(cancel).min.x, "cancel")];
        v.sort_by(|a, b| a.0.total_cmp(&b.0));
        v.map(|(_, n)| n)
    };
    assert_eq!(xs(&rig), ["help", "ok", "cancel"]);
    rig.set(row, "button-order", s("macos"));
    rig.run();
    assert_eq!(xs(&rig), ["help", "cancel", "ok"]);
    // a role change re-lays the box out
    rig.set(help, "dialog-role", s("accept"));
    rig.set(ok, "dialog-role", s("help"));
    rig.run();
    assert_eq!(xs(&rig), ["ok", "cancel", "help"]);
    // none: tree order
    rig.set(row, "button-order", s("none"));
    rig.run();
    assert_eq!(xs(&rig), ["ok", "cancel", "help"]);
}

#[test]
fn fm1_menu_key_and_shift_f10_open_the_context_menu() {
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let panel = rig.leaf(col, "box", &[("padding", f(8.0))]);
    let field = rig.leaf(panel, "text-edit", &[("accessible-name", s("Field"))]);
    let cm = rig.leaf(panel, "context-menu", &[]);
    rig.leaf(cm, "action", &[("label", s("Clear all"))]);
    let other = rig.leaf(col, "text-edit", &[("accessible-name", s("Other"))]);
    rig.run();
    let has = |rig: &Rig| rig.harness.query_by_label("Clear all").is_some();
    // focus outside the owner: nothing
    rig.click(other);
    key(&mut rig, Modifiers::SHIFT, Key::F10);
    assert!(!has(&rig));
    // focus inside it: Shift+F10 opens it
    rig.click(field);
    key(&mut rig, Modifiers::SHIFT, Key::F10);
    rig.run();
    assert!(has(&rig), "Shift+F10");
    key(&mut rig, Modifiers::NONE, Key::Escape);
    rig.run();
    assert!(!has(&rig));
    rig.click(field);
    key(&mut rig, Modifiers::NONE, Key::Menu);
    rig.run();
    assert!(has(&rig), "the Menu key");
}

#[test]
fn fm1_table_header_sorts_hides_and_saves_state() {
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let t = rig.leaf(col, "table", &[("row-count", Value::I64(3)), ("sortable", Value::Bool(true)), ("accessible-name", s("Parts")), ("height", f(200.0))]);
    for (title, name) in [("Ref", "ref"), ("Value", "value"), ("Footprint", "")] {
        rig.leaf(t, "table-column", &[("title", s(title)), ("name", s(name)), ("width", f(80.0))]);
    }
    for e in ["sort-changed", "columns-changed", "header-state-changed", "header-clicked"] {
        rig.listen(t, e);
    }
    rig.observe(t, "header-state", true);
    rig.run();
    rig.take();
    let head = |rig: &Rig, label: &str| rig.harness.get_by_role_and_label(Role::ColumnHeader, label).rect().center();
    // a header click sorts ascending, a second one flips it
    let p = head(&rig, "Value");
    click_at(&mut rig, p, PointerButton::Primary);
    let log = rig.take();
    assert_eq!(events(&rig, &log, t, "sort-changed"), vec![vec![Value::I64(1), s("ascending")]]);
    assert_eq!(events(&rig, &log, t, "header-clicked"), vec![vec![Value::I64(1)]]);
    let state = |rig: &Rig| -> serde_json::Value {
        let Value::Str(st) = rig.get(t, "header-state") else { panic!() };
        serde_json::from_str(&st).unwrap()
    };
    assert_eq!(state(&rig)["sort"], serde_json::json!({"column": "value", "order": "ascending"}));
    assert_eq!(events(&rig, &log, t, "header-state-changed").len(), 1);
    click_at(&mut rig, p, PointerButton::Primary);
    assert_eq!(rig.get(t, "sort-order"), s("descending"));
    rig.take();
    // the column menu hides a column (by name; one without a name by its index)
    let p = head(&rig, "Ref");
    click_at(&mut rig, p, PointerButton::Secondary);
    let item = rig.harness.get_by_role_and_label(Role::CheckBox, "Footprint").rect().center();
    click_at(&mut rig, item, PointerButton::Primary);
    rig.run();
    let log = rig.take();
    assert_eq!(events(&rig, &log, t, "columns-changed"), vec![vec![Value::List(vec![s("2")])]]);
    assert!(rig.harness.query_by_role_and_label(Role::ColumnHeader, "Footprint").is_none());
    assert_eq!(state(&rig)["hidden"], serde_json::json!(["2"]));
    let saved = rig.get(t, "header-state");
    // a saved state restores: widths, hidden columns and the sort
    let restored = r#"{"widths":{"ref":150},"hidden":["value"],"sort":{"column":"ref","order":"descending"}}"#;
    rig.set(t, "header-state", s(restored));
    rig.run();
    rig.run();
    assert_eq!((rig.get(t, "sort-column"), rig.get(t, "sort-order")), (Value::I64(0), s("descending")));
    assert_eq!(rig.get(t, "hidden-columns"), Value::List(vec![s("value")]));
    assert!(rig.harness.query_by_role_and_label(Role::ColumnHeader, "Value").is_none());
    assert!(rig.harness.query_by_role_and_label(Role::ColumnHeader, "Footprint").is_some());
    let w = rig.harness.get_by_role_and_label(Role::ColumnHeader, "Ref").rect().width();
    assert!((w - 150.0).abs() < 2.0, "restored width {w}");
    let log = rig.take();
    assert!(events(&rig, &log, t, "header-state-changed").is_empty(), "the host's own state is not echoed");
    assert_ne!(saved, rig.get(t, "header-state"));
}
