//! Dev loop: source-mapped errors and in-process hot reload, against the log backend.
//!
//! Every test copies the `counter` fixture into `target/m2b/<test>/`, edits the copy and builds it
//! with the real `svelters-build` CLI (one-shot or `--watch`). That needs `pnpm install` to have
//! run; the copy borrows the fixture's `node_modules` through a symlink.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::rc::Rc;
use std::sync::mpsc;
use std::time::{Duration, Instant};
use svelters_abi::Backend;
use svelters_backend_log::{LogBackend, LogOptions};
use svelters_host::{ErrorKind, Host, HostConfig, ReloadReport};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
}

/// A scratch app: the counter fixture's package files, with `App.svelte` replaced by `app`.
struct App {
    dir: PathBuf,
}

impl App {
    fn new(name: &str, app: &str) -> App {
        let repo = repo();
        let fixture = repo.join("fixtures/counter");
        assert!(
            fixture.join("node_modules").exists(),
            "run `pnpm install` first ({} has no node_modules)",
            fixture.display()
        );
        let dir = repo.join("target/m2b").join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        for f in ["package.json", "vite.config.js"] {
            std::fs::copy(fixture.join(f), dir.join(f)).unwrap();
        }
        std::fs::copy(fixture.join("src/main.js"), dir.join("src/main.js")).ok();
        std::os::unix::fs::symlink(fixture.join("node_modules"), dir.join("node_modules")).unwrap();
        let app_ = App { dir };
        app_.write(app);
        app_
    }

    fn svelte(&self) -> PathBuf {
        self.dir.join("src/App.svelte")
    }

    fn bundle(&self) -> PathBuf {
        self.dir.join("dist/app.js")
    }

    fn write(&self, text: &str) {
        std::fs::write(self.svelte(), text).unwrap();
    }

    fn cli(&self) -> Command {
        let mut c = Command::new("node");
        c.arg(repo().join("packages/build/bin/svelters-build.mjs"));
        c
    }

    /// One-shot build; panics with the build output on failure.
    fn build(&self) {
        let out = self.cli().arg(&self.dir).output().expect("run node");
        assert!(out.status.success(), "build failed:\n{}", String::from_utf8_lossy(&out.stderr));
    }

    /// `svelters-build --watch`; returns the child and a channel that receives each build-finished line.
    fn watch(&self) -> (Child, mpsc::Receiver<String>) {
        let mut child = self
            .cli()
            .arg("--watch")
            .arg(&self.dir)
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn watch build");
        let stderr = child.stderr.take().unwrap();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            use std::io::{BufRead, BufReader};
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                if line.contains("rebuilt in") || line.contains("build failed") {
                    let _ = tx.send(line);
                }
            }
        });
        (child, rx)
    }
}

const APP_TEMPLATE: &str = r#"<script>
  let count = $state(0);
</script>

<window title="Reload test">
  <box orientation="vertical" spacing="8">
    <label>LABEL: {count}</label>
    <button onclicked={() => count++}>Increment</button>
  </box>
</window>
"#;

fn app_with(label: &str) -> String {
    APP_TEMPLATE.replace("LABEL", label)
}

fn quiet() -> HostConfig {
    HostConfig { echo_console: false, echo_errors: false, ..HostConfig::default() }
}

// ---- source maps ----

const THROWING: &str = r#"<script>
  console.error(new Error('logged'));
  Promise.reject(new Error('rejected'));
  setTimeout(() => {
    throw new Error('late');
  }, 0);
</script>

<window title="Errors"><label>x</label></window>
"#;

#[test]
fn errors_map_to_svelte_sources() {
    let app = App::new("errors", THROWING);
    app.build();

    let log = LogBackend::with_options(Vec::new(), LogOptions { settle: Duration::from_millis(500) });
    let vt = log.vtable();
    let host = Host::new(vt.backend_ref(), quiet()).unwrap();
    host.load_bundle(app.bundle()).unwrap();
    host.run().unwrap();

    let console: Vec<String> = host.take_console().into_iter().filter(|l| l.level == "error").map(|l| l.text).collect();
    assert_eq!(console.len(), 1, "{console:?}");
    assert!(console[0].contains("src/App.svelte:2:"), "console.error stack not mapped:\n{}", console[0]);
    assert!(!console[0].contains("app.js:"), "bundle position left in:\n{}", console[0]);

    let errors = host.take_errors();
    let by_msg = |m: &str| {
        errors.iter().find(|e| e.message.contains(m)).unwrap_or_else(|| panic!("no '{m}' in {errors:?}"))
    };
    let rejected = by_msg("rejected");
    assert_eq!(rejected.kind, ErrorKind::UnhandledRejection);
    assert!(rejected.stack.as_deref().unwrap_or("").contains("src/App.svelte:3:"), "{rejected}");
    let late = by_msg("late");
    assert_eq!(late.kind, ErrorKind::Exception);
    assert!(late.stack.as_deref().unwrap_or("").contains("src/App.svelte:5:"), "{late}");
}

#[test]
fn load_error_maps_to_svelte_source() {
    let app = App::new("load-error", "<script>\n  const a = 1;\n  throw new Error('boom at mount');\n</script>\n<window><label>x</label></window>\n");
    app.build();
    let log = LogBackend::with_options(Vec::new(), LogOptions::default());
    let vt = log.vtable();
    let host = Host::new(vt.backend_ref(), quiet()).unwrap();
    let err = host.load_bundle(app.bundle()).expect_err("the bundle throws while mounting");
    assert_eq!(err.kind, ErrorKind::Load);
    let text = err.to_string();
    assert!(text.contains("boom at mount"), "{text}");
    assert!(text.contains("src/App.svelte:3:"), "load error not mapped:\n{text}");
}

// ---- hot reload ----

/// Runs `host.run()` on the log backend with a reload hook that records reports and quits after
/// `stop_after` of them. `edit` runs on a helper thread once the loop is up.
fn run_reloading(
    app: &App,
    stop_after: usize,
    edit: impl FnOnce() + Send + 'static,
) -> (String, Vec<ReloadReport>, Instant) {
    let log = LogBackend::with_options(Vec::new(), LogOptions { settle: Duration::from_secs(30) });
    let vt = log.vtable();
    let host = Host::new(vt.backend_ref(), quiet()).unwrap();
    host.load_bundle(app.bundle()).unwrap();
    host.watch().unwrap();

    let reports: Rc<RefCell<Vec<ReloadReport>>> = Rc::default();
    let done_at: Rc<RefCell<Option<Instant>>> = Rc::default();
    {
        let (reports, done_at, log) = (reports.clone(), done_at.clone(), log.clone());
        host.on_reload(move |r| {
            reports.borrow_mut().push(r.clone());
            if reports.borrow().len() >= stop_after {
                *done_at.borrow_mut() = Some(Instant::now());
                Backend::quit(&log);
            }
        });
    }
    let started = Instant::now();
    let helper = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(300));
        edit();
    });
    host.run().unwrap();
    helper.join().unwrap();
    let reports = reports.borrow().clone();
    assert!(reports.len() >= stop_after, "loop ended after {:?} without {stop_after} reload(s): {reports:?}", started.elapsed());
    let oplog = log.log();
    let done = done_at.borrow().unwrap();
    (oplog, reports, done)
}

#[test]
fn watch_build_edit_reloads_in_same_process() {
    let app = App::new("reload-watch", &app_with("Count"));
    app.build();
    let (mut child, built) = app.watch();
    built.recv_timeout(Duration::from_secs(60)).expect("initial watch build");

    let edit_at = Rc::new(RefCell::new(None));
    let svelte = app.svelte();
    let (tx, rx) = mpsc::channel();
    let (oplog, reports, done) = run_reloading(&app, 1, move || {
        tx.send(Instant::now()).unwrap();
        std::fs::write(svelte, app_with("Total")).unwrap();
    });
    *edit_at.borrow_mut() = rx.try_recv().ok();
    let _ = child.kill();
    let _ = child.wait();

    assert!(reports[0].ok, "{:?}", reports[0].error);
    let edit_to_reload = done.duration_since(edit_at.borrow().unwrap());
    eprintln!("edit -> reloaded: {edit_to_reload:?} (host load+teardown {:?})", reports[0].elapsed);

    // The first app is created, the second app is created while the first window is still up,
    // then the first toplevel is destroyed (descendants first).
    let lines: Vec<&str> = oplog.lines().collect();
    let pos = |needle: &str| lines.iter().position(|l| l.contains(needle)).unwrap_or_else(|| panic!("no '{needle}' in\n{oplog}"));
    let first_label = pos("\"Count: 0\"");
    let second_label = pos("\"Total: 0\"");
    let first_destroy = pos("destroy #");
    assert!(first_label < second_label, "{oplog}");
    assert!(second_label < first_destroy, "old window destroyed before the new one was built:\n{oplog}");
    assert_eq!(lines.iter().filter(|l| l.starts_with("create window")).count(), 2, "{oplog}");
    // Everything of the first app is gone: 4 widgets destroyed, the 4 of the second remain.
    assert_eq!(lines.iter().filter(|l| l.starts_with("destroy #")).count(), 4, "{oplog}");
    assert!(lines.iter().any(|l| l.starts_with("call #5 present") || l.contains("present")), "{oplog}");
}

#[test]
fn failed_reload_keeps_the_process_and_recovers() {
    let app = App::new("reload-errors", &app_with("Count"));
    app.build();

    let svelte = app.svelte();
    let bundle = app.bundle();
    let build_dir = app.dir.clone();
    let rebuild = move || {
        let out = Command::new("node")
            .arg(repo().join("packages/build/bin/svelters-build.mjs"))
            .arg(&build_dir)
            .output()
            .unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    };
    let (oplog, reports, _) = run_reloading(&app, 3, move || {
        // 1. Throws while mounting: the build succeeds, the load fails.
        std::fs::write(&svelte, "<script>\n  const a = 1;\n  throw new Error('mount failed');\n</script>\n<window><label>x</label></window>\n").unwrap();
        rebuild();
        std::thread::sleep(Duration::from_millis(600));
        // 2. A bundle that does not parse (as a half-written file would).
        std::fs::write(&bundle, "export default (;\n").unwrap();
        std::thread::sleep(Duration::from_millis(600));
        // 3. Recovery.
        std::fs::write(&svelte, app_with("Fixed")).unwrap();
        rebuild();
    });

    assert_eq!(reports.len(), 3, "{reports:?}");
    let e0 = reports[0].error.as_ref().expect("mount failure is reported");
    assert!(e0.to_string().contains("mount failed"), "{e0}");
    assert!(e0.to_string().contains("src/App.svelte:3:"), "not mapped:\n{e0}");
    let e1 = reports[1].error.as_ref().expect("syntax error is reported");
    assert_eq!(e1.kind, ErrorKind::Load);
    assert!(reports[2].ok, "{:?}", reports[2].error);

    let lines: Vec<&str> = oplog.lines().collect();
    // The original app is untouched until the recovery loads: its window is the only one
    // destroyed, and only after "Fixed" was built.
    let fixed = lines.iter().position(|l| l.contains("\"Fixed: 0\"")).expect("recovered UI");
    let first_destroy = lines.iter().position(|l| l.starts_with("destroy #")).expect("old app destroyed");
    assert!(fixed < first_destroy, "{oplog}");
}
