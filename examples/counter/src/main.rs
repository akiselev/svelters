//! `counter`: loads a fixture bundle into the host and runs it on a real toolkit backend.
//!
//! ```text
//! counter --backend gtk|qt [--fixture <name|dir>] [--script <script.json>] [--screenshot <png>]
//!         [--report <json>] [--end quit|close] [--timing] [--timeout <secs>]
//!         [--watch [--reload-exit <n>]]
//! ```
//!
//! `--watch` is the dev loop: the host watches the fixture's `dist/app.js` (run
//! `pnpm watch fixtures/<name>` next to it) and reloads it in this process, on the same toolkit
//! loop, whenever a rebuild lands. A bundle that fails to load keeps the process alive and prints
//! the source-mapped error. `--reload-exit <n>` ends the run after the n-th successful reload,
//! with the new widget tree in the `--report` (for tests).
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
use std::cell::{Cell, RefCell};
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
    watch: bool,
    reload_exit: Option<u32>,
}

const USAGE: &str = "usage: counter --backend gtk|qt [--fixture <name|dir>] [--script <script.json>] \
[--screenshot <png>] [--report <json>] [--end quit|close] [--timing] [--timeout <secs>] \
[--watch [--reload-exit <n>]]";

fn parse_args() -> Result<Args, String> {
    let mut it = std::env::args().skip(1);
    let (mut backend, mut fixture, mut script, mut screenshot, mut report) = (None, "counter".to_string(), None, None, None);
    let (mut end, mut timing, mut timeout) = (End::Quit, false, Duration::from_secs(60));
    let (mut watch, mut reload_exit) = (false, None);
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
            "--watch" => watch = true,
            "--reload-exit" => {
                reload_exit = Some(value("--reload-exit")?.parse().map_err(|_| "--reload-exit needs a count".to_string())?);
            }
            "--timeout" => {
                let s: f64 = value("--timeout")?.parse().map_err(|_| "--timeout needs seconds".to_string())?;
                timeout = Duration::from_secs_f64(s);
            }
            "-h" | "--help" => return Err(USAGE.to_string()),
            other => return Err(format!("unknown argument '{other}'\n{USAGE}")),
        }
    }
    if watch && (script.is_some() || screenshot.is_some()) {
        return Err(format!("--watch cannot be combined with --script or --screenshot\n{USAGE}"));
    }
    if reload_exit.is_some() && !watch {
        return Err(format!("--reload-exit needs --watch\n{USAGE}"));
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
        watch,
        reload_exit,
    })
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn bundle_path(fixture: &str) -> PathBuf {
    let dir = if fixture.contains('/') { PathBuf::from(fixture) } else { repo_root().join("fixtures").join(fixture) };
    dir.join("dist/app.js")
}

/// Names of this process's threads (Linux), to show the host starts none of its own.
fn thread_names() -> Vec<String> {
    let Ok(rd) = std::fs::read_dir("/proc/self/task") else { return Vec::new() };
    rd.filter_map(|e| std::fs::read_to_string(e.ok()?.path().join("comm")).ok()).map(|s| s.trim().to_string()).collect()
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
    if let Err(e) = host.load_bundle(&bundle) {
        let e = format!("load {}: {e}", bundle.display());
        if !args.watch {
            return Err(e);
        }
        // The backend's loop keeps running with no window; the host only quits once a toplevel
        // existed and the last one is gone.
        eprintln!("[counter] {e}\n[counter] waiting for the next change");
    }
    let loaded = setup.start.elapsed();
    (setup.after_load)(first_frame.clone());

    let reloads: Rc<RefCell<Vec<Json>>> = Rc::default();
    let reload_dump: Rc<RefCell<Option<Json>>> = Rc::default();
    let native_toplevels: Rc<Cell<Option<usize>>> = Rc::default();
    if args.watch {
        let weak = Rc::downgrade(&host);
        let (reloads, reload_dump, tk, backend, native_toplevels) = (reloads.clone(), reload_dump.clone(), setup.toolkit.clone(), setup.backend, native_toplevels.clone());
        let (exit_after, ok_count) = (args.reload_exit, Cell::new(0u32));
        host.on_reload(move |r| {
            reloads.borrow_mut().push(json!({
                "ok": r.ok,
                "ms": r.elapsed.as_secs_f64() * 1e3,
                "error": r.error.as_ref().map(|e| e.to_string()),
            }));
            if !r.ok {
                return;
            }
            ok_count.set(ok_count.get() + 1);
            if exit_after == Some(ok_count.get()) {
                // Give the toolkit a moment to map and lay out the new window, then record it.
                let (weak, tk2, reload_dump, native_toplevels) = (weak.clone(), tk.clone(), reload_dump.clone(), native_toplevels.clone());
                tk.schedule(
                    300,
                    Box::new(move || {
                        let tops = weak.upgrade().map(|h| h.toplevels()).unwrap_or_default();
                        let dump: Vec<Json> = tops.iter().map(|t| tk2.dump(*t)).collect();
                        *reload_dump.borrow_mut() = Some(Json::Array(dump));
                        native_toplevels.set(Some(tk2.native_toplevels()));
                        backend.quit();
                    }),
                );
            }
        });
        host.watch().map_err(|e| format!("watch {}: {e}", bundle.display()))?;
    }

    let driven = !args.watch && (args.script.is_some() || args.screenshot.is_some() || args.report.is_some());
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
    let mut dump_all = Json::Null;
    let (mut dump, steps) = match driver.as_ref().map(|d| d.report()) {
        Some(r) => {
            failures.extend(r.failures);
            dump_all = r.dump_all.unwrap_or(Json::Null);
            (r.dump.unwrap_or(Json::Null), r.steps)
        }
        None => (Json::Null, Vec::new()),
    };
    if let Some(d) = reload_dump.borrow_mut().take() {
        dump = d;
    }
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
        "dump_all": dump_all,
        "reloads": reloads.borrow().clone(),
        "native_toplevels": native_toplevels.get(),
        "threads": thread_names(),
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
        // Qt's loop keeps running with no window until `quit`.
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
