//! H1: `--hmr` wraps the component for in-place replacement (plans/research/HMR.md section 3.3).
//! The runtime half is packages/native-runtime/test/hmr.test.mjs.

use svelters_compiler::compile::{CompileOptions, compile};
use svelters_compiler::semantics::SpecBackend;

fn js(hmr: bool) -> String {
    let b = SpecBackend::builtin("gtk").unwrap();
    let mut o = CompileOptions::new("ui/App.svelte", &b);
    o.hmr = hmr;
    compile("<window title=\"t\"><label label=\"x\" /></window>", &o).js.unwrap()
}

#[test]
fn hmr_wraps_the_component_and_exports_it_after() {
    let out = js(true);
    assert!(out.contains("\nfunction App($$anchor, $$props) {"), "{out}");
    assert!(!out.contains("export default function"), "{out}");
    assert!(out.contains("App = $n.hmr(App, \"ui/App.svelte\");"), "{out}");
    assert!(out.contains("import.meta.hot.acceptExports([\"default\"], (m) => m && App[$n.HMR].update(m.default));"), "{out}");
    assert!(out.trim_end().ends_with("export default App;"), "{out}");
}

#[test]
fn without_hmr_the_output_is_unchanged() {
    let out = js(false);
    assert!(out.contains("export default function App($$anchor, $$props) {"), "{out}");
    assert!(!out.contains("import.meta.hot"), "{out}");
}

// ---- H3: keyed declarations and the component signature (plans/research/HMR.md 3.3) ----

fn js_of(src: &str, hmr: bool) -> String {
    let b = SpecBackend::builtin("gtk").unwrap();
    let mut o = CompileOptions::new("ui/App.svelte", &b);
    o.hmr = hmr;
    o.dev = hmr;
    let r = compile(src, &o);
    r.js.unwrap_or_else(|| panic!("{:?}", r.diagnostics))
}

const COUNTER: &str = "<script>\n  let count = $state(0);\n  let todos = $state([1]);\n  let f = $state.raw({a: 1});\n  function inc() { count += 1; todos.push(2); f = {a: 2}; }\n</script>\n<window title=\"t\"><button label={String(count)} onclicked={inc} /></window>";

#[test]
fn state_declarations_are_keyed_and_the_wrapper_carries_the_signature() {
    let out = js_of(COUNTER, true);
    assert!(out.contains("$n.hmr_keep(\"count\", () => (0), () => $.get(count))"), "{out}");
    assert!(out.contains("$.proxy($n.hmr_keep(\"todos\", () => ([1]), () => todos))"), "{out}");
    assert!(out.contains("$n.hmr_keep(\"f\", () => ({a: 1}), () => $.get(f))"), "{out}");
    assert!(out.contains("$n.hmr(App, \"ui/App.svelte\", \""), "{out}");
}

#[test]
fn the_signature_follows_declarations_and_initializers_not_formatting() {
    let sig = |s: &str| {
        let out = js_of(s, true);
        let i = out.find("$n.hmr(App, \"ui/App.svelte\", ").expect(&out);
        out[i..].lines().next().unwrap().to_string()
    };
    let base = sig(COUNTER);
    assert_eq!(base, sig(&COUNTER.replace("$state(0)", "$state( 0 /* zero */ )")));
    assert_eq!(base, sig(&COUNTER.replace("<window title=\"t\">", "<window title=\"changed\">")));
    assert_ne!(base, sig(&COUNTER.replace("$state(0)", "$state(10)")));
    assert_ne!(base, sig(&COUNTER.replace("let count", "let total").replace("count +=", "total +=").replace("String(count)", "String(total)")));
}

#[test]
fn reset_pragma_never_matches_and_release_output_has_no_keys() {
    let out = js_of(&format!("<script>\n  // @hmr:reset\n{}", &COUNTER["<script>\n".len()..]), true);
    assert!(out.contains("$n.hmr(App, \"ui/App.svelte\", null);"), "{out}");
    let rel = js_of(COUNTER, false);
    assert!(!rel.contains("hmr_keep"), "{rel}");
}

#[test]
fn only_top_level_instance_state_is_keyed() {
    let out = js_of("<script>\n  function f() { let inner = $state(1); return inner; }\n  const c = $state({x: 1});\n</script>\n<window title=\"t\" />", true);
    assert!(!out.contains("hmr_keep(\"inner\""), "{out}");
}

#[test]
fn module_state_stores_and_contexts_are_keyed_through_hot_data() {
    use svelters_compiler::compile::{ModuleOptions, compile_module};
    let src = "import { writable } from 'svelte/store';\nimport { createContext } from 'svelte';\nexport const store = writable(0);\nexport const [get, set] = createContext();\nexport const ctx = createContext();\nlet count = $state(0);\nexport function inc() { count += 1; }\nexport const read = () => count;\n";
    let m = |hmr: bool, src: &str| compile_module(src, &ModuleOptions { filename: "state.svelte.js".into(), dev: hmr, hmr }).js.unwrap();
    let out = m(true, src);
    assert!(out.contains("$$hmr().keep(\"count\", () => (0), () => $.get(count))"), "{out}");
    assert!(out.contains("$$hmr().keep_object(\"store\", () => writable(0))"), "{out}");
    assert!(out.contains("$$hmr().keep_object(\"ctx\", () => createContext())"), "{out}");
    assert!(out.contains("$n.hmr_module(import.meta.hot, \""), "{out}");
    assert!(out.trim_end().ends_with("$$hmr().commit();"), "{out}");
    let rel = m(false, src);
    assert!(!rel.contains("hmr"), "{rel}");
    let reset = m(true, &format!("// @hmr:reset\n{src}"));
    assert!(reset.contains("$n.hmr_module(import.meta.hot, null)"), "{reset}");
}
