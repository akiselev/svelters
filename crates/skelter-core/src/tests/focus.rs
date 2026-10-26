//! The focus model (EV3): tab order, positive `tabindex`, disabled and hidden nodes, scopes,
//! `:focus-visible`, dialogs giving focus back, focus events.

use super::*;
use crate::{FocusReason, ListenFlags, StateMask, Value, common, next_focus, open_dialog};

fn toggles(d: &mut Document, parent: NodeId, n: usize) -> Vec<NodeId> {
    (0..n).map(|_| child(d, parent, TOGGLE, "")).collect()
}

#[test]
fn tab_follows_tree_order_with_positive_tabindex_first() {
    let (mut d, root) = doc();
    let t = toggles(&mut d, root, 4);
    d.set_prop(t[3], common::TABINDEX, Value::I64(1)).unwrap();
    d.set_prop(t[1], common::TABINDEX, Value::I64(-1)).unwrap();
    let mut seen = Vec::new();
    for _ in 0..4 {
        let n = next_focus(&d, W, false).unwrap();
        d.focus(n, FocusReason::Keyboard);
        seen.push(n);
    }
    assert_eq!(seen, [t[3], t[0], t[2], t[3]], "tabindex 1 first, -1 skipped, then wrap");
    assert_eq!(next_focus(&d, W, true), Some(t[2]));
}

#[test]
fn disabled_and_hidden_nodes_are_skipped() {
    let (mut d, root) = doc();
    let t = toggles(&mut d, root, 3);
    d.set_prop(t[0], common::DISABLED, Value::Bool(true)).unwrap();
    d.set_prop(t[1], common::HIDDEN, Value::Bool(true)).unwrap();
    assert_eq!(next_focus(&d, W, false), Some(t[2]));
    d.focus(t[0], FocusReason::Programmatic);
    assert_eq!(d.focused(W), None, "a disabled node takes no focus, even programmatically");
}

#[test]
fn tab_dispatch_moves_focus_and_shows_it() {
    let (mut d, root) = doc();
    let t = toggles(&mut d, root, 2);
    frame(&mut d);
    let out = d.dispatch(&key(Key::Tab, Modifiers::default()));
    assert!(out.handled);
    assert_eq!(d.focused(W), Some(t[0]));
    assert!(d.state(t[0]).contains(StateMask::FOCUSED));
    assert!(d.state(t[0]).contains(StateMask::FOCUS_VISIBLE), "keyboard focus is visible");
    assert!(d.state(root).contains(StateMask::FOCUS_WITHIN));
    d.dispatch(&key(Key::Tab, Modifiers::SHIFT));
    assert_eq!(d.focused(W), Some(t[1]), "shift+tab wraps backwards");
    assert!(!d.state(t[0]).contains(StateMask::FOCUSED));
    // a pointer press focuses without :focus-visible
    click(&mut d, 5.0, 5.0);
    assert_eq!(d.focused(W), Some(t[0]));
    assert!(!d.state(t[0]).contains(StateMask::FOCUS_VISIBLE));
}

#[test]
fn a_contain_scope_keeps_tab_inside() {
    let (mut d, root) = doc();
    let outside = child(&mut d, root, TOGGLE, "");
    let scope = child(&mut d, root, BOX, "");
    d.set_prop(scope, common::FOCUS_SCOPE, Value::Enum(1)).unwrap();
    let inner = toggles(&mut d, scope, 2);
    d.focus(inner[0], FocusReason::Keyboard);
    assert_eq!(next_focus(&d, W, false), Some(inner[1]));
    d.focus(inner[1], FocusReason::Keyboard);
    assert_eq!(next_focus(&d, W, false), Some(inner[0]), "wraps inside the scope");
    let _ = outside;
}

#[test]
fn a_dialog_takes_focus_and_gives_it_back() {
    let (mut d, root) = doc();
    let before = child(&mut d, root, TOGGLE, "");
    d.focus(before, FocusReason::Keyboard);
    let dialog = child(&mut d, root, BOX, "");
    d.set_prop(dialog, common::FOCUS_SCOPE, Value::Enum(2)).unwrap();
    let inside = toggles(&mut d, dialog, 1)[0];
    open_dialog(&mut d, dialog);
    assert_eq!(d.focused(W), Some(inside));
    d.destroy(dialog);
    assert_eq!(d.focused(W), Some(before), "closing the dialog restores focus");
}

#[test]
fn focus_moves_queue_focus_events() {
    let (mut d, root) = doc();
    let t = toggles(&mut d, root, 2);
    d.listen(root, common::FOCUSIN, ListenFlags::EMPTY);
    d.listen(t[0], common::BLUR, ListenFlags::EMPTY);
    d.focus(t[0], FocusReason::Programmatic);
    d.focus(t[1], FocusReason::Programmatic);
    let evs = d.take_events();
    let names: Vec<(crate::SignalId, NodeId)> = evs.iter().map(|e| (e.signal, e.target)).collect();
    assert_eq!(names, [(common::FOCUSIN, t[0]), (common::BLUR, t[0]), (common::FOCUSIN, t[1])]);
    assert!(!d.busy().contains(crate::Busy::EVENTS));
}

#[test]
fn the_keymap_hook_runs_before_the_focused_kind() {
    let (mut d, root) = doc();
    let t = toggles(&mut d, root, 1)[0];
    d.focus(t, FocusReason::Keyboard);
    d.set_key_hook(Box::new(|_, k| k.key == Some(Key::Space)));
    let out = d.dispatch(&key(Key::Space, Modifiers::default()));
    assert!(out.handled);
    assert_eq!(d.get_prop(t, crate::testkinds::TOGGLE_CHECKED), Some(Value::Bool(false)), "the keymap took Space");
}

// ---- QA11Y F1, F2: focus that follows style ----

fn show(d: &mut Document, n: NodeId, css: &str) {
    d.set_style(n, crate::StyleInput { inline: Some(css.into()), ..Default::default() });
}

fn dialog_scope(d: &mut Document, parent: NodeId, css: &str) -> NodeId {
    let s = child(d, parent, BOX, css);
    d.set_prop(s, common::FOCUS_SCOPE, Value::Enum(2)).unwrap();
    s
}

#[test]
fn focus_under_a_backdrop_shown_in_the_same_transaction_applies_after_restyle() {
    let (mut d, root) = doc();
    let backdrop = child(&mut d, root, BOX, "display: none");
    let field = child(&mut d, backdrop, TOGGLE, "");
    frame(&mut d);
    // focus first, then the class change that shows it (cadatomic's palette order), and the reverse
    for focus_first in [true, false] {
        d.blur(W);
        show(&mut d, backdrop, "display: none");
        frame(&mut d);
        if focus_first {
            d.focus(field, FocusReason::Programmatic);
            show(&mut d, backdrop, "");
        } else {
            show(&mut d, backdrop, "");
            d.focus(field, FocusReason::Programmatic);
        }
        assert_eq!(d.focused(W), None, "computed style still hides it: the request waits");
        assert_eq!(d.pending_focus(W), Some(field));
        d.listen(field, common::FOCUS, ListenFlags::EMPTY);
        let c = frame(&mut d).expect("a frame");
        assert_eq!(d.focused(W), Some(field), "applied after the frame's restyle");
        assert_eq!(d.pending_focus(W), None);
        assert_eq!(c.a11y.expect("an a11y update").focus, accesskit::NodeId(field.0), "TreeUpdate.focus names the field");
        assert!(d.take_events().iter().any(|e| e.signal == common::FOCUS && e.target == field), "focus fired in that frame");
    }
}

#[test]
fn focus_on_a_node_still_hidden_after_restyle_is_dropped() {
    let (mut d, root) = doc();
    let backdrop = child(&mut d, root, BOX, "display: none");
    let field = child(&mut d, backdrop, TOGGLE, "");
    frame(&mut d);
    d.focus(field, FocusReason::Programmatic);
    assert_eq!(d.pending_focus(W), Some(field));
    assert!(frame(&mut d).is_some(), "the request brings a frame");
    assert_eq!(d.focused(W), None);
    assert_eq!(d.pending_focus(W), None, "dropped, not kept for a later frame");
    show(&mut d, backdrop, "");
    frame(&mut d);
    assert_eq!(d.focused(W), None, "showing it later doesn't apply the old request");
    // a disabled node never queues one
    d.set_prop(field, common::DISABLED, Value::Bool(true)).unwrap();
    d.focus(field, FocusReason::Programmatic);
    assert_eq!(d.pending_focus(W), None);
}

#[test]
fn hiding_the_focused_nodes_container_moves_focus_to_the_restore_entry_or_none() {
    let (mut d, root) = doc();
    let opener = child(&mut d, root, TOGGLE, "");
    let panel = child(&mut d, root, BOX, "");
    let inner = child(&mut d, panel, TOGGLE, "");
    frame(&mut d);
    d.focus(inner, FocusReason::Keyboard);
    show(&mut d, panel, "display: none");
    frame(&mut d);
    assert_eq!(d.focused(W), None, "nothing to restore: focus goes to nothing");
    assert!(!d.state(inner).contains(StateMask::FOCUSED));
    // inside a dialog scope that hides: back to its opener
    show(&mut d, panel, "");
    d.set_prop(panel, common::FOCUS_SCOPE, Value::Enum(2)).unwrap();
    d.focus(opener, FocusReason::Keyboard);
    frame(&mut d);
    // the scope was already shown: it counts as appearing only once it is a scope at a frame
    assert_eq!(d.focused(W), Some(inner), "the dialog scope took focus when it became one");
    show(&mut d, panel, "visibility: hidden");
    frame(&mut d);
    assert_eq!(d.focused(W), Some(opener), "the hidden dialog gave focus back");
}

#[test]
fn a_dialog_that_appears_focuses_its_autofocus_node_and_gives_focus_back_on_hide() {
    let (mut d, root) = doc();
    let opener = child(&mut d, root, TOGGLE, "");
    let dialog = dialog_scope(&mut d, root, "display: none");
    let first = child(&mut d, dialog, TOGGLE, "");
    let field = child(&mut d, dialog, TOGGLE, "");
    d.set_prop(field, common::AUTOFOCUS, Value::Bool(true)).unwrap();
    frame(&mut d);
    d.focus(opener, FocusReason::Keyboard);
    frame(&mut d);
    assert_eq!(d.focused(W), Some(opener), "a hidden dialog takes nothing");
    show(&mut d, dialog, "");
    frame(&mut d);
    assert_eq!(d.focused(W), Some(field), "the autofocus node, not the first stop");
    d.dispatch(&key(Key::Tab, Modifiers::default()));
    assert_eq!(d.focused(W), Some(first), "Tab wraps inside the dialog");
    show(&mut d, dialog, "display: none");
    frame(&mut d);
    assert_eq!(d.focused(W), Some(opener), "hidden: focus back on the opener");
    // shown again: it opens again (the first stop when no autofocus node)
    d.set_prop(field, common::AUTOFOCUS, Value::Bool(false)).unwrap();
    show(&mut d, dialog, "");
    frame(&mut d);
    assert_eq!(d.focused(W), Some(first));
}

#[test]
fn a_dialog_focused_into_before_it_shows_still_restores_the_opener() {
    let (mut d, root) = doc();
    let opener = child(&mut d, root, TOGGLE, "");
    let dialog = dialog_scope(&mut d, root, "display: none");
    let _first = child(&mut d, dialog, TOGGLE, "");
    let field = child(&mut d, dialog, TOGGLE, "");
    d.focus(opener, FocusReason::Keyboard);
    frame(&mut d);
    // the palette's order: focus() on its field, then show it
    d.focus(field, FocusReason::Programmatic);
    show(&mut d, dialog, "");
    frame(&mut d);
    assert_eq!(d.focused(W), Some(field), "the app's focus() wins over the first stop");
    show(&mut d, dialog, "display: none");
    frame(&mut d);
    assert_eq!(d.focused(W), Some(opener));
}

#[test]
fn initial_focus_is_opt_in() {
    for opt_in in [false, true] {
        let (mut d, root) = doc();
        if opt_in {
            d.set_prop(root, common::INITIAL_FOCUS, Value::Bool(true)).unwrap();
        }
        let _a = child(&mut d, root, TOGGLE, "");
        let b = child(&mut d, root, TOGGLE, "");
        d.set_prop(b, common::AUTOFOCUS, Value::Bool(true)).unwrap();
        let c = frame(&mut d).expect("the first frame");
        if opt_in {
            assert_eq!(d.focused(W), Some(b), "the first frame focuses the autofocus node");
            assert_eq!(c.a11y.expect("the full tree").focus, accesskit::NodeId(b.0));
        } else {
            assert_eq!(d.focused(W), None, "without initial-focus nothing changes");
        }
    }
}

#[test]
fn autofocus_outside_a_dialog_takes_focus_once_and_never_steals_it() {
    let (mut d, root) = doc();
    d.set_prop(root, common::INITIAL_FOCUS, Value::Bool(true)).unwrap();
    let other = child(&mut d, root, TOGGLE, "");
    frame(&mut d);
    let late = child(&mut d, root, TOGGLE, "display: none");
    d.set_prop(late, common::AUTOFOCUS, Value::Bool(true)).unwrap();
    frame(&mut d);
    assert_eq!(d.focused(W), None);
    show(&mut d, late, "");
    frame(&mut d);
    assert_eq!(d.focused(W), Some(late), "focused when it appeared");
    d.focus(other, FocusReason::Keyboard);
    frame(&mut d);
    assert_eq!(d.focused(W), Some(other), "once only");
    show(&mut d, late, "display: none");
    frame(&mut d);
    show(&mut d, late, "");
    frame(&mut d);
    assert_eq!(d.focused(W), Some(other), "appearing again doesn't steal focus");
}

// ---- QA11Y F3: what TreeUpdate.focus names ----

static GRAFT: crate::KindSpec = crate::KindSpec { focusable: true, role: accesskit::Role::GenericContainer, ..crate::KindSpec::new("graft") };

/// An island's host node: its AccessKit node grafts a subtree (`tree_id`), and has no children.
#[derive(Default)]
struct Graft;

impl crate::Kind for Graft {
    fn spec() -> &'static crate::KindSpec {
        &GRAFT
    }
    fn record(&self, cx: &mut crate::RecordCx) {
        cx.draw_box();
    }
    fn a11y(&self, cx: &mut crate::A11yCx) {
        cx.default_a11y();
        cx.out.set_tree_id(accesskit::TreeId(accesskit::Uuid::from_u128(7)));
    }
}

static PARTS: crate::KindSpec = crate::KindSpec { focusable: true, role: accesskit::Role::ListBox, ..crate::KindSpec::new("parts") };

/// A focusable list of three virtual options.
#[derive(Default)]
struct Parts;

impl crate::Kind for Parts {
    fn spec() -> &'static crate::KindSpec {
        &PARTS
    }
    fn record(&self, cx: &mut crate::RecordCx) {
        cx.draw_box();
    }
    fn a11y(&self, cx: &mut crate::A11yCx) {
        cx.default_a11y();
        for _ in 0..3 {
            cx.add_virtual_child(accesskit::Node::new(accesskit::Role::ListBoxOption));
        }
    }
}

fn f3_doc() -> (Document, NodeId) {
    let mut r = crate::KindRegistry::new();
    r.register::<testkinds::BoxKind>().unwrap();
    r.register::<testkinds::ToggleKind>().unwrap();
    r.register::<Graft>().unwrap();
    r.register::<Parts>().unwrap();
    let mut d = Document::new(std::sync::Arc::new(r), Box::new(FixedText));
    let root = d.create_window(W, BOX, Size::new(200.0, 100.0), Scale(1.0));
    (d, root)
}

fn focus_of(c: Option<skelter_base::commit::Commit>) -> Option<accesskit::NodeId> {
    c.and_then(|c| c.a11y).map(|u| u.focus)
}

#[test]
fn a_graft_is_never_named_before_its_subtree_went_out() {
    let (mut d, root) = f3_doc();
    let pane = child(&mut d, root, BOX, "");
    let island = child(&mut d, pane, KindId::of("graft"), "");
    frame(&mut d);
    d.focus(island, FocusReason::Pointer);
    assert_eq!(d.focused(W), Some(island));
    assert_eq!(focus_of(frame(&mut d)), Some(accesskit::NodeId(pane.0)), "the graft's parent until its subtree is out");
    d.a11y_subtree_sent(island);
    assert_eq!(focus_of(frame(&mut d)), Some(accesskit::NodeId(island.0)), "the frame after the subtree: the graft");
    // focus away and back: the graft stays sent
    let other = child(&mut d, root, TOGGLE, "");
    d.focus(other, FocusReason::Keyboard);
    frame(&mut d);
    d.focus(island, FocusReason::Keyboard);
    assert_eq!(focus_of(frame(&mut d)), Some(accesskit::NodeId(island.0)));
    // the graft leaves the tree and comes back: its subtree must go out again first
    d.remove(pane, island).unwrap();
    frame(&mut d);
    d.insert(pane, island, None, None).unwrap();
    frame(&mut d);
    d.focus(island, FocusReason::Keyboard);
    assert_eq!(focus_of(frame(&mut d)), Some(accesskit::NodeId(pane.0)));
}

#[test]
fn a_focused_virtual_child_is_the_trees_focus() {
    let (mut d, root) = f3_doc();
    let list = child(&mut d, root, KindId::of("parts"), "");
    frame(&mut d);
    d.focus(list, FocusReason::Keyboard);
    assert_eq!(focus_of(frame(&mut d)), Some(accesskit::NodeId(list.0)));
    d.set_focus_sub(W, Some(2));
    assert_eq!(d.focus_target(W), Some(crate::FocusTarget { node: list, sub: Some(2) }));
    assert_eq!(focus_of(frame(&mut d)), Some(crate::a11y::virtual_id(list, 2)));
    // an index past the children names the owner
    d.set_focus_sub(W, Some(9));
    assert_eq!(focus_of(frame(&mut d)), Some(accesskit::NodeId(list.0)));
    // a focus move clears it
    d.set_focus_sub(W, Some(1));
    frame(&mut d);
    d.blur(W);
    d.focus(list, FocusReason::Keyboard);
    assert_eq!(d.focus_target(W).and_then(|t| t.sub), None);
}

#[test]
fn the_root_transform_is_the_window_scale() {
    let (mut d, root) = f3_doc();
    let b = child(&mut d, root, TOGGLE, "margin-left: 10px");
    let u = frame(&mut d).unwrap().a11y.unwrap();
    let node = |u: &accesskit::TreeUpdate, n: NodeId| u.nodes.iter().find(|(id, _)| id.0 == n.0).map(|(_, x)| x.clone());
    assert_eq!(node(&u, root).unwrap().transform(), None, "scale 1: identity");
    let before = node(&u, b).unwrap().transform().copied();
    // a zoom (or OS scale) change: only the root re-sends, with the new scale
    d.resize_window(W, Size::new(160.0, 80.0), Scale(1.25));
    let u = frame(&mut d).unwrap().a11y.unwrap();
    let r = node(&u, root).expect("the root re-sends");
    assert_eq!(r.transform(), Some(&accesskit::Affine::scale(1.25)));
    if let Some(n) = node(&u, b) {
        assert_eq!(n.transform().copied(), before, "children stay relative to their parent");
    }
}
