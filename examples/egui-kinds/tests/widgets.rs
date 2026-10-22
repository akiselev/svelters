//! RA3 (ARCH-EMBED §10 RA3 row): the fn form, `wrap_widget!` (a wrapped `DragValue`) and a
//! container kind (`collapsing-section`, children from Svelte), driven from Svelte by kittest.

use egui_kittest::kittest::Queryable;
use svelters_egui::egui;
use svelters_egui::egui::accesskit::Role;
use svelters_egui::harness::TestApp;
use svelters_egui::kinds::{self, KindRegistry};

#[test]
fn fn_form_wrap_and_container_kinds_are_registered() {
    egui_kinds_demo::link();
    let reg = KindRegistry::global();
    for (tag, id) in [("level-meter", "cad.LevelMeter"), ("step-count", "cad.StepCount"), ("wrapped-drag-value", "cad.DragValueKind"), ("collapsing-section", "cad.CollapsingSection")] {
        let k = reg.tag_index(tag).unwrap_or_else(|| panic!("{tag}"));
        assert_eq!(reg.provider(k).unwrap().entry.id, id);
    }
    let meter = reg.tag_index("level-meter").unwrap();
    for p in ["level", "width", "label"] {
        assert!(kinds::find_prop(meter, p).is_some(), "{p}: one prop per parameter after the Ui");
    }
    let f = svelters_egui::spec::fragment();
    assert_eq!(f["kinds"]["cad.LevelMeter"]["props"]["width"]["default"], 120.0);
    assert_eq!(f["kinds"]["cad.LevelMeter"]["props"]["label"]["required"], true);
    // `&mut i64` is the bound prop: exact
    assert_eq!(f["kinds"]["cad.StepCount"]["props"]["count"]["observe"]["class"], "exact");
    assert_eq!(f["kinds"]["cad.DragValueKind"]["props"]["value"]["observe"]["class"], "exact");
    // only the container takes children
    assert_eq!(kinds::slot_of(reg.tag_index("collapsing-section").unwrap()), Some("children"));
    assert_eq!(f["kinds"]["cad.CollapsingSection"]["children"]["default_slot"], "children");
    assert_eq!(kinds::slot_of(meter), None);
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
fn svelte_drives_fn_form_wrapped_and_container_kinds() {
    egui_kinds_demo::link();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("app/src/Widgets.svelte");
    let mut app = TestApp::from_source(path).unwrap();
    app.run();
    assert!(has(&app, "count 2 x 1.5 open true"), "{:#?}", app.harness.root());

    // the fn form draws from its props (and its measured size is what it allocated)
    assert!(has(&app, "Load 25%"), "{:#?}", app.harness.root());
    let h = app.backend.handles_of("level-meter")[0];
    let r = app.backend.with_scene(|s| s.n(h).rect);
    assert_eq!((r.width(), r.height()), (120.0, 14.0), "measured by a sizing pass");
    press(&mut app, "More load");
    assert!(has(&app, "Load 75%"), "{:#?}", app.harness.root());

    // a bound `&mut` parameter: what the fn writes reaches bind:count
    press(&mut app, "+");
    assert!(has(&app, "count 3 x 1.5 open true"), "{:#?}", app.harness.root());

    // the wrapped DragValue shows the bound value; a JS write reaches it, and egui's own write
    // (typing a new value) is announced to bind:value
    press(&mut app, "Set x");
    assert!(has(&app, "count 3 x 4 open true"), "{:#?}", app.harness.root());
    let dv = app.harness.get_by_role(Role::SpinButton);
    assert_eq!(dv.value().as_deref(), Some("4 mm"), "{:#?}", app.harness.root());
    dv.click();
    app.run();
    app.harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    app.harness.get_by_role(Role::SpinButton).type_text("7");
    app.run();
    app.harness.key_press(egui::Key::Enter);
    app.run();
    app.run();
    assert!(has(&app, "count 3 x 7 open true"), "{:#?}", app.harness.root());

    // the container's children come from Svelte and are drawn under its header while open
    assert!(has(&app, "inside one") && has(&app, "inside two"), "{:#?}", app.harness.root());
    let sec = app.backend.handles_of("collapsing-section")[0];
    let one = app.harness.get_by_label("inside one").rect();
    let head = app.backend.with_scene(|s| s.n(sec).rect);
    assert!(one.min.y >= head.min.y + 20.0 && one.min.x >= head.min.x + 12.0, "child placed by Widget::layout: {one:?} in {head:?}");
    press(&mut app, "Advanced");
    assert!(has(&app, "count 3 x 7 open false"), "{:#?}", app.harness.root());
    assert!(!has(&app, "inside one"), "closed: the children are not drawn");
    let closed = app.backend.with_scene(|s| s.n(sec).rect);
    assert_eq!(closed.height(), 20.0, "relaid out to the header");

    let errors = app.errors();
    assert!(errors.is_empty(), "{errors:?}");
}

/// The paint cache (`cx.cached_shapes`): frames that repaint an unchanged `<spark-line>` reuse its
/// shapes; a new `values` (a new version) rebuilds them once.
#[test]
fn paint_cache_hit_rate() {
    egui_kinds_demo::link();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("app/src/Widgets.svelte");
    let mut app = TestApp::from_source(path).unwrap();
    app.run();
    let stats = |app: &TestApp| app.backend.scene().read(|s| s.stats());
    let s0 = stats(&app);
    assert!(s0.shape_cache_misses >= 1, "the first paint builds the shapes");
    for _ in 0..20 {
        app.harness.step();
    }
    let s1 = stats(&app);
    let (hits, misses) = (s1.shape_cache_hits - s0.shape_cache_hits, s1.shape_cache_misses - s0.shape_cache_misses);
    assert_eq!(misses, 0, "nothing changed: no rebuild");
    assert!(hits >= 20, "every repaint reused the shapes: {hits}");
    press(&mut app, "Sample");
    for _ in 0..10 {
        app.harness.step();
    }
    let s2 = stats(&app);
    assert_eq!(s2.shape_cache_misses - s1.shape_cache_misses, 1, "a new version rebuilds once");
    let rate = (s2.shape_cache_hits) as f64 / (s2.shape_cache_hits + s2.shape_cache_misses) as f64;
    assert!(rate > 0.9, "hit rate {rate}");
}
