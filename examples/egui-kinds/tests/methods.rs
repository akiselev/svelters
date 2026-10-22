//! RA3: kind methods (`#[svelters::widget] impl QuantityField`) called from Svelte on the element
//! `bind:this` gives: the registry and spec list them with their Rust signatures, the host checks
//! arguments against the descriptor, `cx.set`/`cx.emit` in a method reach `bind:value` and the
//! listeners after the next pass, and `request_focus` focuses the field.

use egui_kittest::kittest::Queryable;
use svelters_egui::egui;
use svelters_egui::egui::accesskit::Role;
use svelters_egui::harness::TestApp;
use svelters_egui::kinds::{self, KindRegistry};

#[test]
fn methods_are_registered_with_their_signatures() {
    egui_kinds_demo::link();
    let reg = KindRegistry::global();
    let k = reg.tag_index("quantity-field").unwrap();
    let names: Vec<&str> = kinds::kind(k).methods.iter().map(|m| m.name).collect();
    assert_eq!(names, ["focus", "setText", "commit", "valueIn", "moveCaret"], "fns with a receiver, camelCase or #[method(rename)], skips left out");
    let (id, _) = kinds::find_method(k, "valueIn").unwrap();
    let i = reg.provider_method_index(k, id).unwrap();
    let d = reg.provider_method_desc(k, i).unwrap();
    assert_eq!(d.name, "valueIn");
    assert_eq!(d.params.iter().map(|p| p.name.as_deref().unwrap_or("")).collect::<Vec<_>>(), ["unit", "places"]);
    assert!(d.params[1].has(svelters_abi::SVL_D_OPTIONAL) && !d.params[0].has(svelters_abi::SVL_D_OPTIONAL));
    assert!(d.ret.has(svelters_abi::SVL_D_NULLABLE));
    // the spec fragment lists them (the compiler checks `call` names against it)
    let f = svelters_egui::spec::fragment();
    let m = &f["kinds"]["cad.QuantityField"]["methods"];
    assert_eq!(m["setText"]["params"][0]["name"], "text");
    assert_eq!(m["setText"]["returns"]["type"], "bool");
    assert_eq!(m["focus"]["returns"]["type"], "void");
    assert_eq!(m["valueIn"]["params"][1]["optional"], true);
    // and the typings give `bind:this` its methods
    let ts = svelters_egui::spec::kinds_typings();
    assert!(ts.contains("export interface QuantityFieldElement extends NativeElement {"), "{ts}");
    assert!(ts.contains("  valueIn(unit: string, places?: number | null): number | null;"), "{ts}");
    assert!(ts.contains("'quantity-field': QuantityFieldElement;"), "{ts}");
}

fn press(app: &mut TestApp, label: &str) {
    let pos = app.harness.get_by_label(label).rect().center();
    app.harness.hover_at(pos);
    for pressed in [true, false] {
        app.harness.event(egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: egui::Modifiers::NONE });
    }
    app.run();
    app.run();
}

fn has(app: &TestApp, label: &str) -> bool {
    app.harness.query_by_label(label).is_some()
}

#[test]
fn svelte_calls_kind_methods_through_bind_this() {
    egui_kinds_demo::link();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("app/src/Methods.svelte");
    let mut app = TestApp::from_source(path).unwrap();
    app.run();
    assert!(has(&app, "width 0.012 accepted 0 edits 0"), "{:#?}", app.harness.root());

    // a method with an argument and a result, called as a member (`field.setText(..)`): its
    // `cx.emit` reaches `onedited` after the next pass
    press(&mut app, "Set text");
    assert!(has(&app, "result parses true"), "{:#?}", app.harness.root());
    assert!(has(&app, "width 0.012 accepted 0 edits 1"), "{:#?}", app.harness.root());
    // `commit` writes the bound prop with `cx.set` (bind:value hears it) and emits `accepted`
    press(&mut app, "Commit");
    assert!(has(&app, "result committed 0.04"), "{:#?}", app.harness.root());
    assert!(has(&app, "width 0.04 accepted 0.04 edits 1"), "{:#?}", app.harness.root());
    // a `&self` method with an optional trailing parameter
    press(&mut app, "Read");
    assert!(has(&app, "result cm 4 in 1.57"), "{:#?}", app.harness.root());

    // through `call` too; state bits set by a method apply at once
    press(&mut app, "Set bad");
    assert!(has(&app, "result bad false"), "{:#?}", app.harness.root());
    let h = app.backend.handles_of("quantity-field")[0];
    let invalid = egui_kinds_demo::cad::INVALID.bits();
    assert!(app.backend.with_scene(|s| s.n(h).state().contains(invalid)), "cad.invalid is set");
    press(&mut app, "Commit");
    assert!(has(&app, "result committed undefined"), "an unparsable text commits nothing (null result)");
    assert!(has(&app, "width 0.04 accepted 0.04 edits 2"));

    // the host checks arguments against the method's descriptor before Rust runs
    press(&mut app, "Wrong");
    assert!(
        app.harness.query_by_label_contains("valueIn").is_some(),
        "{:#?}",
        app.harness.root()
    );

    // K1's shapes: a `Json<T>` argument, a `Result<bool, E>` result, and the `Err` thrown in JS as
    // a native error carrying the `svelters::Error` code
    press(&mut app, "Caret");
    assert!(has(&app, "result moved true"), "{:#?}", app.harness.root());
    assert_eq!(app.backend.with_scene(|s| s.n(h).widget(|w: &egui_kinds_demo::QuantityField| w.caret())), Some(Some(2)));
    press(&mut app, "Caret out");
    assert!(has(&app, "result threw out-of-range"), "{:#?}", app.harness.root());

    // `request_focus` focuses the field at the next pass
    press(&mut app, "Focus");
    assert!(app.harness.get_by_role_and_label(Role::TextInput, "Width").is_focused(), "{:#?}", app.harness.root());

    // `Widget::text_input`: EV3's edit commands (the Edit menu) reach the provider's field
    use svelters_egui::focus::TextCommand;
    let ctx = app.harness.ctx.clone();
    assert!(app.embed.text_command(&ctx, TextCommand::SelectAll), "the focused provider field is a text target");
    app.run();
    assert!(app.embed.text_command(&ctx, TextCommand::Insert("33 mm".into())));
    app.run();
    assert_eq!(app.harness.get_by_role_and_label(Role::TextInput, "Width").value().as_deref(), Some("33 mm"));

    let errors = app.errors();
    assert!(errors.is_empty(), "{errors:?}");
}

/// An object-handle prop (`Option<Obj<DocHandle>>`, K1's `doc`): Svelte passes exported objects,
/// the kind reads and writes the shared Rust object, a method reads it through the handle.
#[test]
fn object_handle_props_reach_the_kind() {
    egui_kinds_demo::link();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("app/src/Doc.svelte");
    let mut app = TestApp::from_source(path).unwrap();
    app.run();
    assert!(has(&app, "doc plan.md"), "{:#?}", app.harness.root());
    press(&mut app, "doc plan.md");
    press(&mut app, "Ask");
    assert!(has(&app, "result name plan.md edits 1"), "the kind wrote the shared object: {:#?}", app.harness.root());
    press(&mut app, "Swap");
    assert!(has(&app, "doc notes.md"), "{:#?}", app.harness.root());
    press(&mut app, "Clear");
    assert!(has(&app, "no document"), "{:#?}", app.harness.root());
    press(&mut app, "Ask");
    assert!(has(&app, "result name null edits 1"), "{:#?}", app.harness.root());
    let errors = app.errors();
    assert!(errors.is_empty(), "{errors:?}");
}
