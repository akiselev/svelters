//! WG2a: the egui menu family and overlays as a compile target (the spec generated from
//! crates/svelters-egui/src/kinds/{menus,popups}.rs): the gallery page compiles, `context-menu` and
//! `popup` attach to a leaf, menu entries stay out of boxes, `bind:checked` lowers onto observation
//! with `toggled` as its fallback signal.

use svelters_compiler::compile::{CompileOptions, CompileResult, compile};
use svelters_compiler::diag::Severity;
use svelters_compiler::semantics::SpecBackend;

fn run(src: &str) -> CompileResult {
    let b = SpecBackend::builtin("egui").unwrap();
    compile(src, &CompileOptions::new("App.svelte", &b))
}

fn errors(r: &CompileResult) -> Vec<String> {
    r.diagnostics.iter().filter(|d| d.severity == Severity::Error).map(|d| format!("{}: {}", d.code, d.message)).collect()
}

#[test]
fn gallery_page_compiles() {
    let src = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/gallery-egui-menus/src/App.svelte")).unwrap();
    let r = run(&src);
    assert!(errors(&r).is_empty(), "{:?}", errors(&r));
    let js = r.js.unwrap();
    assert!(js.contains(r#""checked""#) && js.contains(r#""toggled""#), "{js}");
    assert!(js.contains(r#""open""#) && js.contains(r#""active""#), "{js}");
}

#[test]
fn attaching_kinds_and_menu_entries() {
    // a context menu and a popup attach to a leaf (spec `attaches_to`)
    let r = run(r#"<window><box><button>Go<context-menu><action label="A" /></context-menu><popup><label>x</label></popup></button></box></window>"#);
    assert!(errors(&r).is_empty(), "{:?}", errors(&r));
    // menu entries are not widgets: a box does not take one, a menu-bar takes no label
    assert!(!errors(&run(r#"<window><box><action label="A" /></box></window>"#)).is_empty());
    assert!(!errors(&run(r#"<window><box><menu-bar><label>x</label></menu-bar></box></window>"#)).is_empty());
    // unknown props are refused
    assert!(!errors(&run(r#"<window><box><menu-bar><menu label="F"><action labell="A" /></menu></menu-bar></box></window>"#)).is_empty());
}
