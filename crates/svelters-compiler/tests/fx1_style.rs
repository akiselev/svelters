//! FX1: setter-style precedence in the compiled output (explicit attribute > style: / style= >
//! rules > native value). The runtime side: experiments/native-compiler/compiled `style-precedence`.

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

// ---- 1. style precedence --------------------------------------------------------------------

const RULES: &str = "<style>\n  .row { spacing: 8px; }\n  #a { spacing: 5px; }\n</style>";

#[test]
fn nullish_style_directive_falls_back_to_the_rules_then_the_default() {
    for b in ["gtk", "qt"] {
        // each toolkit's native names: GTK window/box, Qt main-window/box-layout
        let (w, x) = if b == "gtk" { ("window", "box") } else { ("main-window", "box-layout") };
        let src = format!("<script>let g = $state();</script>\n<{w}><{x}><{x} class=\"row\" style:spacing={{g}}></{x}><{x} style:spacing={{g}}></{x}></{x}></{w}>\n{RULES}");
        let out = js(b, &src);
        // with a matching rule: the rule's value
        assert!(out.contains(r#"$n.setter_value("#) && out.contains(r#"g) ?? (8)"#), "{b}:\n{out}");
        // without: the spec's native default (GTK 0, Qt -1), never a bare null
        let default = if b == "gtk" { "?? (0)" } else { "?? (-1)" };
        assert!(out.contains(default), "{b}:\n{out}");
    }
}

#[test]
fn nullish_style_directive_falls_back_to_the_static_inline_value() {
    let out = js("gtk", &format!("<script>let g = $state();</script>\n<window><box class=\"row\" style=\"spacing: 3px\" style:spacing={{g}}></box></window>\n{RULES}"));
    assert!(out.contains("?? (3)"), "{out}");
}

#[test]
fn dynamic_id_matches_id_rules_at_runtime() {
    let out = js("gtk", &format!("<script>let n = $state('a');</script>\n<window><box id={{n}}></box></window>\n{RULES}"));
    assert!(out.contains("$.derived(() => String("), "{out}");
    assert!(out.contains(r#"=== "a" ? 5 : 0"#), "{out}");
}

#[test]
fn dynamic_inline_style_knows_explicit_props_and_guards_rules() {
    let out = js("gtk", &format!("<script>let s = $state('');</script>\n<window><box><box spacing={{1}} style={{s}}></box><box class=\"row\" style={{s}}></box></box></window>\n{RULES}"));
    assert!(out.contains(r#"$n.explicit($$n3, ["spacing"], []);"#), "{out}");
    assert!(out.contains(r#"$n.explicit($$n4, [], []);"#), "{out}");
    // the rule's value goes through the guard, so the inline style wins at runtime
    assert!(out.contains(r#"$n.rule_set($$n4, "spacing", 8)"#), "{out}");
}

