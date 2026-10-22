//! TR2: the native animator on egui, under kittest's virtual clock (`from_source_stepped`: 16 ms
//! per frame, no sleeping). The fixtures are `fixtures/motion-egui/src/{App,Flip}.svelte`, compiled
//! live. Keyframes go to the scene once (`SVL_STYLE_ANIMATE`); the scene samples them, and
//! `animationend` (the `outro_done`) removes the exiting elements.

use egui_kittest::kittest::Queryable;
use svelters_egui::anim::MotionConfig;
use svelters_egui::harness::TestApp;
use svelters_egui::scene::Visual;

const DT: f32 = 1.0 / 60.0;

fn src(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/motion-egui/src").join(name)
}

fn has(app: &TestApp, label: &str) -> bool {
    app.harness.query_by_label(label).is_some()
}

fn visuals(app: &TestApp) -> Vec<Visual> {
    app.backend.with_scene(|s| s.handles().into_iter().map(|h| s.get(h).unwrap().vis).collect())
}

/// Steps until `done`, at most 120 frames; returns the frames it took and every non-identity visual seen.
fn play(app: &mut TestApp, done: impl Fn(&TestApp) -> bool) -> (u32, Vec<Visual>) {
    let mut seen = Vec::new();
    for n in 1..=120 {
        app.harness.step();
        seen.extend(visuals(app).into_iter().filter(|v| !v.is_identity()));
        if done(app) {
            return (n, seen);
        }
    }
    panic!("the animation did not end in 120 frames");
}

fn start() -> TestApp {
    let mut app = TestApp::from_source_stepped(src("App.svelte"), DT).unwrap();
    app.run();
    assert!(has(&app, "Faded") && has(&app, "Flown") && has(&app, "Slid"));
    app
}

#[test]
fn outro_is_sampled_natively_and_ends_with_outro_done() {
    let mut app = start();
    assert_eq!(app.backend.animations_running(), 0);
    app.harness.get_by_label("Toggle").click();
    let (frames, seen) = play(&mut app, |a| a.backend.animations_running() > 0);
    assert!(frames < 10, "the native animation started ({frames} frames)");
    let (frames, seen2) = play(&mut app, |a| has(a, "Outros ended: 1"));
    // 120 ms at 16.7 ms: about 8 frames, plus the frames the end signal takes to reach JS and back
    assert!((6..=16).contains(&frames), "the 120 ms outro took {frames} frames");
    let seen: Vec<Visual> = seen.into_iter().chain(seen2).collect();
    assert!(seen.iter().any(|v| v.opacity.is_some_and(|o| o > 0.0 && o < 1.0)), "fade: opacity channel");
    assert!(seen.iter().any(|v| v.transform.is_some_and(|t| t[1] > 0.0)), "fly: transform channel");
    assert!(seen.iter().any(|v| v.clip.is_some_and(|c| c[1] >= 0.0)), "slide: clip channel");
    assert!(seen.iter().any(|v| v.inert), "an exiting node is inert");
    assert!(!has(&app, "Faded") && !has(&app, "Flown") && !has(&app, "Slid"), "the outro ended: the nodes are gone");
    assert_eq!(app.backend.animations_running(), 0);
    app.run(); // and then nothing asks for another frame
    assert!(app.errors().is_empty());
}

#[test]
fn exiting_nodes_are_not_pickable() {
    let mut app = start();
    let vp = egui::ViewportId::ROOT;
    let centre = app.harness.get_by_label("Flown").rect().center();
    let picked = |app: &TestApp| app.backend.scene().read(|s| s.hit_node(vp, centre).map(|n| n.id()));
    assert!(picked(&app).is_some(), "a live button is pickable");
    app.harness.get_by_label("Toggle").click();
    // a few frames into the outro: the elements are still drawn, but inert
    for _ in 0..3 {
        app.harness.step();
    }
    assert!(visuals(&app).iter().any(|v| v.inert), "the outro runs");
    assert!(picked(&app).is_none(), "an exiting node is not picked");
}

#[test]
fn reduced_motion_jumps_to_the_end() {
    let mut app = start();
    app.backend.set_motion(MotionConfig { reduced: true, ..MotionConfig::default() });
    app.harness.get_by_label("Toggle").click();
    let (frames, _) = play(&mut app, |a| has(a, "Outros ended: 1"));
    assert!(frames <= 6, "no outro to wait for ({frames} frames)");
    assert!(!has(&app, "Faded"));
}

#[test]
fn slow_motion_scales_the_animation_clock() {
    let normal = {
        let mut app = start();
        app.harness.get_by_label("Toggle").click();
        play(&mut app, |a| has(a, "Outros ended: 1")).0
    };
    let mut app = start();
    app.backend.set_motion(MotionConfig { slow: 0.25, ..MotionConfig::default() });
    app.harness.get_by_label("Toggle").click();
    let slow = play(&mut app, |a| has(a, "Outros ended: 1")).0;
    // 120 ms is about 8 frames at full speed and 29 at a quarter; a few frames of JS round trip are fixed
    assert!(slow >= normal + 15, "a quarter speed took {slow} frames against {normal}");
}

#[test]
fn flip_animates_from_the_scene_bounds() {
    let mut app = TestApp::from_source_stepped(src("Flip.svelte"), DT).unwrap();
    app.run();
    let top = |app: &TestApp, l: &str| app.harness.get_by_label(l).rect().min.y;
    assert!(top(&app, "Alpha") < top(&app, "Beta") && top(&app, "Beta") < top(&app, "Gamma"));
    app.harness.get_by_label("Reverse").click();
    let (_, seen) = play(&mut app, |a| a.backend.animations_running() > 0);
    let (_, seen2) = play(&mut app, |a| a.backend.animations_running() == 0);
    // Alpha moved down two rows: FLIP starts it above its new place
    assert!(seen.iter().chain(&seen2).any(|v| v.transform.is_some_and(|t| t[1].abs() > 1.0)), "flip translates the moved rows");
    assert!(top(&app, "Gamma") < top(&app, "Beta") && top(&app, "Beta") < top(&app, "Alpha"));
    assert!(app.errors().is_empty());
}
