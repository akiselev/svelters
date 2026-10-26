//! A select's options on a real document: skelter-widgets' `select` exposes one AccessKit child per
//! option with the author id from `option-anchors`, and an AccessKit click on an option, found by its
//! anchor, picks it (what cadatomic's tours and the automation bridge do).

use skelter::CoreDocument;
use skelter_base::geom::Size;
use skelter_core::{Document, KindId, KindRegistry, Value};
use skelter_test::{Harness, by};
use std::sync::Arc;

fn text() -> skelter_text::TextSystem {
    skelter_text::TextSystem::new(skelter_text::FontStore::new(
        skelter_text::FontConfig::testing(),
    ))
}

fn prop(d: &Document, name: &str) -> skelter_core::PropId {
    d.registry()
        .get(KindId::of("select"))
        .and_then(|e| e.spec.prop_named(name))
        .unwrap()
}

#[test]
fn an_accesskit_click_on_an_option_found_by_its_anchor_picks_it() {
    let mut reg = KindRegistry::new();
    skelter_widgets::host::register_tier0(&mut reg).unwrap();
    skelter_widgets::views::register_all(&mut reg).unwrap();
    let doc = Document::new(Arc::new(reg), Box::new(text()));
    let doc = CoreDocument::with_widgets(
        doc,
        KindId::of("box"),
        text(),
        skelter_widgets::host::os_policy(),
        skelter_widgets::theme::Theme::light_modern(),
    );
    let mut h = Harness::<CoreDocument>::builder()
        .with_size(Size::new(400.0, 300.0))
        .build(doc);
    let w = h.window();
    let d = &mut h.state_mut().doc;
    let root = d.window_root(w).expect("the window's root");
    let sel = d.create(KindId::of("select"));
    d.insert(root, sel, None, None).unwrap();
    d.set_prop(sel, skelter_core::common::ARIA_LABEL, Value::from("Pin type"))
        .unwrap();
    let opts = ["in", "out", "bidi"].map(Value::from).to_vec();
    d.set_prop(sel, prop(d, "options"), Value::List(opts.into()))
        .unwrap();
    let anchors = [
        "ecad.lib.pins.electrical.0.in",
        "",
        "ecad.lib.pins.electrical.2.bidi",
    ]
    .map(Value::from)
    .to_vec();
    d.set_prop(sel, prop(d, "option-anchors"), Value::List(anchors.into()))
        .unwrap();
    // The tree changed outside input: a key the document ignores brings the frame.
    h.key_press(skelter_base::key::Key::F24);
    h.run();
    let option = h
        .query(by().author_id("ecad.lib.pins.electrical.2.bidi"))
        .expect("the option is in the tree under its anchor");
    assert_eq!(option.label().as_deref(), Some("bidi"));
    assert!(
        h.query(by().author_id("ecad.lib.pins.electrical.1.out"))
            .is_none(),
        "an empty anchor gives no author id"
    );
    option.click_accesskit();
    h.run();
    let d = &h.state().doc;
    assert_eq!(d.get_prop(sel, prop(d, "selected")), Some(Value::I64(2)));
}
