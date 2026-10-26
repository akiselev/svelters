//! Text fields through AccessKit's own consumers (`plans/cloud/qc-a11y/PLAN-focus-and-text.md` §3.1,
//! §5): skelter-widgets' `text-edit` sends `Role::TextRun` children and its selection, so the consumer
//! tree has text ranges and AT-SPI exports Text and EditableText, reads what was typed, sees the
//! selection and moves it.

use skelter::CoreDocument;
use skelter_base::geom::Size;
use skelter_base::ids::NodeId;
use skelter_base::key::Key;
use skelter_core::{Document, KindId, KindRegistry, StyleInput, Value, common};
use skelter_test::Harness;
use std::sync::Arc;

fn text() -> skelter_text::TextSystem {
    skelter_text::TextSystem::new(skelter_text::FontStore::new(skelter_text::FontConfig::testing()))
}

/// A window with one `text-edit` (author id `field`, named "Name") holding `value`, with `props` set.
fn harness(value: &str, props: &[(&str, Value)]) -> (Harness<CoreDocument>, NodeId) {
    let mut reg = KindRegistry::new();
    skelter_widgets::host::register_tier0(&mut reg).unwrap();
    reg.register::<skelter_widgets::text::TextEdit>().unwrap();
    let doc = Document::new(Arc::new(reg), Box::new(text()));
    let doc = CoreDocument::with_widgets(
        doc,
        KindId::of("box"),
        text(),
        skelter_widgets::host::os_policy(),
        skelter_widgets::theme::Theme::light_modern(),
    );
    let mut h = Harness::<CoreDocument>::builder().with_size(Size::new(400.0, 300.0)).with_consumer(true).build(doc);
    let w = h.window();
    let d = &mut h.state_mut().doc;
    let root = d.window_root(w).expect("the window's root");
    let f = d.create(KindId::of("text-edit"));
    d.set_style(f, StyleInput { inline: Some("width: 300px; height: 60px".into()), ..Default::default() });
    let spec = &skelter_widgets::text::kind::SPEC;
    for (name, v) in props {
        d.set_prop(f, spec.prop_named(name).unwrap(), v.clone()).unwrap();
    }
    d.set_prop(f, spec.prop_named("text").unwrap(), Value::from(value)).unwrap();
    d.set_prop(f, common::ANCHOR, Value::from("field")).unwrap();
    d.set_prop(f, common::ARIA_LABEL, Value::from("Name")).unwrap();
    d.insert(root, f, None, None).unwrap();
    // the tree changed outside input: a key the document ignores brings the frame
    h.key_press(Key::F24);
    h.run();
    (h, f)
}

/// Focuses the field through AccessKit and puts the caret at the end, as AT does (`SetTextSelection`
/// on the last run): the same on every platform (focus selects all on macOS and GTK, and End does
/// not move the caret on macOS).
fn focus_at_end(h: &mut Harness<CoreDocument>) {
    use skelter_base::accesskit::{Action, ActionData, ActionRequest, TextPosition, TextSelection, TreeId};
    h.get_by_author_id("field").focus();
    h.run();
    let req = {
        let c = h.consumer();
        let n = c.by_author_id("field").unwrap();
        let last = n.children().last().expect("a run");
        let end = TextPosition { node: last.locate().0, character_index: last.data().character_lengths().len() };
        ActionRequest {
            action: Action::SetTextSelection,
            target_tree: TreeId::ROOT,
            target_node: n.locate().0,
            data: Some(ActionData::SetTextSelection(TextSelection { anchor: end, focus: end })),
        }
    };
    h.accesskit_action(req);
    h.run();
}

#[test]
fn a_focused_field_has_text_ranges_that_follow_typing_and_selection() {
    let (mut h, _) = harness("hello wörld", &[]);
    focus_at_end(&mut h);
    {
        let c = h.consumer();
        let n = c.by_author_id("field").unwrap();
        assert!(n.supports_text_ranges(), "TextRun children make text ranges");
        assert_eq!(n.document_range().text(), "hello wörld");
        assert_eq!(c.focus_author_id(), Some("field"));
    }
    #[cfg(target_os = "linux")]
    let _ = h.consumer().atspi().unwrap().take_events();
    h.type_text("!");
    h.run();
    h.key_press_modifiers(skelter_base::event::Modifiers::SHIFT, Key::ArrowLeft);
    h.run();
    let c = h.consumer();
    let n = c.by_author_id("field").unwrap();
    assert_eq!(n.document_range().text(), "hello wörld!");
    assert_eq!(n.text_selection().map(|r| r.text()).as_deref(), Some("!"), "Shift+Left selects the typed character");
    #[cfg(target_os = "linux")]
    {
        use accesskit_atspi_common::{Event, ObjectEvent};
        use skelter_test::consumer::Interface;
        let a = c.atspi().unwrap();
        let ifs = a.interfaces(n.id());
        assert!(ifs.contains(Interface::Text) && ifs.contains(Interface::EditableText), "{ifs:?}");
        let p = a.node(n.id());
        assert_eq!(p.character_count().unwrap(), 12);
        assert_eq!(p.text(0, -1).unwrap(), "hello wörld!");
        assert_eq!(p.selection(0).unwrap(), (11, 12));
        let events = &a.log().events;
        let inserted = events.iter().any(|e| matches!(e, Event::Object { event: ObjectEvent::TextInserted { content, .. }, .. } if content == "!"));
        let selection = events.iter().any(|e| matches!(e, Event::Object { event: ObjectEvent::TextSelectionChanged, .. }));
        assert!(inserted && selection, "TextInserted and TextSelectionChanged: {events:?}");
    }
}

#[test]
fn an_empty_field_still_has_text() {
    let (mut h, _) = harness("", &[]);
    focus_at_end(&mut h);
    let c = h.consumer();
    let n = c.by_author_id("field").unwrap();
    assert!(n.supports_text_ranges(), "one empty run");
    assert_eq!(n.document_range().text(), "");
    #[cfg(target_os = "linux")]
    {
        use skelter_test::consumer::Interface;
        let a = c.atspi().unwrap();
        assert!(a.interfaces(n.id()).contains(Interface::Text));
        assert_eq!(a.node(n.id()).character_count().unwrap(), 0);
    }
}

#[test]
fn a_600_character_line_is_three_linked_runs() {
    let line = "abcdefghij".repeat(60);
    let (h, _) = harness(&line, &[]);
    let c = h.consumer();
    let n = c.by_author_id("field").unwrap();
    let runs: Vec<_> = n.children().filter(|r| r.role() == skelter_base::accesskit::Role::TextRun).collect();
    assert_eq!(runs.len(), 3);
    assert_eq!(runs.iter().map(|r| r.data().character_lengths().len()).collect::<Vec<_>>(), [255, 255, 90]);
    assert_eq!(runs[0].data().next_on_line(), Some(runs[1].locate().0));
    assert_eq!(runs[1].data().previous_on_line(), Some(runs[0].locate().0));
    assert_eq!(runs[1].data().next_on_line(), Some(runs[2].locate().0));
    assert_eq!(runs[2].data().next_on_line(), None);
    assert_eq!(n.document_range().text(), line);
}

#[test]
fn a_password_field_sends_no_clear_text() {
    let (mut h, _) = harness("hunter2", &[("password", Value::Bool(true))]);
    focus_at_end(&mut h);
    let c = h.consumer();
    let n = c.by_author_id("field").unwrap();
    assert!(n.supports_text_ranges());
    let mut sent: Vec<String> = n.data().value().map(str::to_owned).into_iter().collect();
    sent.extend(n.children().filter_map(|r| r.data().value().map(str::to_owned)));
    assert!(!sent.is_empty());
    for v in &sent {
        assert!(!v.contains("hunter") && !v.contains('2'), "clear text left the process: {v:?}");
    }
    assert_eq!(n.document_range().text().chars().count(), 7, "one mask a character");
}

#[test]
fn set_text_selection_moves_the_editor_selection() {
    let (mut h, f) = harness("hello world", &[]);
    focus_at_end(&mut h);
    // AT-SPI's setSelection(0, 6, 11), as a screen reader calls it, becomes AccessKit's SetTextSelection
    #[cfg(target_os = "linux")]
    let req = {
        let c = h.consumer();
        let a = c.atspi().unwrap();
        let id = c.by_author_id("field").unwrap().id();
        assert!(a.node(id).set_selection(0, 6, 11).unwrap());
        let mut reqs = a.take_actions();
        assert_eq!(reqs.len(), 1);
        reqs.remove(0)
    };
    #[cfg(not(target_os = "linux"))]
    let req = {
        let c = h.consumer();
        let n = c.by_author_id("field").unwrap();
        let run = n.children().next().unwrap().locate().0;
        let at = |c| skelter_base::accesskit::TextPosition { node: run, character_index: c };
        let sel = skelter_base::accesskit::TextSelection { anchor: at(6), focus: at(11) };
        skelter_base::accesskit::ActionRequest {
            action: skelter_base::accesskit::Action::SetTextSelection,
            target_tree: skelter_base::accesskit::TreeId::ROOT,
            target_node: n.locate().0,
            data: Some(skelter_base::accesskit::ActionData::SetTextSelection(sel)),
        }
    };
    assert_eq!(req.action, skelter_base::accesskit::Action::SetTextSelection);
    h.accesskit_action(req);
    h.run();
    let s = h.state().doc.get_prop(f, skelter_widgets::text::kind::SPEC.prop_named("text").unwrap());
    assert_eq!(s, Some(Value::from("hello world")), "the text is unchanged");
    let c = h.consumer();
    let n = c.by_author_id("field").unwrap();
    assert_eq!(n.text_selection().map(|r| r.text()).as_deref(), Some("world"));
}
