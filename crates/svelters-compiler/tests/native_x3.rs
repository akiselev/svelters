//! Child attributes (`parent:<name>`), listener phases, sync handlers, `{@attach}` and static
//! spreads on native elements, compiled against the real GTK/Qt specs: generated calls and
//! diagnostics (docs/notes/native-compiler/runtime-contract.md "Child attributes", "Events").

use svelters_compiler::compile::{CompileOptions, CompileResult, compile};
use svelters_compiler::diag::Severity;
use svelters_compiler::semantics::SpecBackend;

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

fn codes(r: &CompileResult, severity: Severity) -> Vec<&'static str> {
    r.diagnostics.iter().filter(|d| d.severity == severity).map(|d| d.code).collect()
}

const GRID: &str = r#"<script>
  let i = $state(0);
  setTimeout(() => i++);
</script>
<window title="t">
  <grid>
    <label parent:column={i} parent:row="2" parent:columnSpan={2}>a</label>
  </grid>
</window>"#;

#[test]
fn child_attrs_are_typed_against_the_parent_slot_on_both_backends() {
    for b in ["gtk", "qt"] {
        let out = js(b, GRID);
        assert!(out.contains(r#"$.template_effect(() => $n.child_attr($$n3, "column", $.get(i)));"#), "{b}:\n{out}");
        assert!(out.contains(r#"$n.child_attr($$n3, "row", 2);"#), "{b}: static values are coerced:\n{out}");
        assert!(out.contains(r#"$n.child_attr($$n3, "columnSpan", 2);"#), "{b}:\n{out}");
    }
}

#[test]
fn unknown_child_attr_and_bad_values_are_diagnosed() {
    for b in ["gtk", "qt"] {
        let r = run(b, r#"<window><grid><label parent:colum={1}>a</label><label parent:row="two">b</label></grid></window>"#);
        assert_eq!(codes(&r, Severity::Error), vec!["unknown_child_attr", "child_attr_bad_value"], "{b}");
        let d = &r.diagnostics[0];
        assert!(d.help.as_deref().unwrap_or("").contains("parent:column"), "{b}: {:?}", d.help);
    }
    // a slot without child attributes (GTK box packing is the child's own props)
    let r = run("gtk", r#"<window><box><label parent:row={1}>a</label></box></window>"#);
    assert_eq!(codes(&r, Severity::Error), vec!["unknown_child_attr"]);
    assert!(r.diagnostics[0].message.contains("has no child attributes"), "{}", r.diagnostics[0].message);
    // Qt box children: stretch (i32) and align (enum)
    let out = js("qt", r#"<window><box><label parent:stretch="1" parent:align="center">a</label></box></window>"#);
    assert!(out.contains(r#"$n.child_attr($$n3, "align", "center");"#), "{out}");
    let r = run("qt", r#"<window><box><label parent:align="middle">a</label></box></window>"#);
    assert_eq!(codes(&r, Severity::Error), vec!["child_attr_bad_value"]);
}

#[test]
fn snippet_roots_child_attrs_are_checked_at_the_render_site() {
    let src = r#"{#snippet cell()}<label parent:colspan={2}>a</label>{/snippet}
<window><grid>{@render cell()}</grid></window>"#;
    for b in ["gtk", "qt"] {
        let r = run(b, src);
        assert_eq!(codes(&r, Severity::Error), vec!["unknown_child_attr"], "{b}");
        assert!(r.diagnostics[0].message.contains("snippet `cell`"));
    }
}

#[test]
fn capture_suffix_and_phases_follow_the_spec() {
    // `close-request` on Qt is an event filter: no phases at all; GTK accepts `after` on any signal
    let after = r#"<script>import { listen } from '@svelters/renderer';</script>
<window {@attach listen('close', (e) => e.preventDefault(), { after: true })}></window>"#;
    let out = js("gtk", after);
    assert!(out.contains(r#"$.attach($$n1, () => $n.listen("close-request", ((e) => e.preventDefault()), 1));"#), "{out}");
    assert!(out.contains("import { listen } from '@svelters/native-runtime';"), "{out}");
    let r = run("qt", after);
    assert_eq!(codes(&r, Severity::Error), vec!["listen_phase_unsupported"]);
    // capture: GTK only on controllers, Qt only on key-press
    let r = run("gtk", r#"<window><entry onchangedcapture={() => {}} /></window>"#);
    assert_eq!(codes(&r, Severity::Error), vec!["listen_phase_unsupported"]);
    assert!(r.diagnostics[0].message.contains("event controllers"));
    let out = js("qt", r#"<window><box onkey-presscapture={(e) => e.preventDefault()}><entry /></box></window>"#);
    assert!(out.contains(r#"$n.on($$n2, "key-press", (e) => e.preventDefault(), 2);"#), "{out}");
    // an unknown base event is reported by its own name
    let r = run("qt", r#"<window><entry onkey-pressedcapture={() => {}} /></window>"#);
    assert_eq!(codes(&r, Severity::Error), vec!["unknown_event"]);
    assert!(r.diagnostics[0].message.contains("`key-pressed`"), "{}", r.diagnostics[0].message);
}

#[test]
fn sync_handler_misuse_warns() {
    for b in ["gtk", "qt"] {
        let r = run(b, r#"<window onclose={() => false}></window>"#);
        assert_eq!(codes(&r, Severity::Warning), vec!["sync_handler_return"], "{b}");
    }
    let r = run("qt", r#"<window><entry onkey-press={() => { return true; }} /></window>"#);
    assert_eq!(codes(&r, Severity::Warning), vec!["sync_handler_return"]);
    // GTK `mnemonic-activate(group_cycling)` is sync (bool_stop): its first parameter is the event
    let r = run("gtk", r#"<window><button onmnemonic-activate={(group_cycling) => {}} /></window>"#);
    assert_eq!(codes(&r, Severity::Warning), vec!["sync_handler_arguments"]);
    // plain signals are not checked
    let r = run("gtk", r#"<window><button onclick={() => false} /></window>"#);
    assert!(codes(&r, Severity::Warning).is_empty());
}

#[test]
fn static_spreads_and_attachments() {
    let src = r#"<script>
  let n = $state(0);
  const events = { onclicked: () => n++, label: 'x' };
  const attach = (node) => { n++; };
</script>
<window><box>
  <button {...events} {@attach attach} />
  <label {...{ "parent:x": 1, "onactivate-link": () => {} }} />
</box></window>"#;
    let r = run("gtk", src);
    // `parent:x` is not an attribute of a box slot; the rest lowers
    assert_eq!(codes(&r, Severity::Error), vec!["unknown_child_attr"]);
    let ok = src.replace(r#""parent:x": 1, "#, "");
    let out = js("gtk", &ok);
    assert!(out.contains(r#"$n.on($$n3, "clicked", function (...$$args) { events["onclicked"]?.apply(this, $$args); });"#), "{out}");
    assert!(out.contains(r#"$n.set($$n3, "label", events["label"])"#), "{out}");
    assert!(out.contains("$.attach($$n3, () => attach);"), "{out}");
    // a spread whose keys are not static is still rejected
    let r = run("gtk", r#"<script>let p = $props();</script><window><button {...p} /></window>"#);
    assert_eq!(codes(&r, Severity::Error), vec!["native_spread_unsupported"]);
}

#[test]
fn string_form_values_pass_static_text() {
    let out = js("qt", r#"<window><keysequenceedit keySequence="Ctrl+K" /></window>"#);
    assert!(out.contains(r#"$n.set($$n2, "keySequence", "Ctrl+K");"#), "{out}");
}
