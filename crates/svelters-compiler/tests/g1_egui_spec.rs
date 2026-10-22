//! G1: the egui spec (generated from crates/svelters-egui/src/kinds/) as a compile target:
//! the counter twin compiles, `bind:` on widget-changed props lowers onto observation, layout
//! props are setters in `style=` (D3), `<style>` is left to the backend's CSS engine (D9 on egui,
//! CS5e), and set-only props refuse a two-way binding.

use svelters_compiler::compile::{CompileOptions, CompileResult, compile};
use svelters_compiler::diag::Severity;
use svelters_compiler::semantics::SpecBackend;

fn run(src: &str) -> CompileResult {
    let b = SpecBackend::builtin("egui").unwrap();
    compile(src, &CompileOptions::new("App.svelte", &b))
}

fn errors(r: &CompileResult) -> Vec<String> {
    r.diagnostics.iter().filter(|d| d.severity == Severity::Error).map(|d| format!("{}: {}", d.code, d.message)).collect()
}

fn js(src: &str) -> String {
    let r = run(src);
    assert!(errors(&r).is_empty(), "{:?}", errors(&r));
    r.js.unwrap()
}

#[test]
fn counter_twin_compiles_with_bindings_and_style_setters() {
    let src = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/counter-egui/src/App.svelte")).unwrap();
    let out = js(&src);
    assert!(out.contains(r#"$n.toplevel("window", "present")"#), "{out}");
    assert!(out.contains(r#"$n.bind_prop($$n6, "text", () => $.get(name), ($$value) => { $.set(name, $$value); }, "changed", 0);"#), "{out}");
    assert!(out.contains(r#""value""#) && out.contains(r#""checked""#), "{out}");
    // D9 on egui (CS5e): `.row { flex-direction: row; gap: 8px }` is the engine's, not setters
    assert!(!out.contains(r#""flex-direction""#), "{out}");
}

#[test]
fn unknown_props_and_tags_are_errors() {
    let r = run("<window><box><label colour=\"red\">x</label></box></window>");
    assert!(!errors(&r).is_empty());
    let r = run("<window><push-button /></window>");
    assert!(!errors(&r).is_empty());
}

#[test]
fn set_only_props_have_no_two_way_binding() {
    let r = run("<script>let t = $state('');</script><window><box><label bind:text={t} /></box></window>");
    let all: Vec<String> = r.diagnostics.iter().map(|d| format!("{:?} {}: {}", d.severity, d.code, d.message)).collect();
    assert!(!errors(&r).is_empty(), "{all:?}");
}

#[test]
fn inline_setter_values_are_checked() {
    let r = run("<window><box style=\"flex-direction: sideways\" /></window>");
    assert!(!errors(&r).is_empty(), "an enum value outside the table is refused");
}

/// D9 on egui (CS5e): `<style>` rules are not matched at compile time, so states, combinators
/// and `%` in setter properties compile (the engine resolves them at run time).
#[test]
fn style_rules_are_left_to_the_engine() {
    let out = js("<window><box class=\"a\"><button /></box></window><style>.a > button:hover { width: 50%; } .a { gap: 4px; }</style>");
    assert!(!out.contains(r#""gap""#) && !out.contains(r#""width""#), "{out}");
}
