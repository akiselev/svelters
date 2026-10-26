//! `SkelterNativeDriver`: svelters-automation's `Session` (acts, waits, journal, JSON methods) on a
//! skelter-core document, no svelters-egui scene involved.

use serde_json::json;
use skelter::CoreDocument;
use skelter_base::geom::Size;
use skelter_core::testkinds::{FixedText, TEXT, registry};
use skelter_core::{Document, KindId, StyleInput, Value, common};
use skelter_test::{Harness, SkelterNativeDriver};
use svelters_automation::{ActOptions, Expect, Session, TextMatch, Tier, WaitState};

/// A window: a `text` "Settings", a `toggle` "Agree" (anchor `agree`) and a disabled `toggle` "Locked".
fn session() -> Session<SkelterNativeDriver<CoreDocument>> {
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
    Session::for_test(SkelterNativeDriver::new(h).with_document(|d: &CoreDocument| &d.doc))
}

#[test]
fn a_click_by_selector_changes_state_and_waits_resolve() {
    let mut s = session();
    assert_eq!(s.query("role=checkbox", None).unwrap().len(), 2);
    // CSS through the document's matcher
    assert_eq!(s.query("toggle:disabled", None).unwrap().len(), 1);
    let agree = s.query("anchor=agree", None).unwrap().remove(0);
    assert_eq!((agree.role, agree.name.as_str(), agree.states.as_slice()), ("checkbox", "Agree", &[][..]));
    assert!(agree.rect.is_some_and(|r| r.is_positive()), "{agree:?}");

    // A synthetic click at the node goes through skelter's input; the reply says what changed.
    s.journal_subscribe(()).unwrap();
    let r = s.click(r#"role=checkbox[name="Agree"]"#, ActOptions::default()).unwrap();
    assert_eq!(r.tier, Tier::Synthetic);
    assert!(r.changed.iter().any(|(n, what)| *n == agree.r#ref.id && what.contains("states")), "{:?}", r.changed);
    let (changes, overflowed) = s.journal_take().unwrap();
    assert!(!overflowed && changes.iter().any(|(n, _)| *n == agree.r#ref.id), "{changes:?}");
    assert!(s.journal_take().unwrap().0.is_empty(), "nothing changed since the last take");

    // Waits on a selector and on text resolve.
    let found = s.wait_for("role=checkbox[checked]", WaitState::Visible, None, 1_000.0).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].r#ref, agree.r#ref);
    let found = s
        .wait_for("text=settings", WaitState::Visible, Some(&Expect::Text(TextMatch::Exact("Settings".into()))), 1_000.0)
        .unwrap();
    assert_eq!(found[0].role, "label");
    assert_eq!(s.snapshot(Some("anchor=agree")).unwrap(), "- checkbox \"Agree\" [checked]\n");
    // A wait that never holds times out on the virtual clock, with its report.
    let e = s.wait_for("role=checkbox[name=\"Nope\"]", WaitState::Attached, None, 200.0).unwrap_err();
    assert_eq!(e.code, "timeout");
    assert!(e.data.is_some());

    // The ref from before the click still names the node.
    // (a click focuses it: the snapshot leaves focus out, the node's states have it)
    assert_eq!(s.node(agree.r#ref).unwrap().states, vec!["checked", "focused"]);

    // A disabled node is not actionable.
    let e = s
        .click(r#"role=checkbox[name="Locked"]"#, ActOptions { timeout_ms: 200.0, ..ActOptions::default() })
        .unwrap_err();
    assert_eq!(e.code, "node.not_actionable");
    assert!(e.message.contains("disabled"), "{}", e.message);
    // No style engine on this scene.
    assert_eq!(s.style_matched(agree.r#ref).unwrap_err().code, "capability.absent");
}

#[test]
fn json_methods_run_on_a_skelter_document() {
    let mut s = session();
    let hello = s.call("session.hello", &json!({})).unwrap();
    assert_eq!(hello["roots"].as_array().map(Vec::len), Some(1));

    let q = s.call("scene.query", &json!({"selector": "role=checkbox"})).unwrap();
    assert_eq!(q.as_array().map(Vec::len), Some(2));
    let r = s.call("scene.locate", &json!({"selector": "anchor=agree"})).unwrap();
    let id = r["id"].as_u64().unwrap();
    assert_eq!(q[0]["ref"]["id"].as_u64(), Some(id), "the same node, the same id");

    let a = s.call("input.click", &json!({"ref": r})).unwrap();
    assert_eq!(a["tier"], "synthetic");
    assert!(a["changed"].as_array().unwrap().iter().any(|c| c["id"].as_u64() == Some(id)), "{a}");
    let n = s.call("scene.node", &json!({"ref": r})).unwrap();
    assert_eq!(n["states"], json!(["checked", "focused"]));
    assert_eq!(n["name"], "Agree");

    let w = s
        .call("wait.for", &json!({"selector": "role=checkbox", "state": "enabled", "expect": {"count": 1}, "timeout": 500}))
        .unwrap_err();
    assert_eq!(w.code, "timeout", "two checkboxes, one disabled");
    let w = s.call("wait.for", &json!({"selector": "role=checkbox[checked]", "expect": {"count": 1}})).unwrap();
    assert_eq!(w[0]["ref"]["id"].as_u64(), Some(id));

    // A click by selector unchecks it again.
    s.call("input.click", &json!({"selector": "anchor=agree"})).unwrap();
    let q = s.call("scene.query", &json!({"selector": "role=checkbox[checked]"})).unwrap();
    assert_eq!(q, json!([]));
    let snap = s.call("scene.snapshot", &json!({})).unwrap();
    assert!(snap["text"].as_str().unwrap().contains("- checkbox \"Agree\"\n"), "{snap}");

    // An unknown id is no ref.
    assert_eq!(s.call("scene.node", &json!({"ref": {"id": 999_999, "epoch": 0}})).unwrap_err().code, "params.invalid");
}
