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
