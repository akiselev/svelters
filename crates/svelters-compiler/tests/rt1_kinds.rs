//! RT1: what the retired @svelters/renderer did that the compiled path now does itself. Toplevels
//! declared anywhere with a `show` policy (X7), attached kinds (GTK event controllers and context
//! menus, the Qt context `menu`: spec `attaches_to`), the hand-written GTK menu kinds and the
//! recording canvas (metadata/extras). Runtime side: packages/native-runtime/test/toplevels.test.mjs,
//! fixtures dialogs / menus / sync-handlers / listen-phases and the galleries.

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

#[test]
fn toplevels_anywhere_with_a_show_policy() {
    let out = js(
        "gtk",
        r#"<script>let open = $state(true);</script>
<window title="Main"><box>{#if open}<window title="Child" />{/if}</box></window>
<about-dialog show="none" />"#,
    );
    assert!(out.contains(r#"$n.toplevel("window", "present")"#), "{out}");
    // nested: no slot, the runtime makes it transient for the enclosing window
    assert!(out.matches(r#"$n.toplevel("window", "present")"#).count() == 2, "{out}");
    assert!(out.contains(r#"$n.toplevel("about-dialog", "present", "none")"#), "{out}");
    // Qt dialogs have no spec `present`: the runtime picks `open` / `present` from TOPLEVELS
    let qt = js("qt", r#"<main-window show="modeless"><dialog window-title="D" /></main-window>"#);
    assert!(qt.contains(r#"$n.toplevel("main-window", "present", "modeless")"#), "{qt}");
    assert!(qt.contains(r#"$n.toplevel("dialog", null)"#), "{qt}");
    let bad = run("gtk", r#"<script>let m = $state('modal');</script><window show={m} /><box show="none" />"#);
    let e = errors(&bad);
    assert!(e.iter().any(|x| x.starts_with("show_invalid")), "{e:?}");
    assert!(e.iter().any(|x| x.starts_with("unknown_property") && x.contains("show")), "{e:?}");
}

#[test]
fn attached_kinds_go_into_any_widget_and_next_to_text() {
    let out = js(
        "gtk",
        r#"<window><box>
  <entry><event-controller-key onkey-pressed={(e) => e.preventDefault()} /></entry>
  <button>Context <context-menu><action label="Paste" /></context-menu></button>
  <gesture-click onpressed={() => {}} />
</box></window>"#,
    );
    assert!(out.contains(r#"$n.attached("event-controller-key")"#), "{out}");
    assert!(out.contains(r#"$n.attached("context-menu")"#), "{out}");
    assert!(out.contains(r#"$n.attached("gesture-click")"#), "{out}");
    // the button's text still folds into its label
    assert!(out.contains(r#""label", "Context""#), "{out}");
    // actions are ordinary children of the menu's items slot
    assert!(out.contains(r#"$n.element("action")"#), "{out}");
    // Qt: a `menu` in a menu bar is an item; inside any other widget it is that widget's context menu
    let qt = js(
        "qt",
        r#"<main-window><box-layout><menu-bar><menu title="File" /></menu-bar><push-button>Ctx<menu><action text="A" /></menu></push-button></box-layout></main-window>"#,
    );
    assert_eq!(qt.matches(r#"$n.element("menu")"#).count(), 1, "{qt}");
    assert_eq!(qt.matches(r#"$n.attached("menu")"#).count(), 1, "{qt}");
    // a controller cannot attach to a non-widget
    let bad = run("gtk", r#"<window><box><action><gesture-click /></action></box></window>"#);
    assert!(!errors(&bad).is_empty());
}

#[test]
fn dynamic_component_bind_this_closes_the_component_call() {
    // found by the stock-Svelte conformance run (binding-property-non-reactive-ignored): the
    // `$n.component(` call was left open after the bind callback
    let out = js("gtk", r#"<script>let arr = []; let C = $state();</script><svelte:component this={C} bind:this={arr[0]} />"#);
    let line = out.lines().find(|l| l.contains("$n.component(")).unwrap();
    assert_eq!(line.matches('(').count(), line.matches(')').count(), "{line}");
}

#[test]
fn gtk_menu_kinds_and_canvas_are_in_the_spec() {
    let out = js(
        "gtk",
        r#"<script>let on = $state(false);</script>
<window><box>
  <menu-bar><menu label="File"><action label="Open" shortcut="Ctrl+O" onactivate={() => {}} /><menu-separator /><action label="Auto" checkable bind:checked={on} /></menu></menu-bar>
  <tool-bar><action icon="edit-copy" label="Copy" /></tool-bar>
  <canvas content-width={10} content-height={10} />
</box></window>"#,
    );
    assert!(out.contains(r#"$n.element("menu-bar")"#) && out.contains(r#"$n.element("tool-bar")"#), "{out}");
    assert!(out.contains(r#""checkable", true"#), "{out}");
    assert!(out.contains(r#"$n.bind("#) && out.contains(r#""toggled""#), "{out}");
    assert!(out.contains(r#"$n.element("canvas")"#), "{out}");
    let bad = run("gtk", r#"<window><box><menu-bar><label /></menu-bar></box></window>"#);
    assert!(errors(&bad).iter().any(|x| x.starts_with("slot_rejects_child")), "{:?}", errors(&bad));
}
