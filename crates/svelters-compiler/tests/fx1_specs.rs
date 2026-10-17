//! FX1: the AppKit and WinUI specs as compile targets, with their `bind:` targets. The runtime
//! side: experiments/native-compiler/compiled `counter-mac` / `counter-win`.

use svelters_compiler::compile::{CompileOptions, CompileResult, compile};
use svelters_compiler::diag::Severity;
use svelters_compiler::semantics::SpecBackend;

fn run(backend: &str, src: &str) -> CompileResult {
    let b = SpecBackend::builtin(backend).unwrap();
    compile(src, &CompileOptions::new("App.svelte", &b))
}

fn errors(r: &CompileResult) -> Vec<String> {
    r.diagnostics.iter().filter(|d| d.severity == Severity::Error).map(|d| format!("{}: {}", d.code, d.message)).collect()
}

fn js(backend: &str, src: &str) -> String {
    let r = run(backend, src);
    assert!(errors(&r).is_empty(), "{backend}: {:?}", errors(&r));
    r.js.unwrap()
}

fn file(path: &str) -> String {
    std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(path)).unwrap()
}

// ---- 5. AppKit and WinUI specs -----------------------------------------------------------------

#[test]
fn appkit_and_winui_builtin_specs_compile_the_counter_fixtures() {
    let mac = js("appkit", &file("fixtures/counter-mac/src/App.svelte"));
    assert!(mac.contains(r#"$n.element("window", "make-key-and-order-front")"#), "{mac}");
    assert!(mac.contains(r#""string-value""#) && mac.contains(r#""action""#), "{mac}");
    let win = js("winui", &file("fixtures/counter-win/src/App.svelte"));
    assert!(win.contains(r#"$n.element("window", "activate")"#), "{win}");
    assert!(win.contains(r#"$n.child_attr("#) && win.contains(r#""row""#), "{win}");
    assert!(win.contains(r#""click""#) && win.contains(r#""content""#), "{win}");
}

#[test]
fn appkit_and_winui_two_way_bindings() {
    let mac = js(
        "appkit",
        "<script>let s = $state('off'); let v = $state(0); let t = $state('');</script>\
         <window><stack-view><button bind:state={s} /><slider bind:double-value={v} /><text-field bind:string-value={t} /></stack-view></window>",
    );
    assert!(mac.contains(r#"$n.bind($$n3, "state", "action", -1"#), "{mac}");
    assert!(mac.contains(r#"$n.bind($$n4, "double-value", "action", -1"#), "{mac}");
    assert!(mac.contains(r#"$n.bind($$n5, "string-value", "control-text-did-change", 0"#), "{mac}");
    let win = js(
        "winui",
        "<script>let c = $state(false); let v = $state(0); let t = $state('');</script>\
         <window><stack-panel><check-box bind:is-checked={c} /><slider bind:value={v} /><text-box bind:text={t} /></stack-panel></window>",
    );
    assert!(win.contains(r#"$n.bind($$n3, "is-checked", "click", -1"#), "{win}");
    assert!(win.contains(r#"$n.bind($$n4, "value", "value-changed", 0"#), "{win}");
    assert!(win.contains(r#"$n.bind($$n5, "text", "text-changed", 0"#), "{win}");
    let r = run("winui", "<script>let v = $state(0);</script><window><slider bind:minimum={v} /></window>");
    assert!(!errors(&r).is_empty(), "minimum is not bindable");
}
