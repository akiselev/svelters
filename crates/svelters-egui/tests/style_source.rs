//! DZ0: in a dev build, the rules that match a node (`EguiBackend::matched_rules`) say where they
//! and their declarations are in the component's `<style>`, as offsets from the style content's
//! start, so they stay right while the template above is edited. Loaded from source
//! (`svelters-modules`), so it needs `pnpm install` but no built bundle.

use std::path::{Path, PathBuf};
use std::rc::Rc;
use svelters_egui::embed::{EmbedConfig, MountSpec, Props, SlotOptions};
use svelters_egui::harness::EmbedHarness;
use svelters_host::HostConfig;
use svelters_host::modules::{SourceConfig, SourceProvider};

const PANEL: &str = r#"<script>
  let n = $state(0);
</script>

<box>
  <label class="title">Count {n}</label>
</box>

<style>
  /* a comment, kept out of every run */
  .title {
    color: #ff0000;
    padding: 2px;
  }
  label { margin: 1px; }
</style>
"#;

fn scratch(name: &str) -> PathBuf {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/embed-egui").canonicalize().unwrap();
    assert!(fixture.join("node_modules").exists(), "run `pnpm install` first");
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/dz0-style").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(
        dir.join("package.json"),
        r#"{ "name": "dz0s", "private": true, "type": "module", "svelters": { "toolkit": "egui", "embed": { "Panel": "src/Panel.svelte" } } }"#,
    )
    .unwrap();
    std::fs::write(dir.join("src/Panel.svelte"), PANEL).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(fixture.join("node_modules"), dir.join("node_modules")).unwrap();
    dir.canonicalize().unwrap()
}

fn mounted(dir: &Path, dev: bool) -> EmbedHarness {
    let mut cfg = SourceConfig::for_path(dir).unwrap();
    cfg.dev = dev;
    cfg.hmr = dev;
    let provider = Rc::new(SourceProvider::new(cfg).unwrap());
    let ecfg = EmbedConfig {
        id_salt: "dz0s",
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
    e.mount(MountSpec::new(entry, "Panel", s1, "p1").props(Props::new())).unwrap();
    eh.run();
    let errors: Vec<String> = e.host().take_errors().into_iter().map(|x| x.to_string()).collect();
    assert!(errors.is_empty(), "{errors:?}");
    eh
}

/// The offset of `needle` from the start of `src`'s `<style>` content.
fn in_style(src: &str, needle: &str) -> u32 {
    let start = src.find("<style>").unwrap() + "<style>".len();
    (src[start..].find(needle).unwrap_or_else(|| panic!("{needle}"))) as u32
}

#[test]
fn matched_rules_point_into_the_style_block() {
    let dir = scratch("dev");
    let eh = mounted(&dir, true);
    let e = eh.embed.clone();
    let label = e.backend().handles_of("label")[0];
    let rules = e.backend().matched_rules(label);
    let title = rules.iter().find(|r| r.selectors.contains(".title")).unwrap_or_else(|| panic!("{rules:?}"));
    let src = title.source.as_ref().expect("a dev build maps the rule");
    assert!(src.file.ends_with("src/Panel.svelte"), "{src:?}");
    assert_eq!(src.style_offset, in_style(PANEL, ".title {"));
    let decl = |r: &svelters_egui::style::MatchedRuleInfo, name: &str| r.decls.iter().find(|d| d.name == name).cloned().unwrap();
    assert_eq!(decl(title, "color").source.unwrap().style_offset, in_style(PANEL, "color: #ff0000"));
    assert_eq!(decl(title, "padding").source.unwrap().style_offset, in_style(PANEL, "padding: 2px"));
    assert_eq!(decl(title, "color").value, "#ff0000");
    // a type selector gets the scoping class: its start is still the source's `label`
    let by_tag = rules.iter().find(|r| r.decls.iter().any(|d| d.name == "margin")).unwrap();
    assert!(by_tag.selectors.contains("svelte-"), "{}", by_tag.selectors);
    assert_eq!(by_tag.source.as_ref().unwrap().style_offset, in_style(PANEL, "label { margin"));
    assert_eq!(decl(by_tag, "margin").source.unwrap().style_offset, in_style(PANEL, "margin: 1px"));
    assert!(by_tag.sheet.as_deref().is_some_and(|s| s.ends_with("src/Panel.svelte")), "{:?}", by_tag.sheet);

    // an edit above the style block: the sheet does not re-run, and the offsets still hold for
    // the file as it is now
    let edited = PANEL.replace("<box>", "<box gap={4}>\n  <label>New</label>");
    std::fs::write(dir.join("src/Panel.svelte"), &edited).unwrap();
    let report = e.host().apply_changes(vec![dir.join("src/Panel.svelte")]).expect("a reload ran");
    assert!(report.ok, "{report:?}");
    let mut eh = eh;
    eh.run();
    let labels = e.backend().handles_of("label");
    let rules = e.backend().matched_rules(*labels.last().unwrap());
    let title = rules.iter().find(|r| r.selectors.contains(".title")).unwrap();
    assert_eq!(title.source.as_ref().unwrap().style_offset, in_style(&edited, ".title {"));
}

#[test]
fn release_builds_map_nothing() {
    let dir = scratch("release");
    let eh = mounted(&dir, false);
    let label = eh.embed.backend().handles_of("label")[0];
    let rules = eh.embed.backend().matched_rules(label);
    assert!(rules.iter().any(|r| r.selectors.contains(".title")), "{rules:?}");
    assert!(rules.iter().all(|r| r.source.is_none() && r.decls.iter().all(|d| d.source.is_none())));
}
