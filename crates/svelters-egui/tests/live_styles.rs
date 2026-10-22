//! LD2: `<style>` on the live loader path. A component loaded from source (`svelters-modules`) is
//! compiled with Q3w's `scoped_css` and its sheet is sent as text (`registerStyles(id, { css })`, as
//! the Vite plugin does for egui) from its own `svelters-style:` module; the style engine installs
//! it. A `<style>`-only edit re-runs that module alone: the sheet is replaced by id and the engine
//! restyles the same nodes, with no remount and the component's state kept.
//! The app is a scratch dir linked to `fixtures/embed-egui`'s node_modules (needs `pnpm install`).

use egui_kittest::kittest::Queryable;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use svelters_egui::embed::{EmbedConfig, EmbedEvent, MountSpec, SlotOptions};
use svelters_egui::harness::{EmbedHarness, TestApp};
use svelters_host::HostConfig;
use svelters_host::modules::{SourceConfig, SourceProvider};
use svelters_style::{Color, ComputedValue};

const PANEL: &str = r#"<script>
  let n = $state(0);
</script>

<box>
  <label>count {n}</label>
  <button onclicked={() => n++}>Inc</button>
</box>

<style>
  label { color: #ff0000; }
</style>
"#;

fn scratch(name: &str, manifest: &str, files: &[(&str, &str)]) -> PathBuf {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/embed-egui").canonicalize().unwrap();
    assert!(fixture.join("node_modules").exists(), "run `pnpm install` first");
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/ld2").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(dir.join("package.json"), manifest).unwrap();
    for (f, text) in files {
        std::fs::write(dir.join("src").join(f), text).unwrap();
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink(fixture.join("node_modules"), dir.join("node_modules")).unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_dir(fixture.join("node_modules"), dir.join("node_modules")).unwrap();
    dir.canonicalize().unwrap()
}

fn rgb(r: u8, g: u8, b: u8) -> Option<ComputedValue> {
    Some(ComputedValue::Color(Color { r: r as f32 / 255.0, g: g as f32 / 255.0, b: b as f32 / 255.0, a: 1.0 }))
}

/// An app loaded from source is styled: the label's computed colour is its `<style>` rule's.
#[test]
fn an_app_loaded_from_source_is_styled() {
    let app_src = PANEL.replacen("<box>", "<window title=\"ld2\">\n<box>", 1).replacen("</box>", "</box>\n</window>", 1);
    let dir = scratch("styled", r#"{ "name": "ld2", "private": true, "type": "module", "svelters": { "toolkit": "egui" } }"#, &[("App.svelte", &app_src)]);
    let mut app = TestApp::from_source(&dir).unwrap();
    app.run();
    assert!(app.harness.query_by_label("count 0").is_some());
    let labels = app.backend.handles_of("label");
    assert_eq!(labels.len(), 1);
    let color = app.backend.computed_style(labels[0]).and_then(|s| s.get("color").cloned());
    assert_eq!(color, rgb(255, 0, 0), "the <style> rule applies");
    let warnings: Vec<String> = app.host().take_console().into_iter().filter(|l| l.level == "warn").map(|l| l.text).collect();
    assert!(!warnings.iter().any(|w| w.contains("<style>")), "{warnings:?}");
    let errors = app.errors();
    assert!(errors.is_empty(), "{errors:?}");
}

/// Editing `<style>` restyles without remounting: the same node, the state kept, the new colour.
#[test]
fn a_style_edit_restyles_without_remounting() {
    let dir = scratch(
        "restyle",
        r#"{ "name": "ld2", "private": true, "type": "module", "svelters": { "toolkit": "egui", "embed": { "Panel": "src/Panel.svelte" } } }"#,
        &[("Panel.svelte", PANEL)],
    );
    let mut cfg = SourceConfig::for_path(&dir).unwrap();
    cfg.dev = true;
    cfg.hmr = true;
    let provider = Rc::new(SourceProvider::new(cfg).unwrap());
    let ecfg = EmbedConfig {
        id_salt: "ld2",
        host: HostConfig { echo_console: false, echo_errors: false, quit_on_last_window: false, ..HostConfig::default() },
        ..EmbedConfig::default()
    };
    let mut eh = EmbedHarness::new(egui::vec2(400.0, 300.0), ecfg).unwrap();
    let e = eh.embed.clone();
    let entry = e.load_from(provider).unwrap();
    let s1 = e.create_slot("a", SlotOptions::fill());
    eh.set_ui(move |e, ui| {
        e.show_slot(ui, s1);
    });
    e.mount(MountSpec::new(entry, "Panel", s1, "p1")).unwrap();
    eh.run();
    eh.harness.get_by_label("Inc").click();
    eh.run();
    assert!(eh.harness.query_by_label("count 1").is_some());
    let color = |h| e.backend().computed_style(h).and_then(|s| s.get("color").cloned());
    let labels = e.backend().handles_of("label");
    assert_eq!(labels.len(), 1);
    let label = labels[0];
    assert_eq!(color(label), rgb(255, 0, 0));
    let _ = e.take_events();

    let panel = dir.join("src/Panel.svelte");
    std::fs::write(&panel, PANEL.replace("#ff0000", "#0000ff")).unwrap();
    let report = e.host().apply_changes(vec![panel.clone()]).expect("a reload ran");
    assert!(report.ok, "{report:?}");
    let hot = report.hot.clone().expect("applied in place");
    assert!(!hot.is_empty() && hot.iter().all(|m| m.starts_with("svelters-style:")), "only the sheet module re-ran: {hot:?}");
    eh.run();

    assert_eq!(e.backend().handles_of("label"), vec![label], "the same node");
    assert!(eh.harness.query_by_label("count 1").is_some(), "the state is kept");
    assert_eq!(color(label), rgb(0, 0, 255), "the new rule applies");
    let ev = e.take_events();
    assert!(!ev.iter().any(|x| matches!(x, EmbedEvent::Faulted { .. } | EmbedEvent::Mounted(_))), "{ev:?}");

    // a markup edit is still H1's component swap (H3 keeps the state), and the sheet stays
    std::fs::write(&panel, PANEL.replace("#ff0000", "#0000ff").replace("count {n}", "total {n}")).unwrap();
    let report = e.host().apply_changes(vec![panel]).expect("a reload ran");
    assert!(report.ok, "{report:?}");
    let hot = report.hot.clone().expect("applied in place");
    assert!(hot.iter().all(|m| !m.starts_with("svelters-style:")), "the sheet did not change: {hot:?}");
    eh.run();
    assert!(eh.harness.query_by_label("total 1").is_some(), "{hot:?}");
    let labels = e.backend().handles_of("label");
    assert_eq!(color(labels[0]), rgb(0, 0, 255), "the swapped component is still styled");
    let errors: Vec<String> = e.host().take_errors().into_iter().map(|x| x.to_string()).collect();
    assert!(errors.is_empty(), "{errors:?}");
}
