//! OW1a3 (plans/milestones/OW1a3-basic-widgets-3.md): the own label by default (RT2 runs,
//! selection), remeasure on restyle, `::part()` on the indicator kinds, the public focus-stop API.
//! Headless (kittest, no JS: `common::Rig`).

mod common;

use common::Rig;
use egui::{Event, Modifiers, PointerButton, pos2, vec2};
use svelters_abi::*;
use svelters_egui::provider::StateBits;
use svelters_egui::scene::W;

fn s(v: &str) -> Value {
    Value::Str(v.into())
}

fn own(rig: &Rig, h: svl_handle) -> bool {
    rig.backend.with_scene(|s| matches!(s.n(h).w, W::Ext(_)))
}

fn state(rig: &Rig, h: svl_handle) -> StateBits {
    rig.backend.with_scene(|s| s.n(h).state)
}

fn changed(rig: &Rig, h: svl_handle, name: &str) -> usize {
    let p = rig.prop_id(h, name);
    rig.log.borrow().changed.iter().filter(|c| c.0 == h && c.1 == p).count()
}

fn drag(rig: &mut Rig, from: egui::Pos2, to: egui::Pos2) {
    rig.harness.hover_at(from);
    rig.harness.event(Event::PointerButton { pos: from, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    rig.harness.step();
    rig.harness.hover_at(to);
    rig.harness.step();
    rig.harness.event(Event::PointerButton { pos: to, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    rig.run();
}

#[test]
fn ow1a3_label_is_own_by_default() {
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let l = rig.leaf(col, "label", &[("text", s("Hello"))]);
    rig.run();
    assert!(own(&rig, l), "label is in DEFAULT_OWN");
    assert_eq!(rig.backend.with_scene(|s| s.n(l).text().to_string()), "Hello");
}

#[test]
fn ow1a3_labels_are_not_selectable_by_default() {
    // the theme's egui style: app chrome's egui labels are not selectable (cadatomic)
    let style = svelters_egui::theme::Theme::default().egui_style();
    assert!(!style.interaction.selectable_labels);
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let l = rig.leaf(col, "label", &[("text", s("Chrome text that is not selectable"))]);
    rig.observe(l, "selection-end", true);
    rig.run();
    let r = rig.drawn(l);
    drag(&mut rig, r.left_center() + vec2(2.0, 0.0), r.right_center() - vec2(2.0, 0.0));
    assert_eq!(changed(&rig, l, "selection-end"), 0, "no selection");
    assert!(!state(&rig, l).contains(StateBits::READ_ONLY));
    assert_eq!(rig.get(l, "selection-end"), Value::I64(0));
}

#[test]
fn ow1a3_a_selectable_label_selects_copies_and_binds_its_selection() {
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let text = "Selectable read-only text";
    let l = rig.leaf(col, "label", &[("text", s(text)), ("selectable", Value::Bool(true))]);
    rig.observe(l, "selection-start", true);
    rig.observe(l, "selection-end", true);
    rig.run();
    assert!(state(&rig, l).contains(StateBits::READ_ONLY), ":read-only");
    // pointer: drag across the whole text selects it, and `bind:` hears it
    let r = rig.drawn(l);
    drag(&mut rig, pos2(r.left() + 0.5, r.center().y), pos2(r.right() + 20.0, r.center().y));
    assert_eq!(rig.get(l, "selection-start"), Value::I64(0));
    assert_eq!(rig.get(l, "selection-end"), Value::I64(text.chars().count() as i64));
    assert!(changed(&rig, l, "selection-end") >= 1, "the selection is announced");
    // Ctrl+C (egui's Copy event) copies the selection
    rig.harness.event(Event::Copy);
    rig.harness.step();
    let copied = rig.harness.output().platform_output.commands.iter().any(|c| matches!(c, egui::OutputCommand::CopyText(t) if t == text));
    assert!(copied, "{:?}", rig.harness.output().platform_output.commands);
    // JS sets the selection: chars 0..10 ("Selectable")
    rig.set(l, "selection-start", Value::I64(0));
    rig.set(l, "selection-end", Value::I64(10));
    rig.run();
    rig.harness.event(Event::Copy);
    rig.harness.step();
    let copied = rig.harness.output().platform_output.commands.iter().any(|c| matches!(c, egui::OutputCommand::CopyText(t) if t == "Selectable"));
    assert!(copied, "a JS selection: {:?}", rig.harness.output().platform_output.commands);
}

#[test]
fn ow1a3_a_rich_own_label_runs_restyle_and_remeasure() {
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let l = rig.leaf(col, "label", &[]);
    rig.leaf(l, "span", &[("text", s("Hello "))]);
    let b = rig.leaf(l, "b", &[("text", s("bold"))]);
    rig.run();
    assert!(own(&rig, l));
    assert_eq!(rig.backend.with_scene(|s| s.n(l).text().to_string()), "Hello bold", "the text mirror is all runs");
    let before = rig.drawn(l).width();
    rig.set(b, "text", s("bolder and longer"));
    rig.run();
    assert!(rig.drawn(l).width() > before + 20.0, "a run's text remeasures the own label");
}

#[test]
fn ow1a3_own_leaves_remeasure_when_css_changes_their_font_or_padding() {
    let mut rig = Rig::new();
    rig.backend.add_sheet("app", ".big { font-size: 30px; } .roomy { padding: 20px; }");
    let (_w, col) = rig.window();
    let l = rig.leaf(col, "label", &[("text", s("Grows with its font"))]);
    let b = rig.leaf(col, "button", &[("text", s("Pad me"))]);
    rig.run();
    assert!(own(&rig, l) && own(&rig, b));
    let (l0, b0) = (rig.drawn(l), rig.drawn(b));
    rig.backend.set_classes(l, "big").unwrap();
    rig.backend.set_classes(b, "roomy").unwrap();
    rig.run();
    let (l1, b1) = (rig.drawn(l), rig.drawn(b));
    assert!(l1.height() > l0.height() + 10.0 && l1.width() > l0.width() * 1.5, "font-size: {l0:?} -> {l1:?}");
    assert!(b1.height() >= b0.height() + 20.0, "padding: {b0:?} -> {b1:?}");
    // and back
    rig.backend.set_classes(l, "").unwrap();
    rig.run();
    assert!((rig.drawn(l).height() - l0.height()).abs() < 1.0, "font-size back: {:?}", rig.drawn(l));
}
