//! Snapshot tests of the rune transform: every `tests/runes/<case>/` holds `input.svelte` (a
//! component, compiled for gtk) or `input.svelte.js` (a module), optionally `dev` (an empty file:
//! compile with `dev: true`), and the expected `output.js` — or `diagnostics.txt` when
//! compilation must fail (`code: message` per line, then `@line:column`).
//! `UPDATE=1 cargo test -p svelters-compiler --test runes` rewrites the expectations (review the
//! diff!).

use std::fs;
use std::path::Path;
use svelters_compiler::compile::{CompileOptions, CompileResult, ModuleOptions, compile, compile_module};
use svelters_compiler::semantics::SpecBackend;

fn run_case(dir: &Path) -> Result<(), String> {
    let dev = dir.join("dev").exists();
    let (src, res): (String, CompileResult) = if dir.join("input.svelte.js").exists() {
        let src = fs::read_to_string(dir.join("input.svelte.js")).unwrap();
        let res = compile_module(&src, &ModuleOptions { filename: "input.svelte.js".into(), dev, hmr: false });
        (src, res)
    } else {
        let src = fs::read_to_string(dir.join("input.svelte")).unwrap();
        let backend = SpecBackend::builtin("gtk").unwrap();
        let res = compile(&src, &CompileOptions { dev, ..CompileOptions::new("input.svelte", &backend) });
        (src, res)
    };
    let _ = src;
    let (file, got) = match &res.js {
        Some(js) if !res.has_errors() => ("output.js", js.clone()),
        _ => ("diagnostics.txt", res.diagnostics.iter().map(|d| format!("{}: {} @{}:{}\n", d.code, d.message, d.line, d.column)).collect::<String>()),
    };
    let path = dir.join(file);
    let other = dir.join(if file == "output.js" { "diagnostics.txt" } else { "output.js" });
    if std::env::var("UPDATE").as_deref() == Ok("1") {
        fs::write(&path, &got).unwrap();
        let _ = fs::remove_file(other);
        return Ok(());
    }
    match fs::read_to_string(&path) {
        Ok(exp) if exp == got => Ok(()),
        Ok(exp) => Err(format!("{}: {file} differs\n--- expected\n{exp}\n--- got\n{got}", dir.display())),
        Err(_) => Err(format!("{}: no {file}; got\n{got}", dir.display())),
    }
}

#[test]
fn rune_snapshots() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/runes");
    let mut dirs: Vec<_> = fs::read_dir(&root).unwrap().map(|e| e.unwrap().path()).filter(|p| p.is_dir()).collect();
    dirs.sort();
    let failures: Vec<String> = dirs.iter().filter_map(|d| run_case(d).err()).collect();
    assert!(!dirs.is_empty());
    if !failures.is_empty() {
        panic!("{} of {} rune snapshot(s) failed:\n{}", failures.len(), dirs.len(), failures.join("\n"));
    }
}

/// Every generated module must at least be syntactically valid JavaScript (checked with our own
/// acorn port).
#[test]
fn rune_outputs_parse() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/runes");
    for d in fs::read_dir(&root).unwrap() {
        let p = d.unwrap().path().join("output.js");
        let Ok(js) = fs::read_to_string(&p) else { continue };
        let mut comments = Vec::new();
        let opts = svelters_compiler::acorn::Options::default();
        if let Err(e) = svelters_compiler::acorn::parse(&js, &opts, &mut comments) {
            panic!("{}: {}", p.display(), e.message);
        }
    }
}
