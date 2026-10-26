//! A host hands its own skelter-core `Document` to the backend (`SkelterBackend::over`; cadatomic's
//! shell: islands, the dock, the status bar): JS's nodes join the host's tree, and embed slots are
//! host nodes (`Embed::adopt_slot`).

use std::path::{Path, PathBuf};
use std::rc::Rc;
use svelters_abi::*;
use svelters_skelter::SkelterBackend;
use svelters_skelter::embed::{Embed, EmbedConfig, EmbedEvent, MountSpec};
use svelters_skelter::kinds::Kinds;
use skelter_core::{Document, KindId, KindRegistry};

/// A host document on the built-in kinds with a `box` it owns.
fn host_doc() -> (Document, skelter_base::ids::NodeId) {
    let reg = svelters_skelter::builtin_kinds().registry().clone();
    let mut doc = Document::new(reg, Box::new(svelters_skelter::backend::NoText));
    let pane = doc.create(KindId::of("box"));
    (doc, pane)
}

#[test]
fn js_nodes_join_the_host_tree_under_an_adopted_node() {
    let (doc, pane) = host_doc();
    let b = SkelterBackend::over(doc).unwrap();
    let slot = b.adopt(pane).unwrap();
    assert_eq!(b.adopt(pane).unwrap(), slot, "one handle per node");
    assert_eq!(b.handle_info(slot).unwrap().lifetime, SVL_LIFE_BORROWED);
    let label = b.create(b.resolve(NameKind::Kind, 0, "label")).unwrap();
    b.insert(slot, label, None, 0).unwrap();
    let ln = b.node(label).unwrap();
    // the host sees it in its own document
    let doc = b.document();
    assert_eq!(doc.borrow().children(pane), [ln]);
    // JS cannot destroy the host's node; releasing it leaves the node to the host
    b.destroy(slot);
    assert!(doc.borrow().contains(pane));
    b.destroy_borrowed(slot);
    assert!(doc.borrow().contains(pane), "an adopted node is the host's");
    assert!(b.node(slot).is_none());
}

#[test]
fn a_document_on_another_registry_is_refused() {
    let doc = Document::new(std::sync::Arc::new(KindRegistry::new()), Box::new(svelters_skelter::backend::NoText));
    let e = SkelterBackend::shared(svelters_skelter::builtin_kinds(), Rc::new(std::cell::RefCell::new(doc))).err().unwrap();
    assert!(e.contains("registry lacks"), "{e}");
    // `with_text`/`new` stay the convenience: the backend's own document
    let own = SkelterBackend::new(std::sync::Arc::new(Kinds::builtin().unwrap()));
    assert!(own.document().borrow().is_empty());
}

/// A10: a host registry with its own kind and the built-ins registered into it
/// (`kinds::register_builtin`) takes JS's nodes through `SkelterBackend::shared`.
#[test]
fn a_host_registry_takes_the_builtin_kinds() {
    static PANE: skelter_core::KindSpec = skelter_core::KindSpec { container: true, ..skelter_core::KindSpec::new("host-pane") };
    #[derive(Default)]
    struct Pane;
    impl skelter_core::Kind for Pane {
        fn spec() -> &'static skelter_core::KindSpec {
            &PANE
        }
        fn record(&self, _cx: &mut skelter_core::RecordCx) {}
    }
    let mut reg = KindRegistry::new();
    reg.register::<Pane>().unwrap();
    svelters_skelter::kinds::register_builtin(&mut reg).unwrap();
    assert!(reg.by_name("scroll-area").is_some() && reg.by_name("collapsing-header").is_some());
    let mut doc = Document::new(std::sync::Arc::new(reg), Box::new(svelters_skelter::backend::NoText));
    let host_pane = doc.create(KindId::of("host-pane"));
    let pane = doc.create(KindId::of("box"));
    doc.insert(host_pane, pane, None, None).unwrap();
    let doc = Rc::new(std::cell::RefCell::new(doc));
    let b = SkelterBackend::shared(svelters_skelter::builtin_kinds(), doc.clone()).unwrap();
    let slot = b.adopt(pane).unwrap();
    for tag in ["button", "collapsing-header", "combo-box"] {
        let h = b.create(b.resolve(NameKind::Kind, 0, tag)).unwrap();
        b.insert(slot, h, None, 0).unwrap();
    }
    assert_eq!(doc.borrow().children(pane).len(), 3);
    // a host kind under a built-in tag is refused
    static BUTTON: skelter_core::KindSpec = skelter_core::KindSpec::new("button");
    let mut clash = KindRegistry::new();
    clash.register_with(&BUTTON, || Box::new(svelters_skelter::kinds::Placeholder)).unwrap();
    assert!(svelters_skelter::kinds::register_builtin(&mut clash).is_err());
}

/// A scratch app (like svelters-egui's `cv1.rs`): `components` exported by file stem, for skelter.
fn scratch(name: &str, components: &[(&str, &str)]) -> PathBuf {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/embed-egui").canonicalize().unwrap();
    assert!(fixture.join("node_modules").exists(), "run `pnpm install` first");
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/svelters-skelter-host").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).unwrap();
    let embed: Vec<String> = components.iter().map(|(f, _)| format!("\"{}\": \"src/{f}\"", f.trim_end_matches(".svelte"))).collect();
    std::fs::write(dir.join("package.json"), format!(r#"{{ "name": "host-doc", "private": true, "type": "module", "svelters": {{ "toolkit": "skelter", "embed": {{ {} }} }} }}"#, embed.join(", "))).unwrap();
    for (f, src) in components {
        std::fs::write(dir.join("src").join(f), src).unwrap();
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink(fixture.join("node_modules"), dir.join("node_modules")).unwrap();
    dir.canonicalize().unwrap()
}

const PANEL: &str = r#"<script>
  let { title = 'Tasks' } = $props();
</script>
<label text={title} />
"#;

#[test]
fn a_svelte_root_mounts_into_a_node_of_the_host_document() {
    let (doc, pane) = host_doc();
    let backend = SkelterBackend::over(doc).unwrap();
    let host = svelters_host::HostConfig { echo_console: false, echo_errors: false, quit_on_last_window: false, ..svelters_host::HostConfig::default() };
    let embed = Embed::with_backend(backend.clone(), EmbedConfig { host, ..EmbedConfig::default() }).unwrap();
    let dir = scratch("mount", &[("Panel.svelte", PANEL)]);
    let mut cfg = svelters_host::modules::SourceConfig::for_path(&dir).unwrap();
    cfg.backend = "skelter".into();
    let entry = embed.load_from(Rc::new(svelters_host::modules::SourceProvider::new(cfg).unwrap())).unwrap();
    let slot = embed.adopt_slot("panel.tasks", pane).unwrap();
    let root = embed.mount(MountSpec::new(entry, "Panel", slot, "panel.tasks")).unwrap();
    embed.logic();
    let events = embed.take_events();
    assert!(events.iter().any(|e| matches!(e, EmbedEvent::Mounted(r) if *r == root)), "{events:?}");
    // the root's label is a child of the host's pane, in the host's document
    let doc = backend.document();
    let kids = doc.borrow().children(pane).to_vec();
    assert_eq!(kids.len(), 1);
    assert_eq!(doc.borrow().kind_of(kids[0]), Some(KindId::of("label")));
    let h = backend.handle(kids[0]).unwrap();
    let text = backend.resolve(NameKind::Prop, backend.handle_info(h).unwrap().kind, "text");
    assert_eq!(backend.get_prop(h, text).unwrap(), Value::Str("Tasks".into()));
    // destroying the slot unmounts the root and leaves the pane to the host
    embed.destroy_slot(slot);
    embed.logic();
    assert!(doc.borrow().contains(pane));
    assert!(doc.borrow().children(pane).is_empty());
}

const COUNTER: &str = r#"<script>
  let count = $state(0);
</script>
<box>
  <button text="Go" onclicked={() => count++} />
  <label text={`n=${count}`} />
</box>
"#;

/// A18: compiled Svelte through the real svelters-host: a skelter `clicked` runs the `onclicked`
/// handler (the host masks signal ids to `SVL_SIGNAL_ID_MASK`, so wire ids must fit in it).
#[test]
fn a_skelter_click_runs_the_svelte_onclicked_handler() {
    let (doc, pane) = host_doc();
    let backend = SkelterBackend::over(doc).unwrap();
    let host = svelters_host::HostConfig { echo_console: false, echo_errors: false, quit_on_last_window: false, ..svelters_host::HostConfig::default() };
    let embed = Embed::with_backend(backend.clone(), EmbedConfig { host, ..EmbedConfig::default() }).unwrap();
    let dir = scratch("counter", &[("Counter.svelte", COUNTER)]);
    let mut cfg = svelters_host::modules::SourceConfig::for_path(&dir).unwrap();
    cfg.backend = "skelter".into();
    let entry = embed.load_from(Rc::new(svelters_host::modules::SourceProvider::new(cfg).unwrap())).unwrap();
    let slot = embed.adopt_slot("panel.counter", pane).unwrap();
    embed.mount(MountSpec::new(entry, "Counter", slot, "panel.counter")).unwrap();
    embed.logic();
    let doc = backend.document();
    let bx = doc.borrow().children(pane)[0];
    let (button, label) = {
        let d = doc.borrow();
        (d.children(bx)[0], d.children(bx)[1])
    };
    let text = |n| {
        let h = backend.handle(n).unwrap();
        let p = backend.resolve(NameKind::Prop, backend.handle_info(h).unwrap().kind, "text");
        backend.get_prop(h, p).unwrap()
    };
    assert_eq!(text(label), Value::Str("n=0".into()));
    let ev = backend.target_event(button, "clicked", vec![]).expect("the handler listens on the button");
    backend.queue(svelters_skelter::Output { events: vec![ev], ..Default::default() });
    backend.pump();
    embed.logic();
    assert_eq!(text(label), Value::Str("n=1".into()), "the onclicked handler ran");
}

const LIST: &str = r#"<script>
  import { ListView } from '@svelters/lists';
  const items = Array.from({ length: 1000 }, (_, i) => ({ id: i, text: `Item ${i}` }));
  let list;
  export function scrollTo(i) { list.scrollTo(i); }
</script>
<box style="flex-direction: column; flex-grow: 1">
  <ListView bind:this={list} {items} key={(it) => it.id} row-height={24}>
    {#snippet row(item)}
      <box><label text={item.text} /></box>
    {/snippet}
  </ListView>
</box>
"#;

/// A25b: a `ListView` in an embed, run as the shell runs it (`logic`, then the document's frame,
/// nothing else): the rows the list asks for during the frame reach JS at the next `logic`, so the
/// visible rows lay out with their text; the embed is busy until they do, and scrolling brings later
/// rows in. No wall clock: a fixed number of logic/frame rounds.
#[test]
fn an_embedded_list_view_shows_its_rows() {
    let (mut doc, pane) = host_doc();
    let w = skelter_base::ids::WindowId(1);
    let root = doc.create_window(w, KindId::of("box"), skelter_base::geom::Size::new(300.0, 240.0), skelter_base::geom::Scale(1.0));
    doc.insert(root, pane, None, None).unwrap();
    doc.set_style(pane, skelter_core::StyleInput { inline: Some("flex-direction: column; width: 300px; height: 240px".into()), ..Default::default() });
    let backend = SkelterBackend::over(doc).unwrap();
    svelters_skelter::app::install_widget_services(&backend, skelter_text::TextSystem::new(skelter_text::FontStore::new(skelter_text::FontConfig::testing())));
    let host = svelters_host::HostConfig { echo_console: false, echo_errors: true, quit_on_last_window: false, ..svelters_host::HostConfig::default() };
    let embed = Embed::with_backend(backend.clone(), EmbedConfig { host, ..EmbedConfig::default() }).unwrap();
    let dir = scratch("list", &[("List.svelte", LIST)]);
    let mut cfg = svelters_host::modules::SourceConfig::for_path(&dir).unwrap();
    cfg.backend = "skelter".into();
    let entry = embed.load_from(Rc::new(svelters_host::modules::SourceProvider::new(cfg).unwrap())).unwrap();
    let slot = embed.adopt_slot("panel.list", pane).unwrap();
    let root_id = embed.mount(MountSpec::new(entry, "List", slot, "panel.list")).unwrap();
    embed.set_slot_layout(slot, w, [300.0, 240.0], 1.0);
    let doc = backend.document();
    let rounds = |n: usize| {
        for _ in 0..n {
            embed.logic();
            doc.borrow_mut().frame(w, skelter_base::time::Time(0));
        }
    };
    rounds(6);
    let errors: Vec<_> = embed.take_events().into_iter().filter(|e| matches!(e, EmbedEvent::Error { .. } | EmbedEvent::Faulted { .. })).collect();
    assert!(errors.is_empty(), "{errors:?}");
    let list = {
        let d = doc.borrow();
        let mut found = None;
        let mut stack = vec![pane];
        while let Some(n) = stack.pop() {
            if d.kind_of(n) == Some(KindId::of("virtual-list")) {
                found = Some(n);
            }
            stack.extend(d.children(n).iter().copied());
        }
        found.expect("the list")
    };
    let text = skelter_core::PropId::of("label", "text", "string");
    // the visible rows: their labels show their items and have a size
    let shown = |d: &skelter_core::Document| -> Vec<String> {
        let off = d.scroll_offset(list).y;
        let view = d.world_rect(list).unwrap();
        let mut out = Vec::new();
        for &(_, row) in d.virtual_rows(list) {
            let mut stack = vec![row];
            while let Some(n) = stack.pop() {
                if let Some(skelter_core::Value::Str(s)) = d.get_prop(n, text) {
                    let r = d.world_rect(n).unwrap();
                    if r.height() > 0.0 && r.min.y >= view.min.y + off && r.max.y <= view.max.y + off {
                        out.push(s.to_string());
                    }
                }
                stack.extend(d.children(n).iter().copied());
            }
        }
        out.sort_by_key(|s| s[5..].parse::<usize>().unwrap_or(0));
        out
    };
    let rows = shown(&doc.borrow());
    assert!(rows.len() >= 8, "visible rows: {rows:?}");
    assert_eq!(rows[0], "Item 0");
    assert!(embed.settled().busy.is_empty() || !embed.settled().busy.contains(svelters_skelter::embed::Busy::EVENTS), "settled: {:?}", embed.settled());
    // the wheel (a scroll offset) brings later rows in
    doc.borrow_mut().set_scroll_offset(list, skelter_base::geom::Vec2::new(0.0, 500.0 * 24.0));
    rounds(6);
    let rows = shown(&doc.borrow());
    assert!(rows.len() >= 8 && rows[0] == "Item 500", "after scrolling: {rows:?}");
    let _ = root_id;
}
