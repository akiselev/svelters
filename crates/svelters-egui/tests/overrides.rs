//! DZ0 (SV30): a tool's prop overrides and forced styles, a designer's preview while a gesture
//! lasts. An override wins over the runtime's writes (held back, written again when it ends),
//! never reaches `bind:` state, and goes with its node; forced declarations win over every rule.
//! Loaded from source (`svelters-modules`), so it needs `pnpm install` but no built bundle.

use egui_kittest::kittest::Queryable;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use svelters_abi::Value;
use svelters_egui::embed::{EmbedConfig, MountSpec, Props, SlotOptions};
use svelters_egui::harness::EmbedHarness;
use svelters_host::HostConfig;
use svelters_host::modules::{SourceConfig, SourceProvider};
use svelters_style::{Color, ComputedValue};

const PANEL: &str = r#"<script>
  let n = $state(0);
  let on = $state(false);
</script>

<box>
  <label class="count" text={`Count ${n}`} />
  <button text="Inc" onclicked={() => n++} />
  <checkbox bind:checked={on} text="On" />
  <label text={`state ${on}`} />
</box>

<style>
  .count { color: #ff0000; }
</style>
"#;

fn scratch(name: &str) -> PathBuf {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/embed-egui").canonicalize().unwrap();
    assert!(fixture.join("node_modules").exists(), "run `pnpm install` first");
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/dz0-ov").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(
        dir.join("package.json"),
        r#"{ "name": "dz0ov", "private": true, "type": "module", "svelters": { "toolkit": "egui", "embed": { "Panel": "src/Panel.svelte" } } }"#,
    )
    .unwrap();
    std::fs::write(dir.join("src/Panel.svelte"), PANEL).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(fixture.join("node_modules"), dir.join("node_modules")).unwrap();
    dir.canonicalize().unwrap()
}

fn mounted(dir: &Path) -> EmbedHarness {
    let mut cfg = SourceConfig::for_path(dir).unwrap();
    cfg.dev = true;
    cfg.hmr = true;
    let ecfg = EmbedConfig {
        id_salt: "dz0ov",
        host: HostConfig { echo_console: false, echo_errors: false, quit_on_last_window: false, ..HostConfig::default() },
        ..EmbedConfig::default()
    };
    let mut eh = EmbedHarness::new(egui::vec2(400.0, 300.0), ecfg).unwrap();
    let e = eh.embed.clone();
    let entry = e.load_from(Rc::new(SourceProvider::new(cfg).unwrap())).unwrap();
    let s1 = e.create_slot("a", SlotOptions::fill());
    eh.set_ui(move |e, ui| {
        e.show_slot(ui, s1);
    });
    e.mount(MountSpec::new(entry, "Panel", s1, "p1").props(Props::new())).unwrap();
    eh.run();
    eh
}

fn rgb(r: u8, g: u8, b: u8) -> Option<ComputedValue> {
    Some(ComputedValue::Color(Color { r: r as f32 / 255.0, g: g as f32 / 255.0, b: b as f32 / 255.0, a: 1.0 }))
}

#[test]
fn an_override_holds_back_the_runtime_and_ends_with_its_value() {
    let dir = scratch("props");
    let mut eh = mounted(&dir);
    let e = eh.embed.clone();
    let b = e.backend();
    let count = b.handles_of("label")[0];
    b.set_override(count, "text", &Value::Str("Preview".into())).unwrap();
    eh.run();
    assert!(eh.harness.query_by_label("Preview").is_some());
    assert_eq!(b.overrides(), vec![(count, "text")]);

    // the app keeps writing: held back while the override lasts
    eh.harness.get_by_label("Inc").click();
    eh.run();
    eh.harness.get_by_label("Inc").click();
    eh.run();
    assert!(eh.harness.query_by_label("Preview").is_some() && eh.harness.query_by_label("Count 2").is_none());
    // ending it shows the app's latest value
    assert!(b.clear_override(count, "text").unwrap());
    eh.run();
    assert!(eh.harness.query_by_label("Count 2").is_some());
    assert!(!b.clear_override(count, "text").unwrap(), "not overridden any more");
    assert!(b.overrides().is_empty());

    // a bound prop: the preview never reaches the app's state
    let check = b.handles_of("checkbox")[0];
    b.set_override(check, "checked", &Value::Bool(true)).unwrap();
    eh.run();
    assert!(eh.harness.query_by_label("state false").is_some(), "no echo into bind:");
    b.clear_overrides();
    eh.run();
    assert!(eh.harness.query_by_label("state false").is_some());
    assert!(b.set_override(check, "no-such-prop", &Value::Bool(true)).is_err());
}

#[test]
fn forced_styles_win_over_rules_and_clear() {
    let dir = scratch("styles");
    let mut eh = mounted(&dir);
    let e = eh.embed.clone();
    let b = e.backend();
    let count = b.handles_of("label")[0];
    let color = |h| b.computed_style(h).and_then(|s| s.get("color").cloned());
    assert_eq!(color(count), rgb(255, 0, 0));
    b.force_style(count, Some("color: #00ff00")).unwrap();
    eh.run();
    assert_eq!(color(count), rgb(0, 255, 0), "forced over the component's rule");
    b.force_style(count, None).unwrap();
    eh.run();
    assert_eq!(color(count), rgb(255, 0, 0));
    b.force_style(count, Some("color: #0000ff")).unwrap();
    b.clear_overrides();
    eh.run();
    assert_eq!(color(count), rgb(255, 0, 0), "clear_overrides ends forced styles too");
}

#[test]
fn a_swap_takes_the_overrides_with_the_nodes() {
    let dir = scratch("swap");
    let mut eh = mounted(&dir);
    let e = eh.embed.clone();
    let count = e.backend().handles_of("label")[0];
    e.backend().set_override(count, "text", &Value::Str("Preview".into())).unwrap();
    eh.run();
    // the designer writes the source; the component swaps in place with new nodes
    let panel = dir.join("src/Panel.svelte");
    std::fs::write(&panel, PANEL.replace("Count ${n}", "Total ${n}")).unwrap();
    let report = e.host().apply_changes(vec![panel]).expect("a reload ran");
    assert!(report.ok, "{report:?}");
    eh.run();
    assert!(eh.harness.query_by_label("Total 0").is_some());
    assert!(e.backend().overrides().is_empty(), "{:?}", e.backend().overrides());
}
