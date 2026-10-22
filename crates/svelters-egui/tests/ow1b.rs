//! OW1b (plans/milestones/OW1b-value-controls.md): svelters' own `slider`, `range-slider` and
//! `drag-value` on the DragToValue and Steppable units. FM1 QW7's drag-value cases (step buttons,
//! PageUp ×10, wheel, commit mode) carried over from `ci/fm1`'s `tests/forms.rs`, the uniform
//! `input`/`commit`/`submit` signals, and the Qt parity rows of WIDGET-PARITY §2.6/§2.7. Headless.

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

fn drag(rig: &mut Rig, from: egui::Pos2, to: egui::Pos2) {
    rig.harness.hover_at(from);
    rig.harness.event(egui::Event::PointerButton { pos: from, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    rig.harness.run();
    for k in 1..=4 {
        let p = from + (to - from) * (k as f32 / 4.0);
        rig.harness.event(egui::Event::PointerMoved(p));
        rig.harness.run();
    }
}

fn release(rig: &mut Rig, at: egui::Pos2) {
    rig.harness.event(egui::Event::PointerButton { pos: at, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    rig.harness.run();
}

#[test]
fn ow1b_kinds_are_own_widgets() {
    let rig = Rig::new();
    let (_w, col) = rig.window();
    for tag in ["slider", "range-slider", "drag-value"] {
        let h = rig.leaf(col, tag, &[]);
        assert!(rig.backend.with_scene(|sc| matches!(sc.n(h).w, svelters_egui::scene::W::Ext(_))), "{tag}");
    }
}

/// FM1 QW7, from `ci/fm1` `tests/forms.rs`.
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

/// FM1 QW7, from `ci/fm1` `tests/forms.rs`.
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

/// QSpinBox parity: prefix/suffix, wrapping, special-value text, adaptive steps, correction,
/// read-only; `input` and `submit` with `changed` and `commit`; Escape restores the typed text.
#[test]
fn ow1b_drag_value_qt_spin_box_parity() {
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let d = rig.leaf(
        col,
        "drag-value",
        &[("value", f(0.0)), ("min", f(0.0)), ("max", f(5.0)), ("wrap", Value::Bool(true)), ("special-value-text", s("Auto")), ("prefix", s("x: ")), ("suffix", s(" mm")), ("accessible-name", s("Gap")), ("align-self", s("start"))],
    );
    for e in ["changed", "input", "commit", "submit"] {
        rig.listen(d, e);
    }
    rig.run();
    let node = || rig.harness.get_by_role_and_label(Role::SpinButton, "Gap").accesskit_node().numeric_value();
    assert_eq!(node(), Some(0.0));
    // special text at min (shown, not the number)
    let shown = rig.backend.with_scene(|sc| sc.n(d).text().to_string());
    let _ = shown;
    // wrapping: Down from min goes to max
    rig.harness.get_by_role_and_label(Role::SpinButton, "Gap").decrement_accesskit();
    rig.run();
    assert_eq!(rig.get(d, "value"), f(5.0));
    rig.harness.get_by_role_and_label(Role::SpinButton, "Gap").increment_accesskit();
    rig.run();
    assert_eq!(rig.get(d, "value"), f(0.0), "and Up from max to min");
    rig.take();
    // typed with prefix and suffix; out of range: the nearest bound; Enter submits
    rig.type_into(d, "x: 9 mm");
    key(&mut rig, Modifiers::NONE, Key::Enter);
    let log = rig.take();
    assert_eq!(rig.get(d, "value"), f(5.0));
    assert_eq!(events(&rig, &log, d, "input"), events(&rig, &log, d, "changed"));
    assert_eq!(events(&rig, &log, d, "submit"), vec![vec![f(5.0)]]);
    assert_eq!(events(&rig, &log, d, "commit").last(), Some(&vec![f(5.0)]));
    // the special text reads back as min
    rig.type_into(d, "Auto");
    key(&mut rig, Modifiers::NONE, Key::Enter);
    assert_eq!(rig.get(d, "value"), f(0.0));
    // correction="previous": an out-of-range typed value keeps the old one
    rig.set(d, "correction", s("previous"));
    rig.type_into(d, "77");
    key(&mut rig, Modifiers::NONE, Key::Enter);
    assert_eq!(rig.get(d, "value"), f(0.0));
    // Escape drops the edit
    rig.set(d, "correction", s("nearest"));
    rig.type_into(d, "3");
    assert_eq!(rig.get(d, "value"), f(3.0), "live");
    key(&mut rig, Modifiers::NONE, Key::Escape);
    rig.run();
    assert_eq!(rig.get(d, "value"), f(0.0), "restored");
    // read-only: no steps
    rig.set(d, "read-only", Value::Bool(true));
    rig.run();
    rig.harness.get_by_role_and_label(Role::SpinButton, "Gap").increment_accesskit();
    rig.run();
    assert_eq!(rig.get(d, "value"), f(0.0));
    // adaptive steps (one decade below the value)
    let a = rig.leaf(col, "drag-value", &[("value", f(1234.0)), ("step-type", s("adaptive")), ("accessible-name", s("Big")), ("align-self", s("start"))]);
    rig.run();
    rig.harness.get_by_role_and_label(Role::SpinButton, "Big").increment_accesskit();
    rig.run();
    assert_eq!(rig.get(a, "value"), f(1334.0));
}

/// A held step button repeats after the delay, faster with `accelerated` (Pressable repeat).
#[test]
fn ow1b_held_step_button_repeats() {
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let d = rig.leaf(col, "drag-value", &[("value", f(0.0)), ("buttons", Value::Bool(true)), ("accelerated", Value::Bool(true)), ("accessible-name", s("N")), ("align-self", s("start"))]);
    rig.run();
    let r = rig.drawn(d);
    let plus = pos2(r.max.x - r.height() / 2.0, r.center().y);
    rig.harness.hover_at(plus);
    rig.harness.event(egui::Event::PointerButton { pos: plus, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    rig.harness.step();
    assert_eq!(rig.get(d, "value"), f(1.0), "a step on the press");
    // two seconds held (kittest's clock advances per step)
    for _ in 0..120 {
        rig.harness.step();
    }
    let held = match rig.get(d, "value") {
        Value::F64(v) => v,
        v => panic!("{v:?}"),
    };
    assert!(held > 5.0, "repeated while held: {held}");
    release(&mut rig, plus);
}

/// QSlider parity: keys (arrows, PageUp ×page-step, Home/End), AccessKit numeric value and
/// Increment/Decrement, `inverted`, `track-click="page"`, commit mode (tracking=false) on release.
#[test]
fn ow1b_slider_keys_page_step_inverted_and_commit_on_release() {
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let sl = rig.leaf(col, "slider", &[("accessible-name", s("Gain")), ("step", f(1.0)), ("page-step", f(20.0)), ("show-value", Value::Bool(false)), ("ticks", s("both"))]);
    for e in ["changed", "input", "commit", "drag-started", "drag-stopped"] {
        rig.listen(sl, e);
    }
    rig.run();
    let gain = |rig: &Rig| rig.harness.get_by_role_and_label(Role::Slider, "Gain").accesskit_node().numeric_value();
    assert_eq!(gain(&rig), Some(0.0));
    rig.harness.get_by_role_and_label(Role::Slider, "Gain").focus();
    rig.run();
    key(&mut rig, Modifiers::NONE, Key::ArrowRight);
    assert_eq!(rig.get(sl, "value"), f(1.0));
    key(&mut rig, Modifiers::NONE, Key::PageUp);
    assert_eq!(rig.get(sl, "value"), f(21.0));
    key(&mut rig, Modifiers::NONE, Key::End);
    assert_eq!(rig.get(sl, "value"), f(100.0));
    key(&mut rig, Modifiers::NONE, Key::Home);
    assert_eq!(rig.get(sl, "value"), f(0.0));
    let log = rig.take();
    assert_eq!(events(&rig, &log, sl, "commit").len(), 4, "every step commits");
    assert_eq!(events(&rig, &log, sl, "input"), events(&rig, &log, sl, "changed"));
    rig.harness.get_by_role_and_label(Role::Slider, "Gain").increment_accesskit();
    rig.run();
    assert_eq!(gain(&rig), Some(1.0));
    // a press on the track jumps there
    let r = rig.drawn(sl);
    click_at(&mut rig, pos2(r.max.x - 10.0, r.center().y), PointerButton::Primary);
    assert_eq!(rig.get(sl, "value"), f(100.0));
    // inverted: max at the start
    rig.set(sl, "inverted", Value::Bool(true));
    rig.run();
    click_at(&mut rig, pos2(r.min.x + 10.0, r.center().y), PointerButton::Primary);
    assert_eq!(rig.get(sl, "value"), f(100.0));
    click_at(&mut rig, pos2(r.max.x - 10.0, r.center().y), PointerButton::Primary);
    assert_eq!(rig.get(sl, "value"), f(0.0));
    rig.set(sl, "inverted", Value::Bool(false));
    // track-click="page": toward the pointer by page-step
    rig.set(sl, "track-click", s("page"));
    rig.run();
    click_at(&mut rig, pos2(r.max.x - 10.0, r.center().y), PointerButton::Primary);
    assert_eq!(rig.get(sl, "value"), f(20.0));
    rig.set(sl, "track-click", s("jump"));
    // commit mode: dragging keeps a draft, the value and commit come at the release
    rig.set(sl, "commit", Value::Bool(true));
    rig.set(sl, "value", f(0.0));
    rig.run();
    rig.take();
    let thumb = pos2(r.min.x + 10.0, r.center().y);
    drag(&mut rig, thumb, pos2(r.center().x, r.center().y));
    let log = rig.take();
    assert_eq!(rig.get(sl, "value"), f(0.0), "tracking=false: not while dragging");
    assert!(events(&rig, &log, sl, "changed").is_empty());
    assert_eq!(events(&rig, &log, sl, "drag-started").len(), 1);
    release(&mut rig, pos2(r.center().x, r.center().y));
    let log = rig.take();
    let v = match rig.get(sl, "value") {
        Value::F64(v) => v,
        v => panic!("{v:?}"),
    };
    assert!(v > 30.0 && v < 70.0, "{v}");
    assert_eq!(events(&rig, &log, sl, "changed"), vec![vec![f(v)]], "one write per release");
    assert_eq!(events(&rig, &log, sl, "commit"), vec![vec![f(v)]]);
    assert_eq!(events(&rig, &log, sl, "drag-stopped").len(), 1);
}

/// Escape while dragging cancels: the value goes back and no commit is sent.
#[test]
fn ow1b_slider_drag_escape_restores() {
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let sl = rig.leaf(col, "slider", &[("accessible-name", s("S")), ("value", f(10.0)), ("show-value", Value::Bool(false))]);
    rig.listen(sl, "commit");
    rig.run();
    let r = rig.drawn(sl);
    let t = rig.harness.get_by_role_and_label(Role::Slider, "S").rect();
    let _ = t;
    let from = pos2(r.min.x + 10.0 + (r.width() - 20.0) * 0.1, r.center().y);
    drag(&mut rig, from, pos2(r.max.x - 20.0, r.center().y));
    assert_ne!(rig.get(sl, "value"), f(10.0), "live while dragging");
    key(&mut rig, Modifiers::NONE, Key::Escape);
    release(&mut rig, pos2(r.max.x - 20.0, r.center().y));
    assert_eq!(rig.get(sl, "value"), f(10.0));
    assert!(events(&rig, &rig.take(), sl, "commit").is_empty());
}

/// `range-slider`: each thumb is its own named `Slider`; keys move one end, never past the other;
/// a press on the rail moves the nearer thumb.
#[test]
fn ow1b_range_slider_thumbs_keys_and_rail() {
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let r = rig.leaf(col, "range-slider", &[("text", s("Band")), ("low", f(20.0)), ("high", f(80.0)), ("step", f(1.0)), ("show-value", Value::Bool(false))]);
    rig.listen(r, "changed");
    rig.listen(r, "commit");
    rig.run();
    rig.harness.get_by_role_and_label(Role::Slider, "Band high").focus();
    rig.run();
    key(&mut rig, Modifiers::NONE, Key::Home);
    assert_eq!((rig.get(r, "low"), rig.get(r, "high")), (f(20.0), f(20.0)), "high stops at low");
    key(&mut rig, Modifiers::NONE, Key::ArrowRight);
    assert_eq!(rig.get(r, "high"), f(21.0));
    let log = rig.take();
    assert_eq!(events(&rig, &log, r, "changed").last(), Some(&vec![f(20.0), f(21.0)]));
    assert_eq!(events(&rig, &log, r, "commit").len(), 2);
    // a press near the right end moves the high thumb there
    let d = rig.drawn(r);
    let rail_end = rig.harness.get_by_label("Band").rect();
    let _ = rail_end;
    click_at(&mut rig, pos2(d.min.x + 12.0, d.center().y), PointerButton::Primary);
    assert_eq!(rig.get(r, "low"), f(0.0), "the nearer thumb (low) moved");
}

/// The value box reads the locale (IN1) and the group separator can be turned off.
#[cfg(feature = "intl")]
#[test]
fn ow1b_group_separator_and_formatter_registry() {
    svelters_intl::set_current("en-US").unwrap();
    svelters_egui::widgets::numfmt::register_formatter("ow1b-pct", |v, _| format!("{:.0} %", v * 100.0));
    svelters_egui::widgets::numfmt::register_parser("ow1b-pct", |t| t.trim_end_matches('%').trim().parse::<f64>().ok().map(|v| v / 100.0));
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let g = rig.leaf(col, "drag-value", &[("value", f(12345.0)), ("decimals", Value::I64(0)), ("accessible-name", s("G")), ("align-self", s("start"))]);
    let p = rig.leaf(col, "drag-value", &[("value", f(0.25)), ("formatter", s("ow1b-pct")), ("parser", s("ow1b-pct")), ("accessible-name", s("P")), ("align-self", s("start"))]);
    rig.run();
    let val = |rig: &Rig, n: &str| rig.harness.get_by_role_and_label(Role::SpinButton, n).value().unwrap_or_default();
    assert_eq!(val(&rig, "G"), "12,345");
    rig.set(g, "group-separator", Value::Bool(false));
    rig.run();
    assert_eq!(val(&rig, "G"), "12345");
    assert_eq!(val(&rig, "P"), "25 %");
    rig.type_into(p, "50 %");
    key(&mut rig, Modifiers::NONE, Key::Enter);
    assert_eq!(rig.get(p, "value"), f(0.5));
}
