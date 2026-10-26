//! The focus model (architecture §11.6, svelters EV3): one focused node per window; Tab order is tree
//! order with positive `tabindex` first (as the DOM); a `contain` or `dialog` focus scope keeps Tab
//! inside it, and a `dialog` scope takes focus when it opens and gives it back when it closes;
//! `:focus-visible` follows the input modality. Focus changes queue `blur`/`focusout` on the old node
//! and `focus`/`focusin` on the new.
//!
//! Programmatic `focus()` ([`Document::focus`]) takes effect in the same transaction when the node can
//! take focus now. When it can't only because it is hidden (a field under a backdrop whose
//! `display: none` the same transaction clears, before or after the call: computed style changes only
//! at the next restyle), the request is kept and applied after the frame's restyle ([`after_style`]),
//! or dropped if the node is still hidden then: "focus applies at the next style pass".
//!
//! After every restyle the frame also runs the focus rules that follow style (QA11Y F1, F2, ported
//! from svelters-egui EV3):
//! - a `dialog` scope that becomes available (shown, not inert) takes focus: its `autofocus` node,
//!   else its first tab stop, remembering the opener; when it stops being available (hidden, inert,
//!   removed) focus inside it goes back to the opener;
//! - with `initial-focus` on the window's root, an `autofocus` node outside a dialog takes focus once
//!   when it appears (the window's first frame included) if nothing in the window has focus;
//! - the focus fixup rule: a focused node that can no longer take focus (hidden, `display: none`,
//!   concealed, disabled) loses it, to its dialog scope's first stop, else the restore stack's top,
//!   else nothing.

use crate::common;
use crate::document::Document;
use crate::input::JsEvent;
use crate::kind::{Event, FocusReason};
use crate::tree::{Dirty, NodeFlags};
use skelter_base::ids::{NodeId, WindowId};
use smallvec::SmallVec;
use std::collections::HashSet;
use svelters_style::StateMask;

/// What has keyboard focus in a window: a node, and inside it optionally a part the node manages
/// itself (`sub`): a virtual child's index ([`crate::a11y::virtual_id`]), an island's own focus, or
/// later a semantic object's id. AccessKit's focus is then that virtual child (module docs of
/// [`crate::a11y`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FocusTarget {
    pub node: NodeId,
    pub sub: Option<u32>,
}

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

/// `node` could take focus once shown: a focusable kind or an explicit `tabindex`, not disabled, in a
/// window. [`can_focus`] fails for it only because it or an ancestor is hidden.
fn focusable_when_shown(doc: &Document, node: NodeId) -> bool {
    let Some(i) = doc.idx(node) else { return false };
    if doc.core[i].window.is_none() {
        return false;
    }
    let v = doc.view_at(i);
    (v.kind().spec.focusable || v.prop(common::TABINDEX).is_some()) && !v.disabled()
}

/// Programmatic focus ([`Document::focus`]): now when `node` can take it. A node that only is hidden
/// keeps the request until the frame's next restyle (it may be shown in this transaction, before or
/// after the call; the style that shows it is computed then), which applies it or drops it. A node
/// that can't take focus at all takes nothing.
pub(crate) fn request_focus(doc: &mut Document, node: NodeId, reason: FocusReason) {
    if can_focus(doc, node) {
        set_focus(doc, Some(node), reason);
        return;
    }
    if !focusable_when_shown(doc, node) {
        return;
    }
    let Some(window) = doc.window_of(node) else { return };
    if let Some(w) = doc.window_mut(window) {
        w.pending_focus = Some((node, reason));
    }
    // the next frame decides (one is needed even when nothing else changed)
    doc.mark(node, Dirty::A11Y);
}

/// The focus request [`after_style`] applies (`None`: none).
pub(crate) fn pending_focus(doc: &Document, window: WindowId) -> Option<NodeId> {
    doc.window(window).and_then(|w| w.pending_focus).map(|(n, _)| n)
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
    // a focus move supersedes a request still waiting for style
    if let Some(w) = doc.window_mut(window) {
        w.pending_focus = None;
    }
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
        w.focus_sub = None;
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

/// Clears the window's focus (and a request waiting for style).
pub(crate) fn clear_focus(doc: &mut Document, window: WindowId) {
    if let Some(w) = doc.window_mut(window) {
        w.pending_focus = None;
    }
    if let Some(o) = doc.focused(window) {
        blur_node(doc, window, o);
        if let Some(w) = doc.window_mut(window) {
            w.focus = None;
            w.focus_sub = None;
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

/// Opens a `dialog` scope: remembers the focus to give back and focuses the scope's `autofocus` node,
/// else its first tab stop. The frame's [`after_style`] does this for a scope that becomes available;
/// calling it directly opens one now (and the frame then counts it as open).
pub fn open_dialog(doc: &mut Document, scope: NodeId) {
    let Some(window) = doc.window_of(scope) else { return };
    let before = doc.focused(window);
    open_dialog_from(doc, window, scope, before);
    if let Some(w) = doc.window_mut(window) {
        w.focus_seen.insert(scope);
    }
}

/// [`open_dialog`] with the opener `before` (the window's focus before this pass moved it): focus
/// already inside the scope stays where it is.
fn open_dialog_from(doc: &mut Document, window: WindowId, scope: NodeId, before: Option<NodeId>) {
    if let Some(prev) = before.filter(|&b| doc.contains(b) && !doc.is_ancestor_or_self(scope, b))
        && let Some(w) = doc.window_mut(window)
    {
        w.restore.retain(|&(s, _)| s != scope);
        w.restore.push((scope, prev));
    }
    if doc.focused(window).is_some_and(|f| doc.is_ancestor_or_self(scope, f)) {
        return;
    }
    if let Some(first) = initial_in(doc, scope) {
        set_focus(doc, Some(first), FocusReason::Programmatic);
    }
}

/// Where focus goes in a scope that opens: its first `autofocus` node that can take focus (tree
/// order), else its first tab stop.
fn initial_in(doc: &Document, scope: NodeId) -> Option<NodeId> {
    let mut stack = vec![scope];
    while let Some(n) = stack.pop() {
        if n != scope && autofocus(doc, n) && can_focus(doc, n) {
            return Some(n);
        }
        stack.extend(doc.children(n).iter().rev());
    }
    tab_sequence(doc, scope).first().copied()
}

fn autofocus(doc: &Document, n: NodeId) -> bool {
    doc.view(n).is_some_and(|v| v.prop_bool(common::AUTOFOCUS))
}

fn is_dialog(doc: &Document, n: NodeId) -> bool {
    doc.view(n).and_then(|v| v.prop(common::FOCUS_SCOPE)).and_then(|v| v.as_enum()) == Some(2)
}

/// A node the focus rules after style watch: a `dialog` scope or an `autofocus` node.
pub(crate) fn watched(doc: &Document, n: NodeId) -> bool {
    is_dialog(doc, n) || autofocus(doc, n)
}

/// A scope is available while it is in its window, shown and not inert (nor making its content inert:
/// an outro).
fn available(doc: &Document, window: WindowId, n: NodeId) -> bool {
    doc.window_of(n) == Some(window) && shown(doc, n) && doc.idx(n).is_some_and(|i| !doc.core[i].flags.intersects(NodeFlags::INERT.union(NodeFlags::INERT_CHILDREN)))
}

/// The innermost available `dialog` scope around `n` (itself included).
fn dialog_around(doc: &Document, window: WindowId, n: NodeId) -> Option<NodeId> {
    let mut a = Some(n);
    while let Some(x) = a {
        if is_dialog(doc, x) && available(doc, window, x) {
            return Some(x);
        }
        a = doc.parent(x);
    }
    None
}

/// The focus rules that follow style, run by the frame after its restyle (before layout and the
/// accessibility update): the deferred request, `dialog` scopes and `autofocus` nodes that appeared
/// or went away, then the focus fixup. Their focus events are queued as any focus move's.
pub(crate) fn after_style(doc: &mut Document, window: WindowId) {
    let Some(root) = doc.window_root(window) else { return };
    let before = doc.focused(window);
    // 1. the request that waited for style
    if let Some((n, reason)) = doc.window_mut(window).and_then(|w| w.pending_focus.take())
        && doc.window_of(n) == Some(window)
        && can_focus(doc, n)
    {
        set_focus(doc, Some(n), reason);
    }
    // 2. scopes and autofocus nodes: what is available now against the last pass
    let live: HashSet<NodeId> = doc.focus_watch.iter().copied().filter(|&n| doc.contains(n)).collect();
    doc.focus_watch = live;
    let mut watch: Vec<(usize, NodeId)> =
        doc.focus_watch.iter().copied().filter(|&n| doc.window_of(n) == Some(window)).map(|n| (depth(doc, n), n)).collect();
    // outer scopes first, then a stable order
    watch.sort_unstable_by_key(|&(d, n)| (d, n.0));
    let seen_before = doc.window_mut(window).map(|w| std::mem::take(&mut w.focus_seen)).unwrap_or_default();
    let mut seen = HashSet::new();
    let mut opened = Vec::new();
    let mut auto = Vec::new();
    for &(_, n) in &watch {
        if is_dialog(doc, n) {
            if available(doc, window, n) {
                seen.insert(n);
                if !seen_before.contains(&n) {
                    opened.push(n);
                }
            }
        } else if can_focus(doc, n) {
            seen.insert(n);
            if !seen_before.contains(&n) {
                auto.push(n);
            }
        }
    }
    // scopes that went away give focus back (removed ones already did, `scopes_closed`)
    let gone: Vec<NodeId> = seen_before.iter().copied().filter(|n| !seen.contains(n)).collect();
    for s in gone {
        scope_gone(doc, window, s);
    }
    for s in opened {
        open_dialog_from(doc, window, s, before);
    }
    let initial = doc.view(root).is_some_and(|v| v.prop_bool(common::INITIAL_FOCUS));
    if initial
        && doc.focused(window).is_none()
        && let Some(&a) = auto.iter().find(|&&a| dialog_around(doc, window, a).is_none())
    {
        set_focus(doc, Some(a), FocusReason::Programmatic);
    }
    if let Some(w) = doc.window_mut(window) {
        w.focus_seen = seen;
    }
    // 3. the focus fixup rule
    if let Some(f) = doc.focused(window)
        && !can_focus(doc, f)
    {
        let to = dialog_around(doc, window, f).and_then(|s| initial_in(doc, s)).filter(|&t| t != f).or_else(|| {
            doc.window(window).and_then(|w| w.restore.iter().rev().map(|&(_, p)| p).find(|&p| p != f && can_focus(doc, p)))
        });
        match to {
            Some(t) => set_focus(doc, Some(t), FocusReason::Programmatic),
            None => clear_focus(doc, window),
        }
    }
}

fn depth(doc: &Document, mut n: NodeId) -> usize {
    let mut d = 0;
    while let Some(p) = doc.parent(n) {
        d += 1;
        n = p;
    }
    d
}

/// A `dialog` scope stopped being available (hidden, inert, no longer a scope): its restore entries
/// go, and focus inside it (or lost with it) goes back to its opener.
fn scope_gone(doc: &mut Document, window: WindowId, scope: NodeId) {
    let Some(w) = doc.window_mut(window) else { return };
    let back: SmallVec<[NodeId; 2]> = w.restore.iter().filter(|(s, _)| *s == scope).map(|&(_, p)| p).collect();
    w.restore.retain(|(s, _)| *s != scope);
    let Some(&prev) = back.last() else { return };
    let focus = doc.focused(window);
    let lost = focus.is_none_or(|f| !can_focus(doc, f) || (doc.contains(scope) && doc.is_ancestor_or_self(scope, f)));
    if lost && can_focus(doc, prev) {
        set_focus(doc, Some(prev), FocusReason::Programmatic);
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

