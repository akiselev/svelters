//! H1: component HMR with several roots in one runtime (EM1's `EmbedHarness`): editing the component
//! one root shows swaps that root's content in place; the other roots keep their widgets and state.
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
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/h1").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(
        dir.join("package.json"),
        r#"{ "name": "h1", "private": true, "type": "module", "svelters": { "toolkit": "egui", "embed": { "Panel": "src/Panel.svelte", "Pane": "src/Pane.svelte" } } }"#,
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
fn swapping_one_roots_component_leaves_the_other_roots_alone() {
    let dir = scratch("embed-swap");
    let mut cfg = SourceConfig::for_path(&dir).unwrap();
    cfg.dev = true;
    cfg.hmr = true;
    let provider = Rc::new(SourceProvider::new(cfg).unwrap());
    let ecfg = EmbedConfig {
        id_salt: "h1",
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
    let p1 = e.mount(MountSpec::new(entry, "Panel", s1, "p1").props(Props::new().with("title", "One"))).unwrap();
    let p2 = e.mount(MountSpec::new(entry, "Pane", s2, "p2").props(Props::new().with("name", "Two"))).unwrap();
    let p3 = e.mount(MountSpec::new(entry, "Panel", s3, "p3").props(Props::new().with("title", "Three"))).unwrap();
    eh.run();
    eh.harness.get_by_label("Inc Two").click();
    eh.run();
    eh.harness.get_by_label("Inc One").click();
    eh.run();
    assert!(eh.harness.query_by_label("Two count 1").is_some() && eh.harness.query_by_label("One clicks: 1").is_some());
    let _ = e.take_events();
    let two_title = e.root_info(p2).unwrap().title;

    // edit Panel: its roots (p1, p3) swap in place; the Pane root p2 is not touched
    let panel = dir.join("src/Panel.svelte");
    let src = std::fs::read_to_string(&panel).unwrap().replace("Inc {title}", "Plus {title}");
    std::fs::write(&panel, src).unwrap();
    let report = e.host().apply_changes(vec![panel]).expect("a reload ran");
    assert!(report.ok && report.hot.is_some(), "{report:?}");
    eh.run();

    assert!(eh.harness.query_by_label("Plus One").is_some() && eh.harness.query_by_label("Plus Three").is_some());
    assert!(eh.harness.query_by_label("Inc One").is_none(), "the old version's widgets are gone");
    assert!(eh.harness.query_by_label("Two count 1").is_some(), "the Pane root keeps its widgets and its state");
    assert_eq!(e.root_info(p2).unwrap().title, two_title);
    // the swapped roots keep their local state (H3); tests/hmr_state.rs covers when it resets
    assert!(eh.harness.query_by_label("One clicks: 1").is_some());
    // no root faulted, none was re-mounted
    let ev = e.take_events();
    assert!(!ev.iter().any(|x| matches!(x, EmbedEvent::Faulted { .. } | EmbedEvent::Mounted(_))), "{ev:?}");
    let _ = (p1, p3);
}
