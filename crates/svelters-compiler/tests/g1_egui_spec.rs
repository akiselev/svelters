//! G1: the egui spec (generated from crates/svelters-egui/src/kinds/) as a compile target:
//! the counter twin compiles, `bind:` on widget-changed props lowers onto observation, layout
//! props are setters in `<style>` (D3), and set-only props refuse a two-way binding.

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
    // `.row { flex-direction: row; gap: 8px }` became setters at compile time
    assert!(out.contains(r#"$n.set($$n5, "flex-direction", "row");"#), "{out}");
    assert!(out.contains(r#"$n.set($$n5, "gap", 8.0);"#), "{out}");
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
fn style_setter_values_are_checked() {
    let r = run("<window><box class=\"a\" /></window><style>.a { flex-direction: sideways; }</style>");
    assert!(!errors(&r).is_empty(), "an enum value outside the table is refused");
}
