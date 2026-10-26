//! Scope activation (WIDGET-PARITY §2.1, svelters OW7b): Enter in a focus scope (or the window, when
//! no scope holds the focus) that the focused kind did not take presses the scope's default button,
//! Escape its cancel button, through an AccessKit `Click` to the button's kind. A button is one whose
//! kind has a bool `default` (or `cancel`) prop; nested scopes keep their buttons to themselves.
//!
//! `:default` marks the button Enter would press: the one whose `default` prop is set, or, with
//! [`Document::set_auto_default`] on, the focused push button of a `dialog` scope (Qt's `autoDefault`
//! on Windows, KDE and GTK).

use crate::common;
use crate::cx::Emitted;
use crate::document::Document;
use skelter_base::ids::{NodeId, WindowId};
use svelters_style::StateMask;

/// The scope whose buttons Enter and Escape reach from `node`, and whether it is a `dialog`.
fn scope(doc: &Document, node: NodeId) -> Option<(NodeId, bool)> {
    match crate::input::focus::scope_of(doc, node) {
        Some(s) => Some((s, doc.view(s)?.prop(common::FOCUS_SCOPE).and_then(|v| v.as_enum()) == Some(2))),
        None => doc.window_of(node).and_then(|w| doc.window_root(w)).map(|r| (r, false)),
    }
}

/// `node`'s bool prop `name` (`default`, `cancel`): `None` when its kind has no such prop.
fn button_prop(doc: &Document, node: NodeId, name: &str) -> Option<bool> {
    let v = doc.view(node)?;
    let p = v.kind().spec.prop_named(name)?;
    if v.kind().prop(p)?.ty != crate::value::ValueType::Bool {
        return None;
    }
    Some(v.prop_bool(p))
}

/// The nodes under `scope` (not inside nested scopes) whose kind has the bool prop `name`.
fn buttons(doc: &Document, scope: NodeId, name: &str) -> Vec<NodeId> {
    let mut out = Vec::new();
    let mut stack = vec![scope];
    while let Some(n) = stack.pop() {
        if n != scope && doc.view(n).and_then(|v| v.prop(common::FOCUS_SCOPE)).and_then(|v| v.as_enum()).unwrap_or(0) != 0 {
            continue;
        }
        if button_prop(doc, n, name).is_some() {
            out.push(n);
        }
        stack.extend(doc.children(n).iter().rev());
    }
    out
}

fn usable(doc: &Document, n: NodeId) -> bool {
    !doc.state(n).contains(StateMask::DISABLED) && crate::input::focus::shown(doc, n)
}

/// The button Enter presses in `scope` with `focused` focused.
fn effective_default(doc: &Document, scope: NodeId, dialog: bool, focused: Option<NodeId>) -> Option<NodeId> {
    if doc.auto_default
        && dialog
        && let Some(f) = focused
        && button_prop(doc, f, "default").is_some()
        && crate::input::scope::scope(doc, f).map(|s| s.0) == Some(scope)
    {
        return Some(f);
    }
    buttons(doc, scope, "default").into_iter().find(|&b| button_prop(doc, b, "default") == Some(true) && usable(doc, b))
}

/// Recomputes `:default` in the scope of `node` (a focus move, a `default` write).
pub(crate) fn refresh_default(doc: &mut Document, node: NodeId) {
    let Some((s, dialog)) = scope(doc, node) else { return };
    let focused = doc.window_of(node).and_then(|w| doc.focused(w));
    let eff = effective_default(doc, s, dialog, focused);
    for b in buttons(doc, s, "default") {
        doc.set_state(b, StateMask::DEFAULT, Some(b) == eff);
    }
}

/// Focus moved from `old` to `new`: `:default` follows it in dialog scopes when auto-default is on.
pub(crate) fn focus_moved(doc: &mut Document, old: Option<NodeId>, new: Option<NodeId>) {
    if !doc.auto_default {
        return;
    }
    for n in [old, new].into_iter().flatten() {
        if doc.contains(n) && scope(doc, n).is_some_and(|(_, dialog)| dialog) {
            refresh_default(doc, n);
        }
    }
}

/// Enter (`enter`) or Escape from the focused `node` that its kind left: presses the scope's default
/// or cancel button. True when a button took it.
pub(crate) fn activate(doc: &mut Document, window: WindowId, node: NodeId, enter: bool, emitted: &mut Vec<Emitted>) -> bool {
    let Some((s, dialog)) = scope(doc, node) else { return false };
    let target = if enter {
        effective_default(doc, s, dialog, doc.focused(window))
    } else {
        buttons(doc, s, "cancel").into_iter().find(|&b| button_prop(doc, b, "cancel") == Some(true) && usable(doc, b))
    };
    let Some(b) = target else { return false };
    let req = accesskit::ActionRequest { action: accesskit::Action::Click, target_tree: accesskit::TreeId::ROOT, target_node: accesskit::NodeId(b.0), data: None };
    crate::input::dispatch::call_kind(doc, b, &crate::kind::Event::A11y(&req), emitted, None) == crate::kind::Handled::Yes
}
