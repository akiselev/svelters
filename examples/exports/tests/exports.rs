//! E0 acceptance: the Svelte app in `app/` calls this crate's Rust exports on real GTK; the committed
//! typings and stubs match what this binary dumps; and the per-call cost of the registry path.

use gtk::prelude::*;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use svelters_abi::BackendRef;
use svelters_gtk::GtkBackend;
use svelters_gtk::testing::pump;
use svelters_host::{Host, HostConfig};

fn find_button(backend: &GtkBackend, label: &str) -> Option<gtk::Button> {

    (1..4096u64).filter_map(|h| backend.object(h)).filter_map(|o| o.downcast::<gtk::Button>().ok()).find(|b| b.label().as_deref() == Some(label))
}

fn label_texts(backend: &GtkBackend) -> Vec<String> {

    (1..4096u64).filter_map(|h| backend.object(h)).filter_map(|o| o.downcast::<gtk::Label>().ok()).map(|l| l.text().to_string()).collect()
}

/// The app bundle, or `None` (with a note) when `pnpm build` has not run.
fn bundle() -> Option<PathBuf> {
    let b = exports_demo::bundle();
    if b.is_file() {
        Some(b)
    } else {
        eprintln!("SKIP: {} not built (pnpm build in examples/exports/app)", b.display());
        None
    }
}

fn settle(host: &Host<'_>, until: impl Fn(&Host<'_>) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while !until(host) && Instant::now() < deadline {
        pump(10);
        host.checkpoint();
    }
}

#[test]
fn gtk_app_calls_rust_exports() {
    exports_demo::link();
    let Some(bundle) = bundle() else { return };
    let backend = GtkBackend::new().expect("backend");
    let br = unsafe { BackendRef::from_raw(backend.as_raw()) }.unwrap();
    let host = Host::new(br, HostConfig { echo_console: false, ..HostConfig::default() }).unwrap();
    unsafe { backend.attach_host(host.host_callbacks()) };
    host.load_bundle(&bundle).unwrap();
    let mut console = Vec::new();
    settle(&host, |_| find_button(&backend, "Bump").is_some());
    console.extend(host.take_console().into_iter().map(|l| l.text));
    assert!(
        console.iter().any(|l| l == "exports: Hello from Rust, Svelte! fib(30)=832040 same=true notes=1"),
        "console: {console:#?}\nerrors: {:#?}",
        host.take_errors()
    );
    assert!(label_texts(&backend).iter().any(|t| t == "count 5 (step one)"), "{:?}", label_texts(&backend));
    // The tally stays (the handlers hold it); the notebook and its note were only used during mount, so
    // QuickJS may already have collected them and released the Rust objects.
    assert!((1..=3).contains(&host.live_app_objects()), "{}", host.live_app_objects());

    // A click runs Svelte, which calls `Tally::bump` (`&mut self`) and reads the record back.
    find_button(&backend, "Bump").unwrap().emit_clicked();
    settle(&host, |_| label_texts(&backend).iter().any(|t| t == "count 6 (step one)"));
    assert!(label_texts(&backend).iter().any(|t| t == "bumps 1"), "{:?}", label_texts(&backend));

    // The enum setter, then a bump by ten.
    find_button(&backend, "Toggle step").unwrap().emit_clicked();
    settle(&host, |_| label_texts(&backend).iter().any(|t| t == "count 6 (step ten)"));
    find_button(&backend, "Bump").unwrap().emit_clicked();
    settle(&host, |_| label_texts(&backend).iter().any(|t| t == "count 16 (step ten)"));
    assert!(label_texts(&backend).iter().any(|t| t == "count 16 (step ten)"), "{:?}", label_texts(&backend));
    let errors = host.take_errors();
    assert!(errors.is_empty(), "{errors:#?}");
}

/// E1 acceptance: a 200 ms CPU job on a worker (`#[js(worker)] crunch`) keeps both the GTK loop and the
/// JS thread responsive, measured; the same job on the JS thread (`crunchBlocking`) freezes both. Also the
/// JS-thread `async fn` (`wordCount`, which awaits a worker) resolves.
#[test]
fn worker_job_keeps_the_ui_responsive() {
    use gtk::glib;
    exports_demo::link();
    let Some(bundle) = bundle() else { return };
    let backend = GtkBackend::new().expect("backend");
    let br = unsafe { BackendRef::from_raw(backend.as_raw()) }.unwrap();
    let host = Host::new(br, HostConfig { echo_console: false, ..HostConfig::default() }).unwrap();
    unsafe { backend.attach_host(host.host_callbacks()) };
    host.load_bundle(&bundle).unwrap();
    settle(&host, |_| find_button(&backend, "Crunch").is_some() && label_texts(&backend).iter().any(|t| t == "words 5"));
    assert!(label_texts(&backend).iter().any(|t| t == "words 5"), "{:?}", label_texts(&backend));

    // The GTK loop is driven by hand: the longest single dispatch (`iteration`) is how long the loop
    // was blocked; the number of dispatches shows it kept turning.
    let measure = |button: &str, prefix: &str| -> (Duration, u32, String) {
        // The newest button of that label (`{#if}` re-creates them).
        let b = (1..4096u64)
            .filter_map(|h| backend.object(h))
            .filter_map(|o| o.downcast::<gtk::Button>().ok())
            .filter(|b| b.label().as_deref() == Some(button) && b.parent().is_some())
            .last()
            .unwrap_or_else(|| panic!("no {button} button"));
        b.emit_clicked();
        let ctx = glib::MainContext::default();
        let end = Instant::now() + Duration::from_secs(30);
        let (mut longest, mut turns) = (Duration::ZERO, 0u32);
        loop {
            while ctx.pending() {
                let t0 = Instant::now();
                ctx.iteration(false);
                longest = longest.max(t0.elapsed());
                turns += 1;
            }
            if let Some(t) = label_texts(&backend).into_iter().find(|t| t.starts_with(prefix)) {
                return (longest, turns, t);
            }
            assert!(Instant::now() < end, "no {prefix} label: {:?} {:?}", label_texts(&backend), host.take_errors());
            std::thread::sleep(Duration::from_millis(1));
        }
    };
    let ticks = |t: &str| -> u32 { t.rsplit(' ').next().and_then(|n| n.parse().ok()).unwrap_or(0) };

    let (worker_gap, worker_turns, worker_text) = measure("Crunch", "crunched ");
    let (ui_gap, ui_turns, ui_text) = measure("Crunch on the UI thread", "blocked: ");
    println!("worker: longest GTK dispatch {:.1} ms over {worker_turns} dispatches; {worker_text}", worker_gap.as_secs_f64() * 1e3);
    println!("JS thread: longest GTK dispatch {:.1} ms over {ui_turns} dispatches; {ui_text}", ui_gap.as_secs_f64() * 1e3);
    // The worker job: the loop never stalls for anything like the job's 200 ms, and JS timers keep firing.
    assert!(worker_gap < Duration::from_millis(100), "worker job stalled the loop for {worker_gap:?}");
    assert!(ticks(&worker_text) >= 5, "JS timer ticks during the worker job: {worker_text}");
    // The same job on the JS thread: one stall of about the job's length, and no JS timer tick.
    assert!(ui_gap >= Duration::from_millis(150), "blocking job: longest gap only {ui_gap:?}");
    assert_eq!(ticks(&ui_text), 0, "{ui_text}");
    let errors = host.take_errors();
    assert!(errors.is_empty(), "{errors:#?}");
}

/// E2: the app's DSP panel moves 1 Mi samples Rust -> JS -> worker -> JS without a copy (the worker sees
/// the allocation `noise` made; the input array is detached), and reads a `Shared` view in place.
#[test]
fn dsp_round_trips_samples_without_a_copy() {
    exports_demo::link();
    let Some(bundle) = bundle() else { return };
    let backend = GtkBackend::new().expect("backend");
    let br = unsafe { BackendRef::from_raw(backend.as_raw()) }.unwrap();
    let host = Host::new(br, HostConfig { echo_console: false, ..HostConfig::default() }).unwrap();
    unsafe { backend.attach_host(host.host_callbacks()) };
    host.load_bundle(&bundle).unwrap();
    settle(&host, |_| find_button(&backend, "Smooth").is_some());
    assert!(label_texts(&backend).iter().any(|t| t == "wave 4096 samples, max 1.00, in place true"), "{:?} {:?}", label_texts(&backend), host.take_errors());
    find_button(&backend, "Smooth").unwrap().emit_clicked();
    settle(&host, |_| label_texts(&backend).iter().any(|t| t.starts_with("smoothed ")));
    let text = label_texts(&backend).into_iter().find(|t| t.starts_with("smoothed ")).unwrap_or_else(|| panic!("{:?} {:?}", label_texts(&backend), host.take_errors()));
    assert!(text.starts_with("smoothed 1048576 samples, zero-copy true, input length 0, max "), "{text}");
    let s = host.buffer_stats();
    assert_eq!(s.copies, 2, "only the two `stats` snapshots of non-shared arrays copy: {s:?}");
    assert!(s.reclaimed >= 1 && s.shared_in >= 1 && s.shared_out >= 1, "{s:?}");
    let errors = host.take_errors();
    assert!(errors.is_empty(), "{errors:#?}");
}

/// The committed `app/.svelters/app` is what this binary dumps (CI fails on stale typings, like
/// `svelters exports --check` would).
#[test]
fn committed_typings_are_current() {
    exports_demo::link();
    let tmp = std::env::temp_dir().join(format!("svelters-exports-check-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    svelters::exports::dump(&tmp).unwrap();
    let committed = exports_demo::exports_dir();
    for name in ["exports.json", "app.d.ts", "demo.js", "dsp.js"] {
        let want = std::fs::read_to_string(tmp.join(name)).unwrap();
        let have = std::fs::read_to_string(committed.join(name)).unwrap_or_default();
        assert!(
            want == have,
            "{} is stale: run `cargo run -p exports-example -- --dump {}`",
            committed.join(name).display(),
            Path::new("examples/exports/app/.svelters/app").display()
        );
    }
}

/// Per-call cost of the registry path against a plain JS call and a log-backend (C ABI) call.
/// Run with `cargo test --release -p exports-example --test exports -- --ignored --nocapture per_call`.
#[test]
#[ignore]
fn per_call_cost() {
    exports_demo::link();
    let dir = std::env::temp_dir().join(format!("svelters-exports-bench-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("main.js"),
        r#"
import { noop, Tally } from 'svelters:app/demo';
const H = globalThis.__svelters_host;
const N = 200000;
const bench = (name, f) => {
  for (let i = 0; i < 2000; i++) f(i);
  let best = Infinity;
  for (let r = 0; r < 5; r++) {
    const t0 = performance.now();
    for (let i = 0; i < N; i++) f(i);
    best = Math.min(best, (performance.now() - t0) * 1e6 / N);
  }
  console.log(`${name} ${best.toFixed(0)} ns`);
};
const js = (x) => x;
bench('js-call', (i) => js(i));
bench('registry-static', (i) => noop(i));
const t = new Tally(0);
bench('registry-getter', () => t.count);
bench('registry-method', () => t.summary(null));
const w = H.create('window');
bench('abi-setprop', (i) => H.setProp(w, 'title', 'x'));
bench('abi-call', (i) => H.call(w, 'present', []));
"#,
    )
    .unwrap();
    let (cfg, _) = svelters_host::testing::fixture_config();
    let log = svelters_backend_log::LogBackend::new(Vec::new());
    let vt = log.vtable();
    let host = Host::new(vt.backend_ref(), HostConfig { turn_budget: None, ..cfg }).unwrap();
    host.load_bundle(dir.join("main.js")).unwrap();
    for l in host.take_console() {
        println!("{}", l.text);
    }
    for e in host.take_errors() {
        println!("error: {e}");
    }
}

/// E1: the cost of settling offloaded calls (completion queue -> one engine entry per checkpoint), next to
/// the CALLBACK path's ~20 µs per completion (design note §4). Run with
/// `cargo test --release -p exports-example --test exports -- --ignored --nocapture offload_settle`.
#[test]
#[ignore]
fn offload_settle_cost() {
    exports_demo::link();
    let dir = std::env::temp_dir().join(format!("svelters-offload-bench-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let phases = [
        ("local", "const ps = []; for (let i = 0; i < N; i++) ps.push(noopLocal(i)); await Promise.all(ps);", 10000),
        ("in-flight", "const ps = []; for (let i = 0; i < N; i++) ps.push(noopWorker(i)); await Promise.all(ps);", 10000),
        ("sequential", "for (let i = 0; i < N; i++) await noopWorker(i);", 2000),
    ];
    let (cfg, _) = svelters_host::testing::fixture_config();
    let log = svelters_backend_log::LogBackend::new(Vec::new());
    let vt = log.vtable();
    let host = Host::new(vt.backend_ref(), HostConfig { turn_budget: None, ..cfg }).unwrap();
    for (name, body, n) in phases {
        for round in 0..3 {
            let file = dir.join(format!("{name}-{round}.js"));
            std::fs::write(
                &file,
                format!(
                    "import {{ noopWorker, noopLocal }} from 'svelters:app/demo';\nconst N = {n};\n(async () => {{ const t0 = performance.now(); {body} console.log(`{name} ${{((performance.now() - t0) * 1e3 / N).toFixed(2)}}`); }})();\n"
                ),
            )
            .unwrap();
            let s0 = host.offload_stats();
            host.load_bundle(&file).unwrap();
            let mut line = None;
            let end = Instant::now() + Duration::from_secs(60);
            while line.is_none() && Instant::now() < end {
                host.checkpoint();
                host.drain_jobs();
                line = host.take_console().into_iter().map(|l| l.text).find(|t| t.starts_with(name));
            }
            let s = host.offload_stats();
            let settled = (s.settled - s0.settled).max(1);
            println!(
                "{} us/call (JS wall) | drains {} | settle {:.2} us per completion (drain time / settled), of which engine {:.2} | eager {}",
                line.unwrap_or_else(|| format!("{name} timed out")),
                s.drains - s0.drains,
                (s.drain_ns - s0.drain_ns) as f64 / 1e3 / settled as f64,
                (s.engine_ns - s0.engine_ns) as f64 / 1e3 / settled as f64,
                s.settled_eagerly - s0.settled_eagerly
            );
        }
    }
    for e in host.take_errors() {
        println!("error: {e}");
    }
}
