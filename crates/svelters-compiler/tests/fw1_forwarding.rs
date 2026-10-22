//! FW1: `on:` forwarding in runes mode (ARCH-EMBED §8.7; CADATOMIC SV9 per C-2). Golden cases:
//! every `tests/fw1/<case>/` holds `input.svelte` (compiled for gtk, or for the backend named in
//! an optional `backend` file) and `expected.txt`: the diagnostics (severity, code, message,
//! help, position), then the generated module when there is no error.
//! `UPDATE=1 cargo test -p svelters-compiler --test fw1_forwarding` rewrites the expectations
//! (review the diff!).

use std::fs;
use std::path::Path;
use svelters_compiler::compile::{CompileOptions, compile};
use svelters_compiler::diag::Severity;
use svelters_compiler::semantics::SpecBackend;

fn render(dir: &Path) -> String {
    let src = fs::read_to_string(dir.join("input.svelte")).unwrap();
    let backend = fs::read_to_string(dir.join("backend")).map(|s| s.trim().to_string()).unwrap_or_else(|_| "gtk".into());
    let b = SpecBackend::builtin(&backend).unwrap();
    let res = compile(&src, &CompileOptions::new("input.svelte", &b));
    let mut out = format!("backend: {backend}\n\n## diagnostics\n");
    for d in &res.diagnostics {
        let sev = match d.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Note => "note",
        };
        out.push_str(&format!("{sev}[{}] @{}:{}: {}\n", d.code, d.line, d.column, d.message));
        if let Some(h) = &d.help {
            out.push_str(&format!("  help: {h}\n"));
        }
    }
    if let Some(js) = res.js.as_ref().filter(|_| !res.has_errors()) {
        out.push_str("\n## output.js\n");
        out.push_str(js);
    }
    out
}

#[test]
fn fw1_goldens() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fw1");
    let mut dirs: Vec<_> = fs::read_dir(&root).unwrap().map(|e| e.unwrap().path()).filter(|p| p.is_dir()).collect();
    dirs.sort();
    assert!(!dirs.is_empty());
    let mut failures = Vec::new();
    for dir in &dirs {
        let got = render(dir);
        let path = dir.join("expected.txt");
        if std::env::var("UPDATE").as_deref() == Ok("1") {
            fs::write(&path, &got).unwrap();
            continue;
        }
        match fs::read_to_string(&path) {
            Ok(exp) if exp == got => {}
            Ok(exp) => failures.push(format!("{}: expected.txt differs\n--- expected\n{exp}\n--- got\n{got}", dir.display())),
            Err(_) => failures.push(format!("{}: no expected.txt; got\n{got}", dir.display())),
        }
    }
    if !failures.is_empty() {
        panic!("{} of {} FW1 golden(s) failed:\n{}", failures.len(), dirs.len(), failures.join("\n"));
    }
}

fn compile_gtk(src: &str) -> svelters_compiler::compile::CompileResult {
    let b = SpecBackend::builtin("gtk").unwrap();
    compile(src, &CompileOptions::new("App.svelte", &b))
}

/// The note is reported once per component, however many bare `on:` it has, and is not an error
/// or a warning (a forwarding component builds cleanly, also under `--strict-native`).
#[test]
fn the_forwarding_note_is_reported_once_and_fails_nothing() {
    let r = compile_gtk("<script>import Child from './Child.svelte';</script><box><button on:clicked /><button on:clicked /><Child on:accepted /></box>");
    assert!(!r.has_errors());
    let notes: Vec<_> = r.diagnostics.iter().filter(|d| d.code == "legacy_forwarding_lowered").collect();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].severity, Severity::Note);
    assert!(r.diagnostics.iter().all(|d| d.severity == Severity::Note), "{:?}", r.diagnostics);
}

/// An unknown event is still checked against the spec for the bare form.
#[test]
fn a_bare_on_with_an_unknown_event_is_an_error() {
    let r = compile_gtk("<box><button on:clickd /></box>");
    let codes: Vec<_> = r.diagnostics.iter().filter(|d| d.severity == Severity::Error).map(|d| d.code).collect();
    assert_eq!(codes, vec!["unknown_event"]);
}
