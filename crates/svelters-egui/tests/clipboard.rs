//! DX2: `svelters:clipboard` and `svelters:shell` on egui, under kittest. The fixture is
//! `fixtures/counter-egui/src/Clipboard.svelte`, compiled live. The host's platform is a
//! `MemoryClipboard` (the test sink: nothing reaches the system clipboard) and a `RecordingShell`
//! (no file manager starts).

use egui_kittest::kittest::Queryable;
use std::sync::Arc;
use svelters_egui::harness::TestApp;
use svelters_host::platform::{ClipboardBackend, ClipboardKind, MemoryClipboard, MimeData, Platform, RecordingShell, ShellCall};
use svelters_host::HostConfig;

fn start() -> (TestApp, Arc<MemoryClipboard>, Arc<RecordingShell>) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/counter-egui/src/Clipboard.svelte");
    let clip = Arc::new(MemoryClipboard::new());
    let shell = Arc::new(RecordingShell::new());
    let cfg = HostConfig { echo_console: false, echo_errors: false, platform: Platform::new(clip.clone(), shell.clone()), ..HostConfig::default() };
    let mut app = TestApp::from_source_with(path, 1.0 / 60.0, cfg).unwrap();
    app.run();
    (app, clip, shell)
}

fn has(app: &TestApp, label: &str) -> bool {
    app.harness.query_by_label(label).is_some()
}

#[test]
fn table_copies_as_tsv_html_and_json_and_pastes_into_text_edit() {
    let (mut app, clip, _) = start();
    app.harness.get_by_label("Copy table").click();
    app.run();
    assert!(has(&app, "Status: copied 3"), "the copy wrote three formats");

    // The HTML sink: every format is on the clipboard, in the writer's order.
    let got = clip.contents(ClipboardKind::Clipboard);
    assert_eq!(got.types().collect::<Vec<_>>(), ["text/plain", "text/html", "application/x-cad-refs+json"]);
    assert_eq!(got.get_str("text/html"), Some("<table><tr><td>R1</td><td>10k</td></tr><tr><td>C1</td><td>100n</td></tr></table>"));
    assert_eq!(got.get_str("application/x-cad-refs+json"), Some(r#"["R1","C1"]"#));
    // The host-side API a paste command (EV3) uses reads the same data.
    assert_eq!(app.host().platform().clipboard.read(ClipboardKind::Clipboard, &["text/*"]).unwrap().len(), 2);

    // TSV into a text-edit: a paste of the clipboard's text.
    let tsv = got.plain_text().unwrap().to_string();
    assert_eq!(tsv, "R1\t10k\nC1\t100n");
    app.harness.get_by_label("Sheet").click();
    app.run();
    app.harness.event(egui::Event::Paste(tsv));
    app.run();
    assert!(has(&app, "Rows: 2, cells: R1 10k C1 100n"), "the TSV arrived in the text-edit");

    // A typed read: the app type wins over text/plain.
    app.harness.get_by_label("Paste refs").click();
    app.run();
    assert!(has(&app, "Refs: application/x-cad-refs+json: R1,C1"));
    assert!(app.errors().is_empty(), "{:?}", app.errors());
}

#[test]
fn data_written_from_rust_reads_in_js_by_preference() {
    let (mut app, clip, _) = start();
    // A Rust panel copies text only: the JS read falls through to text/plain.
    clip.write(ClipboardKind::Clipboard, MimeData::text("plain words")).unwrap();
    app.harness.get_by_label("Paste refs").click();
    app.run();
    assert!(has(&app, "Refs: text/plain: plain words"));
    // A Rust drag payload type (WG4b `drag-type`) on the clipboard matches the `application/*` pattern.
    clip.write(ClipboardKind::Clipboard, MimeData::single("application/x-part", "P-7").with("text/plain", "P-7 text")).unwrap();
    app.harness.get_by_label("Paste refs").click();
    app.run();
    assert!(has(&app, "Refs: application/x-part: P-7"));
    assert!(app.errors().is_empty(), "{:?}", app.errors());
}

#[test]
fn reveal_and_open_go_to_the_shell_backend() {
    let (mut app, clip, shell) = start();
    let file = std::env::temp_dir().join(format!("svelters-dx2-{}.txt", std::process::id()));
    std::fs::write(&file, "x").unwrap();
    clip.write(ClipboardKind::Clipboard, MimeData::text(file.to_string_lossy())).unwrap();
    app.harness.get_by_label("Reveal").click();
    app.run();
    assert!(has(&app, "Status: shell ok"));
    app.harness.get_by_label("Open").click();
    app.run();
    assert_eq!(shell.calls(), [ShellCall::Reveal(file.clone()), ShellCall::OpenPath(file.clone())]);

    // A missing file rejects (and is not recorded).
    std::fs::remove_file(&file).unwrap();
    app.harness.get_by_label("Reveal").click();
    app.run();
    assert!(has(&app, "Status: shell error"));
    assert_eq!(shell.calls().len(), 2);
    assert!(app.errors().is_empty(), "{:?}", app.errors());
}
