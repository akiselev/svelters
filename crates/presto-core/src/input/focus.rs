//! The focus model (architecture §11.6, svelters EV3): one focused node per window; Tab order is tree
//! order with positive `tabindex` first (as the DOM); a `contain` or `dialog` focus scope keeps Tab
//! inside it, and a `dialog` scope takes focus when it opens and gives it back when it closes;
//! `:focus-visible` follows the input modality; programmatic `focus()` takes effect in the same
//! transaction. Focus changes queue `blur`/`focusout` on the old node and `focus`/`focusin` on the new.

use crate::common;
use crate::document::Document;
use crate::input::JsEvent;
use crate::kind::{Event, FocusReason};
use presto_base::ids::{NodeId, WindowId};
use smallvec::SmallVec;
use svelters_style::StateMask;

/// `node` can take focus at all (programmatically): a focusable kind or an explicit `tabindex`, not
/// disabled, not hidden, in a window.
pub(crate) fn can_focus(doc: &Document, node: NodeId) -> bool {
    let Some(i) = doc.idx(node) else { return false };
    if doc.core[i].window.is_none() {
        return false;
    }
    let v = doc.view_at(i);
    let focusable = v.kind().spec.focusable || v.prop(common::TABINDEX).is_some();
    focusable && !v.disabled() && !v.prop_bool(common::HIDDEN) && !hidden_by_ancestor(doc, i)
}

/// `node` and its ancestors are shown (not hidden, invisible, `display: none` or concealed by a
/// container that places its children).
pub(crate) fn shown(doc: &Document, node: NodeId) -> bool {
    doc.idx(node).is_some_and(|i| !hidden_by_ancestor(doc, i))
}

fn hidden_by_ancestor(doc: &Document, mut i: usize) -> bool {
    loop {
        let v = doc.view_at(i);
        if v.prop_bool(common::HIDDEN)
            || v.style().visibility_hidden
            || doc.layout[i].style.display == taffy::Display::None
            || doc.core[i].flags.contains(crate::tree::NodeFlags::CONCEALED)
        {
            return true;
        }
        match doc.core[i].parent {
            Some(p) => i = doc.ix(p),
            None => return false,
        }
    }
}

/// The node's `tabindex` (`None`: unset).
fn tabindex(doc: &Document, node: NodeId) -> Option<i64> {
    doc.view(node).and_then(|v| v.prop(common::TABINDEX)).and_then(|v| v.as_i64())
}

/// The scope that bounds Tab from `node`: the nearest ancestor-or-self with `focus-scope` set.
pub(crate) fn scope_of(doc: &Document, node: NodeId) -> Option<NodeId> {
    let mut a = Some(node);
    while let Some(n) = a {
        let v = doc.view(n)?;
        if v.prop(common::FOCUS_SCOPE).and_then(|v| v.as_enum()).unwrap_or(0) != 0 {
            return Some(n);
        }
        a = doc.parent(n);
    }
    None
}

/// The tab sequence under `top`: positive tabindex ascending (tree order among equals), then the
/// rest in tree order; `tabindex < 0` is left out.
pub(crate) fn tab_sequence(doc: &Document, top: NodeId) -> Vec<NodeId> {
    let mut positive = Vec::new();
    let mut normal = Vec::new();
    let mut stack = vec![top];
    while let Some(n) = stack.pop() {
        // a peer group is one Tab stop
        if can_focus(doc, n) && crate::input::peers::tab_stop(doc, n).is_none_or(|s| s == n) {
            match tabindex(doc, n) {
                Some(t) if t > 0 => positive.push((t, n)),
                Some(t) if t < 0 => {}
                _ => normal.push(n),
            }
        }
        stack.extend(doc.children(n).iter().rev());
    }
    positive.sort_by_key(|&(t, _)| t);
    positive.into_iter().map(|(_, n)| n).chain(normal).collect()
}

/// The node Tab (or Shift+Tab) moves to from the window's focus, wrapping inside the focus scope.
pub fn next_focus(doc: &Document, window: WindowId, backwards: bool) -> Option<NodeId> {
    let root = doc.window_root(window)?;
    let current = doc.focused(window);
    let top = current.and_then(|c| scope_of(doc, c)).unwrap_or(root);
    let seq = tab_sequence(doc, top);
    if seq.is_empty() {
        return None;
    }
    // a focused member of a peer group that is not its Tab stop counts as the stop
    let at = current.and_then(|c| seq.iter().position(|&n| n == c).or_else(|| crate::input::peers::tab_stop(doc, c).and_then(|s| seq.iter().position(|&n| n == s))));
    let n = seq.len();
    Some(match (at, backwards) {
        (Some(i), false) => seq[(i + 1) % n],
        (Some(i), true) => seq[(i + n - 1) % n],
        (None, false) => seq[0],
        (None, true) => seq[n - 1],
    })
}

/// Moves focus to `node` (or clears it for `None`).
pub(crate) fn set_focus(doc: &mut Document, node: Option<NodeId>, reason: FocusReason) {
    let Some(node) = node else { return };
    if !can_focus(doc, node) {
        return;
    }
    let window = doc.window_of(node).expect("focusable nodes are in a window");
    let old = doc.focused(window);
    if old == Some(node) {
        // the modality may still change :focus-visible
        let visible = visible_for(doc, node, reason, window);
        doc.set_state(node, StateMask::FOCUS_VISIBLE, visible);
        return;
    }
    if let Some(o) = old {
        blur_node(doc, window, o);
    }
    let visible = visible_for(doc, node, reason, window);
    if let Some(w) = doc.window_mut(window) {
        w.focus = Some(node);
        w.focus_visible = visible;
    }
    doc.set_state(node, StateMask::FOCUSED, true);
    doc.set_state(node, StateMask::FOCUS_VISIBLE, visible);
    let mut a = Some(node);
    while let Some(n) = a {
        doc.set_state(n, StateMask::FOCUS_WITHIN, true);
        a = doc.parent(n);
    }
    native_event(doc, node, &Event::FocusIn(reason));
    queue(doc, node, common::FOCUS, false);
    queue(doc, node, common::FOCUSIN, true);
    // the a11y tree's focus changed
    doc.mark(node, crate::tree::Dirty::A11Y);
    crate::input::scope::focus_moved(doc, old, Some(node));
}

/// Clears the window's focus.
pub(crate) fn clear_focus(doc: &mut Document, window: WindowId) {
    if let Some(o) = doc.focused(window) {
        blur_node(doc, window, o);
        if let Some(w) = doc.window_mut(window) {
            w.focus = None;
        }
        crate::input::scope::focus_moved(doc, Some(o), None);
    }
}

fn blur_node(doc: &mut Document, window: WindowId, o: NodeId) {
    if !doc.contains(o) {
        return;
    }
    doc.set_state(o, StateMask(StateMask::FOCUSED.0 | StateMask::FOCUS_VISIBLE.0), false);
    let mut a = Some(o);
    while let Some(n) = a {
        doc.set_state(n, StateMask::FOCUS_WITHIN, false);
        a = doc.parent(n);
    }
    native_event(doc, o, &Event::FocusOut);
    queue(doc, o, common::BLUR, false);
    queue(doc, o, common::FOCUSOUT, true);
    doc.mark(o, crate::tree::Dirty::A11Y);
    let _ = window;
}

/// `:focus-visible` for a focus move: keyboard yes, pointer no (except text inputs, as browsers),
/// programmatic as the window's last modality.
fn visible_for(doc: &Document, node: NodeId, reason: FocusReason, window: WindowId) -> bool {
    match reason {
        FocusReason::Keyboard => true,
        FocusReason::Pointer => doc.view(node).is_some_and(|v| v.kind().spec.role == accesskit::Role::TextInput),
        FocusReason::Programmatic => doc.window(window).is_some_and(|w| w.focus_visible),
    }
}

/// Calls the kind's native handler for a focus event outside dispatch (signals it emits are queued).
fn native_event(doc: &mut Document, node: NodeId, ev: &Event) {
    let mut emitted = Vec::new();
    crate::input::dispatch::call_kind(doc, node, ev, &mut emitted, None);
    for e in emitted {
        crate::input::dispatch::queue_emitted(doc, e);
    }
}

/// Queues a focus event for JS (delivered with the next dispatch result, or by
/// [`Document::take_events`]).
fn queue(doc: &mut Document, target: NodeId, signal: crate::ids::SignalId, bubbles: bool) {
    let path = crate::input::dispatch::listener_path(doc, target, signal, bubbles);
    if path.is_empty() {
        return;
    }
    let ev = JsEvent { signal, target, arg: None, path, cancelable: false, sync: true };
    // a focus move JS made inside the sync deliverer is delivered when its listener returns
    if doc.delivering > 0 {
        doc.nested.push(ev);
    } else {
        doc.input_events.push(ev);
    }
}

/// Opens a `dialog` scope: remembers the focus to give back and focuses the scope's first tab stop.
pub fn open_dialog(doc: &mut Document, scope: NodeId) {
    let Some(window) = doc.window_of(scope) else { return };
    if let Some(prev) = doc.focused(window)
        && let Some(w) = doc.window_mut(window)
    {
        w.restore.push((scope, prev));
    }
    if let Some(first) = tab_sequence(doc, scope).first().copied() {
        set_focus(doc, Some(first), FocusReason::Programmatic);
    }
}

/// Nodes left `window`: a dialog scope among them gives focus back to what had it before it opened.
pub(crate) fn scopes_closed(doc: &mut Document, window: WindowId, gone: &[NodeId]) {
    let Some(w) = doc.window(window) else { return };
    let back: SmallVec<[NodeId; 2]> = w.restore.iter().filter(|(s, p)| gone.contains(s) && !gone.contains(p)).map(|&(_, p)| p).collect();
    if let Some(w) = doc.window_mut(window) {
        w.restore.retain(|(s, _)| !gone.contains(s));
    }
    if let Some(&p) = back.last() {
        set_focus(doc, Some(p), FocusReason::Programmatic);
    }
}

