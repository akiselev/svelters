//! The run path on presto's app runner, headless (presto-test's `Harness` on `HeadlessPlatform`): a
//! window JS presents opens as a platform window with the JS-built node as its root, and a click on
//! its button reaches JS.

mod common;

use common::Rig;
use svelters_abi::*;
use svelters_presto::app::{PrestoDocument, install_widget_services};

#[test]
fn a_presented_window_opens_and_its_button_clicks_reach_js() {
    let r = Rig::new();
    let fonts = presto_text::FontStore::new(presto_text::FontConfig::testing());
    install_widget_services(&r.backend, presto_text::TextSystem::new(fonts));
    let w = r.create("window");
    r.set(w, "title", Value::Str("Counter".into()));
    let b = r.create("button");
    r.set(b, "text", Value::Str("Increment".into()));
    r.listen(b, "clicked");
    r.insert(w, b);
    r.call(w, "present");
    let mut doc = PrestoDocument::new(r.backend.clone());
    let presented = r.backend.take_presents();
    assert_eq!(presented, [w]);
    for h in presented {
        doc.opening(h);
    }
    let mut h = presto_test::Harness::new(doc);
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
    let mut doc = PrestoDocument::new(r.backend.clone());
    for h in r.backend.take_presents() {
        doc.opening(h);
    }
    let h = presto_test::Harness::new(doc);
    let window = h.window();
    drop(h);
    let mut doc = PrestoDocument::new(r.backend.clone());
    presto::seams::DocumentApi::close_window(&mut doc, window);
    assert!(r.backend.windows().is_empty());
    assert_eq!(r.take().gone, [(w, SVL_GONE_CLOSED)]);
}
