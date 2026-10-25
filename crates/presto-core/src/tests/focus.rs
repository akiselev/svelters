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
