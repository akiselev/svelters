//! The run path on skelter's app runner, headless (skelter-test's `Harness` on `HeadlessPlatform`): a
//! window JS presents opens as a platform window with the JS-built node as its root, and a click on
//! its button reaches JS.

mod common;

use common::Rig;
use svelters_abi::*;
use svelters_skelter::app::{SkelterDocument, install_widget_services};

#[test]
fn a_presented_window_opens_and_its_button_clicks_reach_js() {
    let r = Rig::new();
    let fonts = skelter_text::FontStore::new(skelter_text::FontConfig::testing());
    install_widget_services(&r.backend, skelter_text::TextSystem::new(fonts));
    let w = r.create("window");
    r.set(w, "title", Value::Str("Counter".into()));
    let b = r.create("button");
    r.set(b, "text", Value::Str("Increment".into()));
    r.listen(b, "clicked");
    r.insert(w, b);
    r.call(w, "present");
    let mut doc = SkelterDocument::new(r.backend.clone());
    let presented = r.backend.take_presents();
    assert_eq!(presented, [w]);
    for h in presented {
        doc.opening(h);
    }
    let mut h = skelter_test::Harness::new(doc);
    assert_eq!(r.backend.window_id(w), Some(h.window()), "the harness window's root is the JS-built node");
    r.take();
    h.get_by_label("Increment").click();
    h.run();
    let clicked = r.signal_id(b, "clicked");
    assert!(r.take().events.iter().any(|e| e.0 == b && e.1 == clicked), "the click reached JS");
}

#[test]
fn closing_the_window_frees_its_handle() {
    let r = Rig::new();
    let w = r.create("window");
    r.call(w, "present");
    let mut doc = SkelterDocument::new(r.backend.clone());
    for h in r.backend.take_presents() {
        doc.opening(h);
    }
    let h = skelter_test::Harness::new(doc);
    let window = h.window();
    drop(h);
    let mut doc = SkelterDocument::new(r.backend.clone());
    skelter::seams::DocumentApi::close_window(&mut doc, window);
    assert!(r.backend.windows().is_empty());
    assert_eq!(r.take().gone, [(w, SVL_GONE_CLOSED)]);
}

/// FQ-65: a click on a checkbox goes to JS at event time (its `changed` is cancelable) as a call
/// with `SVL_SIGNAL_ARGS` and the value alone, and a `true` answer (`preventDefault`) keeps it
/// unchecked.
#[test]
fn a_checkbox_click_calls_changed_with_its_value_and_can_be_cancelled() {
    let r = Rig::new();
    let fonts = skelter_text::FontStore::new(skelter_text::FontConfig::testing());
    install_widget_services(&r.backend, skelter_text::TextSystem::new(fonts));
    let w = r.create("window");
    let c = r.create("checkbox");
    r.set(c, "text", Value::Str("Copper".into()));
    r.listen(c, "changed");
    r.insert(w, c);
    r.call(w, "present");
    let mut doc = SkelterDocument::new(r.backend.clone());
    for h in r.backend.take_presents() {
        doc.opening(h);
    }
    let mut h = skelter_test::Harness::new(doc);
    r.take();
    r.log.borrow_mut().sync_answer = Some(Value::Null);
    h.get_by_label("Copper").click();
    h.run();
    let wire = r.signal_id(c, "changed") | SVL_SIGNAL_ARGS << SVL_SIGNAL_FLAGS_SHIFT;
    let l = r.take();
    assert!(l.syncs.iter().any(|s| s.0 == c && s.1 == wire && s.2 == [Value::Bool(true)]), "{:?}", l.syncs);
    assert_eq!(r.get(c, "checked"), Value::Bool(true));
    r.log.borrow_mut().sync_answer = Some(Value::Bool(true));
    h.get_by_label("Copper").click();
    h.run();
    assert_eq!(r.get(c, "checked"), Value::Bool(true), "cancelled: still checked");
}

/// FQ-76: the backend counts its JS traffic both ways, so a test asserts that an interaction made
/// no JS round trip (a hover over a button JS only listens to for clicks) and that one did (the click).
#[test]
fn the_backend_counts_its_js_traffic() {
    let r = Rig::new();
    let fonts = skelter_text::FontStore::new(skelter_text::FontConfig::testing());
    install_widget_services(&r.backend, skelter_text::TextSystem::new(fonts));
    let w = r.create("window");
    let b = r.create("button");
    r.set(b, "text", Value::Str("Run".into()));
    r.listen(b, "clicked");
    r.insert(w, b);
    r.call(w, "present");
    let t = r.backend.take_traffic();
    assert!(t.creates == 2 && t.sets >= 1 && t.listens == 1 && t.tree == 1 && t.calls == 1, "{t:?}");
    let mut doc = SkelterDocument::new(r.backend.clone());
    for h in r.backend.take_presents() {
        doc.opening(h);
    }
    let mut h = skelter_test::Harness::new(doc);
    h.run();
    r.backend.take_traffic();
    h.get_by_label("Run").hover();
    h.run();
    let t = r.backend.take_traffic();
    assert_eq!(t.to_js(), 0, "a hover is native only: {t:?}");
    h.get_by_label("Run").click();
    h.run();
    let t = r.backend.take_traffic();
    assert!(t.posts >= 1 && t.checkpoints >= 1, "the click reached JS: {t:?}");
    assert_eq!(t.from_js(), 0, "the mock host asked nothing back: {t:?}");
}

/// A combo-box's `option-anchors` name its options for AccessKit (their author ids), as egui's
/// ComboBox gave each option an id: a tour finds an option by anchor and clicks it, which picks it.
#[test]
fn combo_box_options_take_anchors() {
    let r = Rig::new();
    let fonts = skelter_text::FontStore::new(skelter_text::FontConfig::testing());
    install_widget_services(&r.backend, skelter_text::TextSystem::new(fonts));
    let w = r.create("window");
    let c = r.create("combo-box");
    r.set(c, "options", Value::List(vec![Value::Str("LED_0603".into()), Value::Str("LED_0805".into())]));
    r.set(c, "option-anchors", Value::List(vec![Value::Str("fp:LED_0603".into()), Value::Str("fp:LED_0805".into())]));
    r.set(c, "selected", Value::I64(0));
    r.set(c, "accessible-name", Value::Str("Footprint".into()));
    r.listen(c, "changed");
    r.insert(w, c);
    r.call(w, "present");
    let mut doc = SkelterDocument::new(r.backend.clone());
    for h in r.backend.take_presents() {
        doc.opening(h);
    }
    let mut h = skelter_test::Harness::new(doc);
    h.run();
    assert_eq!(h.get_by_author_id("fp:LED_0805").label().as_deref(), Some("LED_0805"));
    h.get_by_author_id("fp:LED_0805").click_accesskit();
    h.run();
    assert_eq!(r.get(c, "selected"), Value::I64(1), "the option by anchor was picked");
}
