//! TR3 (SV14, in-process half): what a transition costs per frame on egui when
//!   - the scene plays its keyframes (`transition:fade`, `SVL_STYLE_ANIMATE`: native),
//!   - TR1's JS player plays them (the same transition with the native animator switched off by
//!     `globalThis.__svelters_no_native_animation`: one style op per channel per frame), and
//!   - a `tick(t)` transition runs JS every frame (no keyframes exist for it).
//! 40 labels fade out over 400 ms on the virtual clock (1/60 s a frame). Per frame while the outro
//! runs: wall time of the whole frame (JS + host ops + egui layout and paint, debug build), the part
//! the host spent in JS and in ops (`Timing`), and the ops. The asserts are on the op counts, which
//! do not depend on the machine; the times are printed (`--nocapture`) and recorded in
//! `plans/milestones/TR3-tick-profile.md`.

use egui_kittest::kittest::Queryable;
use std::time::Instant;
use svelters_egui::harness::TestApp;
use svelters_host::HostConfig;

const DT: f32 = 1.0 / 60.0;
const ROWS: usize = 40;

#[derive(Debug)]
struct Profile {
    frames: usize,
    wall_us: f64,
    js_us: f64,
    ops_us: f64,
    ops: f64,
    events: f64,
}

fn profile(file: &str) -> Profile {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/motion-egui/src").join(file);
    let cfg = HostConfig { echo_console: false, echo_errors: false, timing: true, ..HostConfig::default() };
    let mut app = TestApp::from_source_with(path, DT, cfg).unwrap();
    app.run();
    assert_eq!(app.backend.handles_of("label").len(), ROWS);
    app.harness.get_by_label("Toggle").click();
    let _ = app.host().take_timing();
    let ev0 = app.backend.stats().events;
    let (mut n, mut wall, mut js, mut ops_ns, mut ops) = (0usize, 0.0, 0.0, 0.0, 0.0);
    for i in 0..120 {
        let t = Instant::now();
        app.harness.step();
        let w = t.elapsed().as_secs_f64() * 1e6;
        let tm = app.host().take_timing();
        if app.backend.handles_of("label").len() < ROWS {
            break;
        }
        if i < 3 {
            continue; // the click and the start of the transitions
        }
        n += 1;
        wall += w;
        js += tm.js_ns as f64 / 1e3;
        ops_ns += tm.ops_ns as f64 / 1e3;
        ops += tm.ops as f64;
    }
    assert!(n > 10, "the outro ran {n} steady frames");
    assert!(app.errors().is_empty());
    let events = (app.backend.stats().events - ev0) as f64;
    let f = n as f64;
    Profile { frames: n, wall_us: wall / f, js_us: js / f, ops_us: ops_ns / f, ops: ops / f, events: events / f }
}

#[test]
fn per_frame_cost_of_native_keyframes_js_player_and_tick() {
    let native = profile("ProfNative.svelte");
    let js = profile("ProfJs.svelte");
    let tick = profile("ProfTick.svelte");
    eprintln!("{ROWS} labels, 400 ms fade-out, per steady frame (debug build, virtual 60 fps):");
    eprintln!("{:<26} {:>6} {:>10} {:>9} {:>9} {:>8}", "", "frames", "wall us", "js us", "ops us", "ops");
    for (name, p) in [("native keyframes", &native), ("JS player (TR1)", &js), ("tick(t)", &tick)] {
        eprintln!("{name:<26} {:>6} {:>10.0} {:>9.0} {:>9.0} {:>8.1}", p.frames, p.wall_us, p.js_us, p.ops_us, p.ops);
    }
    // the keyframes were sent once: no host op per frame, no JS in the frame loop
    assert!(native.ops < 1.0, "native: {} ops a frame", native.ops);
    assert!(native.events * (native.frames as f64) <= ROWS as f64 + 1.0, "one animationend a node, not one a frame");
    // the JS paths write every animated node every frame
    assert!(js.ops >= ROWS as f64 * 0.9, "JS player: {} ops a frame", js.ops);
    assert!(tick.ops >= ROWS as f64 * 0.9, "tick: {} ops a frame", tick.ops);
    // and run JS for it
    assert!(native.js_us < js.js_us && native.js_us < tick.js_us, "{native:?} {js:?} {tick:?}");
}
