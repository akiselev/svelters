//! H1: component HMR on the Rust module loader, against the log backend: an edit to a child
//! component swaps it in place (the window and the parent's widgets keep their handles), a
//! non-accepting change falls back to a full reload, a broken edit keeps the old version, and the
//! swap budget forces a full reload.
//!
//! Needs `pnpm install` (see tests/modules.rs).

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;
use svelters_abi::Backend;
use svelters_backend_log::{LogBackend, LogOptions};
use svelters_host::modules::{SourceConfig, SourceProvider};
use svelters_host::{Host, HostConfig, ReloadReport};

thread_local! {
    static MAX_SWAPS: std::cell::Cell<u32> = const { std::cell::Cell::new(200) };
}

fn unsafe_swaps() -> u32 {
    MAX_SWAPS.with(|c| c.get())
}

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
}

fn quiet() -> HostConfig {
    HostConfig { hmr_max_swaps: unsafe_swaps(), echo_console: false, echo_errors: false, ..HostConfig::default() }
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
        let mut cfg = SourceConfig::for_path(&self.dir).unwrap();
        cfg.dev = true;
        cfg.hmr = true;
        Rc::new(SourceProvider::new(cfg).unwrap())
    }
}

fn log_host(settle: Duration) -> (LogBackend, Host<'static>) {
    let log = LogBackend::with_options(Vec::new(), LogOptions { settle });
    let vt = Box::leak(Box::new(log.vtable()));
    let host = Host::new(vt.backend_ref(), quiet()).unwrap();
    (log, host)
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


const APP: &str = "<script>\n  import Child from './Child.svelte';\n</script>\n<window title=\"hmr\"><box><label>parent</label><Child /></box></window>\n";

fn app_with_child(name: &str, child: &str) -> App {
    let app = App::new(name, APP);
    app.write("src/Child.svelte", child);
    app
}

fn count(hay: &str, needle: &str) -> usize {
    hay.matches(needle).count()
}

#[test]
fn editing_a_child_swaps_it_in_place() {
    let app = app_with_child("hmr-child", "<label>one</label>\n");
    let child = app.path("src/Child.svelte");
    let (oplog, reports) = run_watching(app.provider(), 1, move || {
        std::fs::write(&child, "<label>two</label>\n").unwrap();
    });
    assert!(reports[0].ok, "{:?}", reports[0].error);
    assert!(reports[0].hot.as_ref().is_some_and(|m| !m.is_empty()), "{:?}", reports[0]);
    assert!(oplog.contains("\"one\"") && oplog.contains("\"two\""), "{oplog}");
    // the window and the parent's label were created once: nothing else was re-created
    assert_eq!(count(&oplog, "\"parent\""), 1, "{oplog}");
    assert_eq!(count(&oplog, "\"hmr\""), 1, "{oplog}");
}

#[test]
fn a_runes_module_edit_re_runs_its_accepting_importer() {
    let app = App::new("hmr-module", "<script>\n  import { label } from './state.svelte.js';\n</script>\n<window><label>{label.text}</label></window>\n");
    app.write("src/state.svelte.js", "export const label = $state({ text: 'a' });\n");
    let state = app.path("src/state.svelte.js");
    let (oplog, reports) = run_watching(app.provider(), 1, move || {
        std::fs::write(&state, "export const label = $state({ text: 'b' });\n").unwrap();
    });
    assert!(reports[0].ok, "{:?}", reports[0].error);
    assert!(reports[0].hot.is_some(), "{:?}", reports[0]);
    assert!(oplog.contains("\"b\""), "{oplog}");
}

#[test]
fn a_change_nothing_accepts_falls_back_to_a_full_reload() {
    let app = App::new("hmr-fallback", "");
    app.write("src/main.js", "import { v } from './v.svelte.js';\nconsole.log('v=' + v);\n");
    app.write("src/v.svelte.js", "export const v = 1;\n");
    let v = app.path("src/v.svelte.js");
    let mut cfg = SourceConfig::for_path(&app.path("src/main.js")).unwrap();
    cfg.dev = true;
    cfg.hmr = true;
    let provider = Rc::new(SourceProvider::new(cfg).unwrap());
    let (_oplog, reports) = run_watching(provider, 1, move || {
        std::fs::write(&v, "export const v = 2;\n").unwrap();
    });
    assert!(reports[0].ok, "{:?}", reports[0].error);
    assert!(reports[0].hot.is_none(), "{:?}", reports[0]);
    assert!(reports[0].fallback.as_deref().is_some_and(|w| w.contains("not accepted")), "{:?}", reports[0]);
}

#[test]
fn a_broken_edit_keeps_the_old_version_and_a_fix_applies() {
    let app = app_with_child("hmr-broken", "<label>one</label>\n");
    let child = app.path("src/Child.svelte");
    let (oplog, reports) = run_watching(app.provider(), 2, move || {
        std::fs::write(&child, "<label>{</label>\n").unwrap();
        std::thread::sleep(Duration::from_millis(800));
        std::fs::write(&child, "<label>three</label>\n").unwrap();
    });
    assert!(!reports[0].ok, "{:?}", reports[0]);
    assert!(reports[1].ok && reports[1].hot.is_some(), "{:?}", reports[1]);
    assert!(oplog.contains("\"three\""), "{oplog}");
    assert_eq!(count(&oplog, "\"parent\""), 1, "{oplog}");
}

#[test]
fn the_swap_budget_forces_a_full_reload() {
    MAX_SWAPS.with(|c| c.set(1));
    let app = app_with_child("hmr-budget", "<label>one</label>\n");
    let child = app.path("src/Child.svelte");
    let (_oplog, reports) = run_watching(app.provider(), 2, move || {
        std::fs::write(&child, "<label>two</label>\n").unwrap();
        std::thread::sleep(Duration::from_millis(800));
        std::fs::write(&child, "<label>three</label>\n").unwrap();
    });
    MAX_SWAPS.with(|c| c.set(200));
    assert!(reports[0].hot.is_some(), "{:?}", reports[0]);
    assert!(reports[1].ok && reports[1].hot.is_none(), "{:?}", reports[1]);
    assert!(reports[1].fallback.as_deref().is_some_and(|w| w.contains("swap budget")), "{:?}", reports[1]);
}

#[test]
fn accept_exports_covers_default_only_and_a_changed_other_export_propagates_to_the_importer() {
    let app = app_with_child("hmr-exports", "<script module>\n  export const k = 1;\n</script>\n<label>one</label>\n");
    let child = app.path("src/Child.svelte");
    let (_oplog, reports) = run_watching(app.provider(), 2, move || {
        std::fs::write(&child, "<script module>\n  export const k = 1;\n</script>\n<label>two</label>\n").unwrap();
        std::thread::sleep(Duration::from_millis(800));
        std::fs::write(&child, "<script module>\n  export const k = 2;\n</script>\n<label>three</label>\n").unwrap();
    });
    assert!(reports[0].ok && reports[0].hot.is_some(), "{:?}", reports[0]);
    // H1b: `k` is not accepted by Child, so the change moves up to App (which accepts its own default)
    assert!(reports[1].ok && reports[1].fallback.is_none(), "{:?}", reports[1]);
    let hot = reports[1].hot.clone().unwrap_or_default();
    assert!(hot.iter().any(|m| m.ends_with("Child.svelte")) && hot.iter().any(|m| m.ends_with("App.svelte")), "{hot:?}");
}

