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

fn run_page(tk: &str, page: &str, classes: &[&str], colors: &[(&str, f64)]) {
    run_page_clicks(tk, page, classes, &[], "", colors);
}

/// `run_page` with extra native clicks (handles of buttons the page creates) after the sidebar click;
/// `tag` is appended to the artifact names. Returns the dump so a test can check more.
fn run_page_clicks(tk: &str, page: &str, classes: &[&str], clicks: &[usize], tag: &str, colors: &[(&str, f64)]) -> Vec<Value> {
    let idx = pages(tk).iter().position(|p| p == page).unwrap_or_else(|| panic!("no page {page} in gallery-{tk}"));
    let dir = root().join("target/gallery");
    std::fs::create_dir_all(&dir).unwrap();
    let script = dir.join(format!("{tk}-{page}{tag}.json"));
    let extra: String = clicks.iter().map(|h| format!(r#",{{"event":[{h},"clicked",[]]}},{{"checkpoint":true}}"#)).collect();
    std::fs::write(
        &script,
        format!(r#"[{{"checkpoint":true}},{{"event":[{},"clicked",[]]}},{{"checkpoint":true}}{extra}]"#, SIDEBAR_FIRST_HANDLE + idx),
    )
    .unwrap();
    let png = dir.join(format!("{tk}-{page}{tag}.png"));
    let report = dir.join(format!("{tk}-{page}{tag}.report.json"));
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
    let dump = r["dump_all"].as_array().expect("dump_all");
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
    if !colors.is_empty() {
        check_colors(&png, colors);
    }
    dump.clone()
}

/// Toplevel dump nodes of `class`: (modal, transient, visible).
fn toplevel_flags(dump: &[Value], class: &str) -> Vec<(bool, bool, bool)> {
    dump.iter()
        .filter(|n| n["class"] == class)
        .map(|n| (n["modal"].as_bool().unwrap_or(false), n["transient"].as_bool().unwrap_or(false), n["visible"].as_bool().unwrap_or(false)))
        .collect()
}

/// Styling pages: the screenshot must contain each colour (exact pixels) in at least the given
/// fraction of the image. Needs python3 with PIL; skipped with a note when they are missing.
fn check_colors(png: &Path, colors: &[(&str, f64)]) {
    let out = Command::new("python3").arg(root().join("tools/dev/png_colors.py")).arg(png).args(colors.iter().map(|c| c.0)).output();
    let Ok(out) = out else {
        eprintln!("skipped colour check: python3 not available");
        return;
    };
    if !out.status.success() {
        eprintln!("skipped colour check: {}", String::from_utf8_lossy(&out.stderr));
        return;
    }
    let found: Value = serde_json::from_slice(&out.stdout).expect("png_colors.py prints JSON");
    for (hex, min) in colors {
        let got = found[hex].as_f64().unwrap_or(0.0);
        assert!(got >= *min, "{}: only {:.4} of the pixels are #{hex}, expected at least {min} ({found})", png.display(), got);
    }
}

macro_rules! gallery_page {
    ($name:ident, $tk:literal, $page:literal, [$($class:literal),* $(,)?]) => {
        gallery_page!($name, $tk, $page, [$($class),*], []);
    };
    ($name:ident, $tk:literal, $page:literal, [$($class:literal),* $(,)?], [$(($hex:literal, $min:expr)),* $(,)?]) => {
        #[test]
        fn $name() {
            run_page($tk, $page, &[$($class),*], &[$(($hex, $min)),*]);
        }
    };
}

// ---- GTK pages (fixtures/gallery-gtk/src/pages) ----
gallery_page!(gtk_00_overview, "gtk", "00-overview", ["GtkEntry", "GtkButton", "GtkLabel"]);
gallery_page!(gtk_01_widget, "gtk", "01-widget", ["GtkLabel", "GtkButton", "GtkEntry", "GtkBox"]);

gallery_page!(gtk_05_containers, "gtk", "05-containers", ["GtkCenterBox", "GtkPaned", "GtkFrame", "GtkExpander", "GtkRevealer", "GtkAspectFrame", "GtkOverlay", "GtkWindowHandle", "GtkViewport", "GtkScrolledWindow", "GtkSearchBar"]);
gallery_page!(gtk_06_layout, "gtk", "06-layout", ["GtkStack", "GtkStackSwitcher", "GtkStackSidebar", "GtkNotebook", "GtkFlowBox", "GtkFlowBoxChild", "GtkListBox", "GtkActionBar", "GtkGrid", "GtkFixed", "GtkOverlay"]);
gallery_page!(gtk_03_text, "gtk", "03-text", ["GtkEntry", "GtkPasswordEntry", "GtkSearchEntry", "GtkText", "GtkEditableLabel", "GtkLabel", "GtkInscription", "GtkTextView", "GtkShortcutLabel"]);
gallery_page!(gtk_09_display, "gtk", "09-display", ["GtkImage", "GtkSpinner", "GtkSeparator", "GtkPicture", "GtkDrawingArea", "GtkGraphicsOffload", "GtkCalendar"]);
gallery_page!(gtk_02_buttons, "gtk", "02-buttons", ["GtkButton", "GtkToggleButton", "GtkCheckButton", "GtkSwitch", "GtkLinkButton", "GtkLockButton", "GtkMenuButton", "GtkPopover", "GtkScaleButton", "GtkVolumeButton"]);
gallery_page!(gtk_07_bars, "gtk", "07-bars", ["GtkHeaderBar", "GtkWindowControls"]);
gallery_page!(gtk_08_dialogs, "gtk", "08-dialogs", ["GtkMenuButton", "GtkPopover", "GtkEmojiChooser", "GtkAboutDialog"]);

/// G9: toplevels declared in `if` blocks are real windows, transient for the main one and shown;
/// dialogs are modal by default.
#[test]
fn gtk_08_dialogs_toplevels() {
    let dump = run_page_clicks("gtk", "08-dialogs", &["GtkAboutDialog"], &[], "-toplevels", &[]);
    assert_eq!(toplevel_flags(&dump, "GtkAboutDialog"), vec![(true, true, true)], "about dialog: modal, transient, visible");
    let mut windows = toplevel_flags(&dump, "GtkWindow");
    windows.sort();
    assert_eq!(windows, vec![(false, false, true), (false, true, true)], "main window, then the tool window (modeless, transient, visible)");
}

gallery_page!(gtk_10_menus, "gtk", "10-menus", ["GtkPopoverMenuBar", "GtkBox", "GtkButton", "GtkLabel"]);
// Styling pages (X6): the light page paints the card (--sv-surface) and the accent button; the dark
// variant switches the scheme and overrides the accent token, so its pixels are the dark palette.
gallery_page!(gtk_10_styling, "gtk", "10-styling", ["GtkBox", "GtkLabel", "GtkButton", "GtkEntry"], [("f6f5f4", 0.05), ("ffffff", 0.02), ("3584e4", 0.005)]);
gallery_page!(gtk_11_styling_dark, "gtk", "11-styling-dark", ["GtkBox", "GtkLabel", "GtkButton", "GtkEntry"], [("242424", 0.05), ("303030", 0.02), ("e5a50a", 0.005)]);

// X8: event controllers as elements; the recording canvas (the screenshot holds the three colours it draws).
gallery_page!(gtk_12_events, "gtk", "12-events", ["GtkEntry", "GtkLabel", "GtkBox"]);
gallery_page!(gtk_13_canvas, "gtk", "13-canvas", ["GtkDrawingArea", "GtkButton", "GtkLabel"], [("e01b24", 0.006), ("3584e4", 0.003), ("33d17a", 0.001)]);
gallery_page!(gtk_14_arrange, "gtk", "14-arrange", ["GtkListBox", "GtkFlowBox", "GtkListBoxRow", "GtkLabel"]);

gallery_page!(gtk_04_ranges, "gtk", "04-ranges", ["GtkProgressBar", "GtkLevelBar", "GtkScale", "GtkSpinButton", "GtkScrollbar"]);

// ---- Qt pages (fixtures/gallery-qt/src/pages) ----
gallery_page!(qt_00_overview, "qt", "00-overview", ["QLineEdit", "QPushButton", "QLabel"]);
gallery_page!(qt_01_widget, "qt", "01-widget", ["QWidget", "QPushButton", "QLineEdit", "QLabel"]);
gallery_page!(qt_02_buttons, "qt", "02-buttons", ["QPushButton", "QCheckBox", "QRadioButton", "QToolButton", "QCommandLinkButton"]);
gallery_page!(qt_04_ranges, "qt", "04-ranges", ["QSlider", "QDial", "QScrollBar", "QSpinBox", "QDoubleSpinBox", "QProgressBar", "QLCDNumber"]);
gallery_page!(qt_05_containers, "qt", "05-containers", ["QFrame", "QGroupBox", "QScrollArea", "QSplitter", "QStackedWidget", "QTabWidget", "QToolBox"]);
gallery_page!(qt_06_layout, "qt", "06-layout", ["QMdiArea", "QDockWidget", "QWidget"]);
gallery_page!(qt_07_selection, "qt", "07-selection", ["QComboBox", "QFontComboBox", "QListWidget"]);
gallery_page!(qt_08_bars, "qt", "08-bars", ["QTabBar", "QDialogButtonBox", "QStatusBar", "QMenuBar", "QToolBar"]);
gallery_page!(qt_10_display, "qt", "10-display", ["QCalendarWidget", "QDateEdit", "QTimeEdit", "QDateTimeEdit"]);
gallery_page!(qt_09_dialogs, "qt", "09-dialogs", ["QLabel", "QMessageBox"]);

/// G9: every dialog kind opens non-blockingly from an `if` block: transient for the main window,
/// window-modal (`open()`), visible.
#[test]
fn qt_09_dialogs_all_kinds() {
    // the page opens every kind when it mounts (the "open all dialogs" button reopens them)
    let dump = run_page_clicks("qt", "09-dialogs", &["QMessageBox"], &[], "-all", &[]);
    for c in ["QMessageBox", "QInputDialog", "QProgressDialog", "QColorDialog", "QFontDialog", "QFileDialog", "QDialog"] {
        assert_eq!(toplevel_flags(&dump, c), vec![(true, true, true)], "{c}: modal, transient, visible");
    }
}
gallery_page!(qt_11_menus, "qt", "11-menus", ["QMenuBar", "QToolBar", "QPushButton", "QLabel"]);
gallery_page!(qt_11_styling, "qt", "11-styling", ["QWidget", "QLabel", "QPushButton", "QLineEdit"], [("f6f5f4", 0.05), ("ffffff", 0.02), ("3584e4", 0.005)]);
gallery_page!(qt_12_styling_dark, "qt", "12-styling-dark", ["QWidget", "QLabel", "QPushButton", "QLineEdit"], [("242424", 0.05), ("303030", 0.02), ("e5a50a", 0.005)]);
gallery_page!(qt_12_events, "qt", "12-events", ["QLineEdit", "QLabel", "QWidget"]);
gallery_page!(qt_13_canvas, "qt", "13-canvas", ["QWidget", "QPushButton", "QLabel"], [("e01b24", 0.006), ("3584e4", 0.003), ("33d17a", 0.001)]);
gallery_page!(qt_03_text, "qt", "03-text", ["QLineEdit", "QLabel", "QPlainTextEdit", "QTextEdit", "QTextBrowser", "QKeySequenceEdit"]);

/// G12: the page edits a QTextDocument through a borrowed handle (`browser.call('document')`).
#[test]
fn qt_03_text_document_is_reached_through_a_borrowed_handle() {
    let dump = run_page_clicks("qt", "03-text", &["QTextBrowser"], &[], "-document", &[]);
    assert!(dump.iter().any(|n| n["text"] == "document blocks: 2"), "the borrowed QTextDocument counted two blocks");
}
