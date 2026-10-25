//! The native menu bar on a real document: presto-widgets' `menu-bar` kind publishes its model, the
//! runner hands it to the platform (`set_menu_bar`), hides the in-window bar when the platform shows it
//! natively (`native-bar`), and routes the platform's picks back to the kind (`menu-activated`).
//!
//! The bar's model is not built from its element children in a document yet (presto-widgets' views:
//! "the tree walk is presto-core's"), so the bar here is empty: these tests pin the runner's side.

use presto::CoreDocument;
use presto::platform::{MenuDesc, PlatformCall};
use presto_base::geom::Size;
use presto_core::{Document, KindId, KindRegistry, Value};
use presto_test::Harness;
use std::sync::Arc;

fn text() -> presto_text::TextSystem {
    presto_text::TextSystem::new(presto_text::FontStore::new(
        presto_text::FontConfig::testing(),
    ))
}

/// A window whose root holds a `menu-bar`.
fn harness(native: bool) -> (Harness<CoreDocument>, presto_base::ids::NodeId) {
    let mut reg = KindRegistry::new();
    presto_widgets::host::register_tier0(&mut reg).unwrap();
    presto_widgets::views::register_all(&mut reg).unwrap();
    let doc = Document::new(Arc::new(reg), Box::new(text()));
    let doc = CoreDocument::with_widgets(
        doc,
        KindId::of("box"),
        text(),
        presto_widgets::host::os_policy(),
        presto_widgets::theme::Theme::light_modern(),
    );
    let mut h = Harness::<CoreDocument>::builder()
        .with_size(Size::new(400.0, 300.0))
        .with_native_menu_bar(native)
        .build(doc);
    let w = h.window();
    let d = &mut h.state_mut().doc;
    let root = d.window_root(w).expect("the window's root");
    let bar = d.create(KindId::of("menu-bar"));
    d.insert(root, bar, None, None).unwrap();
    // The tree changed outside input: a key the document ignores brings the frame.
    h.key_press(presto_base::key::Key::F24);
    h.run();
    (h, bar)
}

fn menu_bars(h: &Harness<CoreDocument>) -> Vec<Option<MenuDesc>> {
    h.calls()
        .iter()
        .filter_map(|c| match c {
            PlatformCall::SetMenuBar(m) => Some(m.clone()),
            _ => None,
        })
        .collect()
}

fn native_bar(h: &Harness<CoreDocument>, bar: presto_base::ids::NodeId) -> Option<Value> {
    let d = &h.state().doc;
    let p = d
        .registry()
        .get(KindId::of("menu-bar"))
        .and_then(|e| e.spec.prop_named("native-bar"))
        .unwrap();
    d.get_prop(bar, p)
}

#[test]
fn the_menu_bar_reaches_the_platform_and_hides_itself_when_native() {
    let (h, bar) = harness(true);
    let sent = menu_bars(&h);
    assert_eq!(
        sent.last(),
        Some(&Some(MenuDesc::default())),
        "the (empty) bar went to the platform once: {sent:?}"
    );
    assert_eq!(sent.len(), 1, "sent again only when it changes");
    assert_eq!(native_bar(&h, bar), Some(Value::Bool(true)));
}

#[test]
fn without_a_native_menu_bar_the_in_window_bar_stays() {
    let (h, bar) = harness(false);
    assert!(
        !menu_bars(&h).is_empty(),
        "the bar was offered to the platform"
    );
    assert_ne!(native_bar(&h, bar), Some(Value::Bool(true)));
}

#[test]
fn a_native_pick_reaches_the_menu_bar_kind() {
    let (mut h, _bar) = harness(true);
    use presto::seams::DocumentApi;
    // The call reaches the kind's `menu-activated` arm, which knows no such item in an empty bar.
    assert!(!h.state_mut().menu_activated("new"));
    h.menu_activated("new");
    h.run();
}
