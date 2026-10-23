//! OW7b (plans/milestones/OW7b-toasts-dialog-buttons-status.md): declarative `<toast>` nodes under
//! `toast-host` (per-run CSS, child-button actions, `[level]`, `::part(accent)`, `:hover`,
//! `:exiting`, timeouts on the virtual clock), `dialog-buttons`, `status-bar` and
//! `Policy::auto_default`. Headless (kittest, no JS: `common::Rig`).

mod common;

use common::Rig;
use egui::accesskit::{Live, Role};
use egui_kittest::kittest::{NodeT, Queryable};
use svelters_abi::*;
use svelters_egui::paint::{TraceKind, trace};
use svelters_egui::provider::StateBits;
use svelters_egui::scene::W;

fn s(x: &str) -> Value {
    Value::Str(x.into())
}

fn events(rig: &Rig, h: svl_handle, name: &str) -> Vec<Vec<Value>> {
    let sig = rig.signal_id(h, name);
    rig.log.borrow().events.iter().filter(|e| e.0 == h && e.1 == sig).map(|e| e.2.clone()).collect()
}

fn state(rig: &Rig, h: svl_handle) -> StateBits {
    rig.backend.with_scene(|sc| sc.n(h).state)
}

fn call(rig: &Rig, h: svl_handle, method: &str, args: &[Value]) -> Value {
    let m = rig.br().resolve(NameKind::Method, rig.kind(h), method);
    rig.br().call(h, m, args).unwrap()
}

/// `<toast level duration><label>Disk <b>full</b> <a href>Details</a></label><button>Retry</button></toast>`
fn toast(rig: &Rig, host: svl_handle, level: &str, duration: f64, word: &str) -> (svl_handle, svl_handle, svl_handle) {
    let t = rig.create("toast");
    rig.set(t, "level", s(level));
    rig.set(t, "duration", Value::F64(duration));
    rig.insert(host, t);
    let label = rig.leaf(t, "label", &[]);
    rig.leaf(label, "span", &[("text", s("Disk "))]);
    rig.leaf(label, "b", &[("text", s(word))]);
    let a = rig.leaf(label, "a", &[("text", s(" Details"))]);
    rig.set(a, "href", s("https://svelters.dev"));
    let retry = rig.leaf(t, "button", &[("text", s("Retry"))]);
    for e in ["closed", "changed", "clicked"] {
        rig.listen(t, e);
    }
    rig.listen(retry, "clicked");
    (t, label, retry)
}

#[test]
fn ow7b_declarative_toasts_style_per_run_stack_and_close() {
    let mut rig = Rig::new();
    rig.backend.add_sheet(
        "app",
        "toast b { color: #ff0000; } toast[level=warning] { border-color: #00ff00; } toast::part(accent) { width: 6px; } \
         toast:hover { background-color: #0000ff; } toast { transition: opacity 2s; } toast:exiting { opacity: 0; }",
    );
    let (_w, b) = rig.window();
    let host = rig.leaf(b, "toast-host", &[]);
    rig.listen(host, "closed");
    let (warn, label, retry) = toast(&rig, host, "warning", 0.0, "full");
    let (info, _, _) = toast(&rig, host, "info", 3.0, "fine");
    trace::enable(&rig.harness.ctx, true);
    rig.harness.remove_cursor();
    rig.run();
    // an imperative toast stacks with them (after the children)
    call(&rig, host, "show", &[s("Saved"), s("success"), Value::F64(0.0)]);
    rig.run();
    rig.run();

    // per-run CSS: the `b` run inside the toast's label is red
    let red = rig.backend.with_scene(|sc| match &sc.n(label).w {
        W::Label { rich: Some(r), .. } => r.runs.iter().find(|x| x.fmt.weight == 700).and_then(|x| x.fmt.color),
        other => panic!("not a rich label: {other:?}"),
    });
    assert_eq!(red, Some(egui::Color32::from_rgb(255, 0, 0)), "`toast b` reaches its run");

    // the look: own boxes, `[level=warning]` border, a 6px accent bar; stacked, children first
    let t = trace::last(&rig.harness.ctx);
    let boxes: Vec<_> = t.of(warn, None).filter(|e| e.kind == TraceKind::Box).collect();
    assert!(boxes.iter().any(|e| e.stroke.color == egui::Color32::from_rgb(0, 255, 0)), "{boxes:?}");
    let bar = t.of(warn, Some("accent")).find(|e| e.kind == TraceKind::Box).expect("accent bar");
    assert_eq!(bar.rect.width(), 6.0);
    let (rw, ri, rs) = (rig.drawn(warn), rig.drawn(info), rig.drawn(host));
    assert!(rw.max.y < ri.min.y && rw.max.x > 400.0, "stacked bottom-right: {rw:?} {ri:?}");
    assert!(rs.max.y > ri.max.y, "the `show` toast is lowest: {rs:?}");
    // the content is inside the frame, right of the accent
    assert!(rw.contains_rect(rig.drawn(label)) && rig.drawn(label).min.x > bar.rect.max.x);

    // AccessKit: an Alert named by level and text, assertive for a warning; three in all
    let alert = rig.harness.get_by_role_and_label(Role::Alert, "Warning: Disk full Details");
    assert_eq!(alert.accesskit_node().data().live(), Some(Live::Assertive));
    rig.harness.get_by_role_and_label(Role::Alert, "Info: Disk fine Details");
    rig.harness.get_by_role_and_label(Role::Alert, "Success: Saved");

    // :hover on the toast under the pointer
    rig.harness.hover_at(rw.center());
    rig.run();
    assert!(state(&rig, warn).contains(StateBits::HOVERED));

    // a child button is an action: its own `clicked`, then the toast closes (`action`, open false)
    rig.click(retry);
    rig.run();
    assert_eq!(events(&rig, retry, "clicked").len(), 1);
    assert_eq!(events(&rig, warn, "closed"), vec![vec![s("action")]]);
    assert_eq!(events(&rig, warn, "changed"), vec![vec![Value::Bool(false)]]);
    assert_eq!(rig.get(warn, "open"), Value::Bool(false));

    // the timeout runs on the virtual clock; then `:exiting` while its transition plays out
    rig.harness.remove_cursor();
    let mut steps = 0;
    while events(&rig, info, "closed").is_empty() {
        rig.harness.step();
        steps += 1;
        assert!(steps < 400, "the info toast never timed out");
    }
    assert_eq!(events(&rig, info, "closed"), vec![vec![s("timeout")]]);
    assert!(state(&rig, info).contains(StateBits::EXITING), "exiting while the transition plays");
    assert!(rig.harness.query_by_label("Info: Disk fine Details").is_some(), "still drawn while exiting");
    for _ in 0..200 {
        rig.harness.step();
        if !state(&rig, info).contains(StateBits::EXITING) {
            break;
        }
    }
    rig.run();
    assert!(!state(&rig, info).contains(StateBits::EXITING));
    assert!(rig.harness.query_by_label("Info: Disk fine Details").is_none(), "gone after its exit");
    assert!(events(&rig, host, "closed").is_empty(), "the host's `closed` is for `show` toasts");

    // `open = true` shows it again
    rig.set(warn, "open", Value::Bool(true));
    rig.run();
    rig.run();
    rig.harness.get_by_role_and_label(Role::Alert, "Warning: Disk full Details");
}

/// `<modal><dialog-buttons>` with Help, OK (default), Cancel (cancel), Delete (destructive), Apply.
fn button_row(rig: &mut Rig, order: &str) -> (svl_handle, svl_handle, Vec<svl_handle>) {
    let (_w, b) = rig.window();
    let m = rig.create("modal");
    rig.insert(b, m);
    let row = rig.create("dialog-buttons");
    rig.set(row, "order", s(order));
    rig.insert(m, row);
    let mut v = Vec::new();
    for (text, extra) in [("Help", ("dialog-role", s("help"))), ("OK", ("default", Value::Bool(true))), ("Cancel", ("cancel", Value::Bool(true))), ("Delete", ("dialog-role", s("destructive"))), ("Apply", ("dialog-role", s("apply")))] {
        v.push(rig.leaf(row, "button", &[("text", s(text)), extra]));
    }
    for e in ["clicked", "help"] {
        rig.listen(row, e);
    }
    rig.listen(m, "closed");
    rig.set(m, "open", Value::Bool(true));
    rig.run();
    rig.run();
    (m, row, v)
}

/// The buttons' labels left to right.
fn laid_order(rig: &Rig, v: &[svl_handle]) -> Vec<&'static str> {
    let names = ["Help", "OK", "Cancel", "Delete", "Apply"];
    let mut k: Vec<usize> = (0..v.len()).collect();
    k.sort_by(|a, b| rig.drawn(v[*a]).min.x.total_cmp(&rig.drawn(v[*b]).min.x));
    k.into_iter().map(|i| names[i]).collect()
}

#[test]
fn ow7b_dialog_buttons_platform_order_roles_and_results() {
    for (order, want) in [
        ("windows", ["Help", "OK", "Cancel", "Delete", "Apply"]),
        ("kde", ["Help", "OK", "Apply", "Delete", "Cancel"]),
        ("gnome", ["Help", "Delete", "Apply", "Cancel", "OK"]),
        ("macos", ["Help", "Delete", "Apply", "Cancel", "OK"]),
    ] {
        let mut rig = Rig::new();
        let (_m, _row, v) = button_row(&mut rig, order);
        assert_eq!(laid_order(&rig, &v), want, "{order}");
    }
    // `platform` follows Policy::button_order (GTK: GNOME's)
    let mut rig = Rig::new();
    svelters_egui::native::set(&rig.harness.ctx, svelters_egui::native::Native::Gtk);
    let (m, row, v) = button_row(&mut rig, "platform");
    assert_eq!(laid_order(&rig, &v), ["Help", "Delete", "Apply", "Cancel", "OK"]);
    // right-aligned in the row, 6 apart
    let (ok, cancel) = (rig.drawn(v[1]), rig.drawn(v[2]));
    assert_eq!(ok.min.x - cancel.max.x, 6.0);
    // Tab order stays the tree's: Help, OK, Cancel…
    // clicks report the role; help also `help`; accept closes the modal with "accept"
    rig.click(v[0]);
    rig.run();
    assert_eq!(events(&rig, row, "clicked"), vec![vec![s("help")]]);
    assert_eq!(events(&rig, row, "help").len(), 1);
    rig.click(v[1]);
    rig.run();
    rig.run();
    assert_eq!(events(&rig, row, "clicked").last(), Some(&vec![s("accept")]));
    assert_eq!(events(&rig, m, "closed"), vec![vec![s("accept")]]);
}

#[test]
fn ow7b_auto_default_moves_default_to_the_focused_button() {
    // Windows: the focused button in a dialog is the default (`:default`), and Enter presses it
    let mut rig = Rig::new();
    svelters_egui::native::set(&rig.harness.ctx, svelters_egui::native::Native::Windows);
    let (_m, row, v) = button_row(&mut rig, "windows");
    rig.listen(v[4], "clicked");
    rig.listen(v[1], "clicked");
    // the dialog's initial focus (its first button) is the default; one button shows it
    assert_eq!(v.iter().filter(|h| state(&rig, **h).contains(StateBits::DEFAULT)).count(), 1);
    let apply = rig.backend.with_scene(|sc| sc.n(v[4]).id);
    rig.harness.ctx.memory_mut(|m| m.request_focus(apply));
    rig.run();
    rig.run();
    assert!(state(&rig, v[4]).contains(StateBits::DEFAULT) && !state(&rig, v[1]).contains(StateBits::DEFAULT), "the focused Apply is the default");
    rig.harness.key_press(egui::Key::Enter);
    rig.run();
    assert_eq!(events(&rig, v[4], "clicked").len(), 1);
    assert!(events(&rig, v[1], "clicked").is_empty());
    let _ = row;

    // macOS: no auto-default; Enter goes to OK whatever button is focused
    let mut rig = Rig::new();
    svelters_egui::native::set(&rig.harness.ctx, svelters_egui::native::Native::Mac);
    let (_m, _row, v) = button_row(&mut rig, "macos");
    rig.listen(v[4], "clicked");
    rig.listen(v[1], "clicked");
    let apply = rig.backend.with_scene(|sc| sc.n(v[4]).id);
    rig.harness.ctx.memory_mut(|m| m.request_focus(apply));
    rig.run();
    rig.run();
    assert!(!state(&rig, v[4]).contains(StateBits::DEFAULT) && state(&rig, v[1]).contains(StateBits::DEFAULT));
    rig.harness.key_press(egui::Key::Enter);
    rig.run();
    assert_eq!(events(&rig, v[1], "clicked").len(), 1, "Enter pressed OK");
    assert!(events(&rig, v[4], "clicked").is_empty());
}
