//! The skelter compiler integrations (`plans/research/skelter/compiler-and-js.md` §3), each a
//! backend capability: stable ids (W-C1), native templates (W-C2), change classes (W-C4), passive
//! listeners (W-C5), and the accessibility checks (W-C9). The egui spec with the capabilities
//! switched on stands in for a backend that advertises them; without them the output is today's.

use serde_json::{Value, json};
use svelters_compiler::compile::{CompileOptions, CompileResult, compile};
use svelters_compiler::diag::Severity;
use svelters_compiler::semantics::SpecBackend;

fn spec_json() -> Value {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../metadata/semantics/egui.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn backend(caps: &[&str]) -> SpecBackend {
    let mut j = spec_json();
    j["capabilities"] = json!(caps);
    SpecBackend::from_json(&j.to_string()).unwrap()
}

fn run(src: &str, caps: &[&str], dev: bool) -> CompileResult {
    let b = backend(caps);
    let mut o = CompileOptions::new("App.svelte", &b);
    o.dev = dev;
    compile(src, &o)
}

fn js(src: &str, caps: &[&str]) -> String {
    let r = run(src, caps, false);
    let errors: Vec<String> = r.diagnostics.iter().filter(|d| d.severity == Severity::Error).map(|d| format!("{}: {}", d.code, d.message)).collect();
    assert!(errors.is_empty(), "{errors:?}");
    r.js.unwrap()
}

fn codes(r: &CompileResult) -> Vec<&str> {
    r.diagnostics.iter().map(|d| d.code).collect()
}

// ---- W-C1: stable ids

#[test]
fn ids_name_kinds_and_props_by_the_specs_ids() {
    let spec = spec_json();
    let src = "<window><box><label text=\"hi\" /></box></window>";
    let out = js(src, &["ids"]);
    let box_id = spec["kinds"]["Box"]["id"].as_u64().unwrap();
    let text_id = spec["kinds"]["Label"]["props"]["text"]["id"].as_u64().unwrap();
    assert!(out.contains("$n.element_id($$K_Box)"), "{out}");
    assert!(out.contains("$n.setp($$n3, $$P_Label_text, \"hi\")"), "{out}");
    assert!(out.contains(&format!("const $$K_Box = {box_id};")), "{out}");
    assert!(out.contains(&format!("const $$P_Label_text = {text_id};")), "{out}");
    let hash = spec["spec_hash"].as_str().unwrap();
    assert!(out.contains(&format!("$n.ids(\"egui\", \"{hash}\", [")), "{out}");
    assert!(out.contains("[$$P_Label_text, \"label\", \"text\"]"), "{out}");
    // a toplevel keeps its own op
    assert!(out.contains("$n.toplevel(\"window\""), "{out}");
}

#[test]
fn ids_keep_set_for_props_with_bookkeeping() {
    // a prop the native side also edits (tracked) keeps `$n.set`, as does a non-scalar one
    let src = "<script>let v = $state(0); let items = $state(['a']);</script><window><box><slider value={v} /><combo-box options={items} hint-text=\"Pick\" /></box></window>";
    let out = js(src, &["ids"]);
    assert!(out.contains("$n.set($$n3, \"value\""), "{out}");
    assert!(out.contains("$n.set($$n4, \"options\""), "{out}");
}

#[test]
fn without_the_capability_names_stay() {
    let out = js("<window><box><label text=\"hi\" /></box></window>", &[]);
    assert!(out.contains("$n.element(\"box\")") && out.contains("$n.set($$n3, \"text\", \"hi\")"), "{out}");
    assert!(!out.contains("element_id") && !out.contains("$n.ids("), "{out}");
}

// ---- W-C2: native templates

const COUNTER: &str = "<script>let n = $state(0);</script>\
<window><box><label>Count: {n}</label><button onclicked={() => n++}>+</button>{#if n > 1}<label>big</label>{/if}</box></window>";

#[test]
fn a_static_subtree_is_one_instantiate() {
    let out = js(COUNTER, &["templates"]);
    assert!(out.contains("const $$t1 = $n.instantiate($$T1, $n.slot($$n1, null), ["), "{out}");
    // the hole builds the {#if} into its slot's container, with the branch's own template
    assert!(out.contains("($$c) => {"), "{out}");
    assert!(out.contains("$n.if_block($$c, "), "{out}");
    assert!(out.contains("$n.instantiate($$T0, "), "the branch's label is a template too: {out}");
    // the nodes the code uses
    assert!(out.contains("const $$n3 = $n.at($$t1, 1);"), "{out}");
    assert!(out.contains("const $$n4 = $n.at($$t1, 2);"), "{out}");
    // the dynamic text is a reactive write as before; the listener goes after the template effect
    assert!(out.contains("$n.set($$n3, \"text\", `Count: ${$.get(n) ?? ''}`)"), "{out}");
    assert!(out.contains("$n.template_listen($$t1, [() => $.update(n)]);"), "{out}");
    // the table: box > [label, button "+", hole], with the button's listener slot
    assert!(out.contains("const $$T1 = $n.template(\""), "{out}");
    assert!(out.contains("[\"box\", null, [], [1, 2, [0, null]], null]"), "{out}");
    assert!(out.contains("[\"button\", null, [[\"set\", \"text\", \"+\"]], [], null]"), "{out}");
    assert!(out.contains("], [[2, \"clicked\", 0]]);"), "{out}");
}

#[test]
fn without_the_capability_there_are_no_templates() {
    let out = js(COUNTER, &[]);
    assert!(!out.contains("instantiate") && !out.contains("$n.template("), "{out}");
}

#[test]
fn listeners_split_by_a_binding_are_written_one_by_one() {
    let src = "<script>let v = $state(0); const f = () => {}; const g = () => {};</script>\
<box><button onclicked={f}>a</button><slider bind:value={v} /><button onclicked={g}>b</button></box>";
    let out = js(src, &["templates"]);
    assert!(out.contains("$n.instantiate($$T0, "), "{out}");
    assert!(!out.contains("template_listen"), "{out}");
    assert!(out.contains("$n.on($$n2, \"clicked\", function") && out.contains("$n.on($$n4, \"clicked\", function"), "{out}");
    // the table lists no listener slots
    assert!(out.contains("]], []);") || out.contains("], []);"), "{out}");
}

#[test]
fn elements_with_actions_or_bind_this_are_not_templates() {
    let src = "<script>let el; const act = () => {};</script><box use:act><label>a</label><label bind:this={el}>b</label></box>";
    let out = js(src, &["templates"]);
    assert!(out.contains("$n.element(\"box\")"), "the element with an action keeps its ops: {out}");
    assert!(out.contains("$n.instantiate("), "its plain child is a template: {out}");
    assert!(out.contains("$.bind_this("), "{out}");
}

#[test]
fn dev_builds_keep_source_offsets_in_the_table() {
    let b = backend(&["templates"]);
    let mut o = CompileOptions::new("App.svelte", &b);
    o.dev = true;
    let r = compile("<box><label>a</label></box>", &o);
    let out = r.js.unwrap();
    assert!(out.contains(", $$src);"), "the instance gets the file: {out}");
    assert!(out.contains("[\"box\", null, [], [1], 0]"), "the box's offset: {out}");
}

// ---- W-C4: change classes

#[test]
fn reactive_writes_carry_their_change_class() {
    let src = "<script>let w = $state(10);</script><box width={w} />";
    let out = js(src, &["change_classes"]);
    assert!(out.contains("$n.set($$n1, \"width\", w, 3)"), "width is a layout prop: {out}");
    let out = js(src, &[]);
    assert!(out.contains("$n.set($$n1, \"width\", w)"), "{out}");
}

#[test]
fn a_layout_prop_driven_per_frame_warns_in_dev() {
    let src = "<script>let w = $state(10);</script><box width={w} onpointermove={(e) => (w = e.x)} />";
    let r = run(src, &[], true);
    assert!(codes(&r).contains(&"layout_write_per_frame"), "{:?}", r.diagnostics);
    let r = run(src, &[], false);
    assert!(!codes(&r).contains(&"layout_write_per_frame"), "dev only");
}

// ---- W-C5: passive listeners

#[test]
fn listeners_that_cannot_cancel_are_passive() {
    let src = "<script>let y = $state(0);</script><box onwheel={(e) => (y += e.dy)} />";
    let out = js(src, &["passive"]);
    assert!(out.contains("(e) => $.set(y, $.get(y) + e.dy), 4);") || out.contains(", 4);"), "{out}");
    let out = js(src, &[]);
    assert!(!out.contains(", 4);"), "{out}");
}

#[test]
fn named_handlers_are_followed_to_their_declaration() {
    let src = "<script>let y = $state(0); function onWheel(e) { y += e.dy; } const stop = (e) => e.preventDefault();</script>\
<box onwheel={onWheel} /><box onwheel={stop} />";
    let r = run(src, &["passive"], false);
    let out = r.js.as_deref().unwrap();
    assert!(out.contains("onWheel?.apply(this, $$args); }, 4);"), "{out}");
    assert_eq!(codes(&r).iter().filter(|c| **c == "nonpassive_scroll_listener").count(), 1, "{:?}", r.diagnostics);
}

#[test]
fn a_cancelling_scroll_listener_warns() {
    let src = "<box onwheel={(e) => e.preventDefault()} />";
    let r = run(src, &["passive"], false);
    assert!(codes(&r).contains(&"nonpassive_scroll_listener"), "{:?}", r.diagnostics);
    assert!(!r.js.unwrap().contains(", 4);"));
}

#[test]
fn passive_modifiers_override_with_warnings() {
    let r = run("<box on:wheel|passive={(e) => e.preventDefault()} />", &["passive"], false);
    assert!(codes(&r).contains(&"passive_listener_cancels"), "{:?}", r.diagnostics);
    assert!(r.js.unwrap().contains(", 4);"));
    let r = run("<script>let y = 0;</script><box on:wheel|nonpassive={(e) => (y = e.dy)} />", &["passive"], false);
    assert!(codes(&r).contains(&"nonpassive_redundant"), "{:?}", r.diagnostics);
    // a backend without the capability keeps refusing modifiers
    let r = run("<box on:wheel|passive={(e) => e.dy} />", &[], false);
    assert!(codes(&r).contains(&"unsupported_feature"), "{:?}", r.diagnostics);
}

// ---- W-C9: accessibility

#[test]
fn controls_without_a_name_warn() {
    let r = run("<window><box><text-edit /></box></window>", &[], false);
    assert!(codes(&r).contains(&"a11y_missing_name"), "{:?}", r.diagnostics);
    let r = run("<window><box><button icon=\"save\" /></box></window>", &[], false);
    assert!(codes(&r).contains(&"a11y_icon_only_control"), "{:?}", r.diagnostics);
    for named in ["<button>Save</button>", "<button icon=\"save\" tooltip=\"Save\" />", "<text-edit hint-text=\"Name\" />", "<button accessible-name={label} />"] {
        let src = format!("<script>let label = 'x';</script><window><box>{named}</box></window>");
        let r = run(&src, &[], false);
        assert!(!codes(&r).iter().any(|c| c.starts_with("a11y_")), "{named}: {:?}", r.diagnostics);
    }
}
