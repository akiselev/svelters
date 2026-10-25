//! In-app drag and drop on any node (svelters WG4b), with the common props: `drag-type` (non-empty:
//! a source), `drag-data` (its payload) and `drop-types` (non-empty: a target; types separated by
//! spaces or commas, `*` any, `cad/*` a prefix).
//!
//! A primary press on a source (or inside one) that moves farther than [`THRESHOLD`] with the button
//! held starts a drag: the pressed node's press is cancelled (`PointerCancel`), the source gets
//! `:dragging` and `drag-begin`, and the drag holds the pointer until it ends. While it moves, the
//! deepest node under the pointer whose `drop-types` accept the type is the target: it gets
//! `:drop-target` and `drag-enter(type)`, and `drag-leave` when the drag leaves, is cancelled
//! (Escape, pointer cancel) or is dropped there (before `drop`). The release over a target delivers
//! `drop(type, data, x, y, action)` (the point in the target's own space) and the source's
//! `drag-end(dropped, action)`; the action comes from the modifiers held at the release (Qt's
//! `Qt::DropAction`): `copy` with Ctrl (Option on macOS), `link` with Ctrl+Shift (Cmd+Option on macOS),
//! else `move`; `none` when nothing took it. The cursor is `grabbing` over a target that accepts the
//! drag and `no-drop` elsewhere.

use crate::common;
use crate::document::Document;
use crate::input::Dispatched;
use crate::value::Value;
use presto_base::cursor::CursorIcon;
use presto_base::event::{Modifiers, MouseButton, PointerEvent, PointerPhase};
use presto_base::geom::Point;
use presto_base::ids::{NodeId, WindowId};
use std::sync::Arc;
use svelters_style::StateMask;

/// How far (points) a press moves before it is a drag: egui's `max_click_dist`.
pub const THRESHOLD: f32 = 6.0;

/// A press on a drag source that may become a drag.
#[derive(Clone, Debug)]
pub(crate) struct DragPress {
    pub pointer: u32,
    pub window: WindowId,
    pub source: NodeId,
    pub at: Point,
}

/// A drag in progress.
#[derive(Clone, Debug)]
pub(crate) struct Drag {
    pub pointer: u32,
    pub window: WindowId,
    pub source: NodeId,
    pub ty: Arc<str>,
    pub data: Arc<str>,
    pub target: Option<NodeId>,
}

/// `pattern` (one `drop-types` entry) accepts type `ty`.
pub fn type_matches(pattern: &str, ty: &str) -> bool {
    pattern == "*" || pattern == ty || pattern.strip_suffix('*').is_some_and(|prefix| prefix.ends_with('/') && ty.starts_with(prefix))
}

fn accepts(doc: &Document, node: NodeId, ty: &str) -> bool {
    let Some(v) = doc.view(node) else { return false };
    let types = v.prop_str(common::DROP_TYPES).unwrap_or("");
    !v.disabled() && types.split(|c: char| c.is_whitespace() || c == ',').filter(|t| !t.is_empty()).any(|t| type_matches(t, ty))
}

fn drag_type(doc: &Document, node: NodeId) -> &str {
    doc.view(node).and_then(|v| v.prop_str(common::DRAG_TYPE)).unwrap_or("")
}

/// The drop action for the modifiers held at the release.
pub fn action(m: Modifiers) -> &'static str {
    if cfg!(target_os = "macos") {
        if m.contains(Modifiers::SUPER) && m.contains(Modifiers::ALT) {
            "link"
        } else if m.contains(Modifiers::ALT) {
            "copy"
        } else {
            "move"
        }
    } else if m.contains(Modifiers::CTRL) && m.contains(Modifiers::SHIFT) {
        "link"
    } else if m.contains(Modifiers::CTRL) {
        "copy"
    } else {
        "move"
    }
}

/// A press on `t`: remembers the nearest source around it.
pub(crate) fn pressed(doc: &mut Document, window: WindowId, t: NodeId, p: &PointerEvent) {
    if !matches!(p.button, Some(MouseButton::Primary) | None) {
        return;
    }
    let mut a = Some(t);
    while let Some(n) = a {
        if !drag_type(doc, n).is_empty() && !doc.state(n).contains(StateMask::DISABLED) {
            doc.input.drag_press = Some(DragPress { pointer: p.pointer, window, source: n, at: p.pos });
            return;
        }
        a = doc.parent(n);
    }
}

/// The pointer's button went up (or the pointer was cancelled) outside a drag.
pub(crate) fn released(doc: &mut Document, pointer: u32) {
    if doc.input.drag_press.as_ref().is_some_and(|d| d.pointer == pointer) {
        doc.input.drag_press = None;
    }
}

/// Routes pointer events to an in-app drag (starting one when a press on a source moved far
/// enough). True when the drag took the event.
pub(crate) fn pointer(doc: &mut Document, window: WindowId, p: &PointerEvent, out: &mut Dispatched) -> bool {
    if let Some(d) = doc.input.drag.clone()
        && d.pointer == p.pointer
    {
        match p.phase {
            PointerPhase::Move | PointerPhase::Enter => retarget(doc, window, Some(p.pos), out),
            PointerPhase::Leave => retarget(doc, window, None, out),
            PointerPhase::Up => finish(doc, window, p, out),
            PointerPhase::Cancel => end(doc, None, out),
            PointerPhase::Down => {}
        }
        out.handled = true;
        return true;
    }
    let Some(press) = doc.input.drag_press.clone().filter(|d| d.pointer == p.pointer && d.window == window) else { return false };
    if p.phase != PointerPhase::Move || p.buttons == 0 || (p.pos - press.at).length() <= THRESHOLD || !doc.contains(press.source) {
        return false;
    }
    doc.input.drag_press = None;
    let source = press.source;
    // the press becomes the drag: the pressed node's press ends without a click
    if let Some(n) = doc.input.press.remove(&p.pointer).filter(|&n| doc.contains(n)) {
        doc.set_state(n, StateMask::PRESSED, false);
        let mut emitted = Vec::new();
        crate::input::dispatch::call_kind(doc, n, &crate::kind::Event::PointerCancel, &mut emitted, Some(p.pointer));
        for e in emitted {
            crate::input::dispatch::queue_emitted(doc, e);
        }
    }
    doc.input.release(p.pointer);
    let v = doc.view(source).expect("checked");
    let ty: Arc<str> = Arc::from(v.prop_str(common::DRAG_TYPE).unwrap_or(""));
    let data: Arc<str> = Arc::from(v.prop_str(common::DRAG_DATA).unwrap_or(""));
    doc.input.drag = Some(Drag { pointer: p.pointer, window, source, ty, data, target: None });
    doc.set_state(source, StateMask::DRAGGING, true);
    doc.emit_js(out, source, common::DRAG_BEGIN, None);
    retarget(doc, window, Some(p.pos), out);
    out.handled = true;
    true
}

/// The target under `pos` (`None`: the pointer left the window) takes over from the old one.
fn retarget(doc: &mut Document, window: WindowId, pos: Option<Point>, out: &mut Dispatched) {
    let Some(d) = doc.input.drag.clone() else { return };
    let mut target = None;
    let mut a = pos.and_then(|p| doc.hit_test(window, p));
    while let Some(n) = a {
        if accepts(doc, n, &d.ty) {
            target = Some(n);
            break;
        }
        a = doc.parent(n);
    }
    doc.input.cursor = Some(if target.is_some() { CursorIcon::Grabbing } else { CursorIcon::NoDrop });
    if target == d.target.filter(|&t| doc.contains(t)) {
        return;
    }
    if let Some(old) = d.target.filter(|&t| doc.contains(t)) {
        doc.set_state(old, StateMask::DROP_TARGET, false);
        doc.emit_js(out, old, common::DRAG_LEAVE, None);
    }
    if let Some(t) = target {
        doc.set_state(t, StateMask::DROP_TARGET, true);
        doc.emit_js(out, t, common::DRAG_ENTER, Some(Value::Str(d.ty.clone())));
    }
    if let Some(dr) = doc.input.drag.as_mut() {
        dr.target = target;
    }
}

/// The release: a drop on the target under the pointer, or a drag that nothing took.
fn finish(doc: &mut Document, window: WindowId, p: &PointerEvent, out: &mut Dispatched) {
    retarget(doc, window, Some(p.pos), out);
    let Some(d) = doc.input.drag.clone() else { return };
    let dropped = d.target.filter(|&t| doc.contains(t)).map(|t| (t, action(p.modifiers)));
    if let Some((t, act)) = dropped {
        let local = doc.to_local(window, t, p.pos);
        doc.set_state(t, StateMask::DROP_TARGET, false);
        doc.emit_js(out, t, common::DRAG_LEAVE, None);
        let rec: Vec<(Arc<str>, Value)> = vec![
            ("type".into(), Value::Str(d.ty.clone())),
            ("data".into(), Value::Str(d.data.clone())),
            ("x".into(), Value::F64(local.x as f64)),
            ("y".into(), Value::F64(local.y as f64)),
            ("action".into(), Value::from(act)),
        ];
        doc.emit_js(out, t, common::DROP, Some(Value::Record(rec.into())));
        if let Some(dr) = doc.input.drag.as_mut() {
            dr.target = None;
        }
    }
    end(doc, dropped.map(|(_, a)| a), out);
}

/// Ends the drag: the target (if any) is left, and the source gets `drag-end`.
fn end(doc: &mut Document, dropped: Option<&'static str>, out: &mut Dispatched) {
    let Some(d) = doc.input.drag.take() else { return };
    if let Some(t) = d.target.filter(|&t| doc.contains(t)) {
        doc.set_state(t, StateMask::DROP_TARGET, false);
        doc.emit_js(out, t, common::DRAG_LEAVE, None);
    }
    doc.input.cursor = Some(CursorIcon::Default);
    if doc.contains(d.source) {
        doc.set_state(d.source, StateMask::DRAGGING, false);
        let rec: Vec<(Arc<str>, Value)> = vec![("dropped".into(), Value::Bool(dropped.is_some())), ("action".into(), Value::from(dropped.unwrap_or("none")))];
        doc.emit_js(out, d.source, common::DRAG_END, Some(Value::Record(rec.into())));
    }
}

/// Escape cancels the drag of `window`; true when there was one.
pub(crate) fn cancel(doc: &mut Document, window: WindowId, out: &mut Dispatched) -> bool {
    if !doc.input.drag.as_ref().is_some_and(|d| d.window == window) {
        return false;
    }
    end(doc, None, out);
    true
}
