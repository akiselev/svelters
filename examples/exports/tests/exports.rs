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
    // The tally stays (the handlers hold it), and so do the telemetry objects (two stores, the burst
    // emitter); the notebook and its first note were only used during mount, so QuickJS may already have
    // collected their wrappers and released the handles (`persist` keeps the notebook's Rust object, E3).
    assert!((4..=6).contains(&host.live_app_objects()), "{}", host.live_app_objects());

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

fn find_scale(backend: &GtkBackend) -> Option<gtk::Scale> {
    (1..4096u64).filter_map(|h| backend.object(h)).find_map(|o| o.downcast::<gtk::Scale>().ok())
}

fn has_label(backend: &GtkBackend, text: &str) -> bool {
    label_texts(backend).iter().any(|t| t == text)
}

fn number_after(backend: &GtkBackend, prefix: &str) -> Option<u64> {
    label_texts(backend).iter().find_map(|t| t.strip_prefix(prefix).and_then(|n| n.trim().parse().ok()))
}

/// E3 acceptance on GTK: the `telemetry` service (started with the host) feeds a Rust store that Svelte
/// reads like `$state`; `<scale bind:value={telemetry.speed}>` binds a writable store field (lowered to
/// `bind_prop`: `Gtk.Range:value` is `exact`): a Rust write moves the scale, and a user drag past the
/// range `on_write` allows is clamped, delivered once, with no echo loop.
#[test]
fn telemetry_store_drives_a_bound_scale() {
    exports_demo::link();
    let Some(bundle) = bundle() else { return };
    let backend = GtkBackend::new().expect("backend");
    let br = unsafe { BackendRef::from_raw(backend.as_raw()) }.unwrap();
    let host = Host::new(br, HostConfig { echo_console: false, ..HostConfig::default() }).unwrap();
    unsafe { backend.attach_host(host.host_callbacks()) };
    host.load_bundle(&bundle).unwrap();
    settle(&host, |_| has_label(&backend, "samples flowing") && has_label(&backend, "speed 1.0") && find_scale(&backend).is_some());
    assert!(has_label(&backend, "samples flowing"), "the service feeds the store: {:?}", label_texts(&backend));
    let scale = find_scale(&backend).expect("the scale");
    assert_eq!(scale.value(), 1.0, "the scale shows the store's speed");

    // Rust writes the field: the store change reaches the bound scale.
    find_button(&backend, "Speed 4 from Rust").unwrap().emit_clicked();
    settle(&host, |_| scale.value() == 4.0 && has_label(&backend, "speed 4.0"));
    assert_eq!(scale.value(), 4.0, "{:?}", label_texts(&backend));
    assert!(has_label(&backend, "speed 4.0"));

    // The user drags to 15: JS writes `telemetry.speed = 15`, `on_write` clamps it to 10, the clamped
    // value is delivered once and the scale follows; the echoes (the store's own dirty bit, the scale's
    // notify of our set) are dropped.
    let runs0 = number_after(&backend, "speed runs ").unwrap();
    scale.set_value(15.0);
    settle(&host, |_| scale.value() == 10.0 && has_label(&backend, "speed 10.0"));
    assert_eq!(scale.value(), 10.0, "{:?}", label_texts(&backend));
    assert_eq!(exports_demo::TELEMETRY.get(|t| t.speed), 10.0);
    for _ in 0..30 {
        pump(5);
        host.checkpoint();
    }
    let runs = number_after(&backend, "speed runs ").unwrap() - runs0;
    // One run for JS's own write (a local invalidation), one for the clamped value: no echo loop.
    assert!((1..=2).contains(&runs), "speed effect ran {runs} times after the drag");
    assert_eq!(scale.value(), 10.0);

    // The proportional log field is an invalidation; the label re-reads it.
    settle(&host, |_| label_texts(&backend).iter().any(|t| t.contains("samples, speed")));
    assert!(label_texts(&backend).iter().any(|t| t.contains("samples, speed")), "{:?}", label_texts(&backend));
    let errors = host.take_errors();
    assert!(errors.is_empty(), "{errors:#?}");
}

/// E3 measurement on GTK: N worker writes to a store reach Svelte as one change batch per checkpoint
/// (a handful of effect runs), against N events delivered one by one (N effect runs). N is
/// `SVELTERS_BURST` (default 2000); the milestone numbers use 10000 in a release build:
/// `SVELTERS_BURST=10000 cargo test --release -p exports-example --test exports -- --nocapture burst`.
#[test]
fn store_burst_is_batched_event_burst_is_not() {
    exports_demo::link();
    let Some(bundle) = bundle() else { return };
    let n = exports_demo::burst_size() as u64;
    let backend = GtkBackend::new().expect("backend");
    let br = unsafe { BackendRef::from_raw(backend.as_raw()) }.unwrap();
    let host = Host::new(br, HostConfig { echo_console: false, ..HostConfig::default() }).unwrap();
    unsafe { backend.attach_host(host.host_callbacks()) };
    host.load_bundle(&bundle).unwrap();
    settle(&host, |_| find_button(&backend, "Burst store").is_some());

    let effect_runs = |prefix: &str| -> Option<(u64, f64)> {
        let t = label_texts(&backend).into_iter().find(|t| t.starts_with(prefix))?;
        let runs = t.split(", ").nth(1)?.split(' ').next()?.parse().ok()?;
        let ms = t.rsplit(", ").next()?.trim_end_matches(" ms").parse().ok()?;
        Some((runs, ms))
    };

    let (s0, c0) = (host.reactive_stats(), host.checkpoint_count());
    find_button(&backend, "Burst store").unwrap().emit_clicked();
    settle(&host, |_| effect_runs("store burst:").is_some());
    let (store_runs, store_ms) = effect_runs("store burst:").expect("store burst result");
    let (s1, c1) = (host.reactive_stats(), host.checkpoint_count());

    find_button(&backend, "Burst events").unwrap().emit_clicked();
    settle(&host, |_| effect_runs("event burst:").is_some());
    let (event_runs, event_ms) = effect_runs("event burst:").expect("event burst result");
    let s2 = host.reactive_stats();

    let batches = s1.store_batches - s0.store_batches;
    println!(
        "store burst: {n} worker writes -> {batches} change batch(es) over {} checkpoints, {} store fields delivered, {store_runs} effect run(s), {store_ms:.1} ms",
        c1 - c0,
        s1.store_delivered - s0.store_delivered
    );
    println!("event burst: {n} events -> {} delivered, {event_runs} effect run(s), {event_ms:.1} ms", s2.events - s1.events);
    // Writes coalesce: far fewer deliveries than writes (one per checkpoint at most).
    assert!(store_runs <= batches.max(1) + 1 && store_runs < n / 10, "{store_runs} effect runs for {n} writes");
    // Events do not: every one is delivered and handled.
    assert_eq!(s2.events - s1.events, n);
    assert!(event_runs > store_runs * 10, "{event_runs} vs {store_runs}");
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
    for name in ["exports.json", "app.d.ts", "demo.js", "telemetry.js"] {
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
