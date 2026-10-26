//! QA11Y fixes (`plans/cloud/qc-a11y/LANE-fixes.md`), found by cadatomic's live QC through AT-SPI, on a
//! real skelter-core document with skelter-widgets' kinds, read through AccessKit's consumer (and AT-SPI's
//! translation layer on Linux):
//! 1. a `virtual-list`'s rows are list box options (`selected`, position, set size) whatever renders
//!    them, and a field naming the selected row as its active descendant is a combobox AT follows;
//! 2. a host's `focus()` outside a dispatch repaints the focus ring on both buttons;
//! 3. sibling `dialog` scopes that open in one frame: the last in tree order ends with focus.

use skelter::CoreDocument;
use skelter_base::geom::Size;
use skelter_base::ids::NodeId;
use skelter_base::key::Key;
use skelter_core::{Document, FocusReason, KindId, KindRegistry, StyleInput, Value, common};
use skelter_test::Harness;
use std::sync::Arc;

const BOX: KindId = KindId::of("box");
const LIST: KindId = KindId::of("virtual-list");

fn text() -> skelter_text::TextSystem {
    skelter_text::TextSystem::new(skelter_text::FontStore::new(skelter_text::FontConfig::testing()))
}

/// A 400 × 300 window on a document with skelter-widgets' tier 0 kinds, `virtual-list` and `text-edit`.
fn harness() -> Harness<CoreDocument> {
    let mut reg = KindRegistry::new();
    skelter_widgets::host::register_tier0(&mut reg).unwrap();
    skelter_widgets::host::register::<skelter_widgets::views::virtual_list::VirtualList>(&mut reg).unwrap();
    reg.register::<skelter_widgets::text::TextEdit>().unwrap();
    let doc = Document::new(Arc::new(reg), Box::new(text()));
    let doc = CoreDocument::with_widgets(doc, BOX, text(), skelter_widgets::host::os_policy(), skelter_widgets::theme::Theme::light_modern());
    Harness::<CoreDocument>::builder().with_size(Size::new(400.0, 300.0)).with_consumer(true).build(doc)
}

fn add(d: &mut Document, parent: NodeId, kind: KindId, css: &str) -> NodeId {
    let n = d.create(kind);
    d.set_style(n, StyleInput { inline: Some(css.into()), ..Default::default() });
    d.insert(parent, n, None, None).unwrap();
    n
}

fn prop(d: &mut Document, n: NodeId, kind: KindId, name: &str, v: Value) {
    let p = d.registry().get(kind).unwrap().spec.prop_named(name).unwrap();
    d.set_prop(n, p, v).unwrap();
}

fn call(d: &mut Document, n: NodeId, name: &str, args: &[Value]) {
    let m = d.registry().get(LIST).unwrap().spec.method_named(name).unwrap();
    d.call(n, m, args).unwrap();
}

/// The node's anchor (its author id), and its accessible name.
fn name(d: &mut Document, n: NodeId, anchor: &str) {
    d.set_prop(n, common::ANCHOR, Value::from(anchor)).unwrap();
    d.set_prop(n, common::ARIA_LABEL, Value::from(anchor)).unwrap();
}

/// The tree changed outside input: a key the document ignores brings the frame.
fn settle(h: &mut Harness<CoreDocument>) {
    h.key_press(Key::F24);
    h.run();
}

#[test]
fn list_rows_are_options_and_a_combobox_names_the_selected_one() {
    use skelter_base::accesskit::Role;
    let mut h = harness();
    let w = h.window();
    let labels = ["Open", "Save", "Close", "Quit", "Help"];
    let list = {
        let d = &mut h.state_mut().doc;
        let root = d.window_root(w).unwrap();
        let field = add(d, root, KindId::of("text-edit"), "width: 300px; height: 24px");
        d.set_prop(field, common::ANCHOR, Value::from("query")).unwrap();
        d.set_prop(field, common::ARIA_LABEL, Value::from("Command")).unwrap();
        d.set_prop(field, common::ROLE, Value::from("combobox")).unwrap();
        d.set_prop(field, common::ARIA_CONTROLS, Value::from("list")).unwrap();
        let list = add(d, root, LIST, "width: 300px; height: 200px");
        d.set_prop(list, common::ANCHOR, Value::from("list")).unwrap();
        d.set_prop(list, common::ARIA_LABEL, Value::from("Commands")).unwrap();
        prop(d, list, LIST, "row-height", Value::F64(20.0));
        prop(d, list, LIST, "uniform-rows", Value::Bool(true));
        let ids = Value::List(labels.iter().enumerate().map(|(i, _)| Value::I64(10 + i as i64)).collect::<Vec<_>>().into());
        let names = Value::List(labels.iter().map(|l| Value::from(*l)).collect::<Vec<_>>().into());
        call(d, list, "splice", &[Value::I64(0), Value::I64(0), ids, names]);
        // `@svelters/lists`' part: a row outlet per slot (a plain box with a label, not a button)
        for k in 0..labels.len() {
            let row = add(d, list, BOX, "");
            add(d, row, KindId::of("label"), "");
            call(d, list, "setRow", &[Value::I64(k as i64), Value::I64(row.0 as i64)]);
        }
        list
    };
    settle(&mut h);
    // the rows' anchors (the palette's `palette-row:<key>`)
    {
        let d = &mut h.state_mut().doc;
        let rows = d.virtual_rows(list).to_vec();
        assert_eq!(rows.len(), labels.len(), "every row realised");
        for (pos, n) in rows {
            d.set_prop(n, common::ANCHOR, Value::from(format!("opt-{pos}"))).unwrap();
        }
    }
    settle(&mut h);
    let options = |h: &Harness<CoreDocument>| -> Vec<(Role, Option<usize>, Option<usize>, Option<bool>, Option<String>)> {
        let c = h.consumer();
        (0..labels.len())
            .map(|p| {
                let n = c.by_author_id(&format!("opt-{p}")).unwrap();
                let d = n.data();
                (d.role(), d.position_in_set(), d.size_of_set(), d.is_selected(), d.label().map(str::to_owned))
            })
            .collect()
    };
    assert_eq!(h.consumer().by_author_id("list").unwrap().role(), Role::ListBox);
    let o = options(&h);
    for (p, row) in o.iter().enumerate() {
        assert_eq!(*row, (Role::ListBoxOption, Some(p + 1), Some(5), Some(false), Some(labels[p].to_owned())), "row {p}");
    }

    // the selection moves (the palette's `bind:selected`): the rows go out again
    prop(&mut h.state_mut().doc, list, LIST, "selected", Value::I64(2));
    settle(&mut h);
    let sel: Vec<Option<bool>> = options(&h).into_iter().map(|o| o.3).collect();
    assert_eq!(sel, [Some(false), Some(false), Some(true), Some(false), Some(false)]);

    // the row's own props win: `role` and `aria-selected` from Svelte
    {
        let d = &mut h.state_mut().doc;
        let r4 = d.find_anchor(w, "opt-4").unwrap();
        d.set_prop(r4, common::ROLE, Value::from("treeitem")).unwrap();
        d.set_prop(r4, common::ARIA_SELECTED, Value::Enum(1)).unwrap();
    }
    settle(&mut h);
    let o = options(&h);
    assert_eq!((o[4].0, o[4].3), (Role::TreeItem, Some(true)));

    // the combobox pattern: the field keeps focus, AT sees the selected row focused
    {
        let d = &mut h.state_mut().doc;
        let field = d.find_anchor(w, "query").unwrap();
        d.set_prop(field, common::ARIA_ACTIVEDESCENDANT, Value::from("opt-2")).unwrap();
        d.focus(field, FocusReason::Keyboard);
    }
    settle(&mut h);
    assert_eq!(h.state().doc.focused(w), h.state().doc.find_anchor(w, "query"));
    assert_eq!(h.consumer().focus_author_id(), Some("opt-2"), "the active descendant is AT's focus");
    assert_eq!(h.consumer().by_author_id("query").unwrap().role(), Role::ComboBox);

    #[cfg(target_os = "linux")]
    {
        use accesskit_atspi_common::State;
        let c = h.consumer();
        let at = c.atspi().unwrap();
        let s2 = at.node(c.by_author_id("opt-2").unwrap().id()).state();
        let s1 = at.node(c.by_author_id("opt-1").unwrap().id()).state();
        assert!(s2.contains(State::Selectable) && s2.contains(State::Selected), "the selected option on AT-SPI");
        assert!(s1.contains(State::Selectable) && !s1.contains(State::Selected));
        assert_eq!(at.focused_events().last(), Some(&c.by_author_id("opt-2").unwrap().id()), "AT-SPI announced the option");
    }
}

#[test]
fn a_hosts_focus_move_repaints_both_rings() {
    let mut h = harness();
    let w = h.window();
    let (a, b) = {
        let d = &mut h.state_mut().doc;
        let root = d.window_root(w).unwrap();
        let a = add(d, root, KindId::of("button"), "width: 60px; height: 24px; margin: 8px");
        let b = add(d, root, KindId::of("button"), "width: 60px; height: 24px; margin: 8px");
        name(d, a, "a");
        name(d, b, "b");
        (a, b)
    };
    settle(&mut h);
    // the last list each button recorded reaches further out than without focus: the ring
    let left = |h: &Harness<CoreDocument>, n: NodeId| -> Option<f32> {
        let applied = h.compositor().applied();
        applied.iter().rev().find_map(|c| c.lists.iter().find(|(k, _)| *k == n).map(|(_, l)| l.bounds.min.x))
    };
    let plain = left(&h, a).expect("recorded");
    assert_eq!(left(&h, b), Some(plain));
    let ring = |h: &Harness<CoreDocument>, n: NodeId| left(h, n).map(|x| x < plain);
    h.key_press(Key::Tab);
    h.run();
    assert_eq!(h.consumer().focus_author_id(), Some("a"));
    assert_eq!(ring(&h, a), Some(true), "Tab draws the ring");
    // cadatomic's title bar: Right moves focus to the next button from the host, outside a dispatch
    h.state_mut().doc.focus(b, FocusReason::Keyboard);
    settle(&mut h);
    assert_eq!(h.consumer().focus_author_id(), Some("b"), "AT follows");
    assert_eq!(ring(&h, a), Some(false), "the old button's ring is gone");
    assert_eq!(ring(&h, b), Some(true), "the new button draws one");
}

#[test]
fn sibling_dialogs_opening_in_one_frame_focus_the_last_in_tree_order() {
    let mut h = harness();
    let w = h.window();
    let (early, late) = {
        let d = &mut h.state_mut().doc;
        let root = d.window_root(w).unwrap();
        let opener = add(d, root, KindId::of("button"), "width: 60px; height: 24px");
        name(d, opener, "opener");
        // `late` has the lower id but comes second in the tree
        let late = d.create(BOX);
        let early = d.create(BOX);
        for (s, scope) in [(early, "early"), (late, "late")] {
            d.set_style(s, StyleInput { inline: Some("display: none; width: 100px; height: 40px".into()), ..Default::default() });
            d.set_prop(s, common::FOCUS_SCOPE, Value::Enum(2)).unwrap();
            d.insert(root, s, None, None).unwrap();
            let stop = add(d, s, KindId::of("button"), "width: 60px; height: 24px");
            name(d, stop, &format!("{scope}-stop"));
        }
        assert!(late.0 < early.0);
        d.focus(opener, FocusReason::Keyboard);
        (early, late)
    };
    settle(&mut h);
    assert_eq!(h.consumer().focus_author_id(), Some("opener"));
    {
        let d = &mut h.state_mut().doc;
        for s in [late, early] {
            d.set_style(s, StyleInput { inline: Some("width: 100px; height: 40px".into()), ..Default::default() });
        }
    }
    settle(&mut h);
    assert_eq!(h.consumer().focus_author_id(), Some("late-stop"), "the last sibling in tree order");
}
