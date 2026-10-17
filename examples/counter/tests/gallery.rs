//! Widget gallery screenshot tests: one test per page of `fixtures/gallery-<toolkit>`.
//!
//! Needs a display and the built bundles: `pnpm -r build`, then
//! `xvfb-run -a cargo nextest run -p counter --test gallery`. Each test runs the `counter` binary
//! on the gallery bundle, clicks the page's sidebar button natively, then checks:
//! - the run is clean (no host errors, no driver failures, no stale handles);
//! - the native widget tree contains every listed native class;
//! - every `echo <name>: N` label on the page reads 0 (the app's own writes did not come back as
//!   change events: echo suppression for each two-way binding on the page);
//! - a non-empty screenshot exists at `target/gallery/<toolkit>-<page>.png`.
//!
//! Adding a page: put `NN-family.svelte` under `fixtures/gallery-<toolkit>/src/pages/` and add one
//! `gallery_page!` line below (fixtures/gallery-common/check-pages.mjs enforces both).

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Handles: window #1, outer box #2, sidebar box #3, sidebar button i = #(4 + i).
const SIDEBAR_FIRST_HANDLE: usize = 4;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn pages(tk: &str) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(root().join(format!("fixtures/gallery-{tk}/src/pages")))
        .expect("pages dir")
        .filter_map(|e| e.ok()?.file_name().to_str()?.strip_suffix(".svelte").map(str::to_string))
        .collect();
    v.sort();
    v
}

fn run_page(tk: &str, page: &str, classes: &[&str]) {
    let idx = pages(tk).iter().position(|p| p == page).unwrap_or_else(|| panic!("no page {page} in gallery-{tk}"));
    let dir = root().join("target/gallery");
    std::fs::create_dir_all(&dir).unwrap();
    let script = dir.join(format!("{tk}-{page}.json"));
    std::fs::write(
        &script,
        format!(r#"[{{"checkpoint":true}},{{"event":[{},"clicked",[]]}},{{"checkpoint":true}}]"#, SIDEBAR_FIRST_HANDLE + idx),
    )
    .unwrap();
    let png = dir.join(format!("{tk}-{page}.png"));
    let report = dir.join(format!("{tk}-{page}.report.json"));
    let _ = std::fs::remove_file(&report);
    let out = Command::new(env!("CARGO_BIN_EXE_counter"))
        .env("GSK_RENDERER", std::env::var("GSK_RENDERER").unwrap_or_else(|_| "cairo".into()))
        .args(["--backend", tk, "--fixture", &format!("gallery-{tk}"), "--timeout", "30"])
        .arg("--script")
        .arg(&script)
        .arg("--screenshot")
        .arg(&png)
        .arg("--report")
        .arg(&report)
        .output()
        .expect("spawn counter");
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    let r: Value = serde_json::from_str(&std::fs::read_to_string(&report).unwrap_or_else(|_| panic!("no report; stderr:\n{stderr}"))).unwrap();
    assert!(out.status.success(), "exit {:?}; stderr:\n{stderr}", out.status);
    assert_eq!(r["failures"], serde_json::json!([]), "driver failures");
    assert_eq!(r["errors"], serde_json::json!([]), "host errors");
    for bad in ["stale", "CRITICAL", "foreign handle"] {
        assert!(!stderr.contains(bad), "stderr mentions '{bad}':\n{stderr}");
    }
    let dump = r["dump"].as_array().expect("dump");
    let have: Vec<&str> = dump.iter().filter_map(|n| n["class"].as_str()).collect();
    for c in classes {
        assert!(have.contains(c), "page {page}: no native {c} in the widget tree: {have:?}");
    }
    for n in dump {
        let text = n["text"].as_str().unwrap_or("");
        if let Some(rest) = text.strip_prefix("echo ") {
            assert!(rest.ends_with(": 0"), "page {page}: '{text}': the app's own writes came back as change events");
        }
    }
    assert!(png.is_file() && std::fs::metadata(&png).unwrap().len() > 500, "screenshot {} missing or empty", png.display());
}

macro_rules! gallery_page {
    ($name:ident, $tk:literal, $page:literal, [$($class:literal),* $(,)?]) => {
        #[test]
        fn $name() {
            run_page($tk, $page, &[$($class),*]);
        }
    };
}

// ---- GTK pages (fixtures/gallery-gtk/src/pages) ----
gallery_page!(gtk_00_overview, "gtk", "00-overview", ["GtkEntry", "GtkButton", "GtkLabel"]);
gallery_page!(gtk_01_widget, "gtk", "01-widget", ["GtkLabel", "GtkButton", "GtkEntry", "GtkBox"]);

gallery_page!(gtk_05_containers, "gtk", "05-containers", ["GtkCenterBox", "GtkPaned", "GtkFrame", "GtkExpander", "GtkRevealer", "GtkAspectFrame", "GtkOverlay", "GtkWindowHandle", "GtkViewport", "GtkScrolledWindow", "GtkSearchBar"]);

// ---- Qt pages (fixtures/gallery-qt/src/pages) ----
gallery_page!(qt_00_overview, "qt", "00-overview", ["QLineEdit", "QPushButton", "QLabel"]);
gallery_page!(qt_01_widget, "qt", "01-widget", ["QWidget", "QPushButton", "QLineEdit", "QLabel"]);
gallery_page!(qt_05_containers, "qt", "05-containers", ["QFrame", "QGroupBox", "QScrollArea", "QSplitter", "QStackedWidget", "QTabWidget", "QToolBox"]);
