//! Q3w: the Rust port of Svelte's CSS pipeline (Q3) through `compile()` (`CompileOptions::scoped_css`)
//! and `svelters-compile --css`. The scoped CSS and its `css_*` diagnostics must equal svelte@5.57.2's
//! (the checked-in corpus of `tests/css_scope.rs`), for HTML-syntax samples (the DOM spec) and for
//! this repo's native components compiled for their backend.

use serde_json::Value;
use svelters_compiler::compile::{CompileOptions, CompileResult, compile};
use svelters_compiler::diag::Severity;
use svelters_compiler::semantics::SpecBackend;

fn corpus() -> Value {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/css_scope/corpus.json");
    serde_json::from_str(&std::fs::read_to_string(path).expect("corpus.json")).expect("json")
}

fn case<'c>(corpus: &'c Value, id: &str) -> &'c Value {
    corpus["cases"].as_array().unwrap().iter().find(|c| c["id"] == id).unwrap_or_else(|| panic!("no corpus case {id}"))
}

fn backend(name: &str) -> SpecBackend {
    if name == "dom" {
        SpecBackend::from_path(concat!(env!("CARGO_MANIFEST_DIR"), "/../../metadata/semantics/dom.json")).expect("dom spec")
    } else {
        SpecBackend::builtin(name).expect("builtin spec")
    }
}

fn utf16(src: &str, byte: u32) -> u64 {
    src[..byte as usize].encode_utf16().count() as u64
}

fn compile_case(c: &Value, backend_name: &str) -> CompileResult {
    let src = c["source"].as_str().unwrap();
    let sem = backend(backend_name);
    let filename = c["options"]["filename"].as_str().unwrap_or("input.svelte").to_string();
    let mut opts = CompileOptions::new(filename, &sem);
    opts.dev = c["options"]["dev"] == true;
    opts.css_scope = Some(c["hash"].as_str().unwrap_or("svelte-xyz").to_string());
    opts.scoped_css = true;
    compile(src, &opts)
}

/// The `css_*` diagnostics as the corpus writes warnings (UTF-16 offsets).
fn css_warnings(src: &str, res: &CompileResult) -> Vec<Value> {
    res.diagnostics
        .iter()
        .filter(|d| d.code.starts_with("css_") && d.severity == Severity::Warning)
        .map(|d| serde_json::json!({"code": d.code, "message": d.message, "start": utf16(src, d.start), "end": utf16(src, d.end)}))
        .collect()
}

fn assert_same_css(c: &Value, res: &CompileResult) {
    let id = c["id"].as_str().unwrap();
    let src = c["source"].as_str().unwrap();
    let css = res.css.as_ref().unwrap_or_else(|| panic!("{id}: no css; diagnostics {:?}", res.diagnostics));
    assert_eq!(Some(css.code.as_str()), c["external"]["code"].as_str(), "{id}: css.code");
    assert_eq!(Value::Bool(css.has_global), c["external"]["hasGlobal"], "{id}: hasGlobal");
    assert_eq!(css.hash, c["hash"].as_str().unwrap_or("svelte-xyz"), "{id}: hash");
    let expected = c["external"]["warnings"].as_array().cloned().unwrap_or_default();
    assert_eq!(css_warnings(src, res), expected, "{id}: css_* warnings");
}

/// Svelte's own samples (HTML syntax) through `compile()` with the DOM spec.
#[test]
fn compile_scopes_svelte_samples_like_svelte() {
    let corpus = corpus();
    for id in [
        "css/basic/input.svelte",
        "css/animations/input.svelte",
        "css/child-combinator/input.svelte",
        "css/class-directive/input.svelte",
        "css/descendant-selector-unmatched/input.svelte",
        "css/global/input.svelte",
        "css/unused-selector/input.svelte",
    ] {
        let c = case(&corpus, id);
        assert_same_css(c, &compile_case(c, "dom"));
    }
}

/// This repo's native components (native element syntax) through `compile()` for their backend:
/// the same CSS as stock Svelte gives for the HTML-syntax parse of the same source.
#[test]
fn compile_scopes_native_components_like_svelte() {
    let corpus = corpus();
    for (id, be) in [
        ("repo/fixtures/style-precedence/src/App.svelte", "gtk"),
        ("repo/fixtures/gallery-gtk/src/StyledDemo.svelte", "gtk"),
        ("repo/fixtures/gallery-qt/src/StyledDemo.svelte", "qt"),
        ("repo/fixtures/todo-mac/src/App.svelte", "appkit"),
        ("repo/fixtures/counter-egui/src/App.svelte", "egui"),
    ] {
        let c = case(&corpus, id);
        let res = compile_case(c, be);
        assert_same_css(c, &res);
        // the hash is the elements' scope class, as with `--css-scope` alone
        assert!(res.js.as_deref().unwrap_or_else(|| panic!("{id}: {:?}", res.diagnostics)).contains("svelte-xyz"), "{id}: scope class in the JS");
    }
}

/// `@native` blocks are lifted out before scoping (as the Vite plugin's `extractNative` does), so
/// their raw toolkit rules are neither scoped nor pruned; positions of everything else hold.
#[test]
fn compile_blanks_native_blocks() {
    let corpus = corpus();
    let c = case(&corpus, "repo/fixtures/styles/src/App.svelte");
    let res = compile_case(c, "gtk");
    let css = res.css.as_ref().unwrap_or_else(|| panic!("no css: {:?}", res.diagnostics));
    assert!(!css.code.contains("@native") && !css.code.contains("min-height"), "{}", css.code);
    assert!(css.code.contains(".app.svelte-xyz {"), "{}", css.code);
    assert!(css_warnings(c["source"].as_str().unwrap(), &res).iter().all(|w| !w["message"].as_str().unwrap().contains("text-button")));
}

/// Without `scoped_css` nothing changes: no `css`, no `css_*` diagnostics, and no scope class
/// without `css_scope`.
#[test]
fn compile_without_css_flag_is_unchanged() {
    let corpus = corpus();
    let c = case(&corpus, "repo/fixtures/style-precedence/src/App.svelte");
    let sem = backend("gtk");
    let filename = "fixtures/style-precedence/src/App.svelte";
    let plain = compile(c["source"].as_str().unwrap(), &CompileOptions::new(filename, &sem));
    assert!(plain.css.is_none());
    assert!(plain.diagnostics.iter().all(|d| !d.code.starts_with("css_")));
    // default hash: Svelte's `cssHash` of the filename, which also scopes the elements
    let mut opts = CompileOptions::new(filename, &sem);
    opts.scoped_css = true;
    let res = compile(c["source"].as_str().unwrap(), &opts);
    let css = res.css.unwrap();
    assert_eq!(Some(css.hash.as_str()), c["default_hash_filename"].as_str());
    assert!(res.js.unwrap().contains(&css.hash));
    assert!(!plain.js.unwrap().contains(&css.hash));
}

/// Every CSS error of the corpus is a compile error with Svelte's code, message and span.
#[test]
fn compile_reports_css_errors() {
    let corpus = corpus();
    let mut n = 0;
    for c in corpus["cases"].as_array().unwrap().iter().filter(|c| !c["error"].is_null()) {
        let id = c["id"].as_str().unwrap();
        let src = c["source"].as_str().unwrap();
        let res = compile_case(c, "dom");
        assert!(res.js.is_none() && res.css.is_none(), "{id}: compiled");
        let e = &c["error"];
        let d = res.diagnostics.iter().find(|d| d.severity == Severity::Error && d.code == e["code"]).unwrap_or_else(|| panic!("{id}: expected {e}, got {:?}", res.diagnostics));
        assert_eq!(d.message, e["message"].as_str().unwrap(), "{id}");
        assert_eq!((utf16(src, d.start), utf16(src, d.end)), (e["start"].as_u64().unwrap(), e["end"].as_u64().unwrap()), "{id}");
        n += 1;
    }
    assert_eq!(n, 45);
}

/// `svelters-compile --css --json`: the JSON's `css` is Svelte's.
#[test]
fn cli_css_flag() {
    let corpus = corpus();
    let id = "repo/fixtures/gallery-gtk/src/StyledDemo.svelte";
    let c = case(&corpus, id);
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_svelters-compile"))
        .args(["--backend", "gtk", "--css", "--css-scope", "svelte-xyz", "--json", "--filename", "StyledDemo.svelte", "-"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("svelters-compile");
    use std::io::Write;
    child.stdin.take().unwrap().write_all(c["source"].as_str().unwrap().as_bytes()).unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stdout));
    let res: Value = serde_json::from_slice(&out.stdout).expect("json");
    assert_eq!(res["css"]["code"], c["external"]["code"]);
    assert_eq!(res["css"]["hash"], "svelte-xyz");
    assert_eq!(res["css"]["hasGlobal"], c["external"]["hasGlobal"]);
    assert_eq!(res["css"]["injected"], false);
    assert!(res["css"]["scopedElements"].as_array().is_some_and(|a| !a.is_empty()));
    assert!(res["js"].as_str().unwrap().contains("svelte-xyz"));
    // DZ0b2: the map to the `<style>` text (`out,src,len` runs) that the Vite plugin hands
    // `registerStyles` in dev, as the Rust loader does
    let map = res["css"]["map"].as_str().expect("css.map");
    let code = res["css"]["code"].as_str().unwrap();
    let src = c["source"].as_str().unwrap();
    let style = &src[src.find("<style").unwrap()..];
    let style = &style[style.find('>').unwrap() + 1..];
    assert!(!map.is_empty());
    for run in map.split(';') {
        let n: Vec<usize> = run.split(',').map(|x| x.parse().unwrap()).collect();
        assert_eq!(&code[n[0]..n[0] + n[2]], &style[n[1]..n[1] + n[2]], "run {run}: untouched style text");
    }
}
