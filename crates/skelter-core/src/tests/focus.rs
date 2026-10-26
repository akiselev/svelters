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
