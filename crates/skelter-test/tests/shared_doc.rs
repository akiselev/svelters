//! `CoreDocument::shared` (A11): the runner over the document a script host also holds
//! (`SkelterBackend::shared`), borrowed per call only.

use skelter::CoreDocument;
use skelter_base::geom::Size;
use skelter_core::{Document, KindId, KindRegistry, Value, common};
use skelter_test::Harness;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

fn text() -> skelter_text::TextSystem {
    skelter_text::TextSystem::new(skelter_text::FontStore::new(skelter_text::FontConfig::testing()))
}

#[test]
fn the_runner_and_the_script_host_share_one_document() {
    let mut reg = KindRegistry::new();
    skelter_widgets::host::register_tier0(&mut reg).unwrap();
    let mut doc = Document::new(Arc::new(reg), Box::new(text()));
    skelter_widgets::host::install(&mut doc, text(), skelter_widgets::host::os_policy(), skelter_widgets::theme::Theme::light_modern());
    let held = Rc::new(RefCell::new(doc));
    let mut h = Harness::<CoreDocument<Rc<RefCell<Document>>>>::builder()
        .with_size(Size::new(300.0, 200.0))
        .with_accessibility_check(false)
        .build(CoreDocument::shared(held.clone(), KindId::of("box")));
    let w = h.window();
    // the script host writes between the runner's calls: the runner holds no borrow
    {
        let mut d = held.borrow_mut();
        let root = d.window_root(w).expect("the runner opened the window in the shared document");
        let b = d.create(KindId::of("button"));
        d.set_prop(b, common::ARIA_LABEL, Value::from("Shared")).unwrap();
        d.insert(root, b, None, None).unwrap();
    }
    h.key_press(skelter_base::key::Key::F24);
    h.run();
    assert!(h.query_by_label("Shared").is_some(), "the runner framed the script host's node");
    assert!(held.try_borrow_mut().is_ok(), "no borrow is left held after a run");
}

#[test]
fn an_applied_commit_clears_the_documents_commit_in_flight() {
    let mut reg = KindRegistry::new();
    skelter_widgets::host::register_tier0(&mut reg).unwrap();
    let doc = Document::new(Arc::new(reg), Box::new(text()));
    let mut h = Harness::<CoreDocument>::builder().with_size(Size::new(100.0, 50.0)).with_accessibility_check(false).build(CoreDocument::new(doc, KindId::of("box")));
    h.key_press(skelter_base::key::Key::F24);
    h.run();
    assert!(
        !h.state().doc.busy().contains(skelter_core::Busy::COMMIT_IN_FLIGHT),
        "the runner forwards CommitApplied (A12): {:?}",
        h.state().doc.busy()
    );
}
