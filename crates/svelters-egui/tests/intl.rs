//! IN1: the numeric widgets show and read numbers in the current locale (the thread's, which
//! `Host::set_locale` sets): `1,5` typed into a slider's value box under de-DE is 1.5, and a
//! locale switch changes the text the box shows. Headless (kittest).
#![cfg(feature = "intl")]

mod common;

use common::Rig;
use egui_kittest::kittest::Queryable;
use svelters_abi::Value;

fn value_box(rig: &Rig) -> String {
    let node = rig.harness.get_by_role(egui::accesskit::Role::SpinButton);
    node.value().unwrap_or_default()
}

#[test]
fn typing_1_comma_5_under_de_de_and_a_locale_switch() {
    svelters_intl::set_current("de-DE").unwrap();
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let sl = rig.leaf(col, "slider", &[("accessible-name", Value::Str("Gain".into())), ("max", Value::F64(10.0)), ("value", Value::F64(2.5))]);
    rig.run();
    assert_eq!(value_box(&rig), "2,50", "the value box shows de-DE's decimal comma (egui picks 2 decimals for a 0..10 slider)");

    // click the value box (edit mode, all selected), type "1,5", Enter
    rig.harness.get_by_role(egui::accesskit::Role::SpinButton).click();
    rig.run();
    rig.harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    rig.harness.event(egui::Event::Text("1,5".into()));
    rig.run();
    rig.harness.key_press(egui::Key::Enter);
    rig.run();
    assert_eq!(rig.get(sl, "value"), Value::F64(1.5));
    assert_eq!(value_box(&rig), "1,50");

    // the host switches to English: the same widget now shows a decimal point
    svelters_intl::set_current("en-US").unwrap();
    rig.run();
    assert_eq!(value_box(&rig), "1.50");
    // and reads English input; a value with grouping parses too
    rig.harness.get_by_role(egui::accesskit::Role::SpinButton).click();
    rig.run();
    rig.harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    rig.harness.event(egui::Event::Text("7.25".into()));
    rig.run();
    rig.harness.key_press(egui::Key::Enter);
    rig.run();
    assert_eq!(rig.get(sl, "value"), Value::F64(7.25));
}

#[test]
fn format_and_parse_follow_the_locale() {
    use svelters_egui::numeric::{format, parse};
    svelters_intl::set_current("de-DE").unwrap();
    assert_eq!(format(1234.5, 0..=3), "1.234,5");
    assert_eq!(format(2.0, 0..=3), "2");
    assert_eq!(format(2.0, 2..=2), "2,00");
    assert_eq!(parse("1,5"), Some(1.5));
    assert_eq!(parse("1.234,5"), Some(1234.5));
    svelters_intl::set_current("fr").unwrap();
    assert_eq!(format(-1234.5, 1..=1), "-1\u{202f}234,5");
    assert_eq!(parse("-1 234,5"), Some(-1234.5));
    svelters_intl::set_current("en").unwrap();
    assert_eq!(format(1234.5, 0..=1), "1,234.5");
    assert_eq!(parse("1,234.5"), Some(1234.5));
}

#[test]
fn drag_value_shows_and_reads_the_locale() {
    svelters_intl::set_current("de-DE").unwrap();
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let dv = rig.leaf(col, "drag-value", &[("accessible-name", Value::Str("Width".into())), ("decimals", Value::F64(1.0)), ("value", Value::F64(1234.5))]);
    rig.run();
    assert_eq!(value_box(&rig), "1.234,5");
    rig.harness.get_by_role(egui::accesskit::Role::SpinButton).click();
    rig.run();
    rig.harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    rig.harness.event(egui::Event::Text("1,5".into()));
    rig.run();
    rig.harness.key_press(egui::Key::Enter);
    rig.run();
    assert_eq!(rig.get(dv, "value"), Value::F64(1.5));
    assert_eq!(value_box(&rig), "1,5");
}
