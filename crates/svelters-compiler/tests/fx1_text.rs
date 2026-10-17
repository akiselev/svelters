//! FX1: text children folded into each toolkit's text prop (metadata/overrides `text_prop`).

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

// ---- 2. text children ------------------------------------------------------------------------

#[test]
fn text_children_set_each_toolkits_text_prop() {
    for (b, tag, prop) in [("gtk", "button", "label"), ("qt", "button", "label"), ("appkit", "button", "title"), ("winui", "button", "content"), ("winui", "text-block", "text")] {
        let window = if b == "appkit" { "<window><stack-view>" } else if b == "winui" { "<window><stack-panel>" } else { "<window><box>" };
        let close = if b == "appkit" { "</stack-view></window>" } else if b == "winui" { "</stack-panel></window>" } else { "</box></window>" };
        let out = js(b, &format!("<script>let n = $state(1);</script>{window}<{tag}>Save</{tag}><{tag}>n = {{n}}</{tag}>{close}"));
        assert!(out.contains(&format!(r#"$n.set($$n3, "{prop}", "Save")"#)), "{b}:\n{out}");
        assert!(out.contains(&format!(r#"$n.set($$n4, "{prop}", `n = "#)), "{b}:\n{out}");
    }
}

#[test]
fn text_mixed_with_elements_is_an_error() {
    let r = run("gtk", "<window><box><button>Save <image /></button></box></window>");
    let e = errors(&r);
    assert!(e.iter().any(|m| m.starts_with("text_content_children: <button> mixes text and elements: on gtk its text children set `label`")), "{e:?}");
    let r = run("winui", "<window><stack-panel><button>Go <text-block>x</text-block></button></stack-panel></window>");
    assert!(errors(&r).iter().any(|m| m.contains("its text children set `content`")), "{:?}", errors(&r));
}

#[test]
fn only_element_children_stay_children_of_a_text_prop_kind() {
    // GTK's button has no child slot in the spec: the error is about slots, not about text
    let r = run("gtk", "<window><box><button><image /></button></box></window>");
    assert!(errors(&r).iter().all(|m| !m.starts_with("text_content_children")), "{:?}", errors(&r));
    let out = js("winui", "<window><stack-panel><button><text-block>x</text-block></button></stack-panel></window>");
    assert!(out.contains(r#"$n.set($$n4, "text", "x")"#), "{out}");
}

