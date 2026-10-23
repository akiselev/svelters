//! OW7c (plans/milestones/OW7b-toasts-dialog-buttons-status.md, "Status-bar"): the generic
//! `status-bar` kind (sections placed by `status-section`, the message area, `show-message` on the
//! virtual clock, `:open`, AccessKit `Status`) and the `status-tip` feed (`status_tip(ctx)`).
//! Headless (kittest, no JS: `common::Rig`).

mod common;

use common::Rig;
use egui::accesskit::{Live, Role};
use egui_kittest::kittest::{NodeT, Queryable};
use svelters_abi::*;
use svelters_egui::provider::StateBits;

fn s(x: &str) -> Value {
    Value::Str(x.into())
}

fn events(rig: &Rig, h: svl_handle, name: &str) -> Vec<Vec<Value>> {
    let sig = rig.signal_id(h, name);
    rig.log.borrow().events.iter().filter(|e| e.0 == h && e.1 == sig).map(|e| e.2.clone()).collect()
}

fn call(rig: &Rig, h: svl_handle, method: &str, args: &[Value]) -> Value {
    let m = rig.br().resolve(NameKind::Method, rig.kind(h), method);
    rig.br().call(h, m, args).unwrap()
}

/// A window with a body label and a status bar whose sections are given out of order:
/// `[Ln 1 (end), Ready (temporary), NORMAL (start), UTF-8 (end)]`.
fn bar(rig: &mut Rig) -> (svl_handle, svl_handle, [svl_handle; 4]) {
    let (_w, b) = rig.window();
    let body = rig.leaf(b, "label", &[("text", s("Body")), ("status-tip", s("The document"))]);
    let bar = rig.create("status-bar");
    rig.insert(b, bar);
    let ln = rig.leaf(bar, "label", &[("text", s("Ln 1")), ("status-section", s("end"))]);
    let ready = rig.leaf(bar, "label", &[("text", s("Ready"))]);
    let mode = rig.leaf(bar, "label", &[("text", s("NORMAL")), ("status-section", s("start"))]);
    let enc = rig.leaf(bar, "button", &[("text", s("UTF-8")), ("status-section", s("end")), ("status-tip", s("Change the encoding"))]);
    rig.listen(bar, "changed");
    rig.harness.remove_cursor();
    rig.run();
    rig.run();
    (body, bar, [ln, ready, mode, enc])
}

#[test]
fn ow7c_status_bar_sections_message_and_timeout() {
    let mut rig = Rig::new();
    rig.backend.add_sheet("app", "status-bar:open { background-color: #203040; } status-bar::part(message) { color: #ff0000; }");
    let (_body, bar, [ln, ready, mode, enc]) = bar(&mut rig);
    // start, then temporary, then the end sections pushed to the far end, 12 apart
    let [ln_r, ready_r, mode_r, enc_r] = [ln, ready, mode, enc].map(|h| rig.drawn(h));
    assert!(mode_r.max.x <= ready_r.min.x && ready_r.max.x < ln_r.min.x && ln_r.max.x <= enc_r.min.x, "{mode_r:?} {ready_r:?} {ln_r:?} {enc_r:?}");
    assert!((ready_r.min.x - mode_r.max.x - 12.0).abs() < 1.0, "start → temporary gap 12: {mode_r:?} {ready_r:?}");
    assert!(ln_r.min.x - ready_r.max.x > 100.0, "the end sections are pushed to the end: {ready_r:?} {ln_r:?}");
    let status = rig.harness.get_by_role(Role::Status);
    assert_eq!(status.accesskit_node().data().live(), Some(Live::Polite));
    assert!(!rig.backend.with_scene(|sc| sc.n(bar).state).contains(StateBits::OPEN));

    // a message hides the temporary sections, is announced and carried by `changed`
    call(&rig, bar, "show-message", &[s("Saving…"), Value::F64(3.0)]);
    rig.harness.step();
    rig.harness.step();
    assert_eq!(events(&rig, bar, "changed"), vec![vec![s("Saving…")]]);
    assert_eq!(rig.get(bar, "message"), s("Saving…"));
    rig.harness.get_by_role_and_label(Role::Status, "Saving…");
    assert!(rig.harness.query_by_label("Ready").is_none(), "temporary sections hide while a message shows");
    assert!(rig.harness.query_by_label("NORMAL").is_some() && rig.harness.query_by_label("Ln 1").is_some(), "permanent sections stay");
    assert!(rig.backend.with_scene(|sc| sc.n(bar).state).contains(StateBits::OPEN), ":open while a message shows");

    // it times out on the virtual clock
    let mut steps = 0;
    while events(&rig, bar, "changed").len() < 2 {
        rig.harness.step();
        steps += 1;
        assert!(steps < 400, "the message never timed out");
    }
    assert_eq!(events(&rig, bar, "changed")[1], vec![s("")]);
    rig.run();
    assert!(rig.harness.query_by_label("Ready").is_some(), "temporary sections are back");

    // a prop write shows a message until cleared; `clear-message` is announced
    rig.set(bar, "message", s("Pinned"));
    rig.run();
    rig.harness.get_by_role_and_label(Role::Status, "Pinned");
    call(&rig, bar, "clear-message", &[]);
    rig.run();
    assert_eq!(events(&rig, bar, "changed").last(), Some(&vec![s("")]));
    assert!(rig.harness.query_by_label("Ready").is_some());
}

#[test]
fn ow7c_status_tips_follow_hover_and_focus() {
    let mut rig = Rig::new();
    let (body, bar, [_ln, ready, _mode, enc]) = bar(&mut rig);
    rig.set(bar, "message", s("Idle"));
    rig.run();
    assert_eq!(svelters_egui::status_tip(&rig.harness.ctx), None);
    rig.harness.get_by_role_and_label(Role::Status, "Idle");

    // hovering a node with a tip shows it over the message (the body is drawn before the bar)
    rig.harness.hover_at(rig.drawn(body).center());
    rig.run();
    assert_eq!(svelters_egui::status_tip(&rig.harness.ctx).as_deref(), Some("The document"));
    rig.harness.get_by_role_and_label(Role::Status, "The document");
    assert!(rig.harness.query_by_label("Ready").is_none(), "a tip hides the temporary sections too");

    // a section inside the bar (drawn after the bar's own paint started) works the same
    rig.harness.hover_at(rig.drawn(enc).center());
    rig.run();
    assert_eq!(svelters_egui::status_tip(&rig.harness.ctx).as_deref(), Some("Change the encoding"));
    rig.harness.get_by_role_and_label(Role::Status, "Change the encoding");

    // leaving clears it: the message is back
    rig.harness.remove_cursor();
    rig.run();
    assert_eq!(svelters_egui::status_tip(&rig.harness.ctx), None);
    rig.harness.get_by_role_and_label(Role::Status, "Idle");

    // keyboard focus shows the focused node's tip while nothing with a tip is hovered
    let enc_id = rig.backend.with_scene(|sc| sc.n(enc).id);
    rig.harness.ctx.memory_mut(|m| m.request_focus(enc_id));
    rig.run();
    rig.run();
    assert_eq!(svelters_egui::status_tip(&rig.harness.ctx).as_deref(), Some("Change the encoding"));
    // hover wins over focus
    rig.harness.hover_at(rig.drawn(body).center());
    rig.run();
    assert_eq!(svelters_egui::status_tip(&rig.harness.ctx).as_deref(), Some("The document"));
    let _ = ready;
}
