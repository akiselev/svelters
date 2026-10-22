//! Running a bundle against the log backend, and the fixture harness P5 turns on.
//!
//! A fixture directory holds `dist/app.js`, `script.json` and `expected.oplog`
//! (see `fixtures/README.md`). [`run_fixture`] skips a directory that lacks any of them.
// ZU3: test support: reads fixtures from disk for tests, not run in an app.
#![allow(clippy::disallowed_methods)]

use crate::{ConsoleLine, Host, HostConfig, JsError};
use std::path::{Path, PathBuf};
use std::time::Duration;
use svelters_backend_log::{LogBackend, LogOptions, Schema, Step, parse_schema, parse_script};

/// Everything observable after a bundle ran to completion on the log backend.
pub struct RunReport {
    pub oplog: String,
    pub errors: Vec<JsError>,
    pub console: Vec<ConsoleLine>,
    pub wake_count: u64,
    /// `schedule_wake` deadlines that expired and ran a checkpoint.
    pub timer_fires: u64,
    pub checkpoint_count: u64,
}

/// Loads `bundle`, runs the log backend's loop over `script`, then drains remaining jobs.
pub fn run_bundle(
    bundle: &Path,
    script: Vec<Step>,
    cfg: HostConfig,
    opts: LogOptions,
) -> Result<RunReport, String> {
    run_on(LogBackend::with_options(script, opts), bundle, cfg)
}

fn run_on(log: LogBackend, bundle: &Path, cfg: HostConfig) -> Result<RunReport, String> {
    let vt = log.vtable();
    let host = Host::new(vt.backend_ref(), cfg).map_err(|e| e.to_string())?;
    // App-export calls (E0) join the backend's op log in order.
    let sink = log.clone();
    host.set_op_log(move |line| sink.append_line(line));
    host.load_bundle(bundle).map_err(|e| e.to_string())?;
    host.run().map_err(|e| e.to_string())?;
    host.drain_jobs();
    let report = RunReport {
        oplog: log.log(),
        errors: host.take_errors(),
        console: host.take_console(),
        wake_count: log.wake_count(),
        timer_fires: log.timer_fire_count(),
        checkpoint_count: log.checkpoint_count(),
    };
    drop(host);
    Ok(report)
}

/// The configuration fixtures run under: scripted stepping (virtual clock, see
/// [`HostConfig::scripted`]), quiet output, and a loop that returns as soon as the script ends.
pub fn fixture_config() -> (HostConfig, LogOptions) {
    // The descriptors the log backend stands in for; the Node TestHost loads the same file.
    if let Ok(schema) = parse_schema(include_str!("../../../packages/testhost/src/schema.json")) {
        svelters_backend_log::set_default_schema(schema);
    }
    let cfg = HostConfig {
        scripted: true,
        echo_console: false,
        echo_errors: false,
        ..HostConfig::default()
    };
    (
        cfg,
        LogOptions {
            settle: Duration::ZERO,
        },
    )
}

/// The toolkit a fixture is written for: `"svelters": {"toolkit": ...}` in its package.json
/// (`qt` for the `-qt` twins), else `gtk`.
pub fn fixture_toolkit(dir: &Path) -> String {
    std::fs::read_to_string(dir.join("package.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| v["svelters"]["toolkit"].as_str().map(str::to_string))
        .unwrap_or_else(|| "gtk".to_string())
}

/// A log backend standing in for `toolkit`: the Qt twins get the Qt prop schema (the one the Node
/// TestHost uses for them) and QSS style answers; GTK keeps the default schema and CSS.
fn backend_for(toolkit: &str, script: Vec<Step>, opts: LogOptions) -> Result<LogBackend, String> {
    static QT: std::sync::OnceLock<Result<std::sync::Arc<Schema>, String>> = std::sync::OnceLock::new();
    let log = LogBackend::with_options(script, opts);
    match toolkit {
        "gtk" => Ok(log),
        "qt" => {
            let schema = QT
                .get_or_init(|| parse_schema(include_str!("../../../packages/testhost/src/schema.qt.json")).map(std::sync::Arc::new))
                .clone()?;
            Ok(log.with_schema(schema).with_style_target(svelters_abi::SVL_STYLE_TARGET_QSS))
        }
        other => Err(format!("unknown fixture toolkit {other}")),
    }
}

pub enum FixtureOutcome {
    /// A required file is missing (bundle not built, or no expected log yet).
    Skipped(String),
    Pass,
    Fail(String),
}

/// Runs one fixture directory and compares the op log byte for byte with `expected.oplog`.
pub fn run_fixture(dir: &Path) -> FixtureOutcome {
    let tk = fixture_toolkit(dir);
    if tk != "gtk" && tk != "qt" {
        return FixtureOutcome::Skipped(format!(
            "a {tk} fixture (package.json svelters.toolkit): the log backend models GTK and Qt only"
        ));
    }
    let bundle = dir.join("dist/app.js");
    let script = dir.join("script.json");
    let expected = dir.join("expected.oplog");
    for f in [&bundle, &script, &expected] {
        if !f.is_file() {
            return FixtureOutcome::Skipped(format!("{} not found", f.display()));
        }
    }
    let steps = match std::fs::read_to_string(&script)
        .map_err(|e| e.to_string())
        .and_then(|t| parse_script(&t))
    {
        Ok(s) => s,
        Err(e) => return FixtureOutcome::Fail(format!("{}: {e}", script.display())),
    };
    let expected = match std::fs::read_to_string(&expected) {
        Ok(e) => e,
        Err(e) => return FixtureOutcome::Fail(e.to_string()),
    };
    let (cfg, opts) = fixture_config();
    let report = match backend_for(&tk, steps, opts).and_then(|log| run_on(log, &bundle, cfg)) {
        Ok(r) => r,
        Err(e) => return FixtureOutcome::Fail(e),
    };
    let mut problems = Vec::new();
    if !report.errors.is_empty() {
        problems.push(format!(
            "host reported errors:\n{}",
            report
                .errors
                .iter()
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
                .join("\n")
        ));
    }
    if report.oplog != expected {
        problems.push(format!(
            "op log differs from expected.oplog:\n{}",
            diff(&expected, &report.oplog)
        ));
    }
    if problems.is_empty() {
        FixtureOutcome::Pass
    } else {
        FixtureOutcome::Fail(problems.join("\n"))
    }
}

/// Fixture directories under `root` (each child directory), sorted by name.
pub fn find_fixtures(root: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(root)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .collect()
        })
        .unwrap_or_default();
    dirs.sort();
    dirs
}

// ---- SCHED1: a host driven by a deterministic scheduler ----

/// The simulated frame interval of [`SchedHost`]'s `requestAnimationFrame`.
pub const SCHED_FRAME: Duration = Duration::from_millis(16);

/// SCHED1: a host on the log backend (attached through `start`, no script) driven by a
/// [`svelters_sched::TestScheduler`]: its offloaded jobs are the scheduler's tasks, its timers and
/// `performance.now()` run on the scheduler's clock, and every host wake gives its foreground a turn.
/// A turn is what a toolkit loop does on a wake: the host's `checkpoint` callback (a pending reload
/// first, then the checkpoint), and `frame` when a requested animation frame is due (frames come
/// every [`SCHED_FRAME`] of simulated time).
///
/// ```ignore
/// svelters_sched::run_seeds("my_test", 12, |s| {
///     let h = SchedHost::new(s, quiet_config()).unwrap();
///     h.host.load_bundle("main.js").unwrap();
///     s.run_until_idle();
///     assert!(h.console().contains(&"done".to_string()));
/// });
/// ```
pub struct SchedHost {
    pub host: std::rc::Rc<Host<'static>>,
    pub log: LogBackend,
    pub fg: svelters_sched::Foreground,
}

impl SchedHost {
    /// `cfg` with `scripted: false` and every [`crate::HostSched`] hook from `s`.
    pub fn new(s: &svelters_sched::TestScheduler, cfg: HostConfig) -> Result<SchedHost, JsError> {
        use std::cell::Cell;
        use std::rc::Rc;
        let fg = s.foreground("host");
        let cfg = HostConfig { scripted: false, sched: crate::HostSched::test(s, fg), ..cfg };
        let log = LogBackend::new(Vec::new());
        let host = Rc::new(Host::new_owned(log.clone(), cfg)?);
        host.start().map_err(|e| JsError::new(crate::ErrorKind::Host, e.to_string()))?;
        let weak = Rc::downgrade(&host);
        let (clock, waker, key) = (s.handle(), s.waker(fg), svelters_sched::wake_key());
        let frame_at: Cell<Option<Duration>> = Cell::new(None);
        s.set_runner(fg, move || {
            use svelters_sched::Clock;
            let Some(host) = weak.upgrade() else { return };
            // SAFETY: the callbacks live as long as `host`, which this turn holds.
            let cb = unsafe { svelters_abi::HostRef::from_raw(host.host_callbacks()) };
            cb.checkpoint();
            let now = clock.now();
            if frame_at.get().is_some_and(|at| now >= at) {
                frame_at.set(None);
                cb.frame();
            }
            if frame_at.get().is_none() && host.busy().frame {
                let at = now + SCHED_FRAME;
                frame_at.set(Some(at));
                clock.wake_at(key, Some(at), &waker);
            }
        });
        Ok(SchedHost { host, log, fg })
    }

    /// The console lines logged since the last call.
    pub fn console(&self) -> Vec<String> {
        self.host.take_console().into_iter().map(|l| l.text).collect()
    }

    /// The errors reported since the last call, as text.
    pub fn errors(&self) -> Vec<String> {
        self.host.take_errors().into_iter().map(|e| e.to_string()).collect()
    }
}

/// The repository's `fixtures/` directory.
pub fn default_fixtures_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

/// A small line diff: shows the first mismatch with a few lines of context.
pub fn diff(expected: &str, actual: &str) -> String {
    let e: Vec<&str> = expected.split('\n').collect();
    let a: Vec<&str> = actual.split('\n').collect();
    let first = (0..e.len().max(a.len()))
        .find(|&i| e.get(i) != a.get(i))
        .unwrap_or(0);
    let lo = first.saturating_sub(3);
    let mut out = format!("first difference at line {}\n", first + 1);
    for i in lo..(first + 4).min(e.len().max(a.len())) {
        let (le, la) = (e.get(i), a.get(i));
        if le == la {
            out.push_str(&format!("  {}\n", le.unwrap_or(&"")));
        } else {
            if let Some(l) = le {
                out.push_str(&format!("- {l}\n"));
            }
            if let Some(l) = la {
                out.push_str(&format!("+ {l}\n"));
            }
        }
    }
    out
}
