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
    assert!((first.rect().left() as f32 - grid.left()).abs() < 1.0, "{:?} in {grid:?}", first.rect());
    let d = rig.harness.get_by_role_and_label(Role::GridCell, "2026-10-22");
    let desc = d.accesskit_node().description().unwrap_or_default().to_string();
    assert!(desc.contains("Thursday 22 October 2026") && desc.contains("milestone"), "{desc}");
    let w = rig.harness.get_by_role_and_label(Role::GridCell, "2026-10-24");
    assert!(w.accesskit_node().description().unwrap_or_default().contains("weekend"));
    assert_eq!(rig.get(c, "first-day"), s("sunday"));
}
