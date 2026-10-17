//! Runs the `counter` binary against real toolkits and checks the final native widget state.
//!
//! Needs a display and the built fixture bundles: `pnpm install && pnpm -r build`, then
//! `xvfb-run -a cargo nextest run -p counter`. Each test runs the binary as a child process (the
//! toolkits want the main thread and one `QApplication`/`GtkApplication` per process). The binary
//! drives the fixture's `script.json` through the real toolkit (native button click, typing into
//! the native entry), reads the widget tree back from the toolkit and writes a JSON report; the
//! screenshots land in `target/p5/<backend>-<fixture>.png`.

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn out_dir() -> PathBuf {
    let d = root().join("target/p5");
    std::fs::create_dir_all(&d).unwrap();
    d
}

struct Run {
    out: Output,
    report: Option<Value>,
}

impl Run {
    fn stderr(&self) -> String {
        String::from_utf8_lossy(&self.out.stderr).into_owned()
    }

    fn report(&self) -> &Value {
        self.report.as_ref().unwrap_or_else(|| panic!("no report; stderr:\n{}", self.stderr()))
    }

    /// The dump nodes as `(depth, class, text)`.
    fn nodes(&self) -> Vec<(u64, String, String)> {
        self.report()["dump"]
            .as_array()
            .expect("dump")
            .iter()
            .map(|n| (n["d"].as_u64().unwrap(), n["class"].as_str().unwrap().to_string(), n["text"].as_str().unwrap().to_string()))
            .collect()
    }

    /// Texts of all labels in layout order.
    fn labels(&self) -> Vec<String> {
        self.nodes().into_iter().filter(|(_, c, _)| is_label(c)).map(|(_, _, t)| t).collect()
    }

    /// Texts of the labels inside the scrolled window, i.e. the list rows in order.
    fn rows(&self) -> Vec<String> {
        let nodes = self.nodes();
        let scroll = nodes.iter().position(|(_, c, _)| c == "GtkScrolledWindow" || c == "QScrollArea").expect("a scrolled window");
        let depth = nodes[scroll].0;
        nodes[scroll + 1..]
            .iter()
            .take_while(|(d, _, _)| *d > depth)
            .filter(|(_, c, _)| is_label(c))
            .map(|(_, _, t)| t.clone())
            .collect()
    }

    fn assert_clean(&self) {
        let r = self.report();
        assert!(self.out.status.success(), "exit {:?}; stderr:\n{}", self.out.status, self.stderr());
        assert_eq!(r["failures"], serde_json::json!([]), "driver failures");
        assert_eq!(r["errors"], serde_json::json!([]), "host errors (stale handles show up here)");
        assert_eq!(r["run_returned"], true);
        // Timers run on the toolkit: the host has no timer thread (the `ps -T` check).
        let threads = r["threads"].as_array().expect("threads");
        assert!(!threads.is_empty() && threads.iter().all(|t| !t.as_str().unwrap_or("").starts_with("svelters-timer")), "{threads:?}");
        let err = self.stderr();
        for bad in ["stale", "CRITICAL", "foreign handle"] {
            assert!(!err.contains(bad), "stderr mentions '{bad}':\n{err}");
        }
    }
}

fn is_label(class: &str) -> bool {
    class == "GtkLabel" || class == "QLabel"
}

fn run_script(backend: &str, fixture: &str, script: &Path, extra: &[&str]) -> Run {
    let dir = out_dir();
    let tag = script.file_stem().unwrap().to_string_lossy().into_owned();
    let default_script = tag == "script";
    let mut name = if default_script { format!("{backend}-{fixture}") } else { format!("{backend}-{fixture}-{tag}") };
    if !extra.is_empty() {
        name.push_str("-close");
    }
    let report_path = dir.join(format!("{name}.json"));
    let _ = std::fs::remove_file(&report_path);
    let png = dir.join(format!("{name}.png"));
    let out = Command::new(env!("CARGO_BIN_EXE_counter"))
        // GDK's Vulkan path under Xvfb (lavapipe) can assert in gdk_vulkan_context_surface_detach
        // when a window closes mid-present; cairo avoids the GPU path. An explicit GSK_RENDERER wins.
        .env("GSK_RENDERER", std::env::var("GSK_RENDERER").unwrap_or_else(|_| "cairo".into()))
        .args(["--backend", backend, "--fixture", fixture, "--timeout", "30"])
        .arg("--script")
        .arg(script)
        .arg("--screenshot")
        .arg(&png)
        .arg("--report")
        .arg(&report_path)
        .args(extra)
        .output()
        .expect("spawn counter");
    let report = std::fs::read_to_string(&report_path).ok().map(|t| serde_json::from_str(&t).expect("report json"));
    if report.is_some() && extra.is_empty() {
        assert!(png.is_file(), "screenshot {} missing", png.display());
        let len = std::fs::metadata(&png).unwrap().len();
        assert!(len > 500, "screenshot {} is empty ({len} bytes)", png.display());
    }
    Run { out, report }
}

fn run_fixture(backend: &str, fixture: &str) -> Run {
    run_script(backend, fixture, &root().join("fixtures").join(fixture).join("script.json"), &[])
}

fn write_script(name: &str, json: &str) -> PathBuf {
    let p = out_dir().join(name);
    std::fs::write(&p, json).unwrap();
    p
}

// ---- counter ----

fn counter(backend: &str) {
    let r = run_fixture(backend, "counter");
    r.assert_clean();
    assert_eq!(r.labels(), ["Count: 3"]);
}

#[test]
fn counter_gtk() {
    counter("gtk");
}

#[test]
fn counter_qt() {
    counter("qt");
}

// ---- keyed list ----

fn keyed_list(backend: &str) {
    // The fixture script adds alpha, beta, gamma, reverses, then removes beta and gamma.
    let r = run_fixture(backend, "keyed-list");
    r.assert_clean();
    assert_eq!(r.rows(), ["alpha"]);

    // Add three rows and reverse: the native order must follow.
    let script = write_script(
        "keyed-list-reverse.json",
        r#"[{"checkpoint":true},
            {"event":[4,"changed",["alpha"]]},{"event":[5,"clicked",[]]},{"checkpoint":true},
            {"event":[4,"changed",["beta"]]},{"event":[5,"clicked",[]]},{"checkpoint":true},
            {"event":[4,"changed",["gamma"]]},{"event":[5,"clicked",[]]},{"checkpoint":true},
            {"event":[6,"clicked",[]]},{"checkpoint":true}]"#,
    );
    let r = run_script(backend, "keyed-list", &script, &[]);
    r.assert_clean();
    assert_eq!(r.rows(), ["gamma", "beta", "alpha"]);
    // The entry was cleared by the app after each add.
    let entry = r.nodes().into_iter().find(|(_, c, _)| c == "GtkEntry" || c == "QLineEdit").unwrap();
    assert_eq!(entry.2, "");

    // Reverse twice: back to insertion order.
    let script = write_script(
        "keyed-list-reverse2.json",
        r#"[{"checkpoint":true},
            {"event":[4,"changed",["alpha"]]},{"event":[5,"clicked",[]]},{"checkpoint":true},
            {"event":[4,"changed",["beta"]]},{"event":[5,"clicked",[]]},{"checkpoint":true},
            {"event":[4,"changed",["gamma"]]},{"event":[5,"clicked",[]]},{"checkpoint":true},
            {"event":[6,"clicked",[]]},{"checkpoint":true},{"event":[6,"clicked",[]]},{"checkpoint":true}]"#,
    );
    let r = run_script(backend, "keyed-list", &script, &[]);
    r.assert_clean();
    assert_eq!(r.rows(), ["alpha", "beta", "gamma"]);
}

#[test]
fn keyed_list_gtk() {
    keyed_list("gtk");
}

#[test]
fn keyed_list_qt() {
    keyed_list("qt");
}

// ---- keyed stress: reverse / insert / remove / shuffle with the expected order in a label ----

fn keyed_stress(backend: &str) {
    let r = run_fixture(backend, "keyed-stress");
    r.assert_clean();
    let labels = r.labels();
    let expected: Vec<String> = labels[0].split(',').map(str::to_string).collect();
    assert!(expected.len() > 3, "summary label '{}'", labels[0]);
    assert_eq!(labels[1..], expected[..], "native row order must equal the app's list");
}

#[test]
fn keyed_stress_gtk() {
    keyed_stress("gtk");
}

#[test]
fn keyed_stress_qt() {
    keyed_stress("qt");
}

// ---- timers ----

fn timers(backend: &str) {
    let r = run_fixture(backend, "timers");
    r.assert_clean();
    assert_eq!(r.labels(), ["done"]);
}

#[test]
fn timers_gtk() {
    timers("gtk");
}

#[test]
fn timers_qt() {
    timers("qt");
}

// ---- if-toggle: needs a header bar (GTK only) ----

#[test]
fn if_toggle_gtk() {
    let r = run_fixture("gtk", "if-toggle");
    r.assert_clean();
    // Three toggles: hidden -> shown -> hidden -> shown.
    assert_eq!(r.labels(), ["Shown"]);
    let nodes = r.nodes();
    let bar = nodes.iter().find(|(_, c, _)| c == "GtkHeaderBar").expect("header bar");
    assert_eq!(bar.2, "Toggle");
}

#[test]
fn if_toggle_qt_reports_missing_headerbar() {
    let r = run_fixture("qt", "if-toggle");
    assert!(!r.out.status.success(), "Qt has no headerbar kind and must not pretend to run");
    let err = r.stderr();
    assert!(err.contains("headerbar"), "error should name the missing kind:\n{err}");
    assert!(!err.contains("SIGSEGV") && !err.contains("panicked"), "must be a clean error:\n{err}");
}

// ---- run() returns when the last window closes natively ----

fn closes(backend: &str) {
    let r = run_script(backend, "counter", &root().join("fixtures/counter/script.json"), &["--end", "close"]);
    r.assert_clean();
    assert_eq!(r.report()["end"], "close");
    assert_eq!(r.labels(), ["Count: 3"]);
}

#[test]
fn run_returns_after_native_close_gtk() {
    closes("gtk");
}

#[test]
fn run_returns_after_native_close_qt() {
    closes("qt");
}

// ---- native-close: a child window closed natively, then the app toggles it ----

fn native_close(backend: &str) {
    // The fixture script closes the child window (#5) natively, toggles twice and ends with a new
    // child window. Stale-handle complaints would show up as host errors or in stderr.
    let r = run_fixture(backend, "native-close");
    r.assert_clean();
    assert_eq!(r.labels(), ["child open"]);

    // Closed and toggled once: the app's state says closed, and nothing was sent for the dead window.
    let script = write_script(
        "native-close-once.json",
        r#"[{"checkpoint":true},{"gone":[5,"closed"]},{"checkpoint":true},{"event":[4,"clicked",[]]},{"checkpoint":true}]"#,
    );
    let r = run_script(backend, "native-close", &script, &[]);
    r.assert_clean();
    assert_eq!(r.labels(), ["child closed"]);
}

#[test]
fn native_close_gtk() {
    native_close("gtk");
}

#[test]
fn native_close_qt() {
    native_close("qt");
}
