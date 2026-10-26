//! Keys are the focused node's, not the hovered one's: skelter-widgets' controls claim an event
//! because the pointer is over them only for pointer events. Before, a button returned
//! `Handled::Yes` for every event while hovered, so with the pointer still resting on a button just
//! clicked, Tab (and every other key) stopped at the button and skelter's focus traversal never ran.

use skelter::CoreDocument;
use skelter_base::geom::Size;
use skelter_base::key::Key;
use skelter_core::{Document, KindId, KindRegistry, StyleInput, Value, common};
use skelter_test::Harness;
use std::sync::Arc;

fn text() -> skelter_text::TextSystem {
    skelter_text::TextSystem::new(skelter_text::FontStore::new(skelter_text::FontConfig::testing()))
}

/// A window with buttons "One", "Two" and "Three" in a column.
fn harness() -> Harness<CoreDocument> {
    let mut reg = KindRegistry::new();
    skelter_widgets::host::register_tier0(&mut reg).unwrap();
    let doc = Document::new(Arc::new(reg), Box::new(text()));
    let doc = CoreDocument::with_widgets(
        doc,
        KindId::of("box"),
        text(),
        skelter_widgets::host::os_policy(),
        skelter_widgets::theme::Theme::light_modern(),
    );
    let mut h = Harness::<CoreDocument>::builder().with_size(Size::new(300.0, 200.0)).build(doc);
    let w = h.window();
    let d = &mut h.state_mut().doc;
    let root = d.window_root(w).expect("the window's root");
    for label in ["One", "Two", "Three"] {
        let b = d.create(KindId::of("button"));
        d.set_prop(b, common::ARIA_LABEL, Value::from(label)).unwrap();
        d.set_style(b, StyleInput { inline: Some("width: 80px; height: 30px".into()), ..Default::default() });
        d.insert(root, b, None, None).unwrap();
    }
    h.key_press(Key::F24);
    h.run();
    h
}

fn focused(h: &Harness<CoreDocument>) -> Option<String> {
    h.focused().and_then(|n| n.label())
}

#[test]
fn tab_after_clicking_a_button_moves_focus_on() {
    let mut h = harness();
    h.get_by_label("One").click();
    h.run();
    // the pointer stays on "One"
    h.key_press(Key::Tab);
    h.run();
    assert_eq!(focused(&h).as_deref(), Some("Two"), "Tab moved focus past the hovered button");
    h.key_press(Key::Tab);
    h.run();
    assert_eq!(focused(&h).as_deref(), Some("Three"));
}

#[test]
fn a_key_with_the_pointer_on_another_button_reaches_the_focused_one() {
    let mut h = harness();
    h.get_by_label("One").focus();
    h.run();
    h.get_by_label("Three").hover();
    h.run();
    h.key_press(Key::Tab);
    h.run();
    assert_eq!(focused(&h).as_deref(), Some("Two"), "the hovered button did not take the key");
}
