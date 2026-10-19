//! NT2 end to end on Qt: the real host runs Svelte bundles that import `svelters:qt/core` and `svelters:qt/gui`
//! (the Qt twins of NON-GUI-TYPES section 7, compare crates/svelters-gtk/tests/host_types.rs):
//! - `fixtures/qt-types`: one QTextDocument made by JS shared by two editors (its `contentsChanged` drives a
//!   label), a QStringListModel built from JS, a QIcon from a path on a button, a QUrl value's method;
//! - `tests/qt-types-lists`: a QStringListModel and a QStandardItemModel feeding `@svelters/lists`;
//! - `tests/qt-types-gc`: dropped wrappers release their SHARED, VALUE and unique handles.
//! Needs a display and the built bundles (`pnpm -r build`); main thread, so no libtest harness.

use counter_qt::hooks;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use svelters_abi::BackendRef;
use svelters_host::{Host, HostConfig};

fn bundle(rel: &str) -> PathBuf {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").join(rel).join("dist/app.js");
    assert!(p.is_file(), "build the bundle first (`pnpm -r build`): {}", p.display());
    p
}

fn host_for(rel: &str) -> (Host, BackendRef<'static>) {
    let raw: BackendRef<'static> = unsafe { BackendRef::from_raw(counter_qt::backend()) }.unwrap();
    let cfg = HostConfig { echo_console: false, echo_errors: false, ..HostConfig::default() };
    let host = Host::new(raw, cfg).unwrap();
    host.load_bundle(bundle(rel)).unwrap();
    (host, raw)
}

/// Runs `checks` from the Qt loop at the given delays (ms), then quits.
fn run(host: &Host, raw: BackendRef<'static>, checks: Vec<(i32, Box<dyn Fn()>)>, end: i32) {
    for (ms, f) in checks {
        hooks::run_later(ms, f);
    }
    hooks::run_later(end, Box::new(move || raw.quit()));
    host.run().unwrap();
    let errs: Vec<String> = host.take_errors().into_iter().map(|e| e.message).collect();
    assert!(errs.is_empty(), "{errs:?}");
}

fn qt_types_fixture() {
    // The fixture's icon path (a file icon, not a theme name).
    // A 1x1 PNG (QIcon reads it without the SVG plugin).
    const PNG: [u8; 70] = [137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6, 0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120, 156, 99, 248, 207, 192, 240, 31, 0, 5, 0, 1, 255, 137, 153, 61, 29, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130];
    std::fs::write("/tmp/svelters-qt-types.png", PNG).unwrap();
    let (host, raw) = host_for("fixtures/qt-types");
    let seen: Rc<RefCell<Vec<String>>> = Rc::default();
    let s = seen.clone();
    let first: Box<dyn Fn()> = Box::new(move || {
        let (n, shared, text) = hooks::text_edits(None);
        let mut log = s.borrow_mut();
        log.push(format!("editors={n} shared={shared} text={text}"));
        log.push(format!("label={}", hooks::label_starting("5 ")));
        log.push(format!("icon={:?}", hooks::button_icon("example.com")));
        // Typing into the document reaches JS through the native contentsChanged signal.
        hooks::text_edits(Some("Hello, native world"));
    });
    let s = seen.clone();
    let second: Box<dyn Fn()> = Box::new(move || s.borrow_mut().push(format!("label2={}", hooks::label_starting("19 "))));
    run(&host, raw, vec![(400, first), (800, second)], 1200);
    let seen = seen.borrow();
    assert_eq!(seen[0], "editors=2 shared=true text=Hello", "{seen:?}");
    assert_eq!(seen[1], "label=5 characters", "{seen:?}");
    assert_eq!(seen[2], "icon=Some(true)", "{seen:?}");
    assert_eq!(seen[3], "label2=19 characters", "{seen:?}");
}

fn native_models_feed_lists() {
    let (host, raw) = host_for("tests/qt-types-lists");
    let seen: Rc<RefCell<Vec<String>>> = Rc::default();
    let snap = |s: &Rc<RefCell<Vec<String>>>, tag: &'static str| {
        let s = s.clone();
        Box::new(move || {
            let mut rows = hooks::vlist_rows();
            rows.sort();
            s.borrow_mut().push(format!("{tag}: {} {}", hooks::label_starting("rows="), rows.join(",")));
        }) as Box<dyn Fn()>
    };
    let click = |label: &'static str| Box::new(move || assert!(hooks::click_button(label), "no button {label}")) as Box<dyn Fn()>;
    run(&host, raw, vec![
        (400, snap(&seen, "start")),
        (450, click("addName")),
        (650, snap(&seen, "addName")),
        (700, click("addItem")),
        (900, snap(&seen, "addItem")),
        (950, click("rename")),
        (1150, snap(&seen, "rename")),
        (1200, click("removeFirst")),
        (1400, snap(&seen, "removeFirst")),
    ], 1700);
    let seen = seen.borrow();
    assert_eq!(seen[0], "start: rows=4 items one,items two,names alpha,names beta", "{seen:?}");
    assert_eq!(seen[1], "addName: rows=5 items one,items two,names alpha,names beta,names gamma0", "{seen:?}");
    assert_eq!(seen[2], "addItem: rows=6 items one,items three,items two,names alpha,names beta,names gamma0", "{seen:?}");
    assert_eq!(seen[3], "rename: rows=6 items three,items two,items uno,names alpha,names beta,names gamma0", "{seen:?}");
    assert_eq!(seen[4], "removeFirst: rows=5 items three,items two,names alpha,names beta,names gamma0", "{seen:?}");
}

fn dropped_wrappers_release_their_handles() {
    let (host, raw) = host_for("tests/qt-types-gc");
    let _ = raw;
    // SAFETY: the host outlives every backend call of this test.
    unsafe { counter_qt::set_host(host.host_callbacks()) };
    host.checkpoint();
    hooks::pump(50);
    let count = || {
        let (v, p, _) = hooks::type_counts();
        hooks::live_handles() + v + p
    };
    let baseline = count();
    assert!(hooks::click_button("make 0"), "no make button");
    host.checkpoint();
    hooks::pump(20);
    let before = count();
    assert!(before >= baseline + 120, "the handler minted 120 handles: {baseline} -> {before}; errors {:?}", host.take_errors().into_iter().map(|e| e.message).collect::<Vec<_>>());
    assert!(hooks::click_button("drop"));
    host.checkpoint();
    for _ in 0..5 {
        host.collect_garbage();
        host.drain_jobs();
        host.checkpoint();
        hooks::pump(50);
    }
    let after = count();
    assert!(after <= baseline + 5, "garbage collection released the dropped wrappers: {baseline} -> {before} -> {after}");
    let errs: Vec<String> = host.take_errors().into_iter().map(|e| e.message).collect();
    assert!(errs.is_empty(), "{errs:?}");
}

fn main() {
    // SAFETY: single-threaded at this point.
    unsafe {
        std::env::set_var("QT_ACCESSIBILITY", "0");
        std::env::set_var("DBUS_SESSION_BUS_ADDRESS", "unix:path=/nonexistent");
    }
    counter_qt::run_cases(&[
        ("qt_types_fixture", qt_types_fixture),
        ("native_models_feed_lists", native_models_feed_lists),
        ("dropped_wrappers_release_their_handles", dropped_wrappers_release_their_handles),
    ]);
}
