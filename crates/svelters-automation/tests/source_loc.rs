//! DZ0: in a dev build, `scene.query` reports where each node was declared (`source`: the
//! `.svelte` file, the element's byte offset, the component call sites), and the `source=` engine
//! finds nodes by it: `source=FILE#OFFSET` (FILE a path suffix) or `source=FILE`.

use serde_json::json;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use svelters_automation::Session;
use svelters_egui::embed::{EmbedConfig, MountSpec, Props, SlotOptions};
use svelters_egui::harness::EmbedHarness;
use svelters_host::HostConfig;
use svelters_host::modules::{SourceConfig, SourceProvider};

const PANEL: &str = r#"<script>
  import Row from './Row.svelte';
  const items = ['a', 'b', 'c'];
</script>

<box anchor="top">
  <label anchor="title">Title</label>
  {#each items as it (it)}
    <Row name={it} />
  {/each}
</box>
"#;

const ROW: &str = r#"<script>
  let { name } = $props();
</script>

<button anchor="row">{name}</button>
"#;

fn scratch(name: &str) -> PathBuf {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/embed-egui").canonicalize().unwrap();
    assert!(fixture.join("node_modules").exists(), "run `pnpm install` first");
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/dz0-au").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(
        dir.join("package.json"),
        r#"{ "name": "dz0au", "private": true, "type": "module", "svelters": { "toolkit": "egui", "embed": { "Panel": "src/Panel.svelte" } } }"#,
    )
    .unwrap();
    std::fs::write(dir.join("src/Panel.svelte"), PANEL).unwrap();
    std::fs::write(dir.join("src/Row.svelte"), ROW).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(fixture.join("node_modules"), dir.join("node_modules")).unwrap();
    dir.canonicalize().unwrap()
}

fn embedded(name: &str, dev: bool) -> Session<EmbedHarness> {
    let dir = scratch(name);
    let mut cfg = SourceConfig::for_path(&dir).unwrap();
    cfg.dev = dev;
    let ecfg = EmbedConfig {
        id_salt: "dz0au",
        host: HostConfig { echo_console: false, echo_errors: false, quit_on_last_window: false, ..HostConfig::default() },
        ..EmbedConfig::default()
    };
    let mut eh = EmbedHarness::new(egui::vec2(400.0, 300.0), ecfg).unwrap();
    let e = eh.embed.clone();
    let entry = e.load_from(Rc::new(SourceProvider::new(cfg).unwrap())).unwrap();
    let slot = e.create_slot("panel", SlotOptions::fill());
    eh.set_ui(move |e, ui| {
        e.show_slot(ui, slot);
    });
    e.mount(MountSpec::new(entry, "Panel", slot, "panel").props(Props::new())).unwrap();
    let mut s = Session::for_test(eh);
    s.wait_settled(10_000.0).unwrap();
    s
}

fn at(src: &str, needle: &str) -> u32 {
    src.find(needle).unwrap() as u32
}

#[test]
fn query_reports_sources_and_the_source_engine_finds_them() {
    let mut s = embedded("dev", true);
    let title = s.node(s.locate("anchor=title").unwrap()).unwrap();
    let src = title.source.clone().expect("a dev build reports the source");
    assert!(src.file.ends_with("src/Panel.svelte"), "{src:?}");
    assert_eq!(src.offset, at(PANEL, "<label"));
    assert!(src.sites.is_empty());

    // FILE#OFFSET with FILE a path suffix: every row of Row.svelte's button
    let rows = s.query(&format!("source=src/Row.svelte#{}", at(ROW, "<button")), None).unwrap();
    assert_eq!(rows.len(), 3);
    for r in &rows {
        let src = r.source.as_ref().unwrap();
        assert_eq!(src.sites.len(), 1);
        assert!(src.sites[0].0.ends_with("src/Panel.svelte"));
        assert_eq!(src.sites[0].1, at(PANEL, "<Row"));
    }
    // FILE alone: every node declared in it; chained with other engines
    assert_eq!(s.query("source=Panel.svelte", None).unwrap().len(), 2, "the box and the label");
    assert_eq!(s.query("key=b >> source=Row.svelte", None).unwrap().len(), 1);
    assert!(s.query("source=nel.svelte", None).unwrap().is_empty(), "a suffix matches at a `/` only");

    // the JSON method carries it too
    let j = s.call("scene.query", &json!({"selector": "anchor=title"})).unwrap();
    assert_eq!(j[0]["source"]["offset"], at(PANEL, "<label"));
    assert_eq!(j[0]["source"]["sites"], json!([]));
}

#[test]
fn release_builds_report_no_source() {
    let mut s = embedded("release", false);
    let title = s.node(s.locate("anchor=title").unwrap()).unwrap();
    assert_eq!(title.source, None);
    assert!(s.query("source=Panel.svelte", None).unwrap().is_empty());
    let j = s.call("scene.query", &json!({"selector": "anchor=title"})).unwrap();
    assert!(j[0].get("source").is_none(), "{j}");
}
