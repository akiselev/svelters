//! SP1: spreads whose keys are only known at run time and `<svelte:element this>` on native
//! elements, compiled against the real GTK/Qt specs: the generated `$n.spread` /
//! `$n.dynamic_element` calls, the spec-table module (`--kinds`), and the compile-time checks that
//! stay (docs/notes/native-compiler/runtime-contract.md "Spread attributes").

use svelters_compiler::compile::{CompileOptions, CompileResult, compile};
use svelters_compiler::diag::Severity;
use svelters_compiler::semantics::{SpecBackend, kinds_module};

fn run(backend: &str, src: &str) -> CompileResult {
    let b = SpecBackend::builtin(backend).unwrap();
    compile(src, &CompileOptions::new("App.svelte", &b))
}

fn js(backend: &str, src: &str) -> String {
    let r = run(backend, src);
    let errs: Vec<String> = r.diagnostics.iter().filter(|d| d.severity == Severity::Error).map(|d| format!("{}: {}", d.code, d.message)).collect();
    assert!(errs.is_empty(), "{backend}: {errs:?}");
    r.js.unwrap()
}

fn errors(r: &CompileResult) -> Vec<&'static str> {
    r.diagnostics.iter().filter(|d| d.severity == Severity::Error).map(|d| d.code).collect()
}

/// (window, box, button, the button's label prop) in each backend's vocabulary.
fn vocab(backend: &str) -> (&'static str, &'static str, &'static str, &'static str) {
    if backend == "qt" { ("main-window", "box-layout", "push-button", "text") } else { ("window", "box", "button", "label") }
}

const KINDS_IMPORT: &str = "import * as $$kinds from '@svelters/native-runtime/kinds';";

#[test]
fn a_runtime_spread_puts_every_attribute_in_one_object_in_source_order() {
    for b in ["gtk", "qt"] {
        let (w, bx, btn, label) = vocab(b);
        let src = format!(
            r#"<script>let p = $props(); let on = $state(true); setTimeout(() => on = false);</script>
<{w}><{bx}><{btn} {label}="x" {{...p}} onclicked={{() => {{}}}} class:big={{on}} /></{bx}></{w}>"#
        );
        let out = js(b, &src);
        assert!(out.contains(KINDS_IMPORT), "{b}:\n{out}");
        // the inline handler is hoisted, so a re-run of the effect passes the same function
        assert!(out.contains("const $$handler1 = () => {};"), "{b}:\n{out}");
        assert!(
            out.contains(&format!(r#"$n.spread($$n3, $$kinds, () => ({{ {label}: "x", ...(p), onclicked: $$handler1, [$n.CLASS]: {{ big: $.get(on) }} }}));"#)),
            "{b}:\n{out}"
        );
    }
}

#[test]
fn an_object_literal_spread_stays_static_and_needs_no_tables() {
    for b in ["gtk", "qt"] {
        let (w, bx, btn, label) = vocab(b);
        let out = js(b, &format!(r#"<{w}><{bx}><{btn} {{...{{ {label}: 'y' }}}} /></{bx}></{w}>"#));
        assert!(out.contains(&format!(r#"$n.set($$n3, "{label}", 'y');"#)), "{b}:\n{out}");
        assert!(!out.contains("$$kinds") && !out.contains("$n.spread("), "{b}:\n{out}");
    }
}

#[test]
fn explicit_attributes_next_to_a_spread_are_still_checked_at_compile_time() {
    for b in ["gtk", "qt"] {
        let (w, bx, btn, _) = vocab(b);
        let r = run(b, &format!(r#"<script>let p = $props();</script><{w}><{bx}><{btn} {{...p}} labl="x" onclickd={{() => {{}}}} /></{bx}></{w}>"#));
        assert_eq!(errors(&r), vec!["unknown_property", "unknown_event"], "{b}");
    }
}

#[test]
fn svelte_element_builds_its_element_at_run_time() {
    for b in ["gtk", "qt"] {
        let (w, bx, _, label) = vocab(b);
        let src = format!(
            r#"<script>let tag = $state('label'); setTimeout(() => tag = null); let n = $state();</script>
<{w}><{bx}><svelte:element this={{tag}} {label}="z" bind:this={{n}} /></{bx}></{w}>"#
        );
        let out = js(b, &src);
        assert!(out.contains(KINDS_IMPORT), "{b}:\n{out}");
        assert!(out.contains("$n.dynamic_element($n.slot($$n2, null), () => $.get(tag), ($$n3) => {"), "{b}:\n{out}");
        assert!(out.contains(&format!(r#"$n.spread($$n3, $$kinds, () => ({{ {label}: "z" }}));"#)), "{b}:\n{out}");
        assert!(out.contains("}, $$kinds);"), "{b}:\n{out}");
    }
}

#[test]
fn svelte_element_rejects_bindings_that_need_the_kind() {
    for b in ["gtk", "qt"] {
        let (w, bx, _, _) = vocab(b);
        let r = run(b, &format!(r#"<script>let t = $state('entry'); let v = $state('');</script><{w}><{bx}><svelte:element this={{t}} bind:text={{v}} /></{bx}></{w}>"#));
        assert_eq!(errors(&r), vec!["dynamic_element_bind"], "{b}");
    }
}

#[test]
fn the_kinds_module_carries_the_spec_of_each_backend() {
    for (b, tag, prop, event) in [("gtk", "button", "\"label\":[\"label\"", "\"clicked\":"), ("qt", "push-button", "\"text\":[\"text\"", "\"clicked\":")] {
        let sem = SpecBackend::builtin(b).unwrap();
        let m = kinds_module(sem.spec());
        assert!(m.contains(&format!("export const backend = \"{b}\";")), "{b}");
        assert!(m.contains("export const tags = {") && m.contains("export const kinds = {") && m.contains("export const aliases = "), "{b}");
        assert!(m.contains(&format!("\"{tag}\":")), "{b}: no {tag}");
        assert!(m.contains(prop) && m.contains(event), "{b}");
    }
}
