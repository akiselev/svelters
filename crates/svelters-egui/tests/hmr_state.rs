//! H3: `$state` kept across component HMR, with several roots in one runtime (EM1's `EmbedHarness`):
//! editing `Panel` keeps each Panel root's own `clicks`; a changed initializer or `// @hmr:reset`
//! starts them fresh; the Pane root is never touched.
//! The app is `fixtures/embed-egui`'s Panel and Pane copied to a scratch dir and loaded from source
//! (`svelters-modules`), so it needs `pnpm install` but no built bundle.

use egui_kittest::kittest::Queryable;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use svelters_egui::embed::{EmbedConfig, EmbedEvent, MountSpec, Props, SlotOptions};
use svelters_egui::harness::EmbedHarness;
use svelters_host::HostConfig;
use svelters_host::modules::{SourceConfig, SourceProvider};

fn scratch(name: &str) -> PathBuf {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/embed-egui").canonicalize().unwrap();
    assert!(fixture.join("node_modules").exists(), "run `pnpm install` first");
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/h3").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(
        dir.join("package.json"),
        r#"{ "name": "h3", "private": true, "type": "module", "svelters": { "toolkit": "egui", "embed": { "Panel": "src/Panel.svelte", "Pane": "src/Pane.svelte", "Shell": "src/Shell.svelte" } } }"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("src/Shell.svelte"),
        "<script>\n  import Panel from './Panel.svelte';\n  let { name = 'Shell' } = $props();\n</script>\n<box gap={4}>\n  <label>{name} header</label>\n  <Panel title=\"A\" />\n  <Panel title=\"B\" />\n</box>\n",
    )
    .unwrap();
    for f in ["Panel.svelte", "Pane.svelte"] {
        std::fs::copy(fixture.join("src").join(f), dir.join("src").join(f)).unwrap();
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink(fixture.join("node_modules"), dir.join("node_modules")).unwrap();
    dir.canonicalize().unwrap()
}

#[test]
fn panel_state_survives_an_edit_in_every_root_and_resets_when_its_shape_changes() {
    let dir = scratch("state-kept");
    let mut cfg = SourceConfig::for_path(&dir).unwrap();
    cfg.dev = true;
    cfg.hmr = true;
    let provider = Rc::new(SourceProvider::new(cfg).unwrap());
    let ecfg = EmbedConfig {
        id_salt: "h3",
        host: HostConfig { echo_console: false, echo_errors: false, quit_on_last_window: false, ..HostConfig::default() },
        ..EmbedConfig::default()
    };
    let mut eh = EmbedHarness::new(egui::vec2(800.0, 600.0), ecfg).unwrap();
    let e = eh.embed.clone();
    let entry = e.load_from(provider).unwrap();
    let (s1, s2, s3) = (e.create_slot("a", SlotOptions::fill()), e.create_slot("b", SlotOptions::fill()), e.create_slot("c", SlotOptions::fill()));
    eh.set_ui(move |e, ui| {
        ui.columns(3, |cols| {
            for (i, s) in [s1, s2, s3].iter().enumerate() {
                e.show_slot(&mut cols[i], *s);
            }
        });
    });
    eh.run();
    e.mount(MountSpec::new(entry, "Panel", s1, "p1").props(Props::new().with("title", "One"))).unwrap();
    let p2 = e.mount(MountSpec::new(entry, "Pane", s2, "p2").props(Props::new().with("name", "Two"))).unwrap();
    e.mount(MountSpec::new(entry, "Panel", s3, "p3").props(Props::new().with("title", "Three"))).unwrap();
    eh.run();
    for label in ["Inc One", "Inc One", "Inc Three", "Inc Two"] {
        eh.harness.get_by_label(label).click();
        eh.run();
    }
    assert!(eh.harness.query_by_label("One clicks: 2").is_some() && eh.harness.query_by_label("Three clicks: 1").is_some());
    assert!(eh.harness.query_by_label("Two count 1").is_some());
    let _ = e.take_events();
    let two_title = e.root_info(p2).unwrap().title;
    let panel = dir.join("src/Panel.svelte");
    let edit = |from: &str, to: &str| {
        let src = std::fs::read_to_string(&panel).unwrap();
        assert!(src.contains(from), "{from}");
        std::fs::write(&panel, src.replacen(from, to, 1)).unwrap();
        let report = e.host().apply_changes(vec![panel.clone()]).expect("a reload ran");
        assert!(report.ok && report.hot.is_some() && report.fallback.is_none(), "{report:?}");
    };

    // a template edit: each Panel root keeps its own clicks, the Pane root is not touched
    edit("Inc {title}", "Plus {title}");
    eh.run();
    assert!(eh.harness.query_by_label("Plus One").is_some() && eh.harness.query_by_label("Plus Three").is_some());
    assert!(eh.harness.query_by_label("One clicks: 2").is_some(), "root one kept its $state");
    assert!(eh.harness.query_by_label("Three clicks: 1").is_some(), "root three kept its own $state");
    assert!(eh.harness.query_by_label("Two count 1").is_some());
    assert_eq!(e.root_info(p2).unwrap().title, two_title);

    // the kept state is live: clicking continues from it
    eh.harness.get_by_label("Plus One").click();
    eh.run();
    assert!(eh.harness.query_by_label("One clicks: 3").is_some());

    // a changed initializer changes the signature: the whole component starts fresh in every root
    edit("$state(0)", "$state(10)");
    eh.run();
    assert!(eh.harness.query_by_label("One clicks: 10").is_some() && eh.harness.query_by_label("Three clicks: 10").is_some());

    // `// @hmr:reset` always resets
    eh.harness.get_by_label("Plus Three").click();
    eh.run();
    assert!(eh.harness.query_by_label("Three clicks: 11").is_some());
    edit("<script>\n", "<script>\n  // @hmr:reset\n");
    eh.run();
    assert!(eh.harness.query_by_label("Three clicks: 10").is_some());
    assert!(eh.harness.query_by_label("Two count 1").is_some(), "the Pane root still untouched");

    let ev = e.take_events();
    assert!(!ev.iter().any(|x| matches!(x, EmbedEvent::Faulted { .. } | EmbedEvent::Mounted(_))), "{ev:?}");
}

#[test]
fn editing_a_parent_keeps_the_state_of_its_children() {
    // H3c: Shell renders two Panels; editing Shell re-creates them, and each takes back its own `clicks`
    let dir = scratch("parent-kept");
    let mut cfg = SourceConfig::for_path(&dir).unwrap();
    cfg.dev = true;
    cfg.hmr = true;
    let provider = Rc::new(SourceProvider::new(cfg).unwrap());
    let ecfg = EmbedConfig {
        id_salt: "h3c",
        host: HostConfig { echo_console: false, echo_errors: false, quit_on_last_window: false, ..HostConfig::default() },
        ..EmbedConfig::default()
    };
    let mut eh = EmbedHarness::new(egui::vec2(800.0, 600.0), ecfg).unwrap();
    let e = eh.embed.clone();
    let entry = e.load_from(provider).unwrap();
    let s1 = e.create_slot("a", SlotOptions::fill());
    eh.set_ui(move |e, ui| {
        e.show_slot(ui, s1);
    });
    eh.run();
    e.mount(MountSpec::new(entry, "Shell", s1, "shell")).unwrap();
    eh.run();
    for label in ["Inc A", "Inc A", "Inc B"] {
        eh.harness.get_by_label(label).click();
        eh.run();
    }
    assert!(eh.harness.query_by_label("A clicks: 2").is_some() && eh.harness.query_by_label("B clicks: 1").is_some());
    let shell = dir.join("src/Shell.svelte");
    let src = std::fs::read_to_string(&shell).unwrap().replace("header", "top");
    std::fs::write(&shell, src).unwrap();
    let report = e.host().apply_changes(vec![shell]).expect("a reload ran");
    assert!(report.ok && report.fallback.is_none(), "{report:?}");
    eh.run();
    assert!(eh.harness.query_by_label("Shell top").is_some());
    assert!(eh.harness.query_by_label("A clicks: 2").is_some(), "child A kept its $state");
    assert!(eh.harness.query_by_label("B clicks: 1").is_some(), "child B kept its own $state");
}
