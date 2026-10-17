//! `counter`: loads a fixture bundle into the host and runs it on a real toolkit backend.
//!
//! ```text
//! counter --backend gtk|qt [--fixture <name|dir>] [--script <script.json>] [--screenshot <png>]
//!         [--report <json>] [--end quit|close] [--timing] [--timeout <secs>]
//! ```
//!
//! Without `--script`, `--screenshot` and `--report` it opens the window and runs until the user
//! closes it. With any of them the driver (see `driver.rs`) performs the script through the real
//! toolkit, optionally saves a screenshot, writes a JSON report and ends the run.

mod driver;
#[cfg(feature = "gtk")]
mod gtk_tk;
#[cfg(feature = "qt")]
mod qt_tk;

use driver::{Driver, End, Options, Toolkit};
use serde_json::{Value as Json, json};
use std::cell::Cell;
use std::path::PathBuf;
use std::process::ExitCode;
use std::rc::Rc;
use std::time::{Duration, Instant};
use svelters_abi::BackendRef;
use svelters_host::{Host, HostConfig};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum BackendKind {
    Gtk,
    Qt,
}

struct Args {
    backend: BackendKind,
    fixture: String,
    script: Option<PathBuf>,
    screenshot: Option<String>,
    report: Option<PathBuf>,
    end: End,
    timing: bool,
    timeout: Duration,
}

const USAGE: &str = "usage: counter --backend gtk|qt [--fixture <name|dir>] [--script <script.json>] \
[--screenshot <png>] [--report <json>] [--end quit|close] [--timing] [--timeout <secs>]";

fn parse_args() -> Result<Args, String> {
    let mut it = std::env::args().skip(1);
    let (mut backend, mut fixture, mut script, mut screenshot, mut report) = (None, "counter".to_string(), None, None, None);
    let (mut end, mut timing, mut timeout) = (End::Quit, false, Duration::from_secs(60));
    while let Some(a) = it.next() {
        let mut value = |name: &str| it.next().ok_or_else(|| format!("{name} needs a value"));
        match a.as_str() {
            "--backend" => {
                backend = Some(match value("--backend")?.as_str() {
                    "gtk" => BackendKind::Gtk,
                    "qt" => BackendKind::Qt,
                    other => return Err(format!("unknown backend '{other}' (gtk|qt)")),
                })
            }
            "--fixture" => fixture = value("--fixture")?,
            "--script" => script = Some(PathBuf::from(value("--script")?)),
            "--screenshot" => screenshot = Some(value("--screenshot")?),
            "--report" => report = Some(PathBuf::from(value("--report")?)),
            "--end" => {
                end = match value("--end")?.as_str() {
                    "quit" => End::Quit,
                    "close" => End::Close,
                    other => return Err(format!("unknown --end '{other}' (quit|close)")),
                }
            }
            "--timing" => timing = true,
            "--timeout" => {
                let s: f64 = value("--timeout")?.parse().map_err(|_| "--timeout needs seconds".to_string())?;
                timeout = Duration::from_secs_f64(s);
            }
            "-h" | "--help" => return Err(USAGE.to_string()),
            other => return Err(format!("unknown argument '{other}'\n{USAGE}")),
        }
    }
    Ok(Args {
        backend: backend.ok_or_else(|| format!("--backend is required\n{USAGE}"))?,
        fixture,
        script,
        screenshot,
        report,
        end,
        timing,
        timeout,
    })
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn bundle_path(fixture: &str) -> PathBuf {
    let dir = if fixture.contains('/') { PathBuf::from(fixture) } else { repo_root().join("fixtures").join(fixture) };
    dir.join("dist/app.js")
}

/// `VmHWM` / `VmRSS` in KiB from /proc/self/status.
fn memory_kb() -> (u64, u64) {
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let field = |name: &str| {
        status
            .lines()
            .find_map(|l| l.strip_prefix(name))
            .and_then(|r| r.trim().trim_end_matches("kB").trim().parse().ok())
            .unwrap_or(0)
    };
    (field("VmHWM:"), field("VmRSS:"))
}

struct Setup<'a> {
    args: &'a Args,
    start: Instant,
    backend: BackendRef<'static>,
    toolkit: Rc<dyn Toolkit>,
    /// Runs before the bundle loads (Qt installs its paint filter here).
    before_load: &'a dyn Fn(Rc<Cell<Option<Duration>>>),
    /// Runs after the bundle loaded and the window exists (GTK hooks its frame clock here).
    after_load: &'a dyn Fn(Rc<Cell<Option<Duration>>>),
}

fn run(setup: Setup) -> Result<Json, String> {
    let args = setup.args;
    let cfg = HostConfig { timing: args.timing, ..HostConfig::default() };
    let host = Rc::new(Host::new(setup.backend, cfg).map_err(|e| e.to_string())?);
    let first_frame = Rc::new(Cell::new(None));
    (setup.before_load)(first_frame.clone());

    let bundle = bundle_path(&args.fixture);
    if !bundle.is_file() {
        return Err(format!("{} not found (build the fixtures: pnpm install && pnpm -r build)", bundle.display()));
    }
    host.load_bundle(&bundle).map_err(|e| format!("load {}: {e}", bundle.display()))?;
    let loaded = setup.start.elapsed();
    (setup.after_load)(first_frame.clone());

    let driven = args.script.is_some() || args.screenshot.is_some() || args.report.is_some();
    let driver = if driven {
        let steps = match &args.script {
            Some(p) => {
                let text = std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?;
                svelters_backend_log::parse_script(&text)?
            }
            None => Vec::new(),
        };
        let opts = Options {
            steps,
            screenshot: args.screenshot.clone(),
            end: args.end,
            timeout: args.timeout,
            measure: args.timing,
        };
        let d = Driver::new(setup.toolkit.clone(), host.clone(), setup.backend, opts);
        d.start();
        Some(d)
    } else {
        None
    };

    let run_started = Instant::now();
    let run_result = host.run();
    let run_ms = run_started.elapsed().as_secs_f64() * 1e3;

    let errors: Vec<String> = host.take_errors().iter().map(|e| e.to_string()).collect();
    let console: Vec<String> = host.take_console().iter().map(|l| format!("[{}] {}", l.level, l.text)).collect();
    let mut failures = Vec::new();
    if let Err(e) = &run_result {
        failures.push(format!("run() failed: {e}"));
    }
    let (dump, steps) = match driver.as_ref().map(|d| d.report()) {
        Some(r) => {
            failures.extend(r.failures);
            (r.dump.unwrap_or(Json::Null), r.steps)
        }
        None => (Json::Null, Vec::new()),
    };
    let (peak_kb, rss_kb) = memory_kb();
    Ok(json!({
        "backend": format!("{:?}", args.backend).to_lowercase(),
        "fixture": args.fixture,
        "ok": failures.is_empty() && errors.is_empty(),
        "run_returned": run_result.is_ok(),
        "end": format!("{:?}", args.end).to_lowercase(),
        "failures": failures,
        "errors": errors,
        "console": console,
        "dump": dump,
        "steps": steps,
        "checkpoints": host.checkpoint_count(),
        "load_ms": loaded.as_secs_f64() * 1e3,
        "first_frame_ms": first_frame.get().map(|d| d.as_secs_f64() * 1e3),
        "run_ms": run_ms,
        "peak_rss_kb": peak_kb,
        "rss_kb": rss_kb,
    }))
}

#[cfg(feature = "gtk")]
fn run_gtk(args: &Args, start: Instant) -> Result<Json, String> {
    use svelters_abi::BackendRef;
    let backend: &'static svelters_gtk::GtkBackend = Box::leak(Box::new(svelters_gtk::GtkBackend::new()?));
    if args.timing {
        svelters_gtk::timing::set_enabled(true);
    }
    // SAFETY: the backend is leaked, so its vtable lives for the rest of the process.
    let bref = unsafe { BackendRef::from_raw(backend.as_raw()) }.map_err(|e| e.message)?;
    let tk = Rc::new(gtk_tk::GtkToolkit { backend });
    let tk2 = tk.clone();
    run(Setup {
        args,
        start,
        backend: bref,
        toolkit: tk,
        before_load: &|_| {},
        after_load: &move |done| tk2.on_first_frame(driver::ROOT, start, done),
    })
}

#[cfg(not(feature = "gtk"))]
fn run_gtk(_: &Args, _: Instant) -> Result<Json, String> {
    Err("built without the gtk feature".into())
}

#[cfg(feature = "qt")]
fn run_qt(args: &Args, start: Instant) -> Result<Json, String> {
    if args.timing {
        counter_qt::hooks::set_timing(true);
    }
    // SAFETY: the Qt backend vtable is a static.
    let bref = unsafe { BackendRef::from_raw(counter_qt::backend()) }.map_err(|e| e.message)?;
    let tk = Rc::new(qt_tk::QtToolkit { backend: bref });
    run(Setup {
        args,
        start,
        backend: bref,
        toolkit: tk,
        before_load: &move |done| {
            counter_qt::hooks::on_first_paint(Box::new(move || {
                if done.get().is_none() {
                    done.set(Some(start.elapsed()));
                }
            }))
        },
        after_load: &|_| {},
    })
}

#[cfg(not(feature = "qt"))]
fn run_qt(_: &Args, _: Instant) -> Result<Json, String> {
    Err("built without the qt feature".into())
}

fn main() -> ExitCode {
    let start = Instant::now();
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    let result = match args.backend {
        BackendKind::Gtk => run_gtk(&args, start),
        BackendKind::Qt => run_qt(&args, start),
    };
    match result {
        Ok(report) => {
            let ok = report["ok"].as_bool().unwrap_or(false);
            let text = serde_json::to_string_pretty(&report).unwrap_or_default();
            match &args.report {
                Some(p) => {
                    if let Err(e) = std::fs::write(p, text) {
                        eprintln!("cannot write {}: {e}", p.display());
                        return ExitCode::from(2);
                    }
                }
                None if args.script.is_some() || args.screenshot.is_some() => println!("{text}"),
                None => {}
            }
            // Skip static destructors of the toolkits: the process is done.
            if ok { ExitCode::SUCCESS } else { ExitCode::from(1) }
        }
        Err(e) => {
            eprintln!("counter: {e}");
            ExitCode::from(2)
        }
    }
}
