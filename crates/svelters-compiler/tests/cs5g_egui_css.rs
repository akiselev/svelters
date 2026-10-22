//! CS5g: on egui (`style.engine: "backend"`) `<style>` is not lowered, but the compiler still
//! checks it with the CSS engine and reports at the `.svelte` position: a bad value is an error,
//! an unknown property a warning.

use svelters_compiler::compile::{CompileOptions, CompileResult, compile};
use svelters_compiler::diag::{Diagnostic, Severity};
use svelters_compiler::semantics::SpecBackend;

fn run(src: &str) -> CompileResult {
    let b = SpecBackend::builtin("egui").unwrap();
    compile(src, &CompileOptions::new("App.svelte", &b))
}

fn show(r: &CompileResult) -> Vec<String> {
    r.diagnostics.iter().map(|d| d.render("App.svelte")).collect()
}

fn find<'r>(r: &'r CompileResult, code: &str) -> Vec<&'r Diagnostic> {
    r.diagnostics.iter().filter(|d| d.code == code).collect()
}

const APP: &str = r#"<window><box class="card"><label text="hi" /></box></window>

<style>
  .card { background-color: #202020; }
  :global(.x) .card {
    box-shadow: 1px 2px -3px red;
    colour: red;
  }
  .card > label { width: 10px; opacity: lots; }
</style>
"#;

#[test]
fn bad_values_are_errors_at_their_position() {
    let r = run(APP);
    let bad = find(&r, "style_bad_value");
    assert_eq!(bad.len(), 2, "{:#?}", show(&r));
    // after a `:global(...)` rule's selector, on its own line
    assert_eq!((bad[0].severity, bad[0].line, bad[0].column), (Severity::Error, 6, 5), "{:#?}", show(&r));
    assert_eq!(&APP[bad[0].start as usize..bad[0].end as usize], "box-shadow: 1px 2px -3px red");
    assert!(bad[0].message.contains("negative"), "{}", bad[0].message);
    assert_eq!(bad[0].backend.as_deref(), Some("egui"));
    // a second declaration on one line
    assert_eq!((bad[1].line, bad[1].column), (9, 32), "{:#?}", show(&r));
    assert_eq!(&APP[bad[1].start as usize..bad[1].end as usize], "opacity: lots");
    assert!(r.js.is_none(), "errors fail the build");
}

#[test]
fn unknown_properties_are_warnings() {
    let r = run(APP);
    let unknown = find(&r, "style_unknown_declaration");
    assert_eq!(unknown.len(), 1, "{:#?}", show(&r));
    assert_eq!((unknown[0].severity, unknown[0].line, unknown[0].column), (Severity::Warning, 7, 5));
    assert!(unknown[0].message.contains("`colour` is not an egui style property"), "{}", unknown[0].message);
    assert!(find(&r, "style_syntax").is_empty(), "`:global(...)` is Svelte's, not an error: {:#?}", show(&r));

    // only warnings: it compiles
    let ok = run("<window><box class=\"card\" /></window><style>.card { colour: red; box-shadow: 0 1px 2px rgb(0 0 0 / 30%), inset 0 0 0 1px #fff; }</style>");
    assert!(ok.js.is_some(), "{:#?}", show(&ok));
    assert_eq!(ok.diagnostics.iter().map(|d| d.code).collect::<Vec<_>>(), ["style_unknown_declaration"]);
}

#[test]
fn engine_syntax_errors_and_states() {
    // `:state(ns.name)` is a provider state (registered at run time): accepted
    let r = run("<window><button /></window><style>button:state(cad.pending) { opacity: 0.5; }\nbutton:nope { opacity: 1; }</style>");
    let syn = find(&r, "style_syntax");
    assert_eq!(syn.len(), 1, "{:#?}", show(&r));
    assert_eq!(syn[0].line, 2, "{:#?}", show(&r));
    assert!(syn[0].message.contains(":nope"), "{}", syn[0].message);
}
