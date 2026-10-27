//! SV80: an arrow the compiler builds around a template expression (a memoized component prop,
//! a template thunk, a `{@const}`, a block's argument) parenthesizes a body that starts with `{`,
//! as stock Svelte does: `$.derived(() => ({ a: x }))`. Unwrapped, the body is read as a block
//! and the module fails to load (`{ a: x, b: y }` is a syntax error).

use svelters_compiler::compile::{CompileOptions, compile};
use svelters_compiler::diag::Severity;
use svelters_compiler::semantics::SpecBackend;

fn js(template: &str) -> String {
    let src = format!(
        "<script>import Child from './Child.svelte'; let x = $state(1); let on = $state(true); setTimeout(() => {{ x++; on = !on; }});</script>\n{template}"
    );
    let b = SpecBackend::builtin("gtk").unwrap();
    let r = compile(&src, &CompileOptions::new("App.svelte", &b));
    let errs: Vec<String> = r.diagnostics.iter().filter(|d| d.severity == Severity::Error).map(|d| format!("{}: {}", d.code, d.message)).collect();
    assert!(errs.is_empty(), "{template}: {errs:?}");
    let js = r.js.unwrap();
    let mut comments = Vec::new();
    if let Err(e) = svelters_compiler::acorn::parse(&js, &svelters_compiler::acorn::Options::default(), &mut comments) {
        panic!("{template}: output does not parse: {}\n{js}", e.message);
    }
    js
}

/// The text of the first `$.derived(() => ...)` declaration in `js`.
fn first_derived(js: &str) -> &str {
    let at = js.find("$.derived(() => ").unwrap_or_else(|| panic!("no derived in\n{js}"));
    let rest = &js[at..];
    &rest[..rest.find(");\n").unwrap_or(rest.len())]
}

#[test]
fn an_object_literal_prop_is_parenthesized_in_its_derived() {
    let js = js("<Child props={{ a: x, b: 2 }} />");
    let d = first_derived(&js);
    assert!(d.starts_with("$.derived(() => ({"), "{d}\n{js}");
}

#[test]
fn a_nested_object_prop_is_parenthesized_in_its_derived() {
    let js = js("<Child props={{ a: { b: x, c: [x, { d: on }] }, e: 1 }} />");
    let d = first_derived(&js);
    assert!(d.starts_with("$.derived(() => ({"), "{d}\n{js}");
}

#[test]
fn objects_returned_from_a_ternary_compile_to_valid_js() {
    let out = js("<Child props={on ? { a: x, b: 1 } : { a: 0, b: x }} />");
    let d = first_derived(&out);
    assert!(d.starts_with("$.derived(() => $.get(on) ?"), "{d}\n{out}");
    // and a ternary whose test is an object is wrapped, since it starts with `{`
    let out = js("<Child v={{ a: x }.a ? { b: 1, c: 2 } : null} />");
    let d = first_derived(&out);
    assert!(d.starts_with("$.derived(() => ({"), "{d}\n{out}");
}

#[test]
fn an_expression_starting_with_an_object_is_parenthesized() {
    for t in ["<Child v={{ a: x, b: 1 }.a + 1} />", "<Child v={{ a: x, b: 1 }[`a`] ?? 0} />"] {
        let js = js(t);
        let d = first_derived(&js);
        assert!(d.starts_with("$.derived(() => ({"), "{t}: {d}\n{js}");
    }
}

#[test]
fn const_tags_and_block_arguments_with_objects_compile_to_valid_js() {
    js("{#if on}{@const o = { a: x, b: 1 }}<Child v={o.a} />{/if}");
    js("{#key { a: x, b: 1 }}<Child />{/key}");
    js("{#each { length: x, b: 1 } as _, i}<Child v={i} />{/each}");
}
