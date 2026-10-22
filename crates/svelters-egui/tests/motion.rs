//! TR1 on egui: `transition:fade` / `fly` / `slide` (fixtures/motion-egui) on the real QuickJS host,
//! driven by kittest frames in real time. The outro shows through the visual channels (opacity,
//! transform, clip, inert on the scene node), the elements go when it ends, and then the app is
//! idle again: no frame asks for another.

use egui_kittest::kittest::Queryable;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use svelters_egui::harness::TestApp;
use svelters_egui::scene::Visual;

fn bundle(name: &str) -> PathBuf {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(name)
        .join("dist/app.js");
    assert!(
        p.exists(),
        "{} is missing: run `pnpm build` (needs `cargo build -p svelters-compiler` first)",
        p.display()
    );
    p
}

fn has(app: &TestApp, label: &str) -> bool {
    app.harness.query_by_label(label).is_some()
}

/// Every live node's visual channels.
fn visuals(app: &TestApp) -> Vec<Visual> {
    app.backend.with_scene(|s| {
        s.handles()
            .into_iter()
            .map(|h| s.get(h).unwrap().vis)
            .collect()
    })
}

/// Steps frames in real time (16 ms apart) until `done`, recording the channels seen on the way.
fn animate(app: &mut TestApp, done: impl Fn(&TestApp) -> bool) -> (u32, Vec<Visual>) {
    let start = Instant::now();
    let mut steps = 0;
    let mut seen = Vec::new();
    while !done(app) {
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "the transition did not end"
        );
        app.harness.step();
        steps += 1;
        seen.extend(visuals(app).into_iter().filter(|v| !v.is_identity()));
        std::thread::sleep(Duration::from_millis(16));
    }
    (steps, seen)
}

#[test]
fn outro_and_intro_play_through_the_channels_then_idle() {
    let mut app = TestApp::load(bundle("motion-egui")).unwrap();
    app.run();
    assert!(has(&app, "Faded") && has(&app, "Flown") && has(&app, "Slid"));
    assert!(
        visuals(&app).iter().all(Visual::is_identity),
        "no intro on the first render (local transitions)"
    );
    let labels = app.backend.handles_of("label").len();

    // outro: the elements stay while they animate, then go
    app.harness.get_by_label("Toggle").click();
    let (steps, seen) = animate(&mut app, |a| {
        a.backend.handles_of("label").len() < labels && has(a, "Outros ended: 1")
    });
    assert!(steps >= 3, "the 120 ms outro took {steps} frames");
    assert!(
        seen.iter()
            .any(|v| v.opacity.is_some_and(|o| o > 0.0 && o < 1.0)),
        "fade: opacity channel"
    );
    assert!(
        seen.iter().any(|v| v.transform.is_some_and(|t| t[1] > 0.0)),
        "fly: transform channel"
    );
    assert!(
        seen.iter().any(|v| v.clip.is_some_and(|c| c[1] >= 0.0)),
        "slide: clip channel"
    );
    assert!(seen.iter().any(|v| v.inert), "outroing elements are inert");
    assert!(!has(&app, "Faded") && !has(&app, "Flown") && !has(&app, "Slid"));

    // idle: once the last animation ended, no frame asks for another
    app.run();
    assert_eq!(app.run(), 1, "idle after the outro");
    assert!(!app.harness.ctx.has_requested_repaint());

    // intro (a later render): the channels end back at identity
    app.harness.get_by_label("Toggle").click();
    let (_, seen) = animate(&mut app, |a| {
        has(a, "Faded")
            && visuals(a).iter().all(Visual::is_identity)
            && a.backend.handles_of("label").len() == labels
    });
    assert!(
        seen.iter().any(|v| v.opacity.is_some()),
        "the intro animated"
    );
    app.run();
    assert_eq!(app.run(), 1, "idle after the intro");
    let errors = app.errors();
    assert!(errors.is_empty(), "{errors:?}");
}

/// `bounds` (ABI v2.8): the laid-out rect of a node in its window, synchronously up to date.
#[test]
fn bounds_report_the_layout_rect() {
    let mut app = TestApp::load(bundle("motion-egui")).unwrap();
    app.run();
    use svelters_abi::Backend;
    let b = app.backend.clone();
    let toggle = b.handles_of("button")[0];
    let r = b.bounds(toggle).unwrap();
    assert!(
        r[2] > 10.0 && r[3] > 10.0,
        "a laid-out button has a size: {r:?}"
    );
    let faded = b.handles_of("label")[0];
    let f = b.bounds(faded).unwrap();
    assert!(f[1] > r[1], "the label is below the button: {f:?} vs {r:?}");
}
