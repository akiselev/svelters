//! Measures, on the real host (QuickJS) + GTK backend, what observing a native property costs:
//!   - a host op round trip (`getProp`) from JS;
//!   - delivering one queued change event (`value-changed`; `notify::value` cannot be listened to on a
//!     reflected GTK kind yet) to JS at a checkpoint (post_event -> queue ->
//!     `__svelters_dispatch` -> job drain), with and without reading the value back;
//!   - the same with a Svelte reactive getter (`createSubscriber`) and a render effect, when the
//!     optional Svelte bundle (`js/dist/svelte_bench.js`, built by `js/build.sh`) exists.
//!   - R1: the real host batch (`host.observe`, dirty set, one read and one `__svelters_changed` per
//!     checkpoint) under `@svelters/native-runtime`'s tracker (`js/dist/tracker_bench.js`).
//! Each scenario is a JS module (in `js/`) that queues N native changes while it loads (GI calls of
//! `gtk_adjustment_set_value`, which emit `value-changed`); the harness then times one checkpoint.
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Instant;
use svelters_abi::BackendRef;
use svelters_gtk::GtkBackend;
use svelters_gtk::testing::on_gtk;
use svelters_host::{Host, HostConfig};

fn js_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("js")
}

/// Loads `module` with `defines` prepended (as `globalThis.X = v;`), then times one checkpoint.
fn scenario(module: &Path, defines: &[(&str, String)], label: &str) {
    let src = std::fs::read_to_string(module).expect("read module");
    let pre: String = defines.iter().map(|(k, v)| format!("globalThis.{k} = {v};\n")).collect();
    let dir = std::env::temp_dir().join("svl-reactivity-bench");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{label}.js"));
    std::fs::write(&path, format!("{pre}{src}")).unwrap();
    let label = label.to_string();
    on_gtk(move || {
        let backend = Rc::new(GtkBackend::new().expect("gtk"));
        let raw = unsafe { BackendRef::from_raw(backend.as_raw()) }.unwrap();
        let cfg = HostConfig {
            echo_console: true,
            echo_errors: true,
            turn_budget: None,
            max_jobs_per_checkpoint: usize::MAX,
            quit_on_last_window: false,
            ..HostConfig::default()
        };
        let host = Host::new(raw, cfg).unwrap();
        // drive checkpoints ourselves instead of running the GTK loop
        unsafe { backend.attach_host(host.host_callbacks()) };
        host.load_bundle(&path).unwrap();
        let t = Instant::now();
        host.checkpoint();
        let dt = t.elapsed();
        // a second checkpoint reports what the first one's handlers counted (after the 1 ms timer floor)
        std::thread::sleep(std::time::Duration::from_millis(2));
        host.checkpoint();
        for e in host.take_errors() {
            eprintln!("[{label}] error: {e:?}");
        }
        println!("{label}\tcheckpoint {:.1} ms", dt.as_secs_f64() * 1e3);
        drop(host);
    });
}

fn main() {
    let n: usize = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(10_000);
    let raw = js_dir().join("raw.js");
    for (mode, label) in [("'none'", "deliver-noop"), ("'read'", "deliver-getProp"), ("'skip'", "deliver-unlistened")] {
        scenario(&raw, &[("N", n.to_string()), ("MODE", mode.to_string())], label);
    }
    // one event carrying N changes' worth of work: the coalesced lower bound
    scenario(&raw, &[("N", "1".into()), ("MODE", "'read'".into())], "deliver-getProp-N1");
    let sv = js_dir().join("dist/svelte_bench.js");
    if sv.exists() {
        for (mode, label) in [("'subscriber'", "svelte-subscriber-per-event"), ("'coalesced'", "svelte-subscriber-coalesced")] {
            scenario(&sv, &[("N", n.to_string()), ("MODE", mode.to_string())], label);
        }
    } else {
        eprintln!("skipped Svelte scenarios: build {} first (js/build.sh)", sv.display());
    }
    // R1: the real host batch (dirty set, one read, one `__svelters_changed`) and the runtime's tracker.
    let tr = js_dir().join("dist/tracker_bench.js");
    if tr.exists() {
        for (mode, label) in [("'batched'", "r1-tracker-batched"), ("'unobserved'", "r1-tracker-unobserved")] {
            scenario(&tr, &[("N", n.to_string()), ("MODE", mode.to_string())], label);
        }
    } else {
        eprintln!("skipped R1 tracker scenarios: build {} first (js/build.sh)", tr.display());
    }
}
