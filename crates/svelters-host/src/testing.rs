//! Running a bundle against the log backend, and the fixture harness P5 turns on.
//!
//! A fixture directory holds `dist/app.js`, `script.json` and `expected.oplog`
//! (see `fixtures/README.md`). [`run_fixture`] skips a directory that lacks any of them.

use crate::{ConsoleLine, Host, HostConfig, JsError};
use std::path::{Path, PathBuf};
use std::time::Duration;
use svelters_backend_log::{LogBackend, LogOptions, Step, parse_script};

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
    let log = LogBackend::with_options(script, opts);
    let vt = log.vtable();
    let host = Host::new(vt.backend_ref(), cfg).map_err(|e| e.to_string())?;
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

pub enum FixtureOutcome {
    /// A required file is missing (bundle not built, or no expected log yet).
    Skipped(String),
    Pass,
    Fail(String),
}

/// Runs one fixture directory and compares the op log byte for byte with `expected.oplog`.
pub fn run_fixture(dir: &Path) -> FixtureOutcome {
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
    let report = match run_bundle(&bundle, steps, cfg, opts) {
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
