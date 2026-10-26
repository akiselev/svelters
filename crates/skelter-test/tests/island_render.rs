//! `EguiHarness::render`: the island's pass drawn by egui-wgpu into its texture on the harness's device,
//! then the committed tree (the island's surface) by skelter-render. Runs only with `SKELTER_GPU_TESTS=1`
//! (CI sets it: lavapipe on Linux, WARP on Windows, Metal on macOS).

use skelter_test::{EguiHarness, SnapshotOptions};

fn gpu_tests() -> bool {
    let on = std::env::var("SKELTER_GPU_TESTS").as_deref() == Ok("1");
    if !on {
        eprintln!("skipped: set SKELTER_GPU_TESTS=1 to run GPU tests");
    }
    on
}

fn painted() -> EguiHarness<egui::Color32> {
    EguiHarness::builder()
        .with_size(egui::vec2(100.0, 60.0))
        .with_accessibility_check(false)
        .build_ui_state(
            |ui, c: &mut egui::Color32| {
                ui.painter().rect_filled(
                    egui::Rect::from_min_max(egui::pos2(10.0, 10.0), egui::pos2(50.0, 30.0)),
                    0.0,
                    *c,
                );
            },
            egui::Color32::RED,
        )
}

#[test]
fn an_island_pass_reaches_the_pixels() {
    if !gpu_tests() {
        return;
    }
    let mut h = painted();
    let img = h.render().expect("render");
    assert_eq!((img.width, img.height), (100, 60));
    let p = img.pixel(30, 20);
    assert!(p[0] > 200 && p[1] < 40 && p[2] < 40, "red inside the rect: {p:?}");

    *h.state_mut() = egui::Color32::BLUE;
    h.run();
    let img = h.render().expect("render");
    let p = img.pixel(30, 20);
    assert!(p[2] > 200 && p[0] < 40, "blue after the next pass: {p:?}");
}

#[test]
fn render_every_step_keeps_the_last_image() {
    if !gpu_tests() {
        return;
    }
    let mut h = EguiHarness::builder()
        .with_size(egui::vec2(64.0, 32.0))
        .with_render_every_step(true)
        .build_ui(|ui| {
            let _ = ui.button("Paint");
        });
    h.step();
    let img = h.last_render().expect("an image per step");
    assert_eq!((img.width, img.height), (64, 32));
}

#[test]
fn a_snapshot_of_an_island_compares_with_itself() {
    if !gpu_tests() {
        return;
    }
    let out = std::env::temp_dir().join(format!("skelter-test-island-snap-{}", std::process::id()));
    std::fs::remove_dir_all(&out).ok();
    let opts = SnapshotOptions::new().output_path(&out);
    let mut h = painted();
    assert!(h.try_snapshot_options("red", &opts).is_err(), "no golden yet");
    std::fs::rename(out.join("red.new.png"), out.join("red.png")).unwrap();
    h.try_snapshot_options("red", &opts).expect("the same frame");
    std::fs::remove_dir_all(&out).ok();
}

/// A5d: with a pass every step, rendering every step and pointer input, the island's time never moves
/// backwards (egui panics in its pointer history when it does): input arrives at the clock's now,
/// the step's pass runs at the frame target, and `render`'s own frame lands between them.
#[test]
fn rendering_every_step_with_pointer_input_keeps_egui_time_monotonic() {
    if !gpu_tests() {
        return;
    }
    #[derive(Default)]
    struct S {
        clicks: u32,
        dragged: f32,
    }
    let mut h = EguiHarness::builder()
        .with_size(egui::vec2(200.0, 120.0))
        .with_step_dt(1.0 / 60.0)
        .with_render_every_step(true)
        .build_ui_state(
            |ui, s: &mut S| {
                if ui.button("Measure").clicked() {
                    s.clicks += 1;
                }
                ui.add(egui::Slider::new(&mut s.dragged, 0.0..=100.0).text("Length"));
            },
            S::default(),
        );
    for _ in 0..3 {
        h.get_by_label("Measure").click();
        h.run();
    }
    let slider = h.get_by_role(skelter_test::Role::Slider).rect();
    let c = slider.center();
    h.drag(egui::pos2(c.x, c.y), egui::pos2(slider.max.x, c.y), 6);
    h.run();
    assert_eq!(h.state().clicks, 3);
    assert!(h.last_render().is_some());
}
