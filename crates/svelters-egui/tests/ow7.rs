//! OW7 (plans/milestones/OW7-modal-window-toasts.md): svelters' own `modal`, `inner-window` and
//! toasts. The behaviour both implementations share is in `menus.rs` (`modal_dialog`,
//! `inner_window_with_content_and_close_button`), `focus_keys.rs` (EV3 dialog scopes) and
//! `views.rs` (the toast host); this file covers what only the own ones do: dialog results, the
//! backdrop, nested Escape, the size grip, moving, collapsing, resizing and stacking windows, and
//! the toasts' look, actions and live regions.

mod common;

use common::Rig;
use egui::accesskit::{Live, Role};
use egui_kittest::kittest::{NodeT, Queryable};
use svelters_abi::*;
use svelters_egui::paint::{TraceKind, trace};
use svelters_egui::provider::StateBits;

fn s(x: &str) -> Value {
    Value::Str(x.into())
}

fn events(rig: &Rig, h: svl_handle, name: &str) -> Vec<Vec<Value>> {
    let sig = rig.signal_id(h, name);
    rig.log.borrow().events.iter().filter(|e| e.0 == h && e.1 == sig).map(|e| e.2.clone()).collect()
}

fn has(rig: &Rig, label: &str) -> bool {
    rig.harness.query_by_label(label).is_some()
}

fn press(rig: &mut Rig, pos: egui::Pos2, pressed: bool) {
    rig.harness.event(egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
}

fn click_at(rig: &mut Rig, pos: egui::Pos2) {
    rig.harness.hover_at(pos);
    press(rig, pos, true);
    press(rig, pos, false);
    rig.run();
}

/// A pointer drag from `a` to `b` in steps.
fn drag(rig: &mut Rig, a: egui::Pos2, b: egui::Pos2) {
    rig.harness.hover_at(a);
    rig.run();
    press(rig, a, true);
    rig.run();
    for k in 1..=4 {
        rig.harness.hover_at(a + (b - a) * (k as f32 / 4.0));
        rig.run();
    }
    press(rig, b, false);
    rig.run();
}

fn state(rig: &Rig, h: svl_handle) -> StateBits {
    rig.backend.with_scene(|sc| sc.n(h).state)
}

fn call(rig: &Rig, h: svl_handle, method: &str, args: &[Value]) -> Value {
    let m = rig.br().resolve(NameKind::Method, rig.kind(h), method);
    rig.br().call(h, m, args).unwrap()
}

/// A modal holding a label, a select and an OK button, open.
fn modal(rig: &mut Rig) -> (svl_handle, svl_handle) {
    let (_w, b) = rig.window();
    let back = rig.leaf(b, "button", &[("text", s("Behind"))]);
    let m = rig.create("modal");
    rig.set(m, "title", s("Settings"));
    rig.insert(b, m);
    rig.leaf(m, "label", &[("text", s("Pick one"))]);
    rig.leaf(m, "combo-box", &[("options", s("alpha beta")), ("accessible-name", s("choice"))]);
    rig.leaf(m, "button", &[("text", s("OK")), ("default", Value::Bool(true))]);
    for e in ["closed", "accepted", "rejected"] {
        rig.listen(m, e);
    }
    rig.listen(back, "clicked");
    rig.run();
    (m, back)
}

#[test]
fn ow7_modal_look_backdrop_and_results() {
    let mut rig = Rig::new();
    let (m, back) = modal(&mut rig);
    trace::enable(&rig.harness.ctx, true);
    rig.run();
    let t = trace::last(&rig.harness.ctx);
    let raw = m;
    // the backdrop covers the viewport and dims it; the dialog box is centred on it (R7)
    let bd = t.find(raw, Some("backdrop"), TraceKind::Box).expect("the backdrop part");
    assert!(bd.rect.width() >= 480.0 && bd.rect.height() >= 360.0 && bd.fill.a() > 0, "{bd:?}");
    let frame = rig.drawn(m);
    assert!((frame.center().x - 240.0).abs() < 1.5 && (frame.center().y - 180.0).abs() < 1.5, "centred: {frame:?}");
    let bx = t.find(raw, None, TraceKind::Box).expect("the dialog box");
    assert_eq!(bx.rect, frame);
    assert!(bx.stroke.width > 0.0, "R7: a 1px stroke {bx:?}");
    assert!(t.find(raw, Some("title"), TraceKind::Text).is_some(), "the title part");
    assert!(state(&rig, m).contains(StateBits::OPEN), ":open");
    // AccessKit: a modal Dialog named by its title
    let d = rig.harness.get_by_role(Role::Dialog);
    assert!(d.accesskit_node().data().is_modal());
    assert!(has(&rig, "Settings") && has(&rig, "Pick one"));

    // the backdrop takes presses: nothing behind gets them, and a press there rejects it
    let behind = rig.harness.get_by_label("Behind").rect().center();
    click_at(&mut rig, behind);
    assert!(events(&rig, back, "clicked").is_empty());
    assert!(!has(&rig, "Pick one"));
    assert_eq!(events(&rig, m, "closed"), vec![vec![s("reject")]]);
    assert_eq!(events(&rig, m, "rejected").len(), 1);
    assert_eq!(rig.get(m, "open"), Value::Bool(false));

    // close(result) from the app: `accept` also emits `accepted`
    rig.take();
    rig.set(m, "open", Value::Bool(true));
    rig.run();
    call(&rig, m, "close", &[s("accept")]);
    rig.run();
    assert!(!has(&rig, "Pick one"));
    assert_eq!(events(&rig, m, "closed"), vec![vec![s("accept")]]);
    assert_eq!(events(&rig, m, "accepted").len(), 1);
    assert!(events(&rig, m, "rejected").is_empty());
    assert!(!state(&rig, m).contains(StateBits::OPEN));
}

/// Escape closes one level: a select open inside the modal first, then the modal.
#[test]
fn ow7_modal_escape_closes_the_inner_select_first() {
    let mut rig = Rig::new();
    let (m, _) = modal(&mut rig);
    let sel = rig.harness.get_by_label("choice").rect().center();
    click_at(&mut rig, sel);
    assert!(rig.harness.query_by_role(Role::ListBox).is_some(), "the list is open");
    rig.harness.key_press(egui::Key::Escape);
    rig.run();
    assert!(rig.harness.query_by_role(Role::ListBox).is_none());
    assert!(has(&rig, "Pick one"), "the modal stays");
    rig.harness.key_press(egui::Key::Escape);
    rig.run();
    assert!(!has(&rig, "Pick one"));
    assert_eq!(events(&rig, m, "closed"), vec![vec![s("reject")]]);
}

#[test]
fn ow7_modal_size_grip() {
    let mut rig = Rig::new();
    let (m, _) = modal(&mut rig);
    rig.set(m, "resizable", Value::Bool(true));
    trace::enable(&rig.harness.ctx, true);
    rig.run();
    let raw = m;
    assert!(trace::last(&rig.harness.ctx).find(raw, Some("grip"), TraceKind::Path).is_some(), "::part(grip)");
    let before = rig.drawn(m);
    drag(&mut rig, before.max - egui::vec2(3.0, 3.0), before.max + egui::vec2(57.0, 37.0));
    let after = rig.drawn(m);
    assert!((after.width() - before.width() - 60.0).abs() < 2.0 && (after.height() - before.height() - 40.0).abs() < 2.0, "{before:?} → {after:?}");
    assert_eq!(after.min, before.min, "the top-left stays where it was");
    assert!(has(&rig, "Pick one"));
}

/// An inner window with a field; returns (window, field).
fn window(rig: &mut Rig, b: svl_handle, title: &str, x: f64) -> svl_handle {
    let w = rig.create("inner-window");
    rig.set(w, "title", s(title));
    rig.set(w, "default-x", Value::F64(x));
    rig.set(w, "default-y", Value::F64(40.0));
    rig.set(w, "default-width", Value::F64(200.0));
    rig.set(w, "default-height", Value::F64(140.0));
    rig.insert(b, w);
    rig.leaf(w, "label", &[("text", s(&format!("{title} body")))]);
    w
}

#[test]
fn ow7_inner_window_move_collapse_resize_and_stack() {
    let mut rig = Rig::new();
    let (_w, b) = rig.window();
    let a = window(&mut rig, b, "Alpha", 20.0);
    rig.run();
    let f0 = rig.drawn(a);
    assert_eq!(f0, egui::Rect::from_min_size(egui::pos2(20.0, 40.0), egui::vec2(200.0, 140.0)));
    assert!(rig.harness.query_by_role(Role::Window).is_some());

    // drag the title bar: it moves
    let title = f0.min + egui::vec2(100.0, 15.0);
    drag(&mut rig, title, title + egui::vec2(50.0, 30.0));
    rig.run();
    let f1 = rig.drawn(a);
    assert!((f1.min - f0.min - egui::vec2(50.0, 30.0)).length() < 2.0, "{f0:?} → {f1:?}");

    // the right edge resizes (OW2's Resizable)
    let edge = egui::pos2(f1.right(), f1.center().y);
    drag(&mut rig, edge, edge + egui::vec2(40.0, 0.0));
    rig.run();
    let f2 = rig.drawn(a);
    assert!((f2.width() - 240.0).abs() < 2.0 && f2.min == f1.min, "{f1:?} → {f2:?}");
    // the left edge keeps the right one still
    let edge = egui::pos2(f2.left(), f2.center().y);
    drag(&mut rig, edge, edge + egui::vec2(20.0, 0.0));
    rig.run();
    let f3 = rig.drawn(a);
    assert!((f3.right() - f2.right()).abs() < 1.0 && (f3.width() - 220.0).abs() < 2.0, "{f2:?} → {f3:?}");

    // collapse: only the title bar is left; the content is gone
    let c = rig.harness.get_by_label("Collapse window").rect().center();
    click_at(&mut rig, c);
    rig.run();
    assert!(!has(&rig, "Alpha body") && rig.drawn(a).height() < 40.0);
    let c = rig.harness.get_by_label("Collapse window").rect().center();
    click_at(&mut rig, c);
    rig.run();
    assert!(has(&rig, "Alpha body"));

    // two windows: the newer is on top; a press on the older raises it, `:backdrop` moves
    let z = window(&mut rig, b, "Beta", 250.0);
    rig.run();
    rig.run();
    assert!(state(&rig, a).contains(StateBits::BACKDROP) && !state(&rig, z).contains(StateBits::BACKDROP));
    let bar = rig.drawn(a).min + egui::vec2(30.0, 15.0);
    click_at(&mut rig, bar);
    rig.run();
    assert!(!state(&rig, a).contains(StateBits::BACKDROP) && state(&rig, z).contains(StateBits::BACKDROP));

    // the close button rejects
    rig.listen(z, "closed");
    let close = rig.harness.get_all_by_label("Close window").map(|n| n.rect()).find(|r| r.min.x > rig.drawn(z).center().x).unwrap();
    click_at(&mut rig, close.center());
    assert!(!has(&rig, "Beta body"));
    assert_eq!(events(&rig, z, "closed"), vec![vec![s("reject")]]);
}

#[test]
fn ow7_toasts_look_actions_and_live_regions() {
    let mut rig = Rig::new();
    let (_w, b) = rig.window();
    let host = rig.leaf(b, "toast-host", &[]);
    for e in ["closed", "action", "clicked"] {
        rig.listen(host, e);
    }
    trace::enable(&rig.harness.ctx, true);
    rig.run();
    let id = call(&rig, host, "show", &[s("Disk full"), s("warning"), Value::F64(0.0), Value::List(vec![s("Retry"), s("Ignore")])]);
    call(&rig, host, "show", &[s("Saved"), s("success"), Value::F64(0.0)]);
    rig.run();
    rig.run();
    let t = trace::last(&rig.harness.ctx);
    let raw = host;
    // own surfaces (R7) with a level bar, stacked bottom-right, the newest lowest
    let toasts: Vec<_> = t.of(raw, Some("toast")).filter(|e| e.kind == TraceKind::Box).collect();
    assert_eq!(toasts.len(), 2);
    assert!(toasts[0].rect.max.y < toasts[1].rect.min.y && toasts[1].rect.max.x > 400.0, "{toasts:?}");
    assert!(toasts.iter().all(|e| e.stroke.width > 0.0));
    let bars: Vec<_> = t.of(raw, Some("accent")).filter(|e| e.kind == TraceKind::Box).collect();
    assert_eq!(bars.len(), 2);
    assert_ne!(bars[0].fill, bars[1].fill, "warning and success colours");
    // live regions: a warning interrupts, a success waits
    let warn = rig.harness.get_by_role_and_label(Role::Alert, "Warning: Disk full");
    assert_eq!(warn.accesskit_node().data().live(), Some(Live::Assertive));
    let ok = rig.harness.get_by_role_and_label(Role::Alert, "Success: Saved");
    assert_eq!(ok.accesskit_node().data().live(), Some(Live::Polite));
    // an action: `action(id, index)`, then it closes
    let r = rig.harness.get_by_label("Ignore").rect().center();
    click_at(&mut rig, r);
    rig.run();
    assert_eq!(events(&rig, host, "action"), vec![vec![id.clone(), Value::I64(1)]]);
    assert_eq!(events(&rig, host, "closed"), vec![vec![id, s("action")]]);
    assert!(!has(&rig, "Disk full") && has(&rig, "Saved"));
    // a run list: runs with an href are links; the toast's text is its runs' text
    call(&rig, host, "show", &[Value::List(vec![s("See "), Value::Map(vec![("text".into(), s("log")), ("href".into(), s("https://example.com"))])]), Value::Null, Value::F64(0.0)]);
    rig.run();
    assert!(has(&rig, "See log"));
    // the text takes AccessKit clicks for the body (`clicked`)
    rig.harness.get_by_label("Saved").click();
    rig.run();
    assert_eq!(events(&rig, host, "clicked"), vec![vec![Value::I64(2)]]);
}
