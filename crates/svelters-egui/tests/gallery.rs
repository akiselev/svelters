//! The egui widget gallery (`fixtures/gallery-egui`, from `pnpm build`) on the real QuickJS host:
//! every page opens from its sidebar button and its widgets are driven through their bindings, so
//! the `bind:` lowering of each kind (`bind:group`, `bind:checked`, `bind:active`, `bind:low`/
//! `bind:high`, `bind:value`, `bind:size`) is checked end to end. Headless (kittest).

use egui::accesskit::Role;
use egui_kittest::kittest::Queryable;
use std::path::PathBuf;
use svelters_egui::harness::TestApp;

fn bundle() -> PathBuf {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/gallery-egui/dist/app.js");
    assert!(p.exists(), "{} is missing: run `pnpm build` (needs `cargo build -p svelters-compiler` first)", p.display());
    p
}

fn has(app: &TestApp, label: &str) -> bool {
    app.harness.query_by_label(label).is_some()
}

fn png(w: u32, h: u32) -> Vec<u8> {
    let img = image::RgbaImage::from_pixel(w, h, image::Rgba([224, 27, 36, 255]));
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}

/// Loads the gallery and opens page `title` from the sidebar.
fn open(title: &str) -> TestApp {
    let mut app = TestApp::load(bundle()).unwrap();
    app.harness.ctx.include_bytes("bytes://gallery/dot.png", png(32, 16));
    app.run();
    app.harness.get_by_role_and_label(Role::Button, title).click();
    // steps, not `run`: a page may animate (a spinner)
    app.harness.run_steps(6);
    assert!(has(&app, &format!("Page: {title}")), "page {title} opened");
    app
}

#[test]
fn inputs_page_binds_every_value() {
    let mut app = open("inputs");
    // bind:group across three radio buttons
    assert!(has(&app, "group: a"));
    app.harness.get_by_role_and_label(Role::RadioButton, "Gamma").click();
    app.run();
    assert!(has(&app, "group: c"));
    // bind:checked: exclusive siblings
    assert!(has(&app, "checked: true false"));
    app.harness.get_by_role_and_label(Role::RadioButton, "Second").click();
    app.run();
    assert!(has(&app, "checked: false true"));
    // bind:active
    app.harness.get_by_role_and_label(Role::Switch, "Wi-Fi").click();
    app.run();
    assert!(has(&app, "active: true"));
    // bind:low / bind:high
    app.harness.get_by_role_and_label(Role::Slider, "Range high").set_value_accesskit(60.0);
    app.run();
    assert!(has(&app, "range: 10 to 60"));
    // bind:value, clamped to max
    app.harness.get_by_role_and_label(Role::SpinButton, "Width").set_value_accesskit(25.0);
    app.run();
    assert!(has(&app, "width: 10"));
    // the link's click reaches JS
    app.harness.get_by_role_and_label(Role::Link, "Svelte").click();
    app.run();
    assert!(has(&app, "link clicks: 1"));
    let errors = app.errors();
    assert!(errors.is_empty(), "{errors:?}");
}

#[test]
fn containers_page_lays_out_and_switches() {
    let mut app = open("containers");
    for l in ["Panel 140", "Body", "Left side", "Inside the frame", "Stack page one"] {
        assert!(has(&app, l), "{l}");
    }
    assert!(!has(&app, "Stack page two"));
    app.harness.get_by_role_and_label(Role::Button, "Show two").click();
    app.run();
    assert!(has(&app, "Stack page two") && !has(&app, "Stack page one"));
    // the side panel is as wide as its bound size
    let (panel, body) = (app.harness.get_by_label("Panel 140").rect(), app.harness.get_by_label("Body").rect());
    assert!(body.left() - panel.left() >= 140.0 - 1.0, "{panel:?} {body:?}");
    let errors = app.errors();
    assert!(errors.is_empty(), "{errors:?}");
}

#[test]
fn media_page_draws() {
    let mut app = open("media");
    let dot = app.harness.get_by_role_and_label(Role::Image, "Dot").rect();
    assert_eq!(dot.size(), egui::vec2(32.0, 16.0));
    app.harness.get_by_role_and_label(Role::ProgressIndicator, "Upload");
    // the spinner animates: stop it so the app settles
    app.harness.get_by_role_and_label(Role::Button, "Toggle spinner").click();
    app.harness.run_steps(4);
    app.run();
    // the canvas attachment got its size from `resize`
    assert!(has(&app, "canvas: 240x120"));
    app.harness.get_by_role_and_label(Role::Canvas, "Drawing");
    let errors = app.errors();
    assert!(errors.is_empty(), "{errors:?}");
}

#[test]
fn own_buttons_page_styles_parts_from_css() {
    // OW-CSS: the own button's parts and boxes styled by the page's <style>
    let mut app = open("own buttons");
    for b in ["Pill", "Accent", "Tab", "Glow"] {
        app.harness.get_by_role_and_label(Role::Button, b);
    }
    app.harness.get_by_role_and_label(Role::Button, "Accent").click();
    app.harness.run_steps(4);
    assert!(has(&app, "clicks: 1"));
    let errors = app.errors();
    assert!(errors.is_empty(), "{errors:?}");
}

#[test]
fn utilities_page_generates_its_classes() {
    // U3: utility classes turned on after load (live nodes are resolved again), then the page
    let mut app = TestApp::load(bundle()).unwrap();
    app.backend.set_utilities(&svelters_egui::Utilities::on());
    app.run();
    let negative = app.backend.utility_stats().negative;
    app.harness.get_by_role_and_label(Role::Button, "utilities").click();
    app.harness.run_steps(6);
    assert!(has(&app, "Page: utilities"));
    let stats = app.backend.utility_stats();
    assert!(stats.generated >= 18, "{stats:?}");
    assert_eq!(stats.negative, negative, "every class on the page is a valid utility: {stats:?}");
    let slider = app.backend.handles_of("slider")[0];
    let thumb = app.backend.computed_part_style(slider, "thumb").expect("part-thumb:bg-accent reaches the thumb");
    assert!(thumb.get("background-color").is_some());
    // a dynamic `p-{n}`: p-2 exists already (`p-2 bg-surface`), p-3 is generated on first use
    app.harness.get_by_role_and_label(Role::Button, "Padding 1").click();
    app.run();
    assert!(has(&app, "p-2 border rounded-lg"));
    assert_eq!(app.backend.utility_stats().generated, stats.generated);
    app.harness.get_by_role_and_label(Role::Button, "Padding 2").click();
    app.run();
    assert!(has(&app, "p-3 border rounded-lg"));
    assert_eq!(app.backend.utility_stats().generated, stats.generated + 1);
    let errors = app.errors();
    assert!(errors.is_empty(), "{errors:?}");
}
