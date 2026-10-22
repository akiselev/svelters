//! RT2 (plans/milestones/RT2-rich-text.md; WIDGET-PARITY Q4): a `label` with run children
//! (`span`, `b`, `i`, `code`, `a`) is one paragraph of styled runs, and its `a` runs are links
//! (clickable, focusable, AccessKit `Link`). Headless (kittest, no JS: `common::Rig`).

mod common;

use common::Rig;
use egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use svelters_abi::*;
use svelters_egui::rich::Rich;
use svelters_egui::scene::W;

fn s(v: &str) -> Value {
    Value::Str(v.into())
}

/// `Hello <span class="warn">careful</span> <b>bold</b> <i>it</i> <code>x</code> <a href>docs</a>`
fn paragraph(rig: &Rig, col: svl_handle) -> (svl_handle, [svl_handle; 6]) {
    let label = rig.leaf(col, "label", &[]);
    let mut runs = [0; 6];
    for (k, (tag, text)) in [("span", "Hello "), ("span", "careful "), ("b", "bold "), ("i", "it "), ("code", "x"), ("a", " docs")].into_iter().enumerate() {
        runs[k] = rig.leaf(label, tag, &[("text", s(text))]);
    }
    rig.set(runs[5], "href", s("https://svelters.dev/docs"));
    rig.backend.set_classes(runs[1].into(), "warn").unwrap();
    (label, runs)
}

fn rich(rig: &Rig, h: svl_handle) -> Rich {
    rig.backend.with_scene(|s| match &s.n(h).w {
        W::Label { rich: Some(r), .. } => (**r).clone(),
        other => panic!("not a rich label: {other:?}"),
    })
}

#[test]
fn runs_are_one_paragraph_styled_per_run() {
    let mut rig = Rig::new();
    rig.backend.add_sheet("app", ".warn { color: #ff0000; } label code { background-color: #202020; } a { text-decoration-line: underline; }");
    let (_w, col) = rig.window();
    let (label, runs) = paragraph(&rig, col);
    rig.run();
    let r = rich(&rig, label);
    assert_eq!(r.plain, "Hello careful bold it x docs");
    assert_eq!(r.runs.len(), 6, "{r:?}");
    let f = |k: usize| r.runs[k].fmt;
    assert_eq!(f(1).color, Some(egui::Color32::from_rgb(255, 0, 0)), "`.warn` reaches its run");
    assert_eq!(f(0).color, None::<egui::Color32>.or(f(0).color), "other runs keep the label's colour");
    assert_ne!(f(0).color, f(1).color);
    assert_eq!(f(2).weight, 700, "b is bold");
    assert!(f(3).italic, "i is italic");
    assert!(f(4).mono && f(4).background == Some(egui::Color32::from_rgb(0x20, 0x20, 0x20)), "code: monospace, and its CSS background");
    assert!(f(5).underline, "a: its CSS decoration");
    assert!(r.runs[5].link.is_some() && r.runs[..5].iter().all(|x| x.link.is_none()));
    // one label node whose text is all runs, and one link node
    rig.harness.get_by_role_and_label(Role::Label, "Hello careful bold it x docs");
    rig.harness.get_by_role_and_label(Role::Link, " docs");
    // one line: the paragraph is wider than its first run alone, and as tall as a line
    let d = rig.drawn(label);
    assert!(d.width() > 100.0 && d.height() < 30.0, "{d:?}");
    let _ = runs;
}

#[test]
fn run_text_is_reactive_and_remeasures_the_label() {
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let (label, runs) = paragraph(&rig, col);
    rig.run();
    let before = rig.drawn(label).width();
    rig.set(runs[2], "text", s("much bolder text "));
    rig.run();
    assert_eq!(rich(&rig, label).plain, "Hello careful much bolder text it x docs");
    assert!(rig.drawn(label).width() > before + 20.0, "the label grew with its run");
    // a run removed leaves the paragraph
    rig.br().remove(label, runs[1]).unwrap();
    rig.run();
    assert_eq!(rich(&rig, label).plain, "Hello much bolder text it x docs");
}

#[test]
fn a_link_is_clickable_focusable_and_hoverable() {
    let mut rig = Rig::new();
    rig.backend.add_sheet("app", "a:hover { color: #00ff00; }");
    let (_w, col) = rig.window();
    let (label, runs) = paragraph(&rig, col);
    rig.listen(runs[5], "clicked");
    rig.run();
    // AccessKit click
    rig.harness.get_by_role_and_label(Role::Link, " docs").click();
    rig.run();
    let l = rig.take();
    assert_eq!(l.events.iter().filter(|e| e.0 == runs[5]).count(), 1, "the link's clicked: {:?}", l.events);
    // pointer: hover sets `:hover` on the run (its rule applies), a click fires clicked
    let link = rig.harness.get_by_role_and_label(Role::Link, " docs");
    link.hover();
    rig.run();
    rig.run();
    assert_eq!(rich(&rig, label).runs[5].fmt.color, Some(egui::Color32::from_rgb(0, 255, 0)), "`a:hover` reaches the hovered link");
    rig.harness.get_by_role_and_label(Role::Link, " docs").click();
    rig.run();
    assert_eq!(rig.take().events.iter().filter(|e| e.0 == runs[5]).count(), 1);
    // keyboard: Tab focuses the link, Enter activates it
    rig.harness.get_by_role_and_label(Role::Link, " docs").focus();
    rig.run();
    assert!(rig.harness.get_by_role_and_label(Role::Link, " docs").is_focused());
    rig.harness.key_press(egui::Key::Enter);
    rig.run();
    assert_eq!(rig.take().events.iter().filter(|e| e.0 == runs[5]).count(), 1, "Enter on the focused link");
}

#[test]
fn a_toast_takes_a_plain_run_list() {
    let mut rig = Rig::new();
    let (_w, col) = rig.window();
    let host = rig.leaf(col, "toast-host", &[]);
    rig.run();
    let runs = Value::List(vec![
        s("Saved "),
        Value::Map(vec![("text".into(), s("report.pdf")), ("code".into(), Value::Bool(true))]),
        s(", "),
        Value::Map(vec![("text".into(), s("open")), ("href".into(), s("file:///tmp/report.pdf"))]),
    ]);
    let m = rig.br().resolve(NameKind::Method, rig.kind(host), "show");
    rig.br().call(host, m, &[runs, s("success"), Value::F64(0.0)]).unwrap();
    rig.run();
    rig.harness.get_by_role_and_label(Role::Alert, "Success: Saved report.pdf, open");
    // the parsed list: four runs, the code one monospace, the last a link
    let r = svelters_egui::rich::from_value(&Value::List(vec![s("a"), Value::Map(vec![("text".into(), s("b")), ("b".into(), Value::Bool(true)), ("href".into(), s("x"))])])).unwrap();
    assert_eq!(r.plain, "ab");
    assert_eq!((r.runs[1].fmt.weight, r.runs[1].href.as_deref()), (700, Some("x")));
    assert!(svelters_egui::rich::from_value(&Value::List(vec![Value::Map(vec![("color".into(), s("red"))])])).is_err(), "unknown fields are errors, not markup");
}
