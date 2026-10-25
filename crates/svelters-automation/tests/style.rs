//! DZ0b2: `style.matched` and `style.computed` over the JSON path. `style.matched` gives the
//! rules that match a node in cascade order, with where each rule and declaration is in the
//! component's `<style>` and which declaration wins each property; `style.computed` gives the
//! computed values, the custom properties, where inherited values come from, and the same rules.

use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use svelters_automation::Session;
use svelters_egui::embed::{EmbedConfig, MountSpec, Props, SlotOptions};
use svelters_egui::harness::EmbedHarness;
use svelters_host::HostConfig;
use svelters_host::modules::{SourceConfig, SourceProvider};

const PANEL: &str = r#"<box anchor="top" class="outer">
  <label anchor="title" class="title">Title</label>
</box>

<style>
  .outer { color: #00ff00; --accent: #123456; }
  .title { padding: 2px; margin: 3px; }
  label { margin: 1px; }
</style>
"#;

fn scratch(name: &str) -> PathBuf {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/embed-egui").canonicalize().unwrap();
    assert!(fixture.join("node_modules").exists(), "run `pnpm install` first");
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/dz0b2-style").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(
        dir.join("package.json"),
        r#"{ "name": "dz0b2s", "private": true, "type": "module", "svelters": { "toolkit": "egui", "embed": { "Panel": "src/Panel.svelte" } } }"#,
    )
    .unwrap();
    std::fs::write(dir.join("src/Panel.svelte"), PANEL).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(fixture.join("node_modules"), dir.join("node_modules")).unwrap();
    dir.canonicalize().unwrap()
}

fn embedded(name: &str) -> Session<EmbedHarness> {
    let mut cfg = SourceConfig::for_path(&scratch(name)).unwrap();
    cfg.dev = true;
    let ecfg = EmbedConfig {
        id_salt: "dz0b2s",
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

/// The offset of `needle` from the start of `PANEL`'s `<style>` content.
fn in_style(needle: &str) -> u64 {
    let start = PANEL.find("<style>").unwrap() + "<style>".len();
    PANEL[start..].find(needle).unwrap_or_else(|| panic!("{needle}")) as u64
}

fn decl<'a>(rule: &'a Value, name: &str) -> &'a Value {
    rule["decls"].as_array().unwrap().iter().find(|d| d["name"] == name).unwrap_or_else(|| panic!("{name} in {rule}"))
}

#[test]
fn matched_rules_in_cascade_order_with_winners_and_sources() {
    let mut s = embedded("matched");
    let caps = s.call("session.hello", &json!({})).unwrap()["capabilities"].clone();
    for m in ["style.matched", "style.computed"] {
        assert!(caps.as_array().unwrap().iter().any(|c| c == m), "{m} in {caps}");
    }

    let rules = s.call("style.matched", &json!({"selector": "anchor=title"})).unwrap();
    let rules = rules.as_array().unwrap();
    let by_tag = rules.iter().position(|r| r["decls"].as_array().unwrap().iter().any(|d| d["value"] == "1px")).unwrap();
    let title = rules.iter().position(|r| r["selectors"].as_str().unwrap().contains(".title")).unwrap();
    assert!(by_tag < title, "lowest priority first: {rules:?}");
    let (by_tag, title) = (&rules[by_tag], &rules[title]);
    assert!(title["specificity"].as_u64().unwrap() > by_tag["specificity"].as_u64().unwrap());
    assert!(title["sheet"].as_str().unwrap().ends_with("src/Panel.svelte"), "{title}");
    assert!(title["line"].as_u64().unwrap() >= 1 && title["column"].as_u64().unwrap() >= 1);
    assert!(title["source"]["file"].as_str().unwrap().ends_with("src/Panel.svelte"));
    assert_eq!(title["source"]["style_offset"], in_style(".title {"));
    assert_eq!(decl(title, "margin")["source"]["style_offset"], in_style("margin: 3px"));
    assert_eq!(decl(title, "padding")["value"], "2px");
    assert_eq!(decl(title, "padding")["important"], false);
    // `.title` beats `label` for margin; padding has no rival
    assert_eq!(decl(title, "margin")["winner"], true);
    assert_eq!(decl(title, "padding")["winner"], true);
    assert_eq!(decl(by_tag, "margin")["winner"], false);

    // a ref target works like a selector; the outer box's own rule wins its color
    let top = s.call("scene.locate", &json!({"selector": "anchor=top"})).unwrap();
    let rules = s.call("style.matched", &json!({"ref": top})).unwrap();
    let outer = rules.as_array().unwrap().iter().find(|r| r["selectors"].as_str().unwrap().contains(".outer")).unwrap();
    assert_eq!(decl(outer, "color")["winner"], true);

    let e = s.call("style.matched", &json!({"selector": "anchor=nothing"})).unwrap_err();
    assert_eq!(e.code, "node.not_found", "{e:?}");
}

#[test]
fn computed_values_customs_and_inherited_from() {
    let mut s = embedded("computed");
    let top = s.call("scene.locate", &json!({"selector": "anchor=top"})).unwrap();
    let c = s.call("style.computed", &json!({"selector": "anchor=title"})).unwrap();
    assert!(c["values"]["color"].is_string(), "{c}");
    assert_eq!(c["custom"]["--accent"], "#123456", "{c}");
    // color and --accent come from the box; the label declares neither
    assert_eq!(c["inherited"]["color"]["id"], top["id"], "{c}");
    assert_eq!(c["inherited"]["--accent"]["id"], top["id"], "{c}");
    assert!(c["inherited"].get("margin-top").is_none(), "margin is not inherited: {c}");
    assert!(c["rules"].as_array().unwrap().iter().any(|r| r["selectors"].as_str().unwrap().contains(".title")), "{c}");

    // the box declares its color itself
    let b = s.call("style.computed", &json!({"ref": top})).unwrap();
    assert!(b["inherited"].get("color").is_none(), "{b}");
    assert_eq!(b["values"]["color"], c["values"]["color"]);
}
