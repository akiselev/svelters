//! The counter fixture end to end in plain `cargo test`: the compiled Svelte bundle
//! (`fixtures/counter-egui/dist/app.js`, from `pnpm build`) on the real QuickJS host, attached to
//! the egui backend through ABI v2.7 `start`, driven by kittest. Headless: no display, no GPU.

use egui_kittest::kittest::Queryable;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use svelters_egui::harness::TestApp;

fn bundle(name: &str) -> PathBuf {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures").join(name).join("dist/app.js");
    assert!(p.exists(), "{} is missing: run `pnpm build` (needs `cargo build -p svelters-compiler` first)", p.display());
    p
}

fn has(app: &TestApp, label: &str) -> bool {
    app.harness.query_by_label(label).is_some()
}

#[test]
fn counter_runs_under_kittest() {
    let mut app = TestApp::load(bundle("counter-egui")).unwrap();
    app.run();
    assert!(has(&app, "Count: 0"));
    assert!(has(&app, "Level 20, agree: no"));

    app.harness.get_by_label("Increment").click();
    app.run();
    assert!(has(&app, "Count: 1"), "the click reached JS and JS changed the label");
    app.harness.get_by_label("Increment").click();
    app.run();
    app.harness.get_by_label("Increment").click();
    app.run();
    assert!(has(&app, "Count: 3"));

    // bind:text (exact observation): typing reaches JS state, which another label shows
    app.harness.get_by_label("Name").click();
    app.run();
    app.harness.get_by_label("Name").type_text("Ann");
    app.run();
    assert!(has(&app, "Hello Ann"));

    // bind:value on the slider, bind:checked on the checkbox
    app.harness.get_by_label("Level").set_value_accesskit(42.0);
    app.run();
    assert!(has(&app, "Level 42, agree: no"));
    app.harness.get_by_label("Level 42, agree: no").click();
    app.run();
    assert!(has(&app, "Level 42, agree: yes"));

    let errors = app.errors();
    assert!(errors.is_empty(), "{errors:?}");
}

/// Idle is idle: once settled, a frame requests no other frame and runs no checkpoint or layout.
#[test]
fn settled_app_requests_no_frames() {
    let mut app = TestApp::load(bundle("counter-egui")).unwrap();
    app.run();
    let before = app.backend.stats();
    // `run` steps until no immediate repaint is requested: exactly one step when idle.
    assert_eq!(app.run(), 1);
    assert_eq!(app.run(), 1);
    let after = app.backend.stats();
    assert_eq!(after.checkpoints, before.checkpoints, "no checkpoint while idle");
    assert_eq!(after.layouts, before.layouts, "no relayout while idle");
    assert!(!app.harness.ctx.has_requested_repaint(), "nothing asks for another frame");

    // a click costs a bounded number of frames, then idle again
    app.harness.get_by_label("Increment").click();
    // click frame, checkpoint frame, then egui's own press/hover animation frames
    let steps = app.run();
    assert!(steps <= 6, "{steps} frames for one click");
    let s = app.backend.stats();
    // one checkpoint delivers the click; the host may `wake` once more for jobs the handler queued
    assert!(s.checkpoints - after.checkpoints <= 2, "{} checkpoints for one click", s.checkpoints - after.checkpoints);
    assert_eq!(app.run(), 1, "idle again");
    // a counter label going 0 -> 1 keeps its size: remeasured, not relaid out
    assert_eq!(s.layouts, after.layouts, "same-size text change must not relayout");
}

/// Timers: setTimeout chains run from `schedule_wake` deadlines (`request_repaint_after`).
#[test]
fn timers_fire_from_schedule_wake() {
    let mut app = TestApp::load(bundle("timers-egui")).unwrap();
    app.run();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !has(&app, "done") && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
        app.harness.step();
    }
    assert!(has(&app, "done"));
    assert!(app.errors().is_empty());
}

/// Frame time of the counter (one walk + layout check + tessellation-free kittest pass), reported
/// for the milestone record; asserts only a loose bound.
#[test]
fn counter_frame_time() {
    let mut app = TestApp::load(bundle("counter-egui")).unwrap();
    app.run();
    let n = 200;
    let t = Instant::now();
    for _ in 0..n {
        app.harness.step();
    }
    let per = t.elapsed() / n;
    eprintln!("counter-egui: {per:?} per kittest frame ({n} frames, AccessKit on)");
    assert!(per < Duration::from_millis(50), "{per:?}");
}
