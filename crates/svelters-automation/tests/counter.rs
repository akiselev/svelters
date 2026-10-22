//! AU1 on the counter fixture (`fixtures/counter-egui`, compiled live from source): selectors,
//! reads, semantic and synthetic acts, waits on virtual time, the host idle predicate, the
//! journal, the JSON path, refs and epochs.

use serde_json::json;
use std::cell::Cell;
use std::rc::Rc;
use svelters_automation::{ActOptions, Expect, Session, TextMatch, Tier, WaitState};
use svelters_egui::harness::TestApp;

fn app() -> Session<TestApp> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/counter-egui");
    let mut s = Session::for_test(TestApp::from_source(&dir).unwrap());
    s.wait_settled(10_000.0).unwrap();
    s
}

fn semantic() -> ActOptions {
    ActOptions {
        tier: Tier::Semantic,
        ..ActOptions::default()
    }
}

fn text(t: &str) -> Expect {
    Expect::Text(TextMatch::Exact(t.into()))
}

#[test]
fn engines_resolve_the_same_node() {
    let s = app();
    let by_role = s.locate(r#"role=button[name="Increment"]"#).unwrap();
    assert_eq!(
        s.locate("text=increment").unwrap(),
        by_role,
        "text= ignores case for a bare value"
    );
    assert_eq!(
        s.locate("button").unwrap(),
        by_role,
        "css engine (the default)"
    );
    assert_eq!(s.locate("css=button >> nth=0").unwrap(), by_role);
    let info = s.node(by_role).unwrap();
    assert_eq!(
        (info.role, info.name.as_str(), info.tag.as_str()),
        ("button", "Increment", "button")
    );
    assert!(info.rect.is_some_and(|r| r.area() > 0.0), "{info:?}");

    // label= finds inputs by accessible name: the text edit's hint, the slider's accessible-name
    assert_eq!(
        s.node(s.locate(r#"label="Name""#).unwrap()).unwrap().role,
        "textbox"
    );
    assert_eq!(
        s.node(s.locate("role=slider[name=Level]").unwrap())
            .unwrap()
            .value
            .as_deref(),
        Some("20")
    );
    // the checkbox is unchecked; a state filter excludes it
    assert!(s.query("role=checkbox[checked]", None).unwrap().is_empty());
    assert_eq!(s.query("role=checkbox", None).unwrap().len(), 1);

    // the same selector gives the same node id through the JSON path (SL5)
    let mut s = s;
    let j = s
        .call(
            "scene.query",
            &json!({"selector": "role=button[name=\"Increment\"]"}),
        )
        .unwrap();
    assert_eq!(j[0]["ref"]["id"].as_u64(), Some(by_role.id.to_raw()));
    assert_eq!(j[0]["name"], "Increment");

    // strictness and errors
    let err = s.locate("role=label").unwrap_err();
    assert_eq!(err.code, "node.ambiguous", "{err}");
    assert_eq!(
        s.locate("role=button[name=\"Nope\"]").unwrap_err().code,
        "node.not_found"
    );
    assert_eq!(
        s.locate("component=App").unwrap_err().code,
        "capability.absent"
    );
    assert_eq!(
        s.call("scene.nope", &json!({})).unwrap_err().code,
        "method.unknown"
    );
}

#[test]
fn acts_reach_js_and_reply_after_settle() {
    let mut s = app();
    // synthetic: a press and release at the button's centre
    let r = s
        .click("role=button[name=Increment]", ActOptions::default())
        .unwrap();
    assert_eq!(r.tier, Tier::Synthetic);
    assert!(!r.changed.is_empty(), "the reply carries what changed");
    // the reply comes after the settle: the label already reads the new count
    assert_eq!(s.query(r#"text="Count: 1""#, None).unwrap().len(), 1);
    // semantic: an AccessKit Click
    let r = s.click("role=button[name=Increment]", semantic()).unwrap();
    assert_eq!(r.tier, Tier::Semantic);
    s.wait_for(
        "role=label",
        WaitState::Visible,
        Some(&text("Count: 2")),
        1_000.0,
    )
    .unwrap();

    // fill: AccessKit SetValue, then synthetic typing (focus, select all, text)
    s.fill(r#"label="Name""#, "Ann", semantic()).unwrap();
    s.wait_for(
        "role=label",
        WaitState::Visible,
        Some(&text("Hello Ann")),
        1_000.0,
    )
    .unwrap();
    s.fill(r#"label="Name""#, "Bo", ActOptions::default())
        .unwrap();
    s.wait_for(
        "role=label",
        WaitState::Visible,
        Some(&text("Hello Bo")),
        1_000.0,
    )
    .unwrap();

    // a numeric value, then a checkbox through the JSON path
    s.set_value("role=slider[name=Level]", 42.0, ActOptions::default())
        .unwrap();
    let cb = s
        .call(
            "wait.for",
            &json!({"selector": "role=checkbox", "expect": {"text": "Level 42, agree: no"}}),
        )
        .unwrap();
    assert_eq!(cb.as_array().map(Vec::len), Some(1));
    let r = s
        .call(
            "input.click",
            &json!({"selector": "role=checkbox", "tier": "semantic"}),
        )
        .unwrap();
    assert_eq!(r["tier"], "semantic");
    s.wait_for(
        "role=checkbox[checked]",
        WaitState::Visible,
        Some(&text("Level 42, agree: yes")),
        1_000.0,
    )
    .unwrap();

    let snap = s.snapshot(None).unwrap();
    for line in [
        r#"- button "Increment""#,
        r#"- textbox "Name": Bo"#,
        r#"- slider "Level": 42"#,
        r#"- checkbox "Level 42, agree: yes" [checked]"#,
        r#"- label "Count: 2""#,
    ] {
        assert!(snap.contains(line), "missing `{line}` in\n{snap}");
    }
    assert!(s.driver().errors().is_empty(), "{:?}", s.driver().errors());
}

#[test]
fn waits_count_virtual_time_and_the_host_predicate() {
    let mut s = app();
    // 30 s of virtual time costs frames, not wall time
    let t0 = std::time::Instant::now();
    let err = s
        .wait_for(r#"text="Count: 99""#, WaitState::Visible, None, 30_000.0)
        .unwrap_err();
    let wall = t0.elapsed();
    assert_eq!(err.code, "timeout", "{err}");
    assert!(s.now_ms() >= 30_000.0, "{}", s.now_ms());
    eprintln!("30 s virtual wait took {wall:?} wall");
    assert!(wall.as_secs_f64() < 10.0, "{wall:?}");

    // the host's idle predicate holds waits until it is idle (cadatomic's IdleBarrier)
    let calls = Rc::new(Cell::new(0));
    let c = calls.clone();
    s.set_idle(move || {
        c.set(c.get() + 1);
        c.get() > 3
    });
    let frames = s.wait_settled(10_000.0).unwrap();
    assert!(frames >= 3, "{frames}");
    // a predicate that never goes idle times out, naming the host
    s.set_idle(|| false);
    let err = s.wait_settled(1_000.0).unwrap_err();
    assert!(err.message.contains("HOST"), "{err}");
}

#[test]
fn refs_journal_and_tooling_roots() {
    let mut s = app();
    let r = s.locate("role=button[name=Increment]").unwrap();
    s.journal_subscribe(Default::default()).unwrap();
    s.click(r, ActOptions::default()).unwrap();
    let (changes, overflowed) = s.journal_take().unwrap();
    assert!(!changes.is_empty() && !overflowed);

    // a new reload epoch makes old refs stale (§1.3); locators re-resolve
    s.bump_epoch();
    assert_eq!(s.node(r).unwrap_err().code, "node.stale");
    assert_eq!(
        s.click(r, ActOptions::default()).unwrap_err().code,
        "node.stale"
    );
    s.click("role=button[name=Increment]", ActOptions::default())
        .unwrap();

    // the app's window root has no key: it is never a tooling root (tooling roots are keyed
    // embed roots, the inspector's and agents', §1.2)
    let roots = s.roots().unwrap();
    assert!(
        roots.len() == 1 && roots[0].key.is_none() && !roots[0].tooling,
        "{roots:?}"
    );
    s.set_tooling_roots(|_| true);
    assert_eq!(s.query("role=button", None).unwrap().len(), 1);
}
