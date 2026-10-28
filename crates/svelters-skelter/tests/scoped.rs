//! SV85: a native export ties a disposer to the component whose code called it
//! (`svelters::exports::on_destroy`): it runs when that component is destroyed (a keyed item that
//! leaves, the root unmounting) or when the `$effect` that called it re-runs, and not before.

use skelter_base::geom::{Scale, Size};
use skelter_base::ids::WindowId;
use skelter_core::{Document, KindId};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use svelters_skelter::SkelterBackend;
use svelters_skelter::embed::{Embed, EmbedConfig, MountSpec, Props};

thread_local! {
    static LOG: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

fn log() -> Vec<String> {
    LOG.with(|l| std::mem::take(&mut *l.borrow_mut()))
}

/// Defines `name` for as long as the calling component lives.
#[svelters::export(module = "scoped")]
pub fn define(name: String) -> bool {
    LOG.with(|l| l.borrow_mut().push(format!("define {name}")));
    svelters::exports::on_destroy(move || LOG.with(|l| l.borrow_mut().push(format!("release {name}"))))
}

const ITEM: &str = r#"<script>
  import { define } from 'svelters:app/scoped';
  let { name } = $props();
  define(name);
</script>
<label text={name} />
"#;

const LIST: &str = r#"<script>
  import { define } from 'svelters:app/scoped';
  import Item from './Item.svelte';
  let { names = [], round = 0 } = $props();
  $effect(() => {
    define(`effect ${round}`);
  });
</script>
<box>
  {#each names as n (n)}
    <Item name={n} />
  {/each}
</box>
"#;

fn scratch(name: &str) -> PathBuf {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/embed-egui").canonicalize().unwrap();
    assert!(fixture.join("node_modules").exists(), "run `pnpm install` first");
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/svelters-skelter-scoped").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(dir.join("package.json"), r#"{ "name": "scoped", "private": true, "type": "module", "svelters": { "toolkit": "skelter", "embed": { "List": "src/List.svelte" } } }"#).unwrap();
    std::fs::write(dir.join("src/List.svelte"), LIST).unwrap();
    std::fs::write(dir.join("src/Item.svelte"), ITEM).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(fixture.join("node_modules"), dir.join("node_modules")).unwrap();
    dir.canonicalize().unwrap()
}

fn names(v: &[&str]) -> svelters_abi::Value {
    svelters_abi::Value::List(v.iter().map(|s| svelters_abi::Value::Str((*s).into())).collect())
}

#[test]
fn sv85_an_export_ties_a_disposer_to_its_calling_component() {
    let reg = svelters_skelter::builtin_kinds().registry().clone();
    let mut doc = Document::new(reg, Box::new(svelters_skelter::backend::NoText));
    let win = doc.create_window(WindowId(1), KindId::of("box"), Size::new(200.0, 200.0), Scale(1.0));
    let pane = doc.create(KindId::of("box"));
    doc.insert(win, pane, None, None).unwrap();
    let doc = Rc::new(RefCell::new(doc));
    let backend = SkelterBackend::shared(svelters_skelter::builtin_kinds(), doc.clone()).unwrap();
    let host = svelters_host::HostConfig { echo_console: false, echo_errors: true, quit_on_last_window: false, ..svelters_host::HostConfig::default() };
    let embed = Embed::with_backend(backend, EmbedConfig { host, ..EmbedConfig::default() }).unwrap();
    let mut cfg = svelters_host::modules::SourceConfig::for_path(&scratch("ties")).unwrap();
    cfg.backend = "skelter".into();
    let entry = embed.load_from(Rc::new(svelters_host::modules::SourceProvider::new(cfg).unwrap())).unwrap();
    let slot = embed.adopt_slot("panel", pane).unwrap();
    let root = embed.mount(MountSpec::new(entry, "List", slot, "panel").props(Props::new().with("names", names(&["a", "b", "c"])))).unwrap();
    embed.logic();
    let mut got = log();
    got.sort();
    assert_eq!(got, ["define a", "define b", "define c", "define effect 0"]);
    // items that leave release theirs; the ones that stay don't
    embed.set_root_props(root, Props::new().with("names", names(&["a"]))).unwrap();
    embed.logic();
    let mut got = log();
    got.sort();
    assert_eq!(got, ["release b", "release c"]);
    // the `$effect` re-runs: the call it made before is released, the new one stays
    embed.set_root_props(root, Props::new().with("round", 1)).unwrap();
    embed.logic();
    assert_eq!(log(), ["release effect 0", "define effect 1"]);
    // the root unmounts: everything left goes
    embed.unmount(root);
    embed.logic();
    let mut got = log();
    got.sort();
    assert_eq!(got, ["release a", "release effect 1"]);
    let errors: Vec<_> = embed.take_events().into_iter().filter(|e| matches!(e, svelters_skelter::embed::EmbedEvent::Error { .. } | svelters_skelter::embed::EmbedEvent::Faulted { .. })).collect();
    assert!(errors.is_empty(), "{errors:?}");
}
