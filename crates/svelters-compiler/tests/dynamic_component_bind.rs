//! `bind:this` on a dynamic component (`<svelte:component this>` and a component held in state)
//! binds each instance's exports inside its branch (Codex review of PR #3: it bound the result of
//! `$n.snippet`, which is undefined). The runtime half is in
//! packages/native-runtime/test/blocks.test.mjs ("component: bind:this follows the instance").

use svelters_compiler::compile::{CompileOptions, compile};
use svelters_compiler::diag::Severity;
use svelters_compiler::semantics::SpecBackend;

fn js(src: &str) -> String {
    let b = SpecBackend::builtin("gtk").unwrap();
    let r = compile(src, &CompileOptions::new("App.svelte", &b));
    let errs: Vec<String> = r.diagnostics.iter().filter(|d| d.severity == Severity::Error).map(|d| format!("{}: {}", d.code, d.message)).collect();
    assert!(errs.is_empty(), "{errs:?}");
    r.js.unwrap()
}

#[test]
fn svelte_component_binds_the_exports_of_each_instance() {
    let out = js(r#"<script>
  import A from './A.svelte';
  let C = $state(A);
  let ref = $state();
</script>
<window title="t"><box><svelte:component this={C} bind:this={ref} label="x" /></box></window>"#);
    assert!(out.contains("$n.component("), "{out}");
    assert!(out.contains("($$exports) => $.bind_this($$exports, ($$value) => {"), "{out}");
    assert!(!out.contains("$.bind_this($n.snippet("), "{out}");
}

#[test]
fn a_component_in_state_binds_the_exports_of_each_instance() {
    let out = js(r#"<script>
  import A from './A.svelte';
  let C = $state(A);
  let ref = $state();
</script>
<window title="t"><box><C bind:this={ref} /></box></window>"#);
    assert!(out.contains("$n.component("), "{out}");
    assert!(out.contains("($$exports) => $.bind_this($$exports"), "{out}");
}

#[test]
fn a_static_component_still_binds_its_call() {
    let out = js(r#"<script>
  import A from './A.svelte';
  let ref = $state();
</script>
<window title="t"><box><A bind:this={ref} /></box></window>"#);
    assert!(out.contains("$.bind_this(A($n.region("), "{out}");
    assert!(!out.contains("$n.component("), "{out}");
}
