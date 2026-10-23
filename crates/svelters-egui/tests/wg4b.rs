//! WG4b (plans/milestones/WG4b-dialogs-pickers-dnd.md): `date-picker` and `color-picker` with their
//! popups driven by kittest, drag and drop between two nodes, and the dialog exports of
//! `svelters:app/egui` from JS against a fake `DialogBackend` (no display for native dialogs).

mod common;

use common::Rig;
use egui_kittest::kittest::Queryable;
use std::path::PathBuf;
use std::rc::Rc;
use svelters_abi::Value;
use svelters_egui::dialogs::{DialogBackend, DialogFuture, FileDialogOptions, FileMode, MessageBoxOptions};

fn s(x: &str) -> Value {
    Value::Str(x.to_string())
}

fn events_of(rig: &Rig, log: &common::Log, h: u64, name: &str) -> Vec<Vec<Value>> {
    let sig = rig.signal_id(h, name);
    log.events.iter().filter(|(eh, es, _)| *eh == h && *es == sig).map(|(_, _, a)| a.clone()).collect()
}

#[test]
fn date_picker_saves_a_day_through_its_popup() {
    let mut rig = Rig::new();
    let (_, b) = rig.window();
    let d = rig.leaf(b, "date-picker", &[("value", s("2026-10-22"))]);
    rig.observe(d, "value", true);
    rig.listen(d, "changed");
    rig.run();
    assert_eq!(rig.get(d, "value"), s("2026-10-22"));
    assert!(rig.drawn(d).width() > 40.0, "measured: {:?}", rig.drawn(d));

    rig.harness.get_by_label("2026-10-22").click();
    rig.run();
    // OW6: the own calendar's day cells are AccessKit grid cells named by ISO date
    rig.harness.get_by_role_and_label(egui::accesskit::Role::GridCell, "2026-10-23").click(); // one day on
    rig.run();
    rig.harness.get_by_label("Save").click();
    rig.run();
    assert_eq!(rig.get(d, "value"), s("2026-10-23"));
    let log = rig.take();
    assert_eq!(events_of(&rig, &log, d, "changed"), vec![vec![s("2026-10-23")]]);
    assert!(log.changed.contains(&(d, rig.prop_id(d, "value"))), "bind:value is announced");

    // host writes: ISO dates only, "" for none, format is display only
    assert!(rig.br().set_prop(d, rig.prop_id(d, "value"), &s("22/10/2026")).is_err());
    rig.set(d, "format", s("%d.%m.%Y"));
    rig.set(d, "value", s("2027-01-05"));
    rig.run();
    assert!(rig.harness.query_by_label("05.01.2027").is_some());
    assert_eq!(rig.get(d, "value"), s("2027-01-05"));
    rig.set(d, "value", Value::Null);
    assert_eq!(rig.get(d, "value"), s(""));
    assert!(rig.take().changed.is_empty(), "host writes are not echoed");
}

#[test]
fn color_picker_normalizes_hex_and_edits_through_its_popup() {
    let mut rig = Rig::new();
    let (_, b) = rig.window();
    let c = rig.leaf(b, "color-picker", &[("value", s("#F00"))]);
    rig.observe(c, "value", true);
    rig.listen(c, "changed");
    rig.run();
    assert_eq!(rig.get(c, "value"), s("#ff0000"));
    rig.set(c, "value", s("#00ff0080"));
    assert_eq!(rig.get(c, "value"), s("#00ff0080"));
    assert!(rig.br().set_prop(c, rig.prop_id(c, "value"), &s("red")).is_err());
    assert!(rig.br().set_prop(c, rig.prop_id(c, "alpha"), &s("sometimes")).is_err());
    rig.set(c, "value", s("#ff0000"));
    rig.run();
    rig.take();

    // the popup: the own editor's channel boxes (`fields="hex rgb"`), the first is R
    rig.click(c);
    let r = rig.harness.get_all_by_role(egui::accesskit::Role::SpinButton).next().expect("the popup's R value");
    r.set_value_accesskit(0.0);
    rig.run();
    assert_eq!(rig.get(c, "value"), s("#000000"));
    let log = rig.take();
    assert_eq!(events_of(&rig, &log, c, "changed").last(), Some(&vec![s("#000000")]));
    assert!(log.changed.contains(&(c, rig.prop_id(c, "value"))));
}

/// Presses at `from`, moves to `to` in steps (a frame each), releases there.
fn drag_on<S>(h: &mut egui_kittest::Harness<'_, S>, from: egui::Pos2, to: egui::Pos2) {
    let mods = egui::Modifiers::NONE;
    h.event(egui::Event::PointerMoved(from));
    h.step();
    h.event(egui::Event::PointerButton { pos: from, button: egui::PointerButton::Primary, pressed: true, modifiers: mods });
    h.step();
    for k in 1..=8 {
        let p = from + (to - from) * (k as f32 / 8.0);
        h.event(egui::Event::PointerMoved(p));
        h.step();
    }
    // a couple of frames over the target: the claim of the previous pass decides
    h.step();
    h.step();
    h.event(egui::Event::PointerButton { pos: to, button: egui::PointerButton::Primary, pressed: false, modifiers: mods });
    h.run();
}

fn drag(rig: &mut Rig, from: egui::Pos2, to: egui::Pos2) {
    drag_on(&mut rig.harness, from, to);
}

#[test]
fn drag_and_drop_between_two_nodes() {
    let mut rig = Rig::new();
    let (_, b) = rig.window();
    let src = rig.leaf(b, "button", &[("text", s("Part")), ("drag-type", s("cad/part")), ("drag-data", s(r#"{"id":7}"#))]);
    let other = rig.leaf(b, "label", &[("text", s("Notes")), ("drop-types", s("text/plain"))]);
    let dst = rig.leaf(b, "label", &[("text", s("Bin")), ("drop-types", s("cad/*"))]);
    for (h, names) in [(src, &["drag-begin", "drag-end", "clicked"][..]), (dst, &["drag-enter", "drag-leave", "drop"][..]), (other, &["drag-enter", "drag-leave", "drop"][..])] {
        for n in names {
            rig.listen(h, n);
        }
    }
    rig.run();
    assert_eq!(rig.get(dst, "drop-types"), s("cad/*"));

    // a click on the source is still a click
    rig.click(src);
    assert_eq!(events_of(&rig, &rig.take(), src, "clicked").len(), 1);

    let (from, to) = (rig.drawn(src).center(), rig.drawn(dst).center());
    drag(&mut rig, from, to);
    let log = rig.take();
    assert_eq!(events_of(&rig, &log, src, "drag-begin").len(), 1);
    assert_eq!(events_of(&rig, &log, dst, "drag-enter"), vec![vec![s("cad/part")]]);
    assert_eq!(events_of(&rig, &log, dst, "drag-leave").len(), 1);
    let drops = events_of(&rig, &log, dst, "drop");
    assert_eq!(drops.len(), 1, "{log:?}", log = log.events);
    assert_eq!(drops[0][..2], [s("cad/part"), s(r#"{"id":7}"#)]);
    assert_eq!(drops[0][4], s("move"), "WG6: the drop action (no modifiers)");
    let local = rig.drawn(dst).size() / 2.0;
    assert!(matches!((&drops[0][2], &drops[0][3]), (Value::F64(x), Value::F64(y)) if (*x as f32 - local.x).abs() < 2.0 && (*y as f32 - local.y).abs() < 2.0));
    assert_eq!(events_of(&rig, &log, src, "drag-end"), vec![vec![Value::Bool(true), s("move")]]);
    assert!(events_of(&rig, &log, other, "drag-enter").is_empty(), "the target that does not accept the type is passed over");

    // dropped on a target that does not accept it: drag-end(false), no drop
    let to = rig.drawn(other).center();
    drag(&mut rig, from, to);
    let log = rig.take();
    assert_eq!(events_of(&rig, &log, src, "drag-end"), vec![vec![Value::Bool(false), s("none")]]);
    assert!(events_of(&rig, &log, other, "drop").is_empty());
    assert!(events_of(&rig, &log, dst, "drop").is_empty());

    // clearing the props ends both roles
    rig.set(src, "drag-type", Value::Null);
    rig.set(src, "drag-data", Value::Null);
    assert_eq!(rig.get(src, "drag-type"), s(""));
}

#[test]
fn nested_drop_targets_give_the_drop_to_the_innermost() {
    let mut rig = Rig::new();
    let (_, b) = rig.window();
    let src = rig.leaf(b, "label", &[("text", s("Net")), ("drag-type", s("cad/net")), ("drag-data", s("GND"))]);
    let outer = rig.create("box");
    rig.set(outer, "drop-types", s("*"));
    rig.set(outer, "padding", Value::F64(20.0));
    rig.insert(b, outer);
    let inner = rig.leaf(outer, "label", &[("text", s("Pin 1")), ("drop-types", s("cad/net"))]);
    for h in [outer, inner] {
        rig.listen(h, "drop");
    }
    rig.listen(src, "drag-end");
    rig.run();
    let (from, to) = (rig.drawn(src).center(), rig.drawn(inner).center());
    drag(&mut rig, from, to);
    let log = rig.take();
    assert_eq!(events_of(&rig, &log, inner, "drop").len(), 1);
    assert!(events_of(&rig, &log, outer, "drop").is_empty());
    // the outer box's padding is its own
    let to = rig.drawn(inner).left_top() - egui::vec2(10.0, 10.0);
    drag(&mut rig, from, to);
    let log = rig.take();
    assert_eq!(events_of(&rig, &log, outer, "drop").len(), 1, "{:?}", log.events);
    assert_eq!(events_of(&rig, &log, src, "drag-end"), vec![vec![Value::Bool(true), s("move")]]);
}

// ---- dialogs ----

#[derive(Default)]
struct Fake {
    seen: std::cell::RefCell<Vec<String>>,
}

impl DialogBackend for Fake {
    fn files(&self, mode: FileMode, o: FileDialogOptions) -> DialogFuture<Option<Vec<PathBuf>>> {
        self.seen.borrow_mut().push(format!("{mode:?} {o:?}"));
        let r = match mode {
            FileMode::Open if o.title.is_none() => None, // the user cancelled
            FileMode::Open => Some(vec![PathBuf::from("/parts/a.step")]),
            FileMode::OpenMany => Some(vec![PathBuf::from("/parts/a.step"), PathBuf::from("/parts/b.step")]),
            FileMode::Save => Some(vec![PathBuf::from("/out").join(o.file_name.unwrap_or_default())]),
            FileMode::Folder => Some(vec![PathBuf::from("/parts")]),
        };
        Box::pin(std::future::ready(r))
    }

    fn message(&self, o: MessageBoxOptions) -> DialogFuture<String> {
        self.seen.borrow_mut().push(format!("Message {o:?}"));
        if o.title.as_deref() == Some("pending") {
            return Box::pin(std::future::pending());
        }
        let r = match &o.labels {
            Some(l) => l[1].clone(),
            None => "yes".to_string(),
        };
        Box::pin(std::future::ready(r))
    }
}

#[test]
fn dialog_exports_resolve_from_the_backend_and_abort() {
    let fake = Rc::new(Fake::default());
    svelters_egui::dialogs::set_dialog_backend(Some(fake.clone()));
    let js = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/js/dialogs.js");
    let mut app = svelters_egui::harness::TestApp::load(js).unwrap();
    for _ in 0..5 {
        app.run();
    }
    let lines: Vec<String> = app.host().take_console().into_iter().map(|l| l.text).collect();
    svelters_egui::dialogs::set_dialog_backend(None);
    assert_eq!(
        lines,
        [
            r#""open" "/parts/a.step""#,
            r#""open-none" null"#,
            r#""many" ["/parts/a.step","/parts/b.step"]"#,
            r#""save" "/out/board.kicad_pcb""#,
            r#""folder" "/parts""#,
            r#""box" "yes""#,
            r#""labels" "Discard""#,
            r#""aborted" "svelters" "cancelled""#,
            r#""done""#,
        ],
        "errors: {:?}",
        app.errors()
    );
    let seen = fake.seen.borrow();
    assert!(seen[0].contains("Open") && seen[0].contains("Open part") && seen[0].contains("\"stp\""), "{seen:?}");
    assert!(seen[2].starts_with("OpenMany") && seen[2].contains("/parts"), "{seen:?}");
    assert!(seen[5].contains("Warning") && seen[5].contains("YesNoCancel"), "{seen:?}");
    assert_eq!(seen.len(), 8, "{seen:?}");
}

// ---- the gallery (fixtures/gallery-egui) ----

fn gallery() -> PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/gallery-egui")
}

/// The gallery imports `svelters:app/egui`, so it needs the dumped stub; this checks it (and
/// `SVELTERS_UPDATE=1` rewrites it).
#[test]
fn gallery_export_stubs_are_current() {
    let dir = gallery().join(".svelters/app");
    if std::env::var("SVELTERS_UPDATE").as_deref() == Ok("1") {
        svelters_egui::dump(&dir).unwrap();
        return;
    }
    let tmp = std::env::temp_dir().join(format!("svelters-egui-wg4b-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    svelters_egui::dump(&tmp).unwrap();
    let mut dirs = vec![PathBuf::new()];
    while let Some(rel) = dirs.pop() {
        for e in std::fs::read_dir(tmp.join(&rel)).unwrap().flatten() {
            let name = rel.join(e.file_name());
            if e.path().is_dir() {
                dirs.push(name);
                continue;
            }
            let want = std::fs::read_to_string(e.path()).unwrap();
            let have = std::fs::read_to_string(dir.join(&name)).unwrap_or_default();
            assert!(have == want, "fixtures/gallery-egui/.svelters/app/{} is stale: run SVELTERS_UPDATE=1 cargo test -p svelters-egui --test wg4b", name.display());
        }
    }
    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn gallery_pages_drive_pickers_dnd_and_dialogs() {
    let bundle = gallery().join("dist/app.js");
    assert!(bundle.exists(), "{} is missing: run `pnpm build` (needs `cargo build -p svelters-compiler` first)", bundle.display());
    let fake = Rc::new(Fake::default());
    svelters_egui::dialogs::set_dialog_backend(Some(fake.clone()));
    let mut app = svelters_egui::harness::TestApp::load(bundle).unwrap();
    // egui's colour popup has a "📋" button and 0.37's default fonts have no emoji (tofu in apps)
    app.harness.ctx.set_missing_glyph_policy(egui::MissingGlyphPolicy::Tofu);
    app.run();
    let has = |app: &svelters_egui::harness::TestApp, l: &str| app.harness.query_by_label(l).is_some();
    // the gallery's sidebar (src/pages/40-pickers, 41-drag-and-drop, 42-dialogs)
    let open = |app: &mut svelters_egui::harness::TestApp, title: &str| {
        app.harness.get_by_role_and_label(egui::accesskit::Role::Button, title).click();
        app.run();
    };

    // pickers: bind:value shows in the label next to each
    open(&mut app, "pickers");
    assert!(has(&app, "#3366cc") && has(&app, "#ff000080"));
    app.harness.get_by_label("Pick the due date").click();
    app.run();
    // OW6: the own calendar's day cells are AccessKit grid cells named by ISO date
    app.harness.get_by_role_and_label(egui::accesskit::Role::GridCell, "2026-10-23").click();
    app.run();
    app.harness.get_by_label("Save").click();
    app.run();
    assert!(has(&app, "Due: 2026-10-23"), "bind:value on the date picker");

    // drag and drop
    open(&mut app, "drag and drop");
    let from = app.harness.get_by_label("R1 10k").rect().center();
    let to = app.harness.get_by_label("Board").rect().center();
    drag_on(&mut app.harness, from, to);
    app.run();
    assert!(has(&app, "placed R1 10k (cad/part)"), "the drop reached JS");

    // dialogs, from the fake backend
    open(&mut app, "dialogs");
    app.harness.get_by_label("Open file…").click();
    app.run();
    app.run();
    assert!(has(&app, r#"openFile: "/parts/a.step""#));
    app.harness.get_by_label("Custom buttons…").click();
    app.run();
    app.run();
    assert!(has(&app, r#"labels: "Keep mine""#));
    svelters_egui::dialogs::set_dialog_backend(None);
    let errors = app.errors();
    assert!(errors.is_empty(), "{errors:?}");
}
