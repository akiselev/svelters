//! Input latency in frames: for each kind of interaction, the frames from the one that receives the
//! input to the first one that draws its reaction (fixture `fixtures/latency-egui`). A frame is one
//! kittest step (one `run_ui`, all its passes). `1` means the reaction is drawn in the frame that
//! received the input: no frame added. The test prints a table and asserts nothing yet, so it can
//! measure the current code and each fix the same way.
//!
//! Run: `tools/dev/cargo-slot cargo test -p svelters-egui --test input_latency -- --nocapture`

use egui::{Event, Modifiers, PointerButton, Pos2, TouchPhase, Vec2};
use egui_kittest::kittest::Queryable;
use svelters_egui::harness::TestApp;

const DT: f32 = 1.0 / 60.0;
const MAX: usize = 120;

thread_local! {
    /// Wall time of the frame that got the input (debug build, not a gate).
    static FIRST_MS: std::cell::Cell<f64> = const { std::cell::Cell::new(0.0) };
}

fn first_ms() -> String {
    format!("input frame {:.1} ms", FIRST_MS.with(|c| c.get()))
}

fn app() -> TestApp {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/latency-egui");
    let mut app = TestApp::from_source_stepped(&dir, DT).unwrap();
    settle(&mut app);
    app
}

/// Steps until nothing asks for another frame (bounded).
fn settle(app: &mut TestApp) {
    for _ in 0..MAX {
        app.harness.step();
        let busy = !app.embed.settled().is_settled();
        if !busy && !app.harness.ctx.has_requested_repaint() {
            return;
        }
    }
}

fn has(app: &TestApp, label: &str) -> bool {
    app.harness.query_by_label(label).is_some()
}

fn center(app: &TestApp, label: &str) -> Pos2 {
    app.harness.get_by_label(label).rect().center()
}

fn rect_of(app: &TestApp, label: &str) -> Option<egui::Rect> {
    app.harness.query_by_label(label).map(|n| n.rect())
}

/// Puts `events` into the next frame (all in one frame, unlike `Harness::event`).
fn send(app: &mut TestApp, events: Vec<Event>) {
    app.harness.input_mut().events.extend(events);
}

fn press(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE }
}

/// Steps once with the queued input, then until `done`; returns the frames stepped (1 = the frame
/// that got the input drew the reaction) and the passes run.
fn frames_until(app: &mut TestApp, done: impl Fn(&TestApp) -> bool) -> (Option<usize>, u64) {
    let p0 = app.harness.ctx.cumulative_pass_nr();
    for i in 1..=MAX {
        let t = std::time::Instant::now();
        app.harness.step();
        if i == 1 {
            FIRST_MS.with(|c| c.set(t.elapsed().as_secs_f64() * 1e3));
        }
        if done(app) {
            return (Some(i), app.harness.ctx.cumulative_pass_nr() - p0);
        }
    }
    (None, app.harness.ctx.cumulative_pass_nr() - p0)
}

/// Hover, then press in its own frame, then release: frames from the release to `done`.
fn click_frames(app: &mut TestApp, target: &str, done: impl Fn(&TestApp) -> bool) -> (Option<usize>, u64) {
    let pos = center(app, target);
    send(app, vec![Event::PointerMoved(pos)]);
    settle(app);
    send(app, vec![press(pos, true)]);
    settle(app);
    send(app, vec![press(pos, false)]);
    frames_until(app, done)
}

struct Row {
    name: &'static str,
    frames: Option<usize>,
    passes: u64,
    note: String,
}

fn print(rows: &[Row]) {
    println!("\n{:<36} {:>7} {:>7}  note", "interaction", "frames", "passes");
    for r in rows {
        let f = r.frames.map_or("never".to_string(), |f| f.to_string());
        println!("{:<36} {:>7} {:>7}  {}", r.name, f, r.passes, r.note);
    }
    println!("(frames: 1 = drawn in the frame that got the input; each extra is a frame added)\n");
}

#[test]
fn input_latency_in_frames() {
    let mut rows = Vec::new();

    // a widget signal (`onclicked`, from egui's Response during the walk)
    let mut a = app();
    let (frames, passes) = click_frames(&mut a, "Signal", |a| has(a, "signal 1"));
    rows.push(Row { name: "button onclicked", frames, passes, note: first_ms() });

    // a DOM event (`onclick`, dispatched by the input hook before the pass)
    let mut a = app();
    let (frames, passes) = click_frames(&mut a, "Dom", |a| has(a, "dom 1"));
    rows.push(Row { name: "button onclick (DOM)", frames, passes, note: first_ms() });

    // a two-way binding feeding an {#if}
    let mut a = app();
    let (frames, passes) = click_frames(&mut a, "Check", |a| has(a, "checked on"));
    rows.push(Row { name: "checkbox bind:checked -> {#if}", frames, passes, note: first_ms() });

    // typing into a bound field, shown by another label
    let mut a = app();
    let pos = center(&a, "Field");
    send(&mut a, vec![Event::PointerMoved(pos), press(pos, true), press(pos, false)]);
    settle(&mut a);
    let edit_drawn = has(&a, "typed []");
    send(&mut a, vec![Event::Text("x".into())]);
    let (frames, passes) = frames_until(&mut a, |a| has(a, "typed [x]"));
    rows.push(Row { name: "text-edit bind:text -> label", frames, passes, note: format!("field drawn: {edit_drawn}, {}", first_ms()) });

    // the menu bar: open Alpha, then hover Beta
    let mut a = app();
    let (frames, passes) = click_frames(&mut a, "Alpha", |a| has(a, "Alpha item"));
    rows.push(Row { name: "menu-bar click opens menu", frames, passes, note: first_ms() });
    let beta = center(&a, "Beta");
    send(&mut a, vec![Event::PointerMoved(beta)]);
    let (frames, passes) = frames_until(&mut a, |a| has(a, "Beta item") && !has(a, "Alpha item"));
    rows.push(Row { name: "menu-bar hover switches menu", frames, passes, note: first_ms() });

    // one wheel notch over the scroll area (a line, as a mouse wheel on Windows sends it)
    let mut a = app();
    let over = rect_of(&a, "row 0").expect("row 0").center();
    send(&mut a, vec![Event::PointerMoved(over)]);
    settle(&mut a);
    // the last row drawn: it moves up and stays in view (row 0 scrolls out)
    let last = (0..60).rev().find(|i| rect_of(&a, &format!("row {i}")).is_some()).expect("a row");
    let row = format!("row {last}");
    let r0 = rect_of(&a, &row).expect("last row");
    send(
        &mut a,
        vec![Event::MouseWheel {
            unit: egui::MouseWheelUnit::Line,
            delta: Vec2::new(0.0, -1.0),
            phase: TouchPhase::Move,
            source: egui::MouseWheelSource::Wheel,
            modifiers: Modifiers::NONE,
        }],
    );
    // the offset over time: first movement, then 90 % of where it ends
    let mut ys = Vec::new();
    for _ in 0..MAX {
        a.harness.step();
        ys.push(rect_of(&a, &row).map_or(f32::NAN, |r| r0.min.y - r.min.y));
    }
    let end = *ys.last().unwrap();
    let first = ys.iter().position(|&d| d.abs() > 0.01).map(|i| i + 1);
    let ninety = ys.iter().position(|&d| end.abs() > 0.0 && d.abs() >= 0.9 * end.abs()).map(|i| i + 1);
    rows.push(Row { name: "wheel notch: first movement", frames: first, passes: 0, note: format!("moved {end:.1} px in total") });
    rows.push(Row { name: "wheel notch: 90% of the way", frames: ninety, passes: 0, note: format!("{:.0} ms at 60 Hz", ninety.unwrap_or(0) as f32 * DT * 1000.0) });

    print(&rows);
}
