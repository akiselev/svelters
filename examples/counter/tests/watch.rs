//! Dev loop on the real toolkits: `counter --watch` reloads an edited fixture in the same process.
//!
//! The test copies the counter fixture into `target/m2b/`, runs `svelters-build --watch` on the
//! copy, starts the binary with `--watch --reload-exit 1`, edits `App.svelte` and checks the
//! widget tree the binary reports after the reload. Needs `pnpm install` and a display.

use serde_json::Value;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
}

fn watch_reload(backend: &str) {
    let fixture = root().join("fixtures/counter");
    let dir = root().join("target/m2b").join(format!("counter-watch-{backend}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).unwrap();
    for f in ["package.json", "vite.config.js"] {
        std::fs::copy(fixture.join(f), dir.join(f)).unwrap();
    }
    std::os::unix::fs::symlink(fixture.join("node_modules"), dir.join("node_modules")).unwrap();
    let app = dir.join("src/App.svelte");
    std::fs::copy(fixture.join("src/App.svelte"), &app).unwrap();

    // Wait for the watch build's first build so the host starts against a settled bundle.
    let mut build = Command::new("node")
        .arg(root().join("packages/build/bin/svelters-build.mjs"))
        .arg("--watch")
        .arg(&dir)
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn watch build");
    let mut lines = BufReader::new(build.stderr.take().unwrap()).lines();
    loop {
        let l = lines.next().expect("watch build ended").unwrap();
        if l.contains("rebuilt in") {
            break;
        }
    }

    let report_path = root().join("target/m2b").join(format!("{backend}-watch.json"));
    let _ = std::fs::remove_file(&report_path);
    let editor = std::thread::spawn({
        let app = app.clone();
        move || {
            std::thread::sleep(Duration::from_secs(3));
            let t = Instant::now();
            let src = std::fs::read_to_string(&app).unwrap();
            std::fs::write(&app, src.replace("Count:", "Total:")).unwrap();
            t
        }
    });
    let out = Command::new(env!("CARGO_BIN_EXE_counter"))
        // GDK's Vulkan path under Xvfb (lavapipe) can assert in gdk_vulkan_context_surface_detach
        // when a window closes mid-present; cairo avoids the GPU path. An explicit GSK_RENDERER wins.
        .env("GSK_RENDERER", std::env::var("GSK_RENDERER").unwrap_or_else(|_| "cairo".into()))
        .args(["--backend", backend, "--timeout", "60", "--watch", "--reload-exit", "1", "--fixture"])
        .arg(&dir)
        .arg("--report")
        .arg(&report_path)
        .output()
        .expect("spawn counter");
    let t_done = Instant::now();
    let t_edit = editor.join().unwrap();
    let _ = build.kill();
    let _ = build.wait();

    let stderr = String::from_utf8_lossy(&out.stderr);
    let text = std::fs::read_to_string(&report_path).unwrap_or_else(|_| panic!("no report; stderr:\n{stderr}"));
    let r: Value = serde_json::from_str(&text).unwrap();
    assert!(out.status.success(), "exit {:?}; stderr:\n{stderr}\nreport: {r}", out.status);
    assert_eq!(r["errors"], serde_json::json!([]));
    assert_eq!(r["reloads"][0]["ok"], true, "{r}");
    // The old window is really gone from the toolkit, not only from the host's table (leak check).
    assert_eq!(r["native_toplevels"], 1, "native toplevels left after reload: {r}");
    // `dump` is one widget tree per toplevel: only the new window is left.
    let trees = r["dump"].as_array().expect("dump");
    assert_eq!(trees.len(), 1, "old window must be gone: {r}");
    let labels: Vec<&str> = trees[0]
        .as_array()
        .unwrap()
        .iter()
        .filter(|n| matches!(n["class"].as_str(), Some("GtkLabel" | "QLabel")))
        .map(|n| n["text"].as_str().unwrap())
        .collect();
    assert_eq!(labels, ["Total: 0"]);
    eprintln!(
        "{backend}: edit -> reloaded app reported {:?} (host load+teardown {:.0} ms)",
        t_done.duration_since(t_edit),
        r["reloads"][0]["ms"].as_f64().unwrap()
    );
}

#[test]
fn watch_reload_gtk() {
    watch_reload("gtk");
}

#[test]
fn watch_reload_qt() {
    watch_reload("qt");
}
