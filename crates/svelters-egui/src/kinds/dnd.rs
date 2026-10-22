//! WG4b: drag and drop on any node (plans/milestones/WG4b-dialogs-pickers-dnd.md).
//!
//! Three `Widget` props make any element a drag source, a drop target, or both:
//! - `drag-type` (non-empty: a source): the payload's type, a MIME-like string (`cad/part`);
//! - `drag-data`: the payload's data, a string (JSON for structured data);
//! - `drop-types` (non-empty: a target): the types it accepts, separated by spaces or commas;
//!   `*` accepts any type and `cad/*` any type starting with `cad/`.
//!
//! Signals (on the `Widget` base): the source gets `drag-begin` and `drag-end(dropped)`, the
//! target `drag-enter(type)`, `drag-leave` and `drop(type, data, x, y)` (`x`, `y`: the drop point
//! in the target's own points). Every `drag-enter` is matched by a `drag-leave`, a drop included
//! (`drag-leave`, then `drop`).
//!
//! The payload travels in egui's own [`egui::DragAndDrop`] as a [`DragPayload`], so Rust panels of
//! the host app take part: `DragAndDrop::set_payload(ctx, DragPayload::new("cad/part", id))`
//! starts a drag Svelte targets accept, and `response.dnd_release_payload::<DragPayload>()` (then
//! [`DragPayload::accept`]) takes a Svelte drag.
//!
//! Feedback: the cursor is `Grab` over a source, `Grabbing` while the drag is over a target that
//! accepts it and `NoDrop` elsewhere; a label with the source's text (or its type) follows the
//! pointer; the active target is outlined in the selection colour.
//!
//! Nested targets: the deepest (last painted) accepting target under the pointer wins. Targets
//! claim the pointer during the walk; the claim of the previous pass decides, so the target is
//! one frame late, the same as egui's style feedback (a drag repaints every frame).
//!
//! The walk calls [`node`] once per node with any of the props (`walk.rs`: for a leaf after its
//! widget, for a container before its children so they stay on top of the source's drag area).
//!
//! Writer lane: WIDGETS (WG4b).

use super::{P, SignalDef, Ty, signal};
use crate::scene::{Arena, Common};
use crate::walk::Out;
use egui::{CursorIcon, DragAndDrop, Id, LayerId, Order, Rect, Sense, StrokeKind, Ui};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use svelters_abi::Value;

/// The `Widget` signals of drag and drop.
pub(super) static DND_SIGNALS: &[SignalDef] = &[
    signal("drag-begin", super::builtin::NONE, "Drag source: the user started dragging this node (`drag-type` set)."),
    signal("drag-end", &[("dropped", Ty::Bool)], "Drag source: the drag ended; `dropped` is true when a target took it."),
    signal("drag-enter", &[("type", Ty::Str)], "Drop target: a drag it accepts entered it (`drop-types` set); carries the drag's type."),
    signal("drag-leave", super::builtin::NONE, "Drop target: the drag left, was cancelled, or was dropped here (before `drop`)."),
    signal(
        "drop",
        &[("type", Ty::Str), ("data", Ty::Str), ("x", Ty::F64), ("y", Ty::F64)],
        "Drop target: a drag it accepts was released over it; carries the type, the data and the point in the node's own points.",
    ),
];

/// The drag-and-drop props of a node (in [`Common::dnd`], boxed: most nodes have none).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Dnd {
    pub drag_type: String,
    pub drag_data: String,
    pub drop_types: String,
}

/// Writes `drag-type`, `drag-data` or `drop-types`.
pub(crate) fn set(c: &mut Common, key: P, v: String) {
    let d = c.dnd.get_or_insert_with(Default::default);
    match key {
        P::DragType => d.drag_type = v,
        P::DragData => d.drag_data = v,
        P::DropTypes => d.drop_types = v,
        _ => unreachable!("not a drag-and-drop prop"),
    }
    if **d == Dnd::default() {
        c.dnd = None;
    }
}

/// Reads `drag-type`, `drag-data` or `drop-types`.
pub(crate) fn get(c: &Common, key: P) -> &str {
    let Some(d) = c.dnd.as_deref() else { return "" };
    match key {
        P::DragType => &d.drag_type,
        P::DragData => &d.drag_data,
        P::DropTypes => &d.drop_types,
        _ => "",
    }
}

/// What a drag carries in [`egui::DragAndDrop`] (svelters' drags, and drags Rust panels start for
/// Svelte targets).
#[derive(Debug)]
pub struct DragPayload {
    /// The type (`drag-type`).
    pub ty: String,
    /// The data (`drag-data`).
    pub data: String,
    /// The source node's egui id (`None` for drags Rust started).
    pub source: Option<Id>,
    dropped: Arc<AtomicBool>,
}

impl DragPayload {
    pub fn new(ty: impl Into<String>, data: impl Into<String>) -> DragPayload {
        DragPayload { ty: ty.into(), data: data.into(), source: None, dropped: Arc::default() }
    }

    /// A target took the drag: the source's `drag-end` reports `dropped: true`. Svelte targets
    /// call it themselves; a Rust target calls it after `dnd_release_payload`.
    pub fn accept(&self) {
        self.dropped.store(true, Ordering::Relaxed);
    }

    pub fn was_dropped(&self) -> bool {
        self.dropped.load(Ordering::Relaxed)
    }
}

/// Whether `types` (a `drop-types` value) accepts a drag of type `ty`.
pub fn accepts(types: &str, ty: &str) -> bool {
    types.split(|c: char| c.is_whitespace() || c == ',').filter(|t| !t.is_empty()).any(|t| {
        t == "*" || t == ty || t.strip_suffix('*').is_some_and(|prefix| prefix.ends_with('/') && ty.starts_with(prefix))
    })
}

/// The targets' claims on the pointer, per viewport: this pass's (the last accepting target
/// painted under the pointer) and the previous pass's, which decides.
#[derive(Clone, Copy, Default)]
struct Claims {
    pass: u64,
    cur: Option<Id>,
    prev: Option<Id>,
}

fn claims_id(ctx: &egui::Context) -> Id {
    Id::new("svelters-dnd-claims").with(ctx.viewport_id())
}

/// The claims rolled over to this pass.
fn claims(ctx: &egui::Context) -> Claims {
    let pass = ctx.cumulative_pass_nr();
    let mut c: Claims = ctx.data(|d| d.get_temp(claims_id(ctx))).unwrap_or_default();
    if c.pass != pass {
        c = Claims { pass, cur: None, prev: if c.pass + 1 == pass { c.cur } else { None } };
        ctx.data_mut(|d| d.insert_temp(claims_id(ctx), c));
    }
    c
}

fn claim(ctx: &egui::Context, id: Id) {
    let mut c = claims(ctx);
    c.cur = Some(id);
    ctx.data_mut(|d| d.insert_temp(claims_id(ctx), c));
}

/// The source's drag in flight: the payload's `dropped` flag, and whether it ended last pass.
#[derive(Clone)]
struct InFlight {
    dropped: Arc<AtomicBool>,
    ended: bool,
}

/// Drag and drop for node `i` drawn at `r` (screen) in `ui`. Does nothing for nodes without the
/// props.
pub(crate) fn node(arena: &mut Arena, i: usize, r: Rect, ui: &mut Ui, out: &mut Vec<Out>) {
    let Some(d) = arena.common(i).dnd.as_deref() else { return };
    let (drag_type, drag_data, drop_types) = (d.drag_type.clone(), d.drag_data.clone(), d.drop_types.clone());
    let id = arena.core[i].egui_id;
    let kind = arena.core[i].kind as usize;
    let h = arena.slots.id_at(i).expect("live node").to_raw();
    let mut emit = |arena: &Arena, name: &str, args: Vec<Value>| {
        let s = super::signal_id(kind, name);
        if arena.listens(i, s) {
            out.push(Out::Event(h, s, args));
        }
    };
    let ctx = ui.ctx().clone();
    if !drag_type.is_empty() {
        let key = id.with("dnd-flight");
        let flight: Option<InFlight> = ctx.data(|m| m.get_temp(key));
        // the pass after the release: every target has run, so `dropped` is final
        if let Some(f) = flight.as_ref().filter(|f| f.ended) {
            emit(arena, "drag-end", vec![Value::Bool(f.dropped.load(Ordering::Relaxed))]);
            ctx.data_mut(|m| m.remove::<InFlight>(key));
        }
        let resp = ui.interact(r, id.with("dnd-src"), Sense::drag());
        if resp.drag_started() {
            let p = DragPayload { ty: drag_type.clone(), data: drag_data, source: Some(id), dropped: Arc::default() };
            ctx.data_mut(|m| m.insert_temp(key, InFlight { dropped: p.dropped.clone(), ended: false }));
            DragAndDrop::set_payload(&ctx, p);
            emit(arena, "drag-begin", vec![]);
        }
        if resp.dragged() {
            let over = claims(&ctx).prev.is_some();
            ctx.set_cursor_icon(if over { CursorIcon::Grabbing } else { CursorIcon::NoDrop });
            let text = crate::scene::node::text_of(arena.w(i));
            ghost(ui, id, if text.is_empty() { &drag_type } else { text });
        } else if resp.hovered() {
            ctx.set_cursor_icon(CursorIcon::Grab);
        }
        if resp.drag_stopped() {
            if let Some(mut f) = ctx.data(|m| m.get_temp::<InFlight>(key)) {
                f.ended = true;
                ctx.data_mut(|m| m.insert_temp(key, f));
            }
            ctx.request_repaint();
        }
    }
    if !drop_types.is_empty() {
        let key = id.with("dnd-in");
        let was_in: bool = ctx.data(|m| m.get_temp(key)).unwrap_or(false);
        let payload = DragAndDrop::payload::<DragPayload>(&ctx).filter(|p| p.source != Some(id) && accepts(&drop_types, &p.ty));
        let inside = ctx.rect_contains_pointer(ui.layer_id(), r.intersect(ui.clip_rect()));
        if payload.is_some() && inside {
            claim(&ctx, id);
        }
        let active = payload.is_some() && inside && claims(&ctx).prev == Some(id);
        if active {
            ui.painter().rect_stroke(r, 2.0, ui.visuals().selection.stroke, StrokeKind::Inside);
            ctx.request_repaint(); // the claim decides next pass
        }
        if active && !was_in {
            emit(arena, "drag-enter", vec![Value::Str(payload.as_ref().map(|p| p.ty.clone()).unwrap_or_default())]);
        }
        let released = active && ctx.input(|inp| inp.pointer.any_released());
        if was_in && (!active || released) {
            emit(arena, "drag-leave", vec![]);
        }
        if released && let Some(p) = DragAndDrop::take_payload::<DragPayload>(&ctx) {
            p.accept();
            let at = ctx.pointer_interact_pos().unwrap_or(r.min) - r.min;
            emit(arena, "drop", vec![Value::Str(p.ty.clone()), Value::Str(p.data.clone()), Value::F64(at.x as f64), Value::F64(at.y as f64)]);
        }
        let now_in = active && !released;
        if now_in != was_in {
            ctx.data_mut(|m| m.insert_temp(key, now_in));
        }
    }
}

/// The label that follows the pointer during a drag.
fn ghost(ui: &Ui, id: Id, text: &str) {
    let Some(pos) = ui.ctx().pointer_interact_pos() else { return };
    let painter = ui.ctx().layer_painter(LayerId::new(Order::Tooltip, id.with("dnd-ghost")));
    let v = ui.visuals();
    let font = egui::TextStyle::Body.resolve(ui.style());
    let galley = painter.layout_no_wrap(text.to_string(), font, v.text_color());
    let rect = Rect::from_min_size(pos + egui::vec2(14.0, 14.0), galley.size()).expand(4.0);
    painter.rect(rect, 4.0, v.window_fill, v.window_stroke, StrokeKind::Outside);
    painter.galley(rect.min + egui::vec2(4.0, 4.0), galley, v.text_color());
}

#[cfg(test)]
mod tests {
    use super::accepts;

    #[test]
    fn drop_types_match_exactly_by_prefix_or_any() {
        assert!(accepts("cad/part", "cad/part"));
        assert!(accepts("text/plain, cad/part", "cad/part"));
        assert!(accepts("cad/*", "cad/net"));
        assert!(!accepts("cad/*", "cadx/net"));
        assert!(accepts("*", "anything"));
        assert!(!accepts("cad/part", "cad/net"));
        assert!(!accepts("", "cad/part"));
    }
}
