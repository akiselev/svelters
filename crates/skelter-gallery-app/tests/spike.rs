//! Spike: a Rust-owned document with a slot, a story compiled from source mounted in it, an
//! overlay edit applied in place.
use std::path::{Path, PathBuf};
use std::rc::Rc;
use skelter_core::{Document, KindId};
use svelters_skelter::SkelterBackend;
use svelters_skelter::embed::{Embed, EmbedConfig, MountSpec};

fn scratch(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/embed-egui").canonicalize().unwrap();
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/gallery-app-tests").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("package.json"), r#"{ "name": "g", "private": true, "type": "module", "svelters": { "toolkit": "skelter" } }"#).unwrap();
    for (f, src) in files {
        let p = dir.join(f);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, src).unwrap();
    }
    std::os::unix::fs::symlink(fixture.join("node_modules"), dir.join("node_modules")).unwrap();
    dir.canonicalize().unwrap()
}

#[test]
fn spike() {
    let dir = scratch("spike", &[("stories/b/primary.svelte", "<script>let { t = 'one' } = $props();</script>\n<button anchor=\"subject\" text={t} />\n")]);
    let file = dir.join("stories/b/primary.svelte");
    let reg = svelters_skelter::builtin_kinds().registry().clone();
    let mut doc = Document::new(reg, Box::new(svelters_skelter::backend::NoText));
    let pane = doc.create(KindId::of("box"));
    let doc = Rc::new(std::cell::RefCell::new(doc));
    let backend = SkelterBackend::shared(svelters_skelter::builtin_kinds(), doc.clone()).unwrap();
    let host = svelters_host::HostConfig { echo_console: false, echo_errors: false, quit_on_last_window: false, ..Default::default() };
    let embed = Embed::with_backend(backend.clone(), EmbedConfig { host, ..EmbedConfig::default() }).unwrap();
    let mut cfg = svelters_host::modules::SourceConfig::for_path(&file).unwrap();
    cfg.entry = svelters_host::modules::Entry::Embed(vec![("V0".into(), file.clone())]);
    cfg.backend = "skelter".into();
    cfg.dev = true;
    cfg.hmr = true;
    let provider = Rc::new(svelters_host::modules::SourceProvider::new(cfg).unwrap());
    let entry = embed.load_from(provider.clone()).unwrap();
    let slot = embed.adopt_slot("c0", pane).unwrap();
    embed.mount(MountSpec::new(entry, "V0", slot, "c0")).unwrap();
    embed.logic();
    eprintln!("{:?}", embed.take_events());
    let text = || {
        let d = doc.borrow();
        let b = d.children(pane)[0];
        let h = backend.handle(b).unwrap();
        drop(d);
        use svelters_abi::Backend;
        let p = backend.resolve(svelters_abi::NameKind::Prop, backend.handle_info(h).unwrap().kind, "text");
        backend.get_prop(h, p).unwrap()
    };
    eprintln!("before {:?}", text());
    provider.overlay().set(&file, "<script>let { t = 'one' } = $props();</script>\n<button anchor=\"subject\" text={t + '!'} />\n");
    let r = embed.host().apply_changes(vec![file.clone()]);
    eprintln!("report {r:?}");
    embed.logic();
    eprintln!("after {:?}", text());
    provider.overlay().set(&file, "<script>let { t = 'one' } = $props();</script>\n<button text={t + '!'} \n");
    let r = embed.host().apply_changes(vec![file.clone()]);
    eprintln!("report {r:?}");
    embed.logic();
    eprintln!("after err {:?}", text());
    eprintln!("check {:?}", provider.check(&file).diagnostics);
}
