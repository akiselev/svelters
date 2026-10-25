//! DZ0: in a dev build, every scene node knows where in its `.svelte` file it was declared
//! (`source-loc`, `NodeRef::source_loc`): the template element's byte offset, then the component
//! call sites it renders inside. A release build sets nothing. Loaded from source
//! (`svelters-modules`), so it needs `pnpm install` but no built bundle.

use std::path::{Path, PathBuf};
use std::rc::Rc;
use svelters_egui::embed::{EmbedConfig, MountSpec, Props, SlotOptions};
use svelters_egui::harness::EmbedHarness;
use svelters_egui::scene::{SceneRef, SourceSpot};
use svelters_host::HostConfig;
use svelters_host::modules::{SourceConfig, SourceProvider};

const PANEL: &str = r#"<script>
  import Row from './Row.svelte';
  let items = $state(['a', 'b']);
</script>

<box anchor="top">
  <label anchor="title" text="Title" />
  {#each items as it (it)}
    <Row name={it} />
  {/each}
</box>
"#;

const ROW: &str = r#"<script>
  let { name } = $props();
</script>

<button anchor="row" text={name} />
"#;

/// DZ0b2: `<svelte:component>`, `<X>` with a reactive or member `X`, and `{@render}`.
const DYNAMIC: &str = r#"<script>
  import Named from './Named.svelte';
  let Comp = $state(Named);
  const parts = { Named };
</script>

{#snippet two(name)}
  <Named {name} />
{/snippet}

<box anchor="top">
  <svelte:component this={Comp} name="c" />
  <Comp name="d" />
  <parts.Named name="m" />
  {@render two('s')}
  <label anchor="plain" text="plain" />
</box>
"#;

const NAMED: &str = r#"<script>
  let { name } = $props();
</script>

<button anchor={name} text={name} />
"#;

fn scratch(name: &str) -> PathBuf {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/embed-egui").canonicalize().unwrap();
    assert!(fixture.join("node_modules").exists(), "run `pnpm install` first");
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/dz0").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(
        dir.join("package.json"),
        r#"{ "name": "dz0", "private": true, "type": "module", "svelters": { "toolkit": "egui", "embed": { "Panel": "src/Panel.svelte", "Dynamic": "src/Dynamic.svelte" } } }"#,
    )
    .unwrap();
    std::fs::write(dir.join("src/Panel.svelte"), PANEL).unwrap();
    std::fs::write(dir.join("src/Row.svelte"), ROW).unwrap();
    std::fs::write(dir.join("src/Dynamic.svelte"), DYNAMIC).unwrap();
    std::fs::write(dir.join("src/Named.svelte"), NAMED).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(fixture.join("node_modules"), dir.join("node_modules")).unwrap();
    dir.canonicalize().unwrap()
}

fn mounted(name: &str, dev: bool) -> EmbedHarness {
    mounted_as(name, "Panel", dev)
}

fn mounted_as(name: &str, component: &'static str, dev: bool) -> EmbedHarness {
    let dir = scratch(name);
    let mut cfg = SourceConfig::for_path(&dir).unwrap();
    cfg.dev = dev;
    cfg.hmr = dev;
    let provider = Rc::new(SourceProvider::new(cfg).unwrap());
    let ecfg = EmbedConfig {
        id_salt: "dz0",
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
    e.mount(MountSpec::new(entry, component, s1, "p1").props(Props::new())).unwrap();
    eh.run();
    let errors: Vec<String> = e.host().take_errors().into_iter().map(|x| x.to_string()).collect();
    assert!(errors.is_empty(), "{errors:?}");
    eh
}

fn anchored<'a>(s: SceneRef<'a>, anchor: &str) -> Vec<svelters_egui::scene::NodeRef<'a>> {
    s.nodes().filter(|n| n.anchor() == Some(anchor)).collect()
}

fn at(src: &str, needle: &str) -> u32 {
    src.find(needle).unwrap_or_else(|| panic!("{needle} not in the source")) as u32
}

#[test]
fn dev_nodes_name_their_template_element_and_call_sites() {
    let eh = mounted("dev", true);
    eh.embed.backend().scene().read(|s| {
        let title = anchored(s, "title");
        assert_eq!(title.len(), 1);
        let loc = title[0].source_loc().expect("a dev build sets source-loc");
        let panel = loc.element().file;
        assert!(panel.ends_with("src/Panel.svelte"), "{panel}");
        assert_eq!(loc.element().offset, at(PANEL, "<label"));
        assert_eq!(loc.sites().count(), 0, "declared in the root component: no call site");

        let top = anchored(s, "top");
        assert_eq!(top[0].source_loc().unwrap().element(), SourceSpot { file: panel, offset: at(PANEL, "<box") });

        // each row's button: Row.svelte's element, rendered through Panel's `<Row>` call site
        let rows = anchored(s, "row");
        assert_eq!(rows.len(), 2);
        for r in &rows {
            let loc = r.source_loc().unwrap();
            assert!(loc.element().file.ends_with("src/Row.svelte"), "{}", loc.raw());
            assert_eq!(loc.element().offset, at(ROW, "<button"));
            let sites: Vec<_> = loc.sites().collect();
            assert_eq!(sites, vec![SourceSpot { file: panel, offset: at(PANEL, "<Row") }]);
        }
        let row_file = rows[0].source_loc().unwrap().element().file;
        // one template element, one node per row
        assert_eq!(s.by_source(row_file, at(ROW, "<button")).count(), 2);
        assert_eq!(s.by_source(panel, at(PANEL, "<label")).count(), 1);
        assert_eq!(s.by_source(panel, 9999).count(), 0);
        let mut keys: Vec<_> = rows.iter().map(|r| r.item_key().unwrap().to_string()).collect();
        keys.sort();
        assert_eq!(keys, ["a", "b"], "the keyed row is the each item's key, as before");
    });
}

#[test]
fn dynamic_components_and_renders_add_their_call_sites() {
    let eh = mounted_as("dynamic", "Dynamic", true);
    eh.embed.backend().scene().read(|s| {
        let sites = |anchor: &str| {
            let n = anchored(s, anchor);
            assert_eq!(n.len(), 1, "{anchor}");
            let loc = n[0].source_loc().unwrap_or_else(|| panic!("{anchor} has source-loc"));
            assert!(loc.element().file.ends_with("src/Named.svelte"), "{}", loc.raw());
            assert_eq!(loc.element().offset, at(NAMED, "<button"));
            loc.sites().map(|x| (x.file.rsplit('/').next().unwrap().to_string(), x.offset)).collect::<Vec<_>>()
        };
        let dynamic = |needle: &str| ("Dynamic.svelte".to_string(), at(DYNAMIC, needle));
        assert_eq!(sites("c"), vec![dynamic("<svelte:component")]);
        assert_eq!(sites("d"), vec![dynamic("<Comp")]);
        assert_eq!(sites("m"), vec![dynamic("<parts.Named")]);
        // the call inside the snippet, then the render tag, innermost first
        assert_eq!(sites("s"), vec![dynamic("<Named {name}"), dynamic("{@render")]);
        let plain = anchored(s, "plain")[0].source_loc().unwrap();
        assert_eq!(plain.sites().count(), 0);
    });
}

/// DZ0's designer pick: the stack under a point names the label, then its box, each mapping to its
/// template element; `hit_test` (input) still reports only the label.
#[test]
fn the_hit_stack_lists_boxes_innermost_first() {
    let eh = mounted("pick", true);
    eh.embed.backend().scene().read(|s| {
        let label = anchored(s, "title")[0];
        let top = anchored(s, "top")[0];
        let (vp, rect) = label.screen_rect().expect("drawn");
        let stack: Vec<_> = s.hit_stack(vp, rect.center()).into_iter().map(|t| t.node()).collect();
        assert_eq!(stack.first(), Some(&label.id()), "{stack:?}");
        let k_top = stack.iter().position(|&n| n == top.id()).expect("the label's box is in the stack");
        assert!(k_top > 0);
        // every box listed after the label is one of its ancestors, innermost first
        let ancestors: Vec<_> = label.ancestors().map(|a| a.id()).collect();
        let boxes: Vec<_> = stack[1..].iter().copied().filter(|n| ancestors.contains(n)).collect();
        assert_eq!(boxes, stack[1..].to_vec(), "only the label's ancestors are under it here");
        let mut last = 0;
        for b in &boxes {
            let depth = ancestors.iter().position(|a| a == b).unwrap();
            assert!(depth >= last, "innermost first: {boxes:?}");
            last = depth;
        }
        let at_box = stack[k_top];
        assert_eq!(s.get(at_box).unwrap().source_loc().unwrap().element().offset, at(PANEL, "<box"));
        // input hit testing is unchanged: leaves only
        assert_eq!(s.hit_test(vp, rect.center()).iter().map(|t| t.node()).collect::<Vec<_>>(), vec![label.id()]);
        // a point on the box but beside the rows' buttons: the box (and its ancestors), no leaf
        let (_, top_rect) = top.screen_rect().or_else(|| s.box_rect(vp, top.id()).map(|r| (vp, r))).expect("the box was drawn");
        let beside = egui::pos2(top_rect.right() - 1.0, top_rect.bottom() - 1.0);
        let stack = s.hit_stack(vp, beside);
        assert!(stack.iter().any(|t| t.node() == top.id()), "{stack:?}");
    });
}

#[test]
fn release_nodes_have_no_source_loc() {
    let eh = mounted("release", false);
    eh.embed.backend().scene().read(|s| {
        assert!(!anchored(s, "title").is_empty());
        assert!(s.nodes().all(|n| n.source_loc().is_none()));
    });
}
