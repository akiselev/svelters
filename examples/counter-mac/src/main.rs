//! `counter-mac`: loads a fixture bundle into the host and runs it on the AppKit backend.
//!
//! ```text
//! counter-mac [--fixture <name|dir>] [--watch] [--screenshot <png> [--exit]] [--report <json>]
//! ```
//!
//! - Default: opens the window and runs until it is closed (or Cmd+Q).
//! - `--watch`: the dev loop. Run `pnpm watch fixtures/<name>` next to it; every rebuild of
//!   `dist/app.js` is reloaded in this process without restarting it.
//! - `--screenshot <png>`: 1.5 s after start, captures the first window with `screencapture -l`.
//!   With `--exit` the process then writes the report and exits (a smoke test you can paste back).
//! - `--report <json>`: writes errors, console output and timings when the run ends.

#[cfg(target_os = "macos")]
fn main() -> std::process::ExitCode {
    mac::main()
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("counter-mac runs on macOS only (the AppKit backend); see docs/notes/macos-setup.md");
    std::process::exit(2);
}

#[cfg(target_os = "macos")]
mod mac {
    use serde_json::json;
    use std::path::PathBuf;
    use std::process::ExitCode;
    use std::time::{Duration, Instant};
    use svelters_abi::BackendRef;
    use svelters_appkit::AppKitBackend;
    use svelters_host::{Host, HostConfig};

    struct Args {
        fixture: String,
        watch: bool,
        screenshot: Option<String>,
        exit: bool,
        report: Option<PathBuf>,
    }

    const USAGE: &str = "usage: counter-mac [--fixture <name|dir>] [--watch] [--screenshot <png> [--exit]] [--report <json>]";

    fn parse() -> Result<Args, String> {
        let mut a = Args { fixture: "counter-mac".into(), watch: false, screenshot: None, exit: false, report: None };
        let mut it = std::env::args().skip(1);
        while let Some(x) = it.next() {
            let mut val = |n: &str| it.next().ok_or_else(|| format!("{n} needs a value"));
            match x.as_str() {
                "--fixture" => a.fixture = val("--fixture")?,
                "--watch" => a.watch = true,
                "--screenshot" => a.screenshot = Some(val("--screenshot")?),
                "--exit" => a.exit = true,
                "--report" => a.report = Some(PathBuf::from(val("--report")?)),
                "-h" | "--help" => return Err(USAGE.into()),
                other => return Err(format!("unknown argument '{other}'\n{USAGE}")),
            }
        }
        Ok(a)
    }

    fn bundle_path(fixture: &str) -> PathBuf {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let dir = if fixture.contains('/') { PathBuf::from(fixture) } else { root.join("fixtures").join(fixture) };
        dir.join("dist/app.js")
    }

    pub fn main() -> ExitCode {
        let start = Instant::now();
        let args = match parse() {
            Ok(a) => a,
            Err(e) => {
                eprintln!("{e}");
                return ExitCode::from(2);
            }
        };
        let backend = match AppKitBackend::new() {
            Ok(b) => b,
            Err(e) => {
                eprintln!("counter-mac: {e}");
                return ExitCode::from(2);
            }
        };
        let vt = backend.vtable();
        // SAFETY: the vtable is leaked, so it lives for the rest of the process.
        let bref = unsafe { BackendRef::from_raw(vt.as_raw()) }.expect("vtable");
        let host = match Host::new(bref, HostConfig::default()) {
            Ok(h) => h,
            Err(e) => {
                eprintln!("counter-mac: {e}");
                return ExitCode::from(2);
            }
        };
        let bundle = bundle_path(&args.fixture);
        if !bundle.is_file() {
            eprintln!("counter-mac: {} not found (build it: pnpm install && pnpm -r build)", bundle.display());
            return ExitCode::from(2);
        }
        if let Err(e) = host.load_bundle(&bundle) {
            eprintln!("counter-mac: load {}: {e}", bundle.display());
            if !args.watch {
                return ExitCode::from(1);
            }
        }
        let loaded_ms = start.elapsed().as_secs_f64() * 1e3;
        eprintln!("[counter-mac] loaded {} in {loaded_ms:.1} ms", bundle.display());
        if args.watch {
            host.on_reload(|r| eprintln!("[counter-mac] reload ok={} in {:.1} ms", r.ok, r.elapsed.as_secs_f64() * 1e3));
            if let Err(e) = host.watch() {
                eprintln!("counter-mac: watch: {e}");
            }
        }
        if let Some(png) = args.screenshot.clone() {
            // The window number is read now (main thread); `screencapture` runs from a helper thread.
            let wins: Vec<i64> = backend
                .toplevels()
                .into_iter()
                .filter_map(|h| backend.object(h))
                .map(|w| unsafe { objc2::msg_send![&*w, windowNumber] })
                .map(|n: isize| n as i64)
                .collect();
            let exit = args.exit;
            let report = args.report.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(1500));
                let ok = match wins.first() {
                    Some(n) => std::process::Command::new("screencapture")
                        .args(["-x", "-o", &format!("-l{n}"), &png])
                        .status()
                        .is_ok_and(|s| s.success()),
                    None => false,
                };
                eprintln!("[counter-mac] screenshot {png}: {}", if ok { "ok" } else { "FAILED (no window, or Screen Recording permission missing)" });
                if exit {
                    if let Some(r) = report {
                        let _ = std::fs::write(r, json!({ "screenshot": png, "screenshot_ok": ok }).to_string());
                    }
                    std::process::exit(if ok { 0 } else { 1 });
                }
            });
        }
        let run = host.run();
        let errors: Vec<String> = host.take_errors().iter().map(|e| e.to_string()).collect();
        let console: Vec<String> = host.take_console().iter().map(|l| format!("[{}] {}", l.level, l.text)).collect();
        let ok = run.is_ok() && errors.is_empty();
        let report = json!({
            "backend": "appkit",
            "fixture": args.fixture,
            "ok": ok,
            "run_error": run.err().map(|e| e.to_string()),
            "errors": errors,
            "console": console,
            "checkpoints": host.checkpoint_count(),
            "load_ms": loaded_ms,
            "run_ms": start.elapsed().as_secs_f64() * 1e3,
        });
        let text = serde_json::to_string_pretty(&report).unwrap_or_default();
        match &args.report {
            Some(p) => {
                if let Err(e) = std::fs::write(p, &text) {
                    eprintln!("cannot write {}: {e}", p.display());
                }
            }
            None => eprintln!("{text}"),
        }
        if ok { ExitCode::SUCCESS } else { ExitCode::from(1) }
    }
}
