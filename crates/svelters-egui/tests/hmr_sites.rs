//! DZ0: a swapped parent keeps its children's state by call site, not by position. In a dev build
//! the runtime knows each component call's site (`file#offset`) and the keyed `{#each}` items around
//! it, and the loader tells it the edit that made the new version, so inserting another call of
//! the same component before stateful ones, or reordering a keyed list before an edit, keeps every
//! instance's `$state` with its own call. Loaded from source; needs `pnpm install`.

use egui_kittest::kittest::Queryable;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use svelters_egui::embed::{EmbedConfig, MountSpec, Props, SlotOptions};
use svelters_egui::harness::EmbedHarness;
use svelters_host::HostConfig;
use svelters_host::modules::{SourceConfig, SourceProvider};

const COUNTER: &str = r#"<script>
  let { name } = $props();
  let n = $state(0);
</script>

<button text={`${name} ${n}`} onclicked={() => n++} />
"#;

const PANEL: &str = r#"<script>
  import Counter from './Counter.svelte';
  let items = $state(['x', 'y']);
</script>

<box>
  <Counter name="a" />
  <Counter name="b" />
  {#each items as it (it)}
    <Counter name={it} />
  {/each}
  <button text="Flip" onclicked={() => (items = items.toReversed())} />
</box>
"#;

fn scratch(name: &str) -> PathBuf {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/embed-egui").canonicalize().unwrap();
    assert!(fixture.join("node_modules").exists(), "run `pnpm install` first");
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/dz0-sites").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(
        dir.join("package.json"),
        r#"{ "name": "dz0sites", "private": true, "type": "module", "svelters": { "toolkit": "egui", "embed": { "Panel": "src/Panel.svelte" } } }"#,
    )
    .unwrap();
    std::fs::write(dir.join("src/Panel.svelte"), PANEL).unwrap();
    std::fs::write(dir.join("src/Counter.svelte"), COUNTER).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(fixture.join("node_modules"), dir.join("node_modules")).unwrap();
    dir.canonicalize().unwrap()
}

fn mounted(dir: &Path) -> EmbedHarness {
    let mut cfg = SourceConfig::for_path(dir).unwrap();
    cfg.dev = true;
    cfg.hmr = true;
    let ecfg = EmbedConfig {
        id_salt: "dz0sites",
        host: HostConfig { echo_console: false, echo_errors: false, quit_on_last_window: false, ..HostConfig::default() },
        ..EmbedConfig::default()
    };
    let mut eh = EmbedHarness::new(egui::vec2(500.0, 400.0), ecfg).unwrap();
    let e = eh.embed.clone();
    let entry = e.load_from(Rc::new(SourceProvider::new(cfg).unwrap())).unwrap();
    let s1 = e.create_slot("a", SlotOptions::fill());
    eh.set_ui(move |e, ui| {
        e.show_slot(ui, s1);
    });
    e.mount(MountSpec::new(entry, "Panel", s1, "p1").props(Props::new())).unwrap();
    eh.run();
    let errors: Vec<String> = e.host().take_errors().into_iter().map(|x| x.to_string()).collect();
    assert!(errors.is_empty(), "{errors:?}");
    let _ = e.take_events();
    eh
}

/// Clicks the button labelled `name n` `times` times, starting at `n`.
fn click(eh: &mut EmbedHarness, label: &str, times: usize) {
    let (name, n) = label.rsplit_once(' ').map_or((label, None), |(a, b)| (a, b.parse::<usize>().ok()));
    for k in 0..times {
        let now = n.map_or(name.to_string(), |n| format!("{name} {}", n + k));
        eh.harness.get_by_label(&now).click();
        eh.run();
        let errors: Vec<String> = eh.embed.host().take_errors().into_iter().map(|x| x.to_string()).collect();
        assert!(errors.is_empty(), "{errors:?}");
        let ev = eh.embed.take_events();
        assert!(ev.is_empty(), "{ev:?}");
    }
}

fn swap(eh: &mut EmbedHarness, dir: &Path, src: &str) {
    let panel = dir.join("src/Panel.svelte");
    std::fs::write(&panel, src).unwrap();
    let report = eh.embed.host().apply_changes(vec![panel]).expect("a reload ran");
    assert!(report.ok && report.hot.is_some(), "{report:?}");
    eh.run();
}

fn shown(eh: &EmbedHarness, label: &str) -> bool {
    eh.harness.query_by_label(label).is_some()
}

#[test]
fn inserting_a_call_before_others_keeps_their_state() {
    let dir = scratch("insert");
    let mut eh = mounted(&dir);
    click(&mut eh, "a 0", 1);
    click(&mut eh, "b 0", 3);
    assert!(shown(&eh, "a 1") && shown(&eh, "b 3"));
    // a new call of the same component, before both
    swap(&mut eh, &dir, &PANEL.replace("<box>\n", "<box>\n  <Counter name=\"new\" />\n"));
    assert!(shown(&eh, "new 0"), "the new call starts fresh");
    assert!(shown(&eh, "a 1") && shown(&eh, "b 3"), "a and b keep their counts");
    // and an edit between them
    swap(&mut eh, &dir, &PANEL.replace("<box>\n", "<box>\n  <Counter name=\"new\" />\n").replace("  <Counter name=\"b\" />", "  <label text=\"between\" />\n  <Counter name=\"b\" />"));
    assert!(shown(&eh, "between"));
    assert!(shown(&eh, "a 1") && shown(&eh, "b 3") && shown(&eh, "new 0"));
}

#[test]
fn a_reordered_keyed_list_keeps_each_items_state() {
    let dir = scratch("keyed");
    let mut eh = mounted(&dir);
    click(&mut eh, "y 0", 2);
    assert!(shown(&eh, "x 0") && shown(&eh, "y 2"));
    click(&mut eh, "Flip", 1);
    assert!(shown(&eh, "x 0") && shown(&eh, "y 2"), "a reorder moves the instances");
    // an edit swaps the parent: each item's state stays with its key, not its position
    swap(&mut eh, &dir, &PANEL.replace("text=\"Flip\"", "text=\"Reverse\""));
    assert!(shown(&eh, "Reverse"));
    assert!(shown(&eh, "y 2") && shown(&eh, "x 0"), "y keeps 2, x keeps 0");
}
