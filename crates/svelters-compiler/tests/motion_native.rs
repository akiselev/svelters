//! MOTION-CSS: on a backend with the `native_transitions` capability, transitions imported from
//! svelte/transition become `$n.transition_native` and svelte/animate's `flip` `$n.flip_native`;
//! with `native_motion`, `svelte/motion` is imported from `svelters:motion`. Without the
//! capabilities (every checked-in backend today) the output is unchanged.

use svelters_compiler::compile::{CompileOptions, compile};
use svelters_compiler::semantics::SpecBackend;

const EGUI_JSON: &str = include_str!("../../../metadata/semantics/egui.json");

/// The egui spec with `capabilities` added.
fn backend(caps: &[&str]) -> SpecBackend {
    let mut spec: serde_json::Value = serde_json::from_str(EGUI_JSON).unwrap();
    spec["capabilities"] = serde_json::json!(caps);
    SpecBackend::from_json(&spec.to_string()).unwrap()
}

const APP: &str = r#"<script>
  import { fade as f, fly } from 'svelte/transition';
  import { flip } from 'svelte/animate';
  import { Spring } from 'svelte/motion';
  function custom(node) { return { duration: 1 }; }
  let show = $state(true);
  let items = $state([1, 2]);
  const s = new Spring(0);
</script>
<window><box>
  {#if show}<label transition:f={{ duration: 100 }}>a</label>{/if}
  {#if show}<label in:fly={{ y: 10 }} out:custom>b</label>{/if}
  {#each items as i (i)}<label animate:flip={{ duration: 200 }}>{i}</label>{/each}
</box></window>"#;

fn js(caps: &[&str]) -> String {
    let b = backend(caps);
    let res = compile(APP, &CompileOptions::new("App.svelte", &b));
    assert!(!res.has_errors(), "{:#?}", res.diagnostics);
    res.js.unwrap()
}

#[test]
fn native_transitions_emit_the_native_ops() {
    let out = js(&["native_transitions"]);
    assert!(out.contains(r#"$n.transition_native(3, "#), "{out}");
    assert!(out.contains(r#", "fade", () => ({ duration: 100 }))"#), "a renamed import is still svelte/transition's fade: {out}");
    assert!(out.contains(r#"$n.transition_native(1, "#) && out.contains(r#", "fly", () => ({ y: 10 }))"#), "{out}");
    // a function of the component keeps the JS path
    assert!(out.contains("$n.transition(2, ") && out.contains("() => custom"), "{out}");
    assert!(out.contains("$n.flip_native(") && out.contains("() => ({ duration: 200 }))"), "{out}");
    assert!(!out.contains("$n.animation("), "{out}");
    // native_motion not advertised: svelte/motion stays
    assert!(out.contains("'svelte/motion'") || out.contains("\"svelte/motion\""), "{out}");
}

#[test]
fn native_motion_imports_svelters_motion() {
    let out = js(&["native_motion"]);
    assert!(out.contains(r#"import { Spring } from "svelters:motion";"#), "{out}");
    assert!(!out.contains("svelte/motion"), "{out}");
    // native_transitions not advertised: the generic path
    assert!(!out.contains("transition_native") && !out.contains("flip_native"), "{out}");
}

#[test]
fn without_capabilities_the_output_is_unchanged() {
    let plain = {
        let b = SpecBackend::builtin("egui").unwrap();
        compile(APP, &CompileOptions::new("App.svelte", &b)).js.unwrap()
    };
    assert_eq!(js(&[]), plain);
    assert!(plain.contains("$n.transition(3, ") && plain.contains("$n.animation("));
}
