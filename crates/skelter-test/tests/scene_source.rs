//! `SkelterScene`: svelters-automation's selectors and text snapshot on a skelter-core document through
//! `svelters_automation::SceneSource`, no svelters-egui scene involved.

use skelter::CoreDocument;
use skelter_base::geom::Size;
use skelter_base::ids::NodeId;
use skelter_core::testkinds::{FixedText, TEXT, registry};
use skelter_core::{Document, KindId, StyleInput, Value, common};
use skelter_test::{Harness, SkelterScene};
use svelters_automation::source::{actionable_point, query, snapshot};
use svelters_automation::SceneSource;

/// A window: a `text` "Settings", a `toggle` "Agree" (anchor `agree`) and a disabled `toggle` "Locked".
fn harness() -> (Harness<CoreDocument>, NodeId) {
    let mut doc = Document::new(registry(), Box::new(FixedText));
    doc.set_text_shaper(Box::new(FixedText));
    let mut h = Harness::<CoreDocument>::builder()
        .with_size(Size::new(400.0, 300.0))
        .with_accessibility_check(false)
        .build(CoreDocument::new(doc, KindId::of("box")));
    let w = h.window();
    let d = &mut h.state_mut().doc;
    let root = d.window_root(w).unwrap();
    let add = |d: &mut Document, kind: &str| {
        let n = d.create(KindId::of(kind));
        d.set_style(n, StyleInput { inline: Some("width: 100px; height: 20px".into()), ..Default::default() });
        d.insert(root, n, None, None).unwrap();
        n
    };
    let title = add(d, "text");
    d.set_prop(title, TEXT, Value::from("Settings")).unwrap();
    let agree = add(d, "toggle");
    d.set_prop(agree, common::ARIA_LABEL, Value::from("Agree")).unwrap();
    d.set_prop(agree, common::ANCHOR, Value::from("agree")).unwrap();
    let locked = add(d, "toggle");
    d.set_prop(locked, common::ARIA_LABEL, Value::from("Locked")).unwrap();
    d.set_prop(locked, common::DISABLED, Value::Bool(true)).unwrap();
    // The tree changed outside input: a key the document ignores brings the frame.
    h.key_press(skelter_base::key::Key::F24);
    h.run();
    (h, agree)
}

#[test]
fn selectors_resolve_on_a_skelter_document() {
    let (h, agree) = harness();
    let scene = SkelterScene::new(h.a11y()).with_document(&h.state().doc);
    let key = |n: NodeId| (skelter_base::accesskit::TreeId::ROOT, skelter_base::accesskit::NodeId(n.0));

    assert_eq!(query(&scene, r#"role=checkbox[name="Agree"]"#).unwrap(), vec![key(agree)]);
    assert_eq!(query(&scene, "role=checkbox").unwrap().len(), 2);
    assert_eq!(query(&scene, "role=checkbox[disabled]").unwrap().len(), 1);
    assert_eq!(query(&scene, "label=agree").unwrap(), vec![key(agree)]);
    assert_eq!(query(&scene, "anchor=agree").unwrap(), vec![key(agree)]);
    assert_eq!(query(&scene, "text=settings").unwrap().len(), 1);
    assert_eq!(query(&scene, "role=checkbox >> nth=-1").unwrap().len(), 1);
    // CSS through the document's matcher (tags are kind names).
    assert_eq!(query(&scene, "toggle").unwrap().len(), 2);
    assert_eq!(query(&scene, "toggle:disabled").unwrap().len(), 1);
    // `pos=` hits the topmost node at the point.
    let r = scene.rect(key(agree)).unwrap();
    assert_eq!(query(&scene, &format!("pos={},{}", r.center().x, r.center().y)).unwrap(), vec![key(agree)]);

    // The synthetic act's point is the centre; a disabled node is not actionable.
    assert_eq!(actionable_point(&scene, key(agree)).unwrap(), r.center());
    let locked = query(&scene, "role=checkbox[disabled]").unwrap()[0];
    assert_eq!(actionable_point(&scene, locked).unwrap_err().code, "node.not_actionable");
}

#[test]
fn the_snapshot_reads_a_skelter_document() {
    let (mut h, agree) = harness();
    let text = snapshot(&SkelterScene::new(h.a11y()), None).unwrap();
    assert!(text.contains("- label \"Settings\""), "{text}");
    assert!(text.contains("- checkbox \"Agree\"\n"), "{text}");
    assert!(text.contains("- checkbox \"Locked\" [disabled]"), "{text}");

    // A click through skelter's input checks the box; the next snapshot shows it.
    let r = SkelterScene::new(h.a11y()).rect((skelter_base::accesskit::TreeId::ROOT, skelter_base::accesskit::NodeId(agree.0))).unwrap();
    h.click_at(egui::pos2(r.center().x, r.center().y));
    h.run();
    let text = snapshot(&SkelterScene::new(h.a11y()), Some("anchor=agree")).unwrap();
    assert_eq!(text, "- checkbox \"Agree\" [checked]\n");
    // Without the document there is no CSS matcher.
    assert_eq!(query(&SkelterScene::new(h.a11y()), "toggle").unwrap_err().code, "capability.absent");
}
