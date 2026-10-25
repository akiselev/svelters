//! CX3 (plans/milestones/CX3-bind-targets.md): bind-target lowering. `|commit` lowers onto the kind's
//! commit event (`bindings.<prop>.commit`), the spec's bind sources (`bind_sources`, the lowering
//! table) turn `bind:prop={store.field}` into a native link and `bind:prop={marker(...)}` into a draft
//! binding, and invalid targets are diagnosed.

use svelters_compiler::compile::{CompileOptions, CompileResult, compile};
use svelters_compiler::diag::Severity;
use svelters_compiler::semantics::SpecBackend;

/// A draft bind source as a provider crate's fragment declares it.
const DRAFTS: &str = r##"{
  "schema": "svelters-semantics/v0",
  "backend": "egui",
  "bind_sources": [
    {"module": "svelters:app/task", "export": "slot", "lowering": "draft", "get": "draftValue", "set": "setDraft"}
  ]
}"##;

fn egui() -> SpecBackend {
    SpecBackend::builtin("egui").unwrap()
}

fn run_with(src: &str, b: &SpecBackend) -> CompileResult {
    compile(src, &CompileOptions::new("App.svelte", b))
}

fn errors(r: &CompileResult) -> Vec<String> {
    r.diagnostics.iter().filter(|d| d.severity == Severity::Error).map(|d| format!("{}: {}", d.code, d.message)).collect()
}

fn js_with(src: &str, b: &SpecBackend) -> String {
    let r = run_with(src, b);
    assert!(errors(&r).is_empty(), "{:?}", errors(&r));
    r.js.unwrap()
}

fn js(src: &str) -> String {
    js_with(src, &egui())
}

fn codes(src: &str, b: &SpecBackend) -> Vec<&'static str> {
    run_with(src, b).diagnostics.iter().filter(|d| d.severity == Severity::Error).map(|d| d.code).collect()
}

/// The `$n.bind*` statements of `js`, one per line (a link's fallback is its own line).
fn binds(js: &str) -> Vec<String> {
    js.lines().map(str::trim).filter(|l| l.contains("$n.bind") || l.starts_with("});")).map(str::to_string).collect()
}

#[test]
fn commit_lowers_onto_the_kinds_commit_event() {
    let out = js("<script>let name = $state('');</script><window><text-edit bind:text|commit={name} /></window>");
    assert_eq!(binds(&out), [r#"$n.bind_commit($$n2, "text", () => $.get(name), ($$value) => { $.set(name, $$value); }, "commit", 0);"#], "{out}");
    // no observation, no `changed` listener: typing runs no JS
    assert!(!out.contains("bind_prop") && !out.contains(r#""changed""#), "{out}");
    // a function binding keeps its accessors
    let out = js("<script>let n = $state('');</script><window><text-edit bind:text|commit={() => n, (v) => (n = v.trim())} /></window>");
    assert!(out.contains(r#"$n.bind_commit($$n2, "text", (() => $.get(n)), ((v) => ($.set(n, v.trim(), true))), "commit", 0);"#), "{out}");
    // OW1b: egui's own slider raises `commit(value)`, so its `value` lowers the same way
    let out = js("<script>let v = $state(0);</script><window><slider bind:value|commit={v} /></window>");
    assert!(out.contains(r#"$n.bind_commit($$n2, "value", "#) && out.contains(r#""commit", 0);"#), "{out}");
}

#[test]
fn commit_on_a_kind_without_a_commit_event_is_diagnosed_with_a_fix() {
    // egui's range-slider has no commit event for `low` (its `commit` carries no value: OW1b)
    let r = run_with("<script>let v = $state(0);</script><window><range-slider bind:low|commit={v} /></window>", &egui());
    let d = r.diagnostics.iter().find(|d| d.code == "bind_commit_unsupported").expect("diagnosed");
    assert!(d.message.contains("<range-slider> has no commit event for `low` on egui"), "{}", d.message);
    assert_eq!(d.fixes.len(), 1);
    assert_eq!(d.fixes[0].message, "remove `|commit`");
    // GTK's entry declares none either
    let gtk = SpecBackend::builtin("gtk").unwrap();
    assert_eq!(codes("<script>let t = $state('');</script><window><entry bind:text|commit={t} /></window>", &gtk), ["bind_commit_unsupported"]);
}

#[test]
fn commit_modifier_misuse_is_an_error() {
    let b = egui();
    let src = |m: &str| format!("<script>let t = $state('');</script><window><text-edit hint-text="Name" bind:text{m}={{t}} /></window>");
    assert_eq!(codes(&src("|commit|nonreactive"), &b), ["bind_modifier_invalid"]);
    assert_eq!(codes(&src("|commit|commit"), &b), ["bind_modifier_invalid"]);
    let r = run_with(&src("|commit|nonreactive"), &b);
    assert!(r.diagnostics[0].message.contains("exclude each other"), "{}", r.diagnostics[0].message);
    let r = run_with("<script>let e;</script><window><text-edit bind:this|commit={e} /></window>", &b);
    assert_eq!(errors(&r), ["bind_modifier_invalid: `bind:this|commit`: `|commit` has no meaning on bind:this"]);
}

#[test]
fn a_store_field_links_natively_with_the_planned_binding_as_fallback() {
    let src = "<script>import { telemetry } from 'svelters:app/telemetry';</script><window><box><slider bind:value={telemetry.speed} /><text-edit bind:text={telemetry.label} /></box></window>";
    let out = js(src);
    assert_eq!(
        binds(&out),
        [
            r#"$n.bind_link($$n3, "value", () => telemetry, "speed", () => {"#,
            r#"$n.bind_prop($$n3, "value", () => telemetry.speed, ($$value) => { telemetry.speed = $$value; }, "changed", 0);"#,
            "});",
            r#"$n.bind_link($$n4, "text", () => telemetry, "label", () => {"#,
            r#"$n.bind_prop($$n4, "text", () => telemetry.label, ($$value) => { telemetry.label = $$value; }, "changed", 0);"#,
            "});",
        ],
        "{out}"
    );
    // `|commit` keeps the JS binding (a link is every change); so does `|nonreactive`
    let out = js("<script>import { task } from 'svelters:app/task';</script><window><text-edit bind:text|commit={task.title} /></window>");
    assert!(!out.contains("bind_link") && out.contains(r#"$n.bind_commit($$n2, "text", () => task.title"#), "{out}");
    let out = js("<script>import { task } from 'svelters:app/task';</script><window><text-edit bind:text|nonreactive={task.title} /></window>");
    assert!(!out.contains("bind_link") && out.contains("$n.bind_once"), "{out}");
    // only imports of `svelters:app/*` match the link source; local state never links
    let out = js("<script>import { model } from './model.js'; let s = $state({ a: '' });</script><window><box><text-edit bind:text={model.a} /><text-edit bind:text={s.a} /></box></window>");
    assert!(!out.contains("bind_link"), "{out}");
    // GTK declares no link source: E3's golden is unchanged (bind_prop through JS)
    let gtk = SpecBackend::builtin("gtk").unwrap();
    let out = js_with("<script>import { telemetry } from 'svelters:app/telemetry';</script><window><entry bind:text={telemetry.label} /></window>", &gtk);
    assert!(!out.contains("bind_link") && out.contains("$n.bind_prop"), "{out}");
}

#[test]
fn a_draft_source_lowers_to_calls_of_its_get_and_set() {
    let b = egui().merge_fragment(DRAFTS).unwrap();
    let src = r#"<script>
  import { slot } from 'svelters:app/task';
  let rows = $state([{ name: 'extents' }]);
</script>
<window><box>
  <text-edit bind:text|commit={slot("depth")} />
  {#each rows as r (r.name)}<text-edit bind:text={slot(r.name + "[0]")} />{/each}
</box></window>"#;
    let out = js_with(src, &b);
    assert!(out.contains("import * as $$bind_source_0 from \"svelters:app/task\";"), "{out}");
    assert_eq!(out.matches("import * as $$bind_source_").count(), 1, "one namespace import per module: {out}");
    assert!(
        out.contains(r#"$n.bind_commit($$n3, "text", () => $$bind_source_0.draftValue("depth"), ($$value) => $$bind_source_0.setDraft("depth", $$value), "commit", 0);"#),
        "{out}"
    );
    // arguments are ordinary expressions, read reactively; every change goes to `set`
    assert!(
        out.contains(r#"() => $$bind_source_0.draftValue($.get(r).name + "[0]"), ($$value) => $$bind_source_0.setDraft($.get(r).name + "[0]", $$value), "changed", 0);"#),
        "{out}"
    );
    // a renamed import is still the source
    let out = js_with("<script>import { slot as s } from 'svelters:app/task';</script><window><text-edit bind:text={s('w')} /></window>", &b);
    assert!(out.contains(r#"$$bind_source_0.setDraft('w', $$value)"#), "{out}");
}

#[test]
fn invalid_bind_targets_are_diagnosed() {
    let b = egui().merge_fragment(DRAFTS).unwrap();
    // a call that is no declared source (and would be `f(x) = v`)
    let r = run_with("<script>import { other } from 'svelters:app/task'; let x = 1;</script><window><text-edit bind:text={other(x)} /></window>", &b);
    assert_eq!(r.diagnostics.iter().filter(|d| d.severity == Severity::Error).map(|d| d.code).collect::<Vec<_>>(), ["bind_source_invalid"]);
    let r = run_with("<script>function f() { return ''; }</script><window><text-edit bind:text={f()} /></window>", &egui());
    assert_eq!(errors(&r), ["bind_source_invalid: bind:text: a call can only be bound when it is a draft bind source the spec declares (this spec declares none)"]);
    // spread arguments
    let r = run_with("<script>import { slot } from 'svelters:app/task'; const a = ['x'];</script><window><text-edit bind:text={slot(...a)} /></window>", &b);
    assert_eq!(errors(&r), ["bind_source_invalid: bind:text: a draft bind source takes no spread arguments"]);
}

#[test]
fn bind_source_fragments_are_checked() {
    let bad = r#"{"schema": "svelters-semantics/v0", "bind_sources": [{"module": "m", "export": "slot", "lowering": "draft", "get": "g"}]}"#;
    assert!(egui().merge_fragment(bad).err().unwrap().contains("needs `get`, `set` and a named export"));
    let twice = egui().merge_fragment(DRAFTS).unwrap().merge_fragment(DRAFTS);
    assert!(twice.err().unwrap().contains("already declared"));
}
