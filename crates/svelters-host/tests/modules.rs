//! Q1: apps loaded from source through `svelters_modules::SourceProvider` (no Vite), against the
//! log backend: live compile, resolution through node_modules and the runtime's alias table,
//! in-memory source maps, JSON and asset modules, the overlay, and the graph-driven reload.
//!
//! Every test writes a scratch app under `target/q1/<test>/` with the counter fixture's
//! package.json and borrows its `node_modules` through a symlink, so `pnpm install` must have run.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;
use svelters_abi::Backend;
use svelters_backend_log::{LogBackend, LogOptions};
use svelters_host::modules::{SourceConfig, SourceProvider};
use svelters_host::{ErrorKind, Host, HostConfig, ModuleProvider, ReloadReport};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
}

fn quiet() -> HostConfig {
    HostConfig { echo_console: false, echo_errors: false, ..HostConfig::default() }
}

struct App {
    dir: PathBuf,
}

impl App {
    /// A scratch app with `src/App.svelte` = `app`.
    fn new(name: &str, app: &str) -> App {
        let fixture = repo().join("fixtures/counter");
        assert!(fixture.join("node_modules").exists(), "run `pnpm install` first ({} has no node_modules)", fixture.display());
        let dir = repo().join("target/q1").join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::copy(fixture.join("package.json"), dir.join("package.json")).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(fixture.join("node_modules"), dir.join("node_modules")).unwrap();
        #[cfg(windows)]
        std::os::windows::fs::symlink_dir(fixture.join("node_modules"), dir.join("node_modules")).unwrap();
        let a = App { dir: dir.canonicalize().unwrap() };
        a.write("src/App.svelte", app);
        a
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.dir.join(rel)
    }

    fn write(&self, rel: &str, text: &str) {
        std::fs::write(self.path(rel), text).unwrap();
    }

    fn provider(&self) -> Rc<SourceProvider> {
        Rc::new(SourceProvider::for_path(&self.dir).unwrap())
    }
}

fn log_host(settle: Duration) -> (LogBackend, Host<'static>) {
    let log = LogBackend::with_options(Vec::new(), LogOptions { settle });
    let vt = Box::leak(Box::new(log.vtable()));
    let host = Host::new(vt.backend_ref(), quiet()).unwrap();
    (log, host)
}

const COUNTER: &str = r#"<script>
  import data from './data.json';
  import icon from './icon.png';
  import { label } from './state.svelte.js';
  import { tag } from 'svelters:behaviors/demo';
  let count = $state(data.start + tag);
</script>

<window title={icon.endsWith('/src/icon.png') ? 'asset' : 'no asset'}>
  <box orientation="vertical" spacing="8">
    <label>{label.text}: {count}</label>
    <button onclicked={() => count++}>Increment</button>
  </box>
</window>
"#;

fn counter_app(name: &str) -> App {
    let app = App::new(name, COUNTER);
    app.write("src/data.json", "{ \"start\": 41 }\n");
    app.write("src/icon.png", "not really a png");
    app.write("src/state.svelte.js", "export const label = $state({ text: 'Count' });\n");
    // EV2b: a dumped behaviours module (svelters_egui::dump), resolved from the app root.
    std::fs::create_dir_all(app.path(".svelters/app/behaviors")).unwrap();
    app.write(".svelters/app/behaviors/demo.js", "export const tag = 0;\n");
    app
}

#[test]
fn an_app_loads_from_source_with_json_assets_and_runes_modules() {
    let app = counter_app("from-source");
    let provider = app.provider();
    let (log, host) = log_host(Duration::from_millis(200));
    host.load_from(provider.clone()).unwrap();
    host.run().unwrap();
    let oplog = log.log();
    assert!(oplog.contains("\"Count: 41\""), "{oplog}");
    assert!(oplog.contains("\"asset\""), "{oplog}");
    assert_eq!(host.toplevels().len(), 1, "{oplog}");

    // The graph holds the app's files, the runtime's sources and Svelte's (through the alias table).
    let files = provider.graph_files();
    let has = |suffix: &str| files.iter().any(|f| f.to_string_lossy().ends_with(suffix));
    for f in ["src/App.svelte", "src/data.json", "src/state.svelte.js", "native-runtime/src/index.js", "svelte/src/internal/client/reactivity/effects.js"] {
        assert!(has(f), "{f} not in the graph: {files:#?}");
    }
    // Installed packages are not watched; the app and the workspace's runtime package are.
    let watched = provider.watch_files();
    assert!(watched.iter().any(|f| f.ends_with("src/App.svelte")), "{watched:#?}");
    assert!(!watched.iter().any(|f| f.components().any(|c| c.as_os_str() == "node_modules")), "{watched:#?}");
    let app_id = app.path("src/App.svelte").to_string_lossy().into_owned();
    assert!(provider.importers(&app_id).contains(&"svelters:entry".to_string()));
}

#[test]
fn runtime_errors_map_to_the_svelte_source() {
    let app = App::new("source-map", "<script>\n  const a = 1;\n  throw new Error('boom at mount');\n</script>\n<window><label>x</label></window>\n");
    let (_log, host) = log_host(Duration::from_millis(100));
    let err = host.load_from(app.provider()).expect_err("the component throws while mounting");
    assert_eq!(err.kind, ErrorKind::Load);
    let text = err.to_string();
    assert!(text.contains("boom at mount"), "{text}");
    assert!(text.contains("src/App.svelte:3:"), "load error not mapped to the component:\n{text}");
}

#[test]
fn compile_errors_are_load_errors_with_diagnostics() {
    let app = App::new("compile-error", "<window><label>{</label></window>\n");
    let (_log, host) = log_host(Duration::from_millis(100));
    let err = host.load_from(app.provider()).expect_err("the component does not compile");
    assert_eq!(err.kind, ErrorKind::Load);
    assert!(err.message.contains("App.svelte"), "{err}");
}

#[test]
fn the_overlay_is_read_before_the_disk() {
    let app = App::new("overlay", "<window><label>disk</label></window>\n");
    let provider = app.provider();
    let svelte = app.path("src/App.svelte");

    // Diagnostics of an unsaved buffer, without touching the file or the app.
    provider.overlay().set(&svelte, "<window><label>{</label></window>\n");
    let check = provider.check(&svelte);
    assert!(!check.ok(), "{check:?}");
    assert!(!check.diagnostics.is_empty());
    provider.overlay().set(&svelte, "<window><label>buffer</label></window>\n");
    assert!(provider.check(&svelte).ok());

    let (log, host) = log_host(Duration::from_millis(100));
    host.load_from(provider.clone()).unwrap();
    host.run().unwrap();
    assert!(log.log().contains("\"buffer\""), "{}", log.log());
    assert_eq!(std::fs::read_to_string(&svelte).unwrap(), "<window><label>disk</label></window>\n");
}

#[test]
fn styles_and_typescript_are_reported_not_silently_dropped() {
    let app = App::new("unsupported", "<window><label>x</label></window>\n<style>\n  label { color: red; }\n</style>\n");
    let provider = app.provider();
    let (_log, host) = log_host(Duration::from_millis(100));
    host.load_from(provider.clone()).unwrap();
    let warnings: Vec<String> = host.take_console().into_iter().filter(|l| l.level == "warn").map(|l| l.text).collect();
    assert!(warnings.iter().any(|w| w.contains("<style> is not applied")), "{warnings:?}");

    let ts = App::new("typescript", "<script>\n  import { n } from './n.ts';\n</script>\n<window><label>{n}</label></window>\n");
    ts.write("src/n.ts", "export const n: number = 1;\n");
    let (_log, host) = log_host(Duration::from_millis(100));
    let err = host.load_from(ts.provider()).expect_err("TypeScript is Q2");
    assert!(err.message.contains("TypeScript"), "{err}");
}

/// Runs the loop with `watch()` on; `edit` runs on a helper thread once the loop is up; quits after
/// `stop_after` reload reports.
fn run_watching(provider: Rc<SourceProvider>, stop_after: usize, edit: impl FnOnce() + Send + 'static) -> (String, Vec<ReloadReport>) {
    let (log, host) = log_host(Duration::from_secs(30));
    host.load_from(provider).unwrap();
    host.watch().unwrap();
    let reports: Rc<RefCell<Vec<ReloadReport>>> = Rc::default();
    {
        let (reports, log) = (reports.clone(), log.clone());
        host.on_reload(move |r| {
            reports.borrow_mut().push(r.clone());
            if reports.borrow().len() >= stop_after {
                Backend::quit(&log);
            }
        });
    }
    let helper = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(300));
        edit();
    });
    host.run().unwrap();
    helper.join().unwrap();
    let r = reports.borrow().clone();
    (log.log(), r)
}

#[test]
fn editing_a_graph_file_reloads_and_other_files_do_not() {
    let app = counter_app("watch");
    let (state, unrelated) = (app.path("src/state.svelte.js"), app.path("src/notes.txt"));
    let (oplog, reports) = run_watching(app.provider(), 1, move || {
        // Not in the graph: no reload.
        std::fs::write(&unrelated, "x").unwrap();
        std::thread::sleep(Duration::from_millis(500));
        std::fs::write(&state, "export const label = $state({ text: 'Total' });\n").unwrap();
    });
    assert_eq!(reports.len(), 1);
    assert!(reports[0].ok, "{:?}", reports[0].error);
    let first = oplog.find("\"Count: 41\"").unwrap_or_else(|| panic!("{oplog}"));
    let second = oplog.find("\"Total: 41\"").unwrap_or_else(|| panic!("{oplog}"));
    assert!(first < second, "{oplog}");
}

#[test]
fn a_failed_reload_keeps_the_app_and_a_fix_recovers() {
    let app = App::new("watch-errors", "<window><label>one</label></window>\n");
    let svelte = app.path("src/App.svelte");
    let (oplog, reports) = run_watching(app.provider(), 2, move || {
        std::fs::write(&svelte, "<window><label>{</label></window>\n").unwrap();
        std::thread::sleep(Duration::from_millis(800));
        std::fs::write(&svelte, "<window><label>two</label></window>\n").unwrap();
    });
    assert!(!reports[0].ok, "{reports:?}");
    assert_eq!(reports[0].error.as_ref().unwrap().kind, ErrorKind::Load);
    assert!(reports[1].ok, "{:?}", reports[1].error);
    assert!(oplog.contains("\"two\""), "{oplog}");
}

#[test]
fn an_embed_entry_from_source_keeps_its_namespace() {
    let app = App::new("embed", "<window><label>unused</label></window>\n");
    app.write("src/Panel.svelte", "<script>\n  let { name } = $props();\n</script>\n<label>{name}</label>\n");
    let mut cfg = SourceConfig::for_path(&app.dir).unwrap();
    cfg.entry = svelters_host::modules::Entry::Embed(vec![("Panel".into(), app.path("src/Panel.svelte"))]);
    let provider: Rc<dyn ModuleProvider> = Rc::new(SourceProvider::new(cfg).unwrap());
    let (_log, host) = log_host(Duration::from_millis(100));
    assert_eq!(host.load_entry_from(provider).unwrap(), 0);
}

#[test]
fn an_extension_cannot_import_files_outside_its_package() {
    let app = App::new("confined", "<script>\n  import secret from 'SPEC';\n</script>\n<window><label>{secret.k}</label></window>\n");
    let outside = app.dir.parent().unwrap().join("q1-outside.json");
    std::fs::write(&outside, "{ \"k\": \"leaked\" }").unwrap();
    for spec in ["../../q1-outside.json".to_string(), outside.to_string_lossy().into_owned()] {
        app.write("src/App.svelte", &format!("<script>\n  import secret from '{spec}';\n</script>\n<window><label>{{secret.k}}</label></window>\n"));
        // The primary app host may read it.
        let (log, host) = log_host(Duration::from_millis(100));
        host.load_from(app.provider()).unwrap();
        host.run().unwrap();
        assert!(log.log().contains("\"leaked\""), "{spec}: {}", log.log());
        // An extension may not.
        let ext: Rc<dyn ModuleProvider> = Rc::new(SourceProvider::new(SourceConfig::for_extension(&app.dir).unwrap()).unwrap());
        let (_log, host) = log_host(Duration::from_millis(100));
        let err = host.load_from(ext).expect_err("confined");
        assert!(err.message.contains("cannot find module"), "{spec}: {err}");
    }
    // Inside its own package it loads as usual (the runtime and Svelte resolve as packages).
    app.write("src/App.svelte", "<script>\n  import d from './d.json';\n</script>\n<window><label>{d.k}</label></window>\n");
    app.write("src/d.json", "{ \"k\": \"own\" }");
    let ext: Rc<dyn ModuleProvider> = Rc::new(SourceProvider::new(SourceConfig::for_extension(&app.dir).unwrap()).unwrap());
    let (log, host) = log_host(Duration::from_millis(100));
    host.load_from(ext).unwrap();
    host.run().unwrap();
    assert!(log.log().contains("\"own\""), "{}", log.log());
}
