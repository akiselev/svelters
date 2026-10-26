//! DX1: settings, persisted UI state, paths, `logger`/`console` to `tracing`, crash context.
//! Apps load from source like `modules.rs` (needs `pnpm install` for `fixtures/counter`).

use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use svelters_backend_log::{LogBackend, LogOptions};
use svelters_host::dx::{AppId, Dx, FileUiState, MemorySettings, MemoryUiState, SettingsBackend, UiStateBackend};
use svelters_host::modules::SourceProvider;
use svelters_host::{Host, HostConfig};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
}

fn scratch(name: &str, app: &str) -> PathBuf {
    let fixture = repo().join("fixtures/counter");
    assert!(fixture.join("node_modules").exists(), "run `pnpm install` first");
    let dir = repo().join("target/dx1").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::copy(fixture.join("package.json"), dir.join("package.json")).unwrap();
    std::os::unix::fs::symlink(fixture.join("node_modules"), dir.join("node_modules")).unwrap();
    std::fs::write(dir.join("src/App.svelte"), app).unwrap();
    dir.canonicalize().unwrap()
}

fn run(dir: &Path, dx: Option<Dx>) -> (Vec<String>, Host<'static>) {
    let log = LogBackend::with_options(Vec::new(), LogOptions { settle: Duration::from_millis(100) });
    let vt = Box::leak(Box::new(log.vtable()));
    let cfg = HostConfig { echo_console: false, echo_errors: false, ..HostConfig::default() };
    let host = Host::new(vt.backend_ref(), cfg).unwrap();
    if let Some(dx) = dx {
        host.set_dx(dx);
    }
    host.load_from(Rc::new(SourceProvider::for_path(dir).unwrap())).unwrap();
    let lines = host.take_console().into_iter().map(|l| l.text).collect();
    (lines, host)
}

fn app_id() -> AppId {
    AppId::new("dev", "akiselev", "dx1test")
}

const APP: &str = r#"<script>
  import { setting, uiState, flushUiState } from 'svelters:settings';
  import { paths, app } from 'svelters:paths';
  import { logger } from 'svelters:log';
  const fov = setting('viewport.fov', 45);
  const locked = setting('viewport.locked', 0);
  console.log('fov=' + fov.value);
  fov.value = 60;
  console.log('fov2=' + fov.value);
  let err = 'none';
  try { locked.value = 1; } catch (e) { err = e.code; }
  console.log('err=' + err);
  console.log('locked=' + locked.locked + ' src=' + JSON.stringify(locked.source));
  const ui = uiState({ split: 0.4, columns: [200, 80] }, 'main');
  console.log('split=' + ui.split + ' col0=' + ui.columns[0]);
  ui.split = 0.7;
  ui.columns[0] = 300;
  flushUiState();
  logger('cad.task').debug('hello', { a: 1 });
  console.warn('careful');
  console.log('app=' + app.name + ' cfg=' + paths.config.endsWith('config'));
</script>
<window><label>x</label></window>
"#;

#[test]
fn settings_ui_state_paths_and_logs() {
    let dir = scratch("all", APP);
    let root = dir.join("store");
    let settings = Arc::new(MemorySettings::new());
    settings.lock("viewport.locked", serde_json::json!(5), "project");
    let ui = Arc::new(MemoryUiState::new());
    let dx = Dx { app: app_id(), paths: svelters_host::dx::Paths::under(&root), settings: settings.clone(), ui_state: ui.clone() };

    let events = Arc::new(Mutex::new(Vec::<(String, String, String)>::new()));
    let sub = Capture(events.clone());
    let (lines, _host) = tracing::subscriber::with_default(sub, || run(&dir, Some(dx)));

    assert!(lines.contains(&"fov=45".to_string()), "{lines:#?}");
    assert!(lines.contains(&"fov2=60".to_string()), "{lines:#?}");
    assert_eq!(settings.get("viewport.fov"), Some(serde_json::json!(60)));
    assert!(lines.contains(&"err=settings/locked".to_string()), "{lines:#?}");
    assert!(lines.iter().any(|l| l.contains("locked=true") && l.contains("\"layer\":\"project\"")), "{lines:#?}");
    assert_eq!(settings.get("viewport.locked"), Some(serde_json::json!(5)));
    assert!(lines.contains(&"split=0.4 col0=200".to_string()), "{lines:#?}");
    assert!(lines.contains(&"app=dx1test cfg=true".to_string()), "{lines:#?}");
    assert_eq!(ui.load("app")["main"], serde_json::json!({ "split": 0.7, "columns": [300, 80] }));

    let ev = events.lock().unwrap();
    assert!(ev.contains(&("js".into(), "cad.task".into(), "hello {\"a\":1}".into())), "{ev:#?}");
    assert!(ev.contains(&("js".into(), "".into(), "careful".into())), "{ev:#?}");
}

#[test]
fn ui_state_survives_a_restart_through_a_file_backend() {
    let dir = scratch("restart", APP);
    let state_dir = dir.join("ui");
    let mk = |st: &Path| Dx {
        app: app_id(),
        paths: svelters_host::dx::Paths::under(dir.join("store")),
        settings: Arc::new(MemorySettings::new()),
        ui_state: Arc::new(FileUiState::new(st)),
    };
    let (first, _h1) = run(&dir, Some(mk(&state_dir)));
    assert!(first.contains(&"split=0.4 col0=200".to_string()), "{first:#?}");
    drop(_h1);
    let (second, _h2) = run(&dir, Some(mk(&state_dir)));
    assert!(second.contains(&"split=0.7 col0=300".to_string()), "{second:#?}");
}

#[test]
fn without_a_dx_the_calls_fail_with_unavailable() {
    let dir = scratch("none", "<script>\n  import { setting } from 'svelters:settings';\n  let c = 'none';\n  try { setting('a', 1).value; } catch (e) { c = e.code; }\n  console.log('code=' + c);\n</script>\n<window><label>x</label></window>\n");
    let (lines, _h) = run(&dir, None);
    assert!(lines.contains(&"code=settings/unavailable".to_string()), "{lines:#?}");
}

#[test]
fn crash_context_lists_ops_and_js_errors() {
    let dir = scratch("crash", "<window><label>x</label></window>\n");
    let (_l, _h) = run(&dir, None);
    let c = svelters_host::crash::context();
    assert!(c.contains("last host ops") && c.contains("op_"), "{c}");
}

/// Collects `(target, category, message)` of every event.
struct Capture(Arc<Mutex<Vec<(String, String, String)>>>);

#[derive(Default)]
struct V {
    category: String,
    message: String,
}

impl tracing::field::Visit for V {
    fn record_str(&mut self, f: &tracing::field::Field, v: &str) {
        if f.name() == "category" {
            self.category = v.to_string();
        }
    }
    fn record_debug(&mut self, f: &tracing::field::Field, v: &dyn std::fmt::Debug) {
        if f.name() == "message" {
            self.message = format!("{v:?}");
        }
    }
}

impl tracing::Subscriber for Capture {
    fn enabled(&self, _: &tracing::Metadata<'_>) -> bool {
        true
    }
    fn new_span(&self, _: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        tracing::span::Id::from_u64(1)
    }
    fn record(&self, _: &tracing::span::Id, _: &tracing::span::Record<'_>) {}
    fn record_follows_from(&self, _: &tracing::span::Id, _: &tracing::span::Id) {}
    fn event(&self, e: &tracing::Event<'_>) {
        let mut v = V::default();
        e.record(&mut v);
        self.0.lock().unwrap().push((e.metadata().target().to_string(), v.category, v.message));
    }
    fn enter(&self, _: &tracing::span::Id) {}
    fn exit(&self, _: &tracing::span::Id) {}
}

const LIVE: &str = r#"<script>
  import { setting } from 'svelters:settings';
  const fov = setting('viewport.fov', 45);
  const seen = setting('test.seen', 0);
  // no console here: tracing's callsite cache would hide another test's capture
  $effect(() => { seen.value = fov.value; });
</script>
<window><label>x</label></window>
"#;

/// SV63: a change the backend reports (from any thread) re-runs what read the key at the next
/// checkpoint, without `refresh()`.
#[test]
fn a_backend_change_reaches_the_effects_that_read_it() {
    let dir = scratch("live", LIVE);
    let settings = Arc::new(MemorySettings::new());
    let dx = Dx { app: app_id(), paths: svelters_host::dx::Paths::under(dir.join("store")), settings: settings.clone(), ui_state: Arc::new(MemoryUiState::new()) };
    let (_, host) = run(&dir, Some(dx));
    assert_eq!(settings.get("test.seen"), Some(serde_json::json!(45)));
    let s = settings.clone();
    std::thread::spawn(move || s.set("viewport.fov", serde_json::json!(90)).unwrap()).join().unwrap();
    host.checkpoint();
    assert_eq!(settings.get("test.seen"), Some(serde_json::json!(90)));
}
