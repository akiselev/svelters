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

/// The committed `app/.svelters/app` is what this binary dumps (CI fails on stale typings, like
/// `svelters exports --check` would).
#[test]
fn committed_typings_are_current() {
    exports_demo::link();
    let tmp = std::env::temp_dir().join(format!("svelters-exports-check-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    svelters::exports::dump(&tmp).unwrap();
    let committed = exports_demo::exports_dir();
    for name in ["exports.json", "app.d.ts", "demo.js"] {
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
