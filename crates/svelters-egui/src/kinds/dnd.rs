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
//! pointer (OW5a1b: the source's `::part(ghost)`, [`GHOST`]); the active target gets
//! `:drop-target` and its `::part(drop-line)` outline. The source has `:dragging` meanwhile.
//!
//! OW5a1b: the two halves are the `DragSource` and `DropTarget` units (`interact/dnd.rs`).
//!
//! Nested targets: the deepest (last painted) accepting target under the pointer wins. Targets
//! claim the pointer during the walk; the claim of the previous pass decides, so the target is
//! one frame late, the same as egui's style feedback (a drag repaints every frame).
//!
//! The walk calls [`node`] for each node with any of the props (`walk.rs`): for a leaf, the source
//! half before its widget (the drag area lies under the widget, which keeps its clicks, and its own
//! drags if it senses drags: a draggable slider still slides) and the target half after it; for a
//! container, both before its children (they stay on top).
//!
//! WG6 (QA9, plans/milestones/WG6-widget-polish.md):
//! - **Drop actions**: `drop` and the source's `drag-end` carry an `action` from the modifiers held
//!   at the release (Qt's `Qt::DropAction`): `copy` with Ctrl (Option on macOS), `link` with
//!   Ctrl+Shift (Cmd+Option on macOS), else `move`; the cursor over the target shows it. A file
//!   dropped from the OS is always `copy`. `drag-end`'s action is `none` when nothing took it.
//! - **File drops from the OS on any node**: a target whose `drop-types` accepts `text/uri-list`
//!   (or `*`) takes files dragged in from the file manager (egui's `hovered_files` and
//!   `dropped_files`): `drag-enter("text/uri-list")`, `drag-leave`, then `drop("text/uri-list",
//!   uris, x, y, "copy")`, the `file://` URIs one per line (`\r\n`, RFC 2483).
//! - **Auto-scroll**: a `scroll-area` and a `virtual-list`/`tree-view` scroll while a drag (in-app
//!   or files) is held within [`EDGE`] points of their edge ([`auto_scroll`], through Scrollable).
//!
//! Writer lane: WIDGETS (WG4b).

use super::P;
use crate::interact::{Drag, DragSource, DragSourceParams, DropIntent, DropParams, DropTarget, Intents, Offer, PartResponses, Unit};
use crate::paint::{self, PaintCx, StyleView, TraceKind, Ua, UaRule, UaValue::*};
use crate::provider::StateBits;
use crate::scene::{Arena, Common, Scene};
use crate::walk::Out;
use egui::{CursorIcon, DragAndDrop, Id, LayerId, Order, Rect, Sense, StrokeKind, Ui};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use svelters_abi::Value;
use svelters_host::platform::{type_matches, MimeData};

// The `Widget` signals of drag and drop are in `builtin.rs` (`WIDGET_SIGNALS`, with TR2's
// `animationend`).

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
/// Svelte targets). DX2: its data is a [`MimeData`], the clipboard's type, so a drag and a copy carry
/// the same thing: a Svelte drag is one entry (`drag-type`, `drag-data`); a Rust drag may carry
/// several types (`DragPayload::from_mime`), and a target takes the first of its `drop-types` the
/// payload has. `clipboard.write(kind, payload.data.clone())` copies a drag;
/// `DragPayload::from_mime(clipboard.read(kind, &[])?)` drags what was copied.
#[derive(Debug)]
pub struct DragPayload {
    /// The data by type (a Svelte drag: one entry, `drag-type` and `drag-data`).
    pub data: MimeData,
    /// The source node's egui id (`None` for drags Rust started).
    pub source: Option<Id>,
    dropped: Arc<AtomicBool>,
    /// WG6: the action the target took it with (an index into [`ACTIONS`]).
    action: Arc<AtomicU8>,
}

/// WG6: the drop actions (`drop`'s and `drag-end`'s `action`).
pub const ACTIONS: &[&str] = &["none", "move", "copy", "link"];

/// WG6: the drop action for the modifiers held at the release (Qt's defaults: Ctrl copies,
/// Ctrl+Shift links; on macOS Option copies and Cmd+Option links).
pub fn drop_action(m: egui::Modifiers, mac: bool) -> &'static str {
    if mac {
        match (m.mac_cmd || m.command, m.alt) {
            (true, true) => "link",
            (false, true) => "copy",
            _ => "move",
        }
    } else {
        match (m.ctrl || m.command, m.shift) {
            (true, true) => "link",
            (true, false) => "copy",
            _ => "move",
        }
    }
}

/// WG6: the type a file drop from the OS has.
pub const URI_LIST: &str = "text/uri-list";

impl DragPayload {
    /// One entry: type `ty`, data `data`.
    pub fn new(ty: impl Into<String>, data: impl Into<String>) -> DragPayload {
        DragPayload::from_mime(MimeData::single(ty, data.into()))
    }

    /// OW5a1b: a drag a [`DragSource`] began, linked to its flags.
    pub(crate) fn linked(data: MimeData, source: Option<Id>, dropped: Arc<AtomicBool>, action: Arc<AtomicU8>) -> DragPayload {
        DragPayload { data, source, dropped, action }
    }

    /// Several types (a Rust drag, or the clipboard's contents).
    pub fn from_mime(data: MimeData) -> DragPayload {
        DragPayload { data, source: None, dropped: Arc::default(), action: Arc::default() }
    }

    /// The first type (a Svelte drag's `drag-type`); `""` when empty.
    pub fn ty(&self) -> &str {
        self.data.types().next().unwrap_or("")
    }

    /// The entry a target with `drop-types` `types` takes: the first of its patterns the payload
    /// has a type for, as `(type, data)` (data as text, lossy for non-UTF-8 bytes).
    pub fn best(&self, types: &str) -> Option<(&str, String)> {
        let patterns: Vec<&str> = patterns(types).collect();
        if patterns.is_empty() {
            return None;
        }
        self.data.best(&patterns).map(|(t, d)| (t, String::from_utf8_lossy(d).into_owned()))
    }

    /// A target took the drag: the source's `drag-end` reports `dropped: true`. Svelte targets
    /// call it themselves; a Rust target calls it after `dnd_release_payload`.
    pub fn accept(&self) {
        self.dropped.store(true, Ordering::Relaxed);
    }

    pub fn was_dropped(&self) -> bool {
        self.dropped.load(Ordering::Relaxed)
    }

    /// WG6: [`accept`](Self::accept) with a drop action (one of [`ACTIONS`]).
    pub fn accept_as(&self, action: &str) {
        let k = ACTIONS.iter().position(|a| *a == action).unwrap_or(1);
        self.action.store(k as u8, Ordering::Relaxed);
        self.accept();
    }

    /// WG6: the action a target took it with (`none` before a drop; `move` for a plain `accept`).
    pub fn action(&self) -> &'static str {
        match self.action.load(Ordering::Relaxed) {
            0 if self.was_dropped() => "move",
            k => ACTIONS.get(k as usize).copied().unwrap_or("none"),
        }
    }
}

fn patterns(types: &str) -> impl Iterator<Item = &str> {
    types.split(|c: char| c.is_whitespace() || c == ',').filter(|t| !t.is_empty())
}

/// Whether `types` (a `drop-types` value) accepts a drag of type `ty`. The patterns are the
/// clipboard's ([`svelters_host::platform::type_matches`]).
pub fn accepts(types: &str, ty: &str) -> bool {
    patterns(types).any(|t| type_matches(t, ty))
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
    Id::unique("svelters-dnd-claims").with(ctx.viewport_id())
}

/// The claims rolled over to this pass.
fn claims(ctx: &egui::Context) -> Claims {
    let pass = ctx.cumulative_pass_nr();
    let key = claims_id(ctx);
    let mut c: Claims = ctx.data(|d| d.get_temp(key)).unwrap_or_default();
    if c.pass != pass {
        c = Claims { pass, cur: None, prev: if c.pass + 1 == pass { c.cur } else { None } };
        ctx.data_mut(|d| d.insert_temp(key, c));
    }
    c
}

/// Whether target `id` held the pointer last pass (it decides this pass). WG5b's tree rows use it.
pub(crate) fn claimed(ctx: &egui::Context, id: Id) -> bool {
    claims(ctx).prev == Some(id)
}

/// Whether a drag from any source is over some accepting target (the source's cursor).
pub(crate) fn over_target(ctx: &egui::Context) -> bool {
    claims(ctx).prev.is_some()
}

pub(crate) fn claim(ctx: &egui::Context, id: Id) {
    let mut c = claims(ctx);
    c.cur = Some(id);
    let key = claims_id(ctx);
    ctx.data_mut(|d| d.insert_temp(key, c));
}

/// Which half of [`node`] to run. A leaf's drag area goes under its widget (registered before it,
/// so the widget keeps its clicks, and its own drags when it senses them); its target half after.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Part {
    Source,
    Target,
    Both,
}

/// Drag and drop for node `i` drawn at `r` (screen) in `ui`. Does nothing for nodes without the
/// props. OW5a1b: the halves are the [`DragSource`] and [`DropTarget`] units (their state in
/// egui's temp data, by the node's id: the props are shared between equal nodes, SG6); this
/// function feeds them what the context knows and raises the WG4b/WG6 signals.
pub(crate) fn node(arena: &mut Arena, i: usize, r: Rect, ui: &mut Ui, out: &mut Vec<Out>, part: Part) {
    let Some(d) = arena.common(i).dnd.as_deref() else {
        return;
    };
    let (drag_type, drag_data, drop_types) = (d.drag_type.clone(), d.drag_data.clone(), d.drop_types.clone());
    let id = arena.core[i].egui_id;
    let kind = arena.core[i].kind as usize;
    let h = arena.slots.id_at(i).expect("live node").to_raw();
    let emit = |arena: &Arena, out: &mut Vec<Out>, name: &str, args: Vec<Value>| {
        let s = super::signal_id(kind, name);
        if arena.listens(i, s) {
            out.push(Out::Event(h, s, args));
        }
    };
    let ctx = ui.ctx().clone();
    let mac = mac(&ctx);
    if part != Part::Target && !drag_type.is_empty() {
        let key = id.with("dnd-source");
        let mut src: DragSource = ctx.data(|m| m.get_temp(key)).unwrap_or_default();
        let resp = ui.interact(r, id.with("dnd-src"), Sense::drag());
        let params = DragSourceParams { on: true, mac, over_target: over_target(&ctx) };
        let mut state = arena.core[i].state;
        let mut fired = Intents::default();
        crate::interact::run(ui, &resp, &mut state, &PartResponses::default(), |cx| src.input(&params, cx, &mut fired));
        for f in fired {
            match f {
                Drag::Begin => {
                    DragAndDrop::set_payload(&ctx, src.payload(MimeData::single(drag_type.clone(), drag_data.clone()), Some(id)));
                    emit(arena, out, "drag-begin", vec![]);
                }
                Drag::End { dropped, action } => emit(arena, out, "drag-end", vec![Value::Bool(dropped), Value::Str(action.into())]),
            }
        }
        if let Some(c) = src.cursor() {
            ctx.set_cursor_icon(c);
        }
        if src.dragging() {
            let text = crate::scene::node::text_of(arena.w(i)).to_string();
            ghost(arena, i, ui, id, if text.is_empty() { &drag_type } else { &text });
        }
        if src.next_frame().is_some() {
            ctx.request_repaint();
        }
        ctx.data_mut(|m| m.insert_temp(key, src));
        crate::style::set_own_state(arena, i, state);
    }
    if part != Part::Source && !drop_types.is_empty() {
        let key = id.with("dnd-target");
        let mut tgt: DropTarget = ctx.data(|m| m.get_temp(key)).unwrap_or_default();
        let payload = DragAndDrop::payload::<DragPayload>(&ctx).filter(|p| p.source != Some(id) && p.best(&drop_types).is_some());
        // WG6: files dragged in from the OS (no in-app drag at the same time)
        let files = payload.is_none() && accepts(&drop_types, URI_LIST) && ctx.input(|i| !i.raw.hovered_files.is_empty() || !i.raw.dropped_files.is_empty());
        let inside = if files {
            ctx.input(|i| i.pointer.latest_pos()).is_some_and(|p| r.intersect(ui.clip_rect()).contains(p) && ctx.layer_id_at(p).is_none_or(|l| l == ui.layer_id()))
        } else {
            payload.is_some() && ctx.rect_contains_pointer(ui.layer_id(), r.intersect(ui.clip_rect()))
        };
        if inside {
            claim(&ctx, id);
        }
        let claimed = inside && claims(&ctx).prev == Some(id);
        let (released, offer, action, at) = if files {
            let dropped: Vec<egui::DroppedFileHandle> = ctx.input(|i| i.raw.dropped_files.clone());
            let released = !dropped.is_empty();
            let data = released.then(|| uri_list(dropped.iter().map(|f| f.path())));
            let at = ctx.input(|i| i.pointer.latest_pos()).unwrap_or(r.min) - r.min;
            (released, Some(Offer { ty: URI_LIST.into(), data }), "copy", at)
        } else {
            let released = ctx.input(|inp| inp.pointer.any_released());
            let offer = payload.as_ref().and_then(|p| p.best(&drop_types)).map(|(t, d)| Offer { ty: t.to_string(), data: released.then_some(d) });
            let at = ctx.pointer_interact_pos().unwrap_or(r.min) - r.min;
            (released, offer, drop_action(ctx.input(|i| i.modifiers), mac), at)
        };
        let params = DropParams { offer, inside: claimed, released, at, action };
        let mut state = arena.core[i].state;
        let fired = crate::interact::dnd::run_bare(&ctx, id.with("dnd-tgt"), r, ui.is_enabled(), &mut state, &mut tgt, &params);
        for f in fired {
            match f {
                DropIntent::Enter(ty) => emit(arena, out, "drag-enter", vec![Value::Str(ty)]),
                DropIntent::Leave => emit(arena, out, "drag-leave", vec![]),
                DropIntent::Drop { ty, data, at, action } => {
                    if !files && let Some(p) = DragAndDrop::take_payload::<DragPayload>(&ctx) {
                        p.accept_as(action);
                    }
                    emit(arena, out, "drop", vec![Value::Str(ty), Value::Str(data), Value::F64(at.x as f64), Value::F64(at.y as f64), Value::Str(action.into())]);
                }
            }
        }
        if tgt.active() {
            // OW5a1b: the target's outline is its `::part(drop-line)` (UA: 2 pt `focus`)
            if files {
                ctx.set_cursor_icon(CursorIcon::Copy);
                let painter = ctx.layer_painter(LayerId::new(Order::Foreground, id.with("dnd-files")));
                paint_target(arena, i, &painter, r, ui);
            } else {
                paint_target(arena, i, ui.painter(), r, ui);
            }
        }
        if claimed || tgt.active() {
            ctx.request_repaint(); // the claim decides next pass
        }
        ctx.data_mut(|m| m.insert_temp(key, tgt));
        crate::style::set_own_state(arena, i, state);
    }
}

fn mac(ctx: &egui::Context) -> bool {
    ctx.os() == egui::os::OperatingSystem::Mac
}

/// WG6: the `file://` URIs of dropped files' paths, one per line (`text/uri-list`).
pub fn uri_list<'a>(paths: impl IntoIterator<Item = &'a std::path::Path>) -> String {
    paths.into_iter().map(svelters_host::platform::file_uri).collect::<Vec<_>>().join("\r\n")
}

/// OW5a1b: `::part(ghost)`, the label that follows the pointer during a drag (R12's lift:
/// a floating surface at `elev-3`, 0.9 opacity).
pub const GHOST: Ua = Ua(&[UaRule::base(&[
    ("background-color", Token("menu-bg")),
    ("border-color", Token("menu-border-color")),
    ("border-width", Token("stroke")),
    ("border-radius", Token("menu-radius")),
    ("box-shadow", Token("elev-3")),
    ("color", Token("fg")),
    ("opacity", Number(0.9)),
    ("padding-left", Px(6.0)),
    ("padding-right", Px(6.0)),
    ("padding-top", Px(3.0)),
    ("padding-bottom", Px(3.0)),
])]);

fn dark(ui: &Ui) -> bool {
    ui.ctx().theme() == egui::Theme::Dark
}

/// The drop target's outline: node `i`'s `::part(drop-line)` (its colour and `height` as the
/// stroke) around `r`.
fn paint_target(s: &mut Scene, i: usize, painter: &egui::Painter, r: Rect, ui: &Ui) {
    let node = s.slots.id_at(i).expect("live node");
    let bits = s.core[i].state | StateBits::DROP_TARGET;
    let css = s.part_at(node, "drop-line", bits, (0, 1));
    let host = StyleView::new(s.style.computed(node).map(|c| &**c), Ua(&[]), bits, dark(ui));
    let st = host.part(css.as_deref(), crate::widgets::list_rows::DROP_LINE);
    let color = st.color("background-color").unwrap_or(egui::Color32::LIGHT_BLUE);
    let mut pc = PaintCx::for_node(painter, node.to_raw());
    let mut pc = pc.part("drop-line");
    let stroke = egui::Stroke::new(pc.snap_width(st.px("height").unwrap_or(2.0).max(1.0)), color);
    pc.painter.rect_stroke(r, 2.0, stroke, StrokeKind::Inside);
    pc.record(TraceKind::Box, r, egui::Color32::TRANSPARENT, stroke, 0.0);
}

/// OW5a1b: the drag ghost of node `i` (a source node, or the list whose rows drag) with `text`,
/// at the pointer: `::part(ghost)` of the node over [`GHOST`], on a tooltip layer.
pub(crate) fn ghost(s: &mut Scene, i: usize, ui: &Ui, id: Id, text: &str) {
    let Some(pos) = ui.ctx().pointer_interact_pos() else {
        return;
    };
    let node = s.slots.id_at(i).expect("live node");
    let bits = s.core[i].state | StateBits::DRAGGING;
    let css = s.part_at(node, "ghost", bits, (0, 1));
    let host = StyleView::new(s.style.computed(node).map(|c| &**c), Ua(&[]), bits, dark(ui));
    let st = host.part(css.as_deref(), GHOST);
    let mut painter = ui.ctx().layer_painter(LayerId::new(Order::Tooltip, id.with("dnd-ghost")));
    painter.set_opacity(st.opacity());
    let galley = paint::text_galley(ui.ctx(), &st, text, None);
    let pad = |k: &str| st.px(k).unwrap_or(0.0);
    let content = Rect::from_min_size(pos + egui::vec2(14.0, 14.0), galley.size());
    let rect = Rect::from_min_max(content.min - egui::vec2(pad("padding-left"), pad("padding-top")), content.max + egui::vec2(pad("padding-right"), pad("padding-bottom")));
    let mut pc = PaintCx::for_node(&painter, node.to_raw());
    let mut pc = pc.part("ghost");
    paint::paint_box(&mut pc, &st, rect);
    paint::paint_text(&mut pc, &st, &galley, content, paint::Align::Start);
}

/// WG6: how close to a scroll area's edge a drag scrolls it, in points.
pub const EDGE: f32 = 24.0;

/// WG6 (OW5a: on the Scrollable unit): while a drag (in-app or files from the OS) is held within
/// [`EDGE`] of `view`'s edge, the offset change that scrolls towards it (up to 12 points a frame,
/// faster nearer the edge; positive moves the content towards its end). `None` with no drag.
pub(crate) fn auto_scroll(ctx: &egui::Context, view: Rect) -> Option<egui::Vec2> {
    let dragging = DragAndDrop::has_any_payload(ctx) || ctx.input(|i| !i.raw.hovered_files.is_empty());
    if !dragging {
        return None;
    }
    let p = ctx.input(|i| i.pointer.latest_pos())?;
    let d = -edge_delta(view, p);
    (d != egui::Vec2::ZERO).then_some(d)
}

/// The auto-scroll delta for the pointer at `p` in rect `r` (`scroll_with_delta`: positive
/// reveals the content above or to the left).
pub(crate) fn edge_delta(r: Rect, p: egui::Pos2) -> egui::Vec2 {
    if !r.contains(p) {
        return egui::Vec2::ZERO;
    }
    let f = |d: f32| if d < EDGE { (EDGE - d) / EDGE * 12.0 } else { 0.0 };
    egui::vec2(f(p.x - r.left()) - f(r.right() - p.x), f(p.y - r.top()) - f(r.bottom() - p.y))
}

#[cfg(test)]
mod tests {
    use super::{accepts, DragPayload};
    use svelters_host::platform::MimeData;

    #[test]
    fn wg6_drop_actions_and_uri_list() {
        use super::{drop_action, edge_delta, uri_list, DragPayload};
        use egui::Modifiers as M;
        assert_eq!(drop_action(M::NONE, false), "move");
        assert_eq!(drop_action(M::CTRL, false), "copy");
        assert_eq!(drop_action(M::CTRL | M::SHIFT, false), "link");
        assert_eq!(drop_action(M::ALT, true), "copy");
        assert_eq!(drop_action(M::MAC_CMD | M::ALT, true), "link");
        let p = DragPayload::new("cad/part", "1");
        assert_eq!(p.action(), "none");
        p.accept_as("copy");
        assert!(p.was_dropped());
        assert_eq!(p.action(), "copy");
        let paths = [std::path::Path::new("/a b/x.step"), std::path::Path::new("/y.kicad_pcb")];
        assert_eq!(uri_list(paths), "file:///a%20b/x.step\r\nfile:///y.kicad_pcb");
        let r = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(100.0, 100.0));
        assert_eq!(edge_delta(r, egui::pos2(50.0, 50.0)), egui::Vec2::ZERO);
        assert!(edge_delta(r, egui::pos2(50.0, 2.0)).y > 0.0, "near the top: reveal above");
        assert!(edge_delta(r, egui::pos2(50.0, 98.0)).y < 0.0);
        assert_eq!(edge_delta(r, egui::pos2(150.0, 98.0)), egui::Vec2::ZERO);
    }

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

    #[test]
    fn payload_is_mime_data_and_targets_take_their_first_type() {
        let svelte = DragPayload::new("cad/part", "R1");
        assert_eq!(svelte.ty(), "cad/part");
        assert_eq!(svelte.data, MimeData::single("cad/part", "R1"));
        let rust = DragPayload::from_mime(MimeData::new().with("text/plain", "R1 10k").with("cad/part", "R1"));
        assert_eq!(rust.best("cad/* text/plain"), Some(("cad/part", "R1".to_string())));
        assert_eq!(rust.best("text/plain"), Some(("text/plain", "R1 10k".to_string())));
        assert_eq!(rust.best("image/png"), None);
        assert_eq!(rust.best(""), None);
    }
}
