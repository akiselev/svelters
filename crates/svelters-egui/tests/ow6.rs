//! OW6 (plans/milestones/OW6-pickers.md): svelters' own `calendar`, `color-editor`,
//! `date-picker` and `color-picker`. Headless (kittest, no JS: `common::Rig`).

mod common;

use common::Rig;
use egui::accesskit::Role;
use egui_kittest::kittest::{NodeT, Queryable};
use svelters_abi::*;

fn s(v: &str) -> Value {
    Value::Str(v.into())
}

fn events_of(rig: &Rig, log: &common::Log, h: u64, name: &str) -> Vec<Vec<Value>> {
    let sig = rig.signal_id(h, name);
    log.events.iter().filter(|(eh, es, _)| *eh == h && *es == sig).map(|(_, _, a)| a.clone()).collect()
}

#[test]
fn ow6_calendar_picks_roves_and_pages() {
    let mut rig = Rig::new();
    let (_, b) = rig.window();
    let c = rig.leaf(b, "calendar", &[("value", s("2026-10-22")), ("min", s("2026-10-05")), ("max", s("2026-11-20"))]);
    rig.observe(c, "value", true);
    for e in ["changed", "commit", "page-changed"] {
        rig.listen(c, e);
    }
    rig.run();
    let r = rig.drawn(c);
    assert!(r.width() >= 7.0 * 28.0 && r.height() >= 6.0 * 24.0, "measured: {r:?}");

    // AccessKit: a grid of rows of cells named by ISO date; the value is selected
    let cell = rig.harness.get_by_role_and_label(Role::GridCell, "2026-10-22");
    assert!(cell.accesskit_node().is_selected() == Some(true));
    assert_eq!(rig.harness.get_all_by_role(Role::Row).count(), 6);
    // outside min/max: disabled, a click does nothing
    rig.harness.get_by_role_and_label(Role::GridCell, "2026-10-01").click();
    rig.run();
    assert_eq!(rig.get(c, "value"), s("2026-10-22"));
    assert!(rig.take().events.is_empty());

    // a click picks: changed, then commit; bind:value announced
    rig.harness.get_by_role_and_label(Role::GridCell, "2026-10-23").click();
    rig.run();
    assert_eq!(rig.get(c, "value"), s("2026-10-23"));
    let log = rig.take();
    assert_eq!(events_of(&rig, &log, c, "changed"), vec![vec![s("2026-10-23")]]);
    assert_eq!(events_of(&rig, &log, c, "commit"), vec![vec![s("2026-10-23")]]);
    assert!(log.changed.contains(&(c, rig.prop_id(c, "value"))));

    // keys rove (the grid is one tab stop): +7 days, PageDown a month (clamped to max), Enter picks
    rig.harness.get_by_role(Role::Grid).focus();
    rig.run();
    rig.harness.key_press(egui::Key::ArrowDown);
    rig.run();
    rig.harness.key_press(egui::Key::Enter);
    rig.run();
    assert_eq!(rig.get(c, "value"), s("2026-10-30"));
    rig.take();
    rig.harness.key_press(egui::Key::PageDown);
    rig.run();
    let log = rig.take();
    assert_eq!(events_of(&rig, &log, c, "page-changed").len(), 1, "{:?}", log.events);
    assert!(rig.harness.query_by_label("2026-11-20").is_some());
    rig.harness.key_press(egui::Key::Enter);
    rig.run();
    assert_eq!(rig.get(c, "value"), s("2026-11-20"), "PageDown clamps to max");

    // prev pages back; next is disabled past max's month
    rig.harness.get_by_role_and_label(Role::Button, "Previous month").click();
    rig.run();
    assert!(rig.harness.query_by_label("2026-10-15").is_some());
    rig.harness.get_by_role_and_label(Role::Button, "Next month").click();
    rig.run();
    assert!(rig.harness.get_by_role_and_label(Role::Button, "Next month").accesskit_node().is_disabled());

    // read-only roves but never picks
    rig.set(c, "read-only", Value::Bool(true));
    rig.run();
    rig.take();
    rig.harness.get_by_role_and_label(Role::GridCell, "2026-11-10").click();
    rig.run();
    assert_eq!(rig.get(c, "value"), s("2026-11-20"));
    assert!(rig.take().events.is_empty());

    // host writes: ISO only, not echoed; the page follows the value
    assert!(rig.br().set_prop(c, rig.prop_id(c, "value"), &s("22/10/2026")).is_err());
    assert_eq!(rig.get(c, "value"), s("2026-11-20"));
    rig.set(c, "value", s("2026-10-06"));
    rig.run();
    assert!(rig.harness.get_by_role_and_label(Role::GridCell, "2026-10-06").accesskit_node().is_selected() == Some(true));
    assert!(rig.take().changed.is_empty(), "host writes are not echoed");
}

#[test]
fn ow6_calendar_first_day_weekdays_and_day_parts() {
    let mut rig = Rig::new();
    let (_, b) = rig.window();
    let c = rig.leaf(b, "calendar", &[("value", s("2026-10-22")), ("first-day", s("sunday")), ("weekday-format", s("none")), ("calendar-week", Value::Bool(false))]);
    rig.set(c, "day-parts", Value::List(vec![s("2026-10-22 milestone")]));
    rig.run();
    // the grid starts on Sunday 2026-09-27; no weekday row, no week numbers: 7 cells wide
    let first = rig.harness.get_by_role_and_label(Role::GridCell, "2026-09-27");
    let grid = rig.drawn(c);
    assert!((first.rect().left() - grid.left()).abs() < 1.0, "{:?} in {grid:?}", first.rect());
    let d = rig.harness.get_by_role_and_label(Role::GridCell, "2026-10-22");
    let desc = d.accesskit_node().description().unwrap_or_default().to_string();
    assert!(desc.contains("Thursday 22 October 2026") && desc.contains("milestone"), "{desc}");
    let w = rig.harness.get_by_role_and_label(Role::GridCell, "2026-10-24");
    assert!(w.accesskit_node().description().unwrap_or_default().contains("weekend"));
    assert_eq!(rig.get(c, "first-day"), s("sunday"));
}

/// Presses at `from`, moves to `to` in steps (a frame each), releases there.
fn drag(rig: &mut Rig, from: egui::Pos2, to: egui::Pos2) {
    let h = &mut rig.harness;
    let m = egui::Modifiers::NONE;
    h.event(egui::Event::PointerMoved(from));
    h.step();
    h.event(egui::Event::PointerButton { pos: from, button: egui::PointerButton::Primary, pressed: true, modifiers: m });
    h.step();
    for k in 1..=4 {
        h.event(egui::Event::PointerMoved(from + (to - from) * (k as f32 / 4.0)));
        h.step();
    }
    h.event(egui::Event::PointerButton { pos: to, button: egui::PointerButton::Primary, pressed: false, modifiers: m });
    h.step();
    rig.run();
}

#[test]
fn ow6_color_editor_edits_through_square_strips_fields_and_swatches() {
    let mut rig = Rig::new();
    let (_, b) = rig.window();
    let c = rig.leaf(b, "color-editor", &[("value", s("#F00")), ("alpha", s("blend")), ("fields", s("hex rgb hsv"))]);
    rig.set(c, "swatches", Value::List(vec![s("#3366cc"), s("#00ff0080")]));
    rig.observe(c, "value", true);
    rig.observe(c, "custom-colors", true);
    rig.listen(c, "changed");
    rig.listen(c, "commit");
    rig.run();
    assert_eq!(rig.get(c, "value"), s("#ff0000"));
    assert!(rig.br().set_prop(c, rig.prop_id(c, "value"), &s("red")).is_err());
    rig.take();

    // the square: drag past the bottom-right corner = black (saturation 1, value 0); one commit on release
    let sq = rig.harness.get_by_role_and_label(Role::Slider, "Saturation and value").rect();
    let (from, to) = (egui::pos2(sq.center().x, sq.center().y), egui::pos2(sq.max.x + 20.0, sq.max.y + 20.0));
    drag(&mut rig, from, to);
    assert_eq!(rig.get(c, "value"), s("#000000"));
    let log = rig.take();
    assert!(!events_of(&rig, &log, c, "changed").is_empty());
    assert_eq!(events_of(&rig, &log, c, "commit"), vec![vec![s("#000000")]]);
    assert!(log.changed.contains(&(c, rig.prop_id(c, "value"))));

    // the hue survives black: V back to 100 through the V box gives red again
    rig.harness.get_by_role_and_label(Role::SpinButton, "Value").set_value_accesskit(100.0);
    rig.run();
    assert_eq!(rig.get(c, "value"), s("#ff0000"));
    // R box (AccessKit SetValue), hue slider steps
    rig.harness.get_by_role_and_label(Role::SpinButton, "Green").set_value_accesskit(255.0);
    rig.run();
    assert_eq!(rig.get(c, "value"), s("#ffff00"));
    rig.harness.get_by_role_and_label(Role::Slider, "Alpha").set_value_accesskit(50.0);
    rig.run();
    assert_eq!(rig.get(c, "value"), s("#ffff0080"));
    // hex field (AccessKit SetValue until the text entry lands)
    rig.harness.get_by_role_and_label(Role::TextInput, "Hex").set_text_value_accesskit("#123456");
    rig.run();
    assert_eq!(rig.get(c, "value"), s("#123456"));
    // typing in it (the text-input seam): select all, type, Enter applies; a bad hex is :invalid
    let typed = |rig: &mut Rig, t: &str| {
        rig.harness.get_by_role_and_label(Role::TextInput, "Hex").click();
        rig.run();
        rig.harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
        rig.harness.event(egui::Event::Text(t.into()));
        rig.run();
        rig.harness.key_press(egui::Key::Enter);
        rig.run();
    };
    typed(&mut rig, "#abcdef");
    assert_eq!(rig.get(c, "value"), s("#abcdef"));
    typed(&mut rig, "zz");
    assert_eq!(rig.get(c, "value"), s("#abcdef"));
    assert!(rig.harness.get_by_role_and_label(Role::TextInput, "Hex").accesskit_node().data().invalid().is_some(), "a bad hex is :invalid");
    // swatches pick; the add cell appends to custom-colors (bound)
    rig.harness.get_by_role_and_label(Role::Button, "#3366cc").click();
    rig.run();
    assert_eq!(rig.get(c, "value"), s("#3366cc"));
    rig.take();
    rig.harness.get_by_role_and_label(Role::Button, "Add to custom colours").click();
    rig.run();
    assert_eq!(rig.get(c, "custom-colors"), Value::List(vec![s("#3366cc")]));
    assert!(rig.take().changed.contains(&(c, rig.prop_id(c, "custom-colors"))));
    assert!(rig.harness.query_all_by_role_and_label(Role::Button, "#3366cc").count() == 2, "the custom colour shows");

    // keys: the hue slider is a tab stop; Right steps 1%
    rig.harness.get_by_role_and_label(Role::Slider, "Hue").focus();
    rig.run();
    rig.harness.key_press(egui::Key::End);
    rig.run();
    let v = rig.get(c, "value");
    assert_ne!(v, s("#3366cc"), "End moves the hue");
    // host writes are not echoed
    rig.take();
    rig.set(c, "value", s("#00ff00"));
    rig.run();
    assert!(rig.take().changed.is_empty());
    assert_eq!(rig.harness.get_by_role_and_label(Role::SpinButton, "Green").accesskit_node().numeric_value(), Some(255.0));
}

#[test]
fn ow6_date_picker_opens_its_calendar_and_commits() {
    let mut rig = Rig::new();
    let (_, b) = rig.window();
    let d = rig.leaf(b, "date-picker", &[("value", s("2026-10-22")), ("accessible-name", s("Due"))]);
    rig.observe(d, "value", true);
    for e in ["changed", "commit"] {
        rig.listen(d, e);
    }
    rig.run();
    // closed: a button with R14's chevron, not expanded
    let btn = rig.harness.get_by_role_and_label(Role::Button, "Due");
    assert_eq!(btn.accesskit_node().data().is_expanded(), Some(false));
    btn.click();
    rig.run();
    assert_eq!(rig.harness.get_by_role_and_label(Role::Button, "Due").accesskit_node().data().is_expanded(), Some(true));
    // confirm (default): a pick is pending, Cancel drops it
    rig.harness.get_by_role_and_label(Role::GridCell, "2026-10-25").click();
    rig.run();
    assert_eq!(rig.get(d, "value"), s("2026-10-22"));
    rig.harness.get_by_role_and_label(Role::Button, "Cancel").click();
    rig.run();
    assert!(rig.harness.query_by_role_and_label(Role::GridCell, "2026-10-25").is_none(), "closed");
    assert!(rig.take().events.is_empty());

    // keyboard: Alt+Down opens with focus in the grid; arrows rove; Enter saves; focus returns
    rig.harness.get_by_role_and_label(Role::Button, "Due").focus();
    rig.run();
    rig.harness.key_press_modifiers(egui::Modifiers::ALT, egui::Key::ArrowDown);
    rig.run();
    rig.run();
    rig.harness.key_press(egui::Key::ArrowLeft);
    rig.run();
    rig.harness.key_press(egui::Key::Enter);
    rig.run();
    assert_eq!(rig.get(d, "value"), s("2026-10-21"));
    let log = rig.take();
    assert_eq!(events_of(&rig, &log, d, "changed"), vec![vec![s("2026-10-21")]]);
    assert_eq!(events_of(&rig, &log, d, "commit"), vec![vec![s("2026-10-21")]]);
    assert!(rig.harness.get_by_role_and_label(Role::Button, "Due").accesskit_node().is_focused());

    // immediate: a click picks, commits and closes; Escape closes without a pick
    rig.set(d, "commit-mode", s("immediate"));
    rig.set(d, "min", s("2026-10-10"));
    rig.run();
    rig.harness.get_by_role_and_label(Role::Button, "Due").click();
    rig.run();
    assert!(rig.harness.get_by_role_and_label(Role::GridCell, "2026-10-09").accesskit_node().is_disabled(), "min disables earlier days");
    rig.harness.get_by_role_and_label(Role::GridCell, "2026-10-30").click();
    rig.run();
    assert_eq!(rig.get(d, "value"), s("2026-10-30"));
    assert!(rig.harness.query_by_role_and_label(Role::GridCell, "2026-10-30").is_none(), "a pick closes");
    rig.harness.get_by_role_and_label(Role::Button, "Due").click();
    rig.run();
    rig.harness.key_press(egui::Key::Escape);
    rig.run();
    assert!(rig.harness.query_by_role_and_label(Role::GridCell, "2026-10-30").is_none(), "Escape closes");
    assert_eq!(rig.get(d, "value"), s("2026-10-30"));
}

#[test]
fn ow6_color_picker_live_and_confirm() {
    let mut rig = Rig::new();
    let (_, b) = rig.window();
    let c = rig.leaf(b, "color-picker", &[("value", s("#ff0000")), ("accessible-name", s("Layer"))]);
    rig.observe(c, "value", true);
    for e in ["changed", "commit"] {
        rig.listen(c, e);
    }
    rig.run();
    let well = rig.harness.get_by_role(Role::ColorWell);
    assert_eq!(well.accesskit_node().data().is_expanded(), Some(false));
    // live (default): edits change the value at once; closing commits them
    rig.click(c);
    rig.harness.get_by_role_and_label(Role::SpinButton, "Blue").set_value_accesskit(255.0);
    rig.run();
    assert_eq!(rig.get(c, "value"), s("#ff00ff"));
    rig.harness.key_press(egui::Key::Escape);
    rig.run();
    let log = rig.take();
    assert!(events_of(&rig, &log, c, "changed").contains(&vec![s("#ff00ff")]));
    assert_eq!(events_of(&rig, &log, c, "commit").last(), Some(&vec![s("#ff00ff")]));
    assert!(rig.harness.query_by_role_and_label(Role::SpinButton, "Blue").is_none(), "closed");

    // confirm: pending until Save; Cancel drops it
    rig.set(c, "commit-mode", s("confirm"));
    rig.run();
    rig.click(c);
    rig.harness.get_by_role_and_label(Role::SpinButton, "Green").set_value_accesskit(255.0);
    rig.run();
    assert_eq!(rig.get(c, "value"), s("#ff00ff"), "pending");
    rig.harness.get_by_role_and_label(Role::Button, "Cancel").click();
    rig.run();
    assert_eq!(rig.get(c, "value"), s("#ff00ff"));
    rig.click(c);
    rig.harness.get_by_role_and_label(Role::SpinButton, "Green").set_value_accesskit(255.0);
    rig.run();
    rig.harness.get_by_role_and_label(Role::Button, "Save").click();
    rig.run();
    assert_eq!(rig.get(c, "value"), s("#ffffff"));
    let log = rig.take();
    assert_eq!(events_of(&rig, &log, c, "commit").last(), Some(&vec![s("#ffffff")]));
}
