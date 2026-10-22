//! Q4b: release startup timings of the counter-egui fixture (not part of CI). Run in a release build:
//!
//! ```text
//! tools/dev/cargo-slot cargo test --release -p svelters-egui --test startup -- --ignored --nocapture
//! ```
//!
//! Each case loads the app on a new embed (its own QuickJS runtime) under kittest, headless, and
//! times the load (resolve, compile or read bytecode, evaluate every module, the first jobs) and the
//! time to the first frame (from opening the `.svb` to the end of the first egui frame). Cases:
//! - `source`: compiled live from the fixture's sources (`--entry`), no bytecode cache;
//! - `cold`: the bundle's JS, an empty bytecode cache (the first launch: compile and write);
//! - `warm`: the bundle's JS, the cache the cold launch filled (Q2);
//! - `precompiled`: the bundle's own bytecode (`bytecode::precompile`), the cache empty.
#![cfg(feature = "source")]

use egui_kittest::kittest::Queryable;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};
use svelters_egui::harness::TestApp;
use svelters_host::HostConfig;
use svelters_host::modules::svb::{self, BuildOptions, Svb, SvbProvider};

const RUNS: usize = 7;

fn quiet() -> HostConfig {
    HostConfig { echo_console: false, echo_errors: false, ..HostConfig::default() }
}

struct Sample {
    load: Duration,
    first_frame: Duration,
}

/// One launch: `open` gives the provider (from t0), the cache is `cache`.
fn launch(cache: Option<&Path>, open: impl FnOnce() -> Rc<dyn svelters_host::ModuleProvider>) -> Sample {
    let t0 = Instant::now();
    let provider = open();
    let mut load = Duration::ZERO;
    let mut app = TestApp::with_loader(quiet(), |embed| {
        embed.host().set_bytecode_cache(cache).unwrap();
        let t = Instant::now();
        let r = embed.load_app_from(provider);
        load = t.elapsed();
        r
    })
    .unwrap();
    app.harness.step();
    let first_frame = t0.elapsed();
    app.run();
    assert!(app.harness.query_by_label("Count: 0").is_some());
    assert!(app.errors().is_empty(), "{:?}", app.errors());
    Sample { load, first_frame }
}

fn median(mut v: Vec<Duration>) -> f64 {
    v.sort();
    v[v.len() / 2].as_secs_f64() * 1000.0
}

#[test]
#[ignore = "a measurement: run in release with --ignored --nocapture"]
fn release_startup_timings() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/counter-egui").canonicalize().unwrap();
    let out = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/q4b");
    let _ = std::fs::remove_dir_all(&out);
    std::fs::create_dir_all(&out).unwrap();
    let source_cfg = || {
        let mut c = svelters_host::modules::SourceConfig::for_path(&fixture).unwrap();
        c.backend = "egui".into();
        c
    };
    let t = Instant::now();
    let bytes = svb::build(source_cfg(), &BuildOptions::default()).unwrap();
    let build_ms = t.elapsed().as_secs_f64() * 1000.0;
    let (app_svb, pre_svb) = (out.join("app.svb"), out.join("app.pre.svb"));
    std::fs::write(&app_svb, &bytes).unwrap();
    let t = Instant::now();
    let pre = svelters_host::bytecode::precompile(&Svb::from_bytes(bytes.clone()).unwrap()).unwrap();
    let precompile_ms = t.elapsed().as_secs_f64() * 1000.0;
    std::fs::write(&pre_svb, &pre).unwrap();

    let from_svb = |file: PathBuf| {
        move || -> Rc<dyn svelters_host::ModuleProvider> {
            let svb = Arc::new(Svb::open(&file).unwrap());
            Rc::new(SvbProvider::new(svb).unwrap().engine_build(svelters_host::bytecode::stamp()))
        }
    };
    let cache = out.join("bytecode-cache");
    let mut rows: Vec<(&str, Vec<Sample>)> = Vec::new();
    let source = (0..RUNS)
        .map(|_| launch(None, || Rc::new(svelters_host::modules::SourceProvider::new(source_cfg()).unwrap())))
        .collect();
    rows.push(("source", source));
    let cold = (0..RUNS)
        .map(|_| {
            let _ = std::fs::remove_dir_all(&cache);
            launch(Some(&cache), from_svb(app_svb.clone()))
        })
        .collect();
    rows.push(("cold", cold));
    rows.push(("warm", (0..RUNS).map(|_| launch(Some(&cache), from_svb(app_svb.clone()))).collect()));
    let precompiled = (0..RUNS)
        .map(|_| {
            let _ = std::fs::remove_dir_all(&cache);
            launch(Some(&cache), from_svb(pre_svb.clone()))
        })
        .collect();
    rows.push(("precompiled", precompiled));

    let modules = Svb::from_bytes(bytes.clone()).unwrap().index().modules.len();
    println!("Q4b startup, counter-egui: {modules} modules, bundle {} B, precompiled {} B; build {build_ms:.1} ms, precompile {precompile_ms:.1} ms", bytes.len(), pre.len());
    println!("| case | load (median of {RUNS}) | first frame (median of {RUNS}) |");
    for (name, samples) in rows {
        let (load, ff): (Vec<_>, Vec<_>) = samples.into_iter().map(|s| (s.load, s.first_frame)).unzip();
        println!("| {name} | {:.1} ms | {:.1} ms |", median(load), median(ff));
    }
}
