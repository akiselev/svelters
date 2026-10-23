//! Runs an app in an eframe window:
//!
//! ```text
//! svelters-egui-run [--bundle] <dist/app.js>          # a bundle built by Vite
//! svelters-egui-run --entry <app dir | file> [--dev]  # from source, compiled live (Q1)
//! svelters-egui-run --app <app.svb>                   # a release bundle (Q4b)
//!                   [--watch] [--idle-report <ms>] [--utilities[=<prefix>]]
//! ```
//!
//! `--utilities` turns utility classes on (U3, `EmbedConfig::with_utilities`; off by default);
//! `--utilities=tw` makes only `tw:`-prefixed classes utilities.
//!
//! `--entry` takes an app directory (its `src/App.svelte`, or its package.json `embed` entry), a
//! `.svelte` file (mounted as the App) or a `.js`/`.svelte.js` module (evaluated as is); the app's
//! package.json `"svelters"` field gives the spec fragments. Components are compiled for egui.
//! `--dev` compiles with Svelte's `dev` and resolves the `development` condition. `--watch`
//! reloads in place when a file of the app changes (a bundle: the bundle; from source: the module
//! graph).
//!
//! `--app` loads a `.svb` release bundle (`svelters_modules::svb::build`): its bytecode when it was
//! precompiled by this binary's engine, else its JS through the bytecode cache.
//!
//! `--idle-report <ms>`: after the app has settled (first frames drawn, then 1 s without one), counts the frames drawn and the CPU time
//! the process used over `<ms>`, prints one JSON line (`{"frames":..,"cpu_ms":..,"wall_ms":..}`) and
//! exits. That is the idle-CPU measurement of the G1 milestone (run under tools/dev/headless).

use std::time::{Duration, Instant};

/// utime + stime of this process, in milliseconds (Linux).
fn cpu_ms() -> Option<f64> {
    let stat = std::fs::read_to_string("/proc/self/stat").ok()?;
    let rest = &stat[stat.rfind(')')? + 2..];
    let f: Vec<&str> = rest.split(' ').collect();
    let ticks: f64 = f.get(11)?.parse::<f64>().ok()? + f.get(12)?.parse::<f64>().ok()?;
    Some(ticks * 1000.0 / 100.0) // USER_HZ is 100 on Linux
}

enum Load {
    Bundle(std::path::PathBuf),
    Entry(std::path::PathBuf),
    App(std::path::PathBuf),
}

const USAGE: &str = "usage: svelters-egui-run [--idle-report <ms>] [--watch] [--utilities[=<prefix>]] ([--bundle] <dist/app.js> | --entry <app dir|file> [--dev] | --app <app.svb>)";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (mut load, mut idle_ms, mut dev, mut watch, mut utilities) = (None, None, false, false, None);
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--idle-report" => idle_ms = it.next().and_then(|v| v.parse::<u64>().ok()),
            "--bundle" => load = it.next().map(|p| Load::Bundle(p.into())),
            "--entry" => load = it.next().map(|p| Load::Entry(p.into())),
            "--app" => load = it.next().map(|p| Load::App(p.into())),
            "--dev" => dev = true,
            "--watch" => watch = true,
            "--utilities" => utilities = Some(svelters_egui::Utilities::on()),
            s if s.starts_with("--utilities=") => utilities = Some(svelters_egui::Utilities::on().prefix(&s["--utilities=".len()..])),
            "-h" | "--help" => {
                eprintln!("{USAGE}");
                std::process::exit(0);
            }
            _ => load = Some(Load::Bundle(a.into())),
        }
    }
    let Some(load) = load else {
        eprintln!("{USAGE}");
        std::process::exit(2);
    };
    let fail = |e: String| -> ! {
        eprintln!("{e}");
        std::process::exit(1)
    };
    // The desktop order on an Embed (EM1): the app mounts its windows; eframe's context is bound
    // at the first frame.
    // DX2: the system clipboard and shell for `svelters:clipboard` and `svelters:shell`.
    let host = svelters_host::HostConfig { platform: svelters_egui::platform::system(), ..svelters_host::HostConfig::default() };
    let mut cfg = svelters_egui::embed::EmbedConfig::app(host);
    if let Some(u) = utilities {
        cfg = cfg.with_utilities(u);
    }
    let embed = svelters_egui::Embed::new(&svelters_egui::egui::Context::default(), cfg).unwrap_or_else(|e| fail(e.to_string()));
    let loaded = match load {
        Load::Bundle(bundle) => embed.load_app(&bundle),
        Load::Entry(entry) => {
            let mut cfg = svelters_host::modules::SourceConfig::for_path(&entry).unwrap_or_else(|e| fail(e));
            cfg.backend = "egui".into();
            cfg.dev = dev;
            let provider = svelters_host::modules::SourceProvider::new(cfg).unwrap_or_else(|e| fail(e));
            embed.load_app_from(std::rc::Rc::new(provider))
        }
        Load::App(file) => {
            use svelters_host::modules::svb::{Svb, SvbProvider};
            let svb = Svb::open(&file).unwrap_or_else(|e| fail(e));
            let provider = SvbProvider::new(std::sync::Arc::new(svb)).unwrap_or_else(|e| fail(e)).engine_build(svelters_host::bytecode::stamp());
            embed.load_app_from(std::rc::Rc::new(provider))
        }
    };
    if let Err(e) = loaded {
        fail(e.to_string());
    }
    if watch && let Err(e) = embed.host().watch() {
        fail(e.to_string());
    }
    if let Some(ms) = idle_ms {
        let frames = embed.backend().frame_counter();
        std::thread::spawn(move || {
            // Settled: the first frames are drawn and no frame came for 1 s (up to 30 s on a slow,
            // loaded machine).
            let start = Instant::now();
            let (mut last, mut quiet) = (frames(), 0);
            while start.elapsed() < Duration::from_secs(30) && (last == 0 || quiet < 10) {
                std::thread::sleep(Duration::from_millis(100));
                let f = frames();
                quiet = if f == last { quiet + 1 } else { 0 };
                last = f;
            }
            let (f0, c0, t0) = (frames(), cpu_ms().unwrap_or(f64::NAN), Instant::now());
            std::thread::sleep(Duration::from_millis(ms));
            let (f1, c1) = (frames(), cpu_ms().unwrap_or(f64::NAN));
            println!(
                "{{\"frames_before\":{f0},\"frames\":{},\"cpu_ms\":{:.1},\"wall_ms\":{}}}",
                f1 - f0,
                c1 - c0,
                t0.elapsed().as_millis()
            );
            std::process::exit(if f0 > 0 { 0 } else { 3 });
        });
    }
    if let Err(e) = svelters_egui::runner::run_desktop(embed) {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
