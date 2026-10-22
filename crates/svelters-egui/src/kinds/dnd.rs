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
//! - **Auto-scroll**: a `scroll-area` scrolls while a drag (in-app or files) is held within
//!   [`EDGE`] points of its edge ([`auto_scroll`]).
//!
//! Writer lane: WIDGETS (WG4b).

use super::P;
use crate::scene::{Arena, Common};
use crate::walk::Out;
use egui::{CursorIcon, DragAndDrop, Id, LayerId, Order, Rect, Sense, StrokeKind, Ui};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
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
    action: Arc<std::sync::atomic::AtomicU8>,
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

fn claim(ctx: &egui::Context, id: Id) {
    let mut c = claims(ctx);
    c.cur = Some(id);
    let key = claims_id(ctx);
    ctx.data_mut(|d| d.insert_temp(key, c));
}

/// The source's drag in flight: the payload's `dropped` flag, and whether it ended last pass.
#[derive(Clone)]
struct InFlight {
    dropped: Arc<AtomicBool>,
    action: Arc<std::sync::atomic::AtomicU8>,
    ended: bool,
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
/// props.
pub(crate) fn node(arena: &mut Arena, i: usize, r: Rect, ui: &mut Ui, out: &mut Vec<Out>, part: Part) {
    let Some(d) = arena.common(i).dnd.as_deref() else {
        return;
    };
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
    if part != Part::Target && !drag_type.is_empty() {
        let key = id.with("dnd-flight");
        let flight: Option<InFlight> = ctx.data(|m| m.get_temp(key));
        // the pass after the release: every target has run, so `dropped` is final
        if let Some(f) = flight.as_ref().filter(|f| f.ended) {
            let dropped = f.dropped.load(Ordering::Relaxed);
            let action = match f.action.load(Ordering::Relaxed) {
                0 if dropped => "move",
                k => ACTIONS.get(k as usize).copied().unwrap_or("none"),
            };
            emit(arena, "drag-end", vec![Value::Bool(dropped), Value::Str(action.into())]);
            ctx.data_mut(|m| m.remove::<InFlight>(key));
        }
        let resp = ui.interact(r, id.with("dnd-src"), Sense::drag());
        if resp.drag_started() {
            let p = DragPayload { data: MimeData::single(drag_type.clone(), drag_data), source: Some(id), dropped: Arc::default(), action: Arc::default() };
            ctx.data_mut(|m| m.insert_temp(key, InFlight { dropped: p.dropped.clone(), action: p.action.clone(), ended: false }));
            DragAndDrop::set_payload(&ctx, p);
            emit(arena, "drag-begin", vec![]);
        }
        if resp.dragged() {
            let over = claims(&ctx).prev.is_some();
            let cursor = match (over, drop_action(ctx.input(|i| i.modifiers), mac(&ctx))) {
                (false, _) => CursorIcon::NoDrop,
                (true, "copy") => CursorIcon::Copy,
                (true, "link") => CursorIcon::Alias,
                _ => CursorIcon::Grabbing,
            };
            ctx.set_cursor_icon(cursor);
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
    if part != Part::Source && !drop_types.is_empty() {
        let key = id.with("dnd-in");
        let was_in: bool = ctx.data(|m| m.get_temp(key)).unwrap_or(false);
        let payload = DragAndDrop::payload::<DragPayload>(&ctx).filter(|p| p.source != Some(id) && p.best(&drop_types).is_some());
        // WG6: files dragged in from the OS (no in-app drag at the same time)
        let files = payload.is_none() && accepts(&drop_types, URI_LIST) && ctx.input(|i| !i.raw.hovered_files.is_empty() || !i.raw.dropped_files.is_empty());
        let inside = if files {
            ctx.input(|i| i.pointer.latest_pos()).is_some_and(|p| r.intersect(ui.clip_rect()).contains(p) && ctx.layer_id_at(p).is_none_or(|l| l == ui.layer_id()))
        } else {
            ctx.rect_contains_pointer(ui.layer_id(), r.intersect(ui.clip_rect()))
        };
        if files {
            let dropped: Vec<egui::DroppedFileHandle> = ctx.input(|i| i.raw.dropped_files.clone());
            file_drop(&ctx, id, r, inside, &dropped, |name, args| emit(arena, name, args));
            return;
        }
        if payload.is_some() && inside {
            claim(&ctx, id);
        }
        let active = payload.is_some() && inside && claims(&ctx).prev == Some(id);
        if active {
            ui.painter().rect_stroke(r, 2.0, ui.visuals().selection.stroke, StrokeKind::Inside);
            ctx.request_repaint(); // the claim decides next pass
        }
        if active && !was_in {
            emit(arena, "drag-enter", vec![Value::Str(payload.as_ref().and_then(|p| p.best(&drop_types)).map(|(t, _)| t.to_string()).unwrap_or_default())]);
        }
        let released = active && ctx.input(|inp| inp.pointer.any_released());
        if was_in && (!active || released) {
            emit(arena, "drag-leave", vec![]);
        }
        if released && let Some(p) = DragAndDrop::take_payload::<DragPayload>(&ctx) {
            let action = drop_action(ctx.input(|i| i.modifiers), mac(&ctx));
            p.accept_as(action);
            let at = ctx.pointer_interact_pos().unwrap_or(r.min) - r.min;
            let (ty, data) = p.best(&drop_types).map(|(t, d)| (t.to_string(), d)).unwrap_or_default();
            emit(arena, "drop", vec![Value::Str(ty), Value::Str(data), Value::F64(at.x as f64), Value::F64(at.y as f64), Value::Str(action.into())]);
        }
        let now_in = active && !released;
        if now_in != was_in {
            ctx.data_mut(|m| m.insert_temp(key, now_in));
        }
    }
}

fn mac(ctx: &egui::Context) -> bool {
    ctx.os() == egui::os::OperatingSystem::Mac
}

/// WG6: the `file://` URIs of dropped files' paths, one per line (`text/uri-list`).
pub fn uri_list<'a>(paths: impl IntoIterator<Item = &'a std::path::Path>) -> String {
    paths.into_iter().map(svelters_host::platform::file_uri).collect::<Vec<_>>().join("\r\n")
}

/// WG6: the target half of [`node`] for files dragged in from the OS: `inside`, the pointer is
/// over it; `dropped`, this pass's dropped files.
fn file_drop(ctx: &egui::Context, id: Id, r: Rect, inside: bool, dropped: &[egui::DroppedFileHandle], mut emit: impl FnMut(&str, Vec<Value>)) {
    let key = id.with("dnd-in");
    let was_in: bool = ctx.data(|m| m.get_temp(key)).unwrap_or(false);
    if inside {
        claim(ctx, id);
    }
    let active = inside && claims(ctx).prev == Some(id);
    if active {
        ctx.layer_painter(LayerId::new(Order::Foreground, id.with("dnd-files"))).rect_stroke(r, 2.0, ctx.global_style().visuals.selection.stroke, StrokeKind::Inside);
        ctx.set_cursor_icon(CursorIcon::Copy);
        ctx.request_repaint();
    }
    if active && !was_in {
        emit("drag-enter", vec![Value::Str(URI_LIST.into())]);
    }
    let released = active && !dropped.is_empty();
    if was_in && (!active || released) {
        emit("drag-leave", vec![]);
    }
    if released {
        let at = ctx.input(|i| i.pointer.latest_pos()).unwrap_or(r.min) - r.min;
        emit("drop", vec![Value::Str(URI_LIST.into()), Value::Str(uri_list(dropped.iter().map(|f| f.path()))), Value::F64(at.x as f64), Value::F64(at.y as f64), Value::Str("copy".into())]);
    }
    let now_in = active && !released;
    if now_in != was_in {
        ctx.data_mut(|m| m.insert_temp(key, now_in));
    }
}

/// WG6: how close to a scroll area's edge a drag scrolls it, in points.
pub const EDGE: f32 = 24.0;

/// WG6: scrolls the scroll area showing `ui` (its rect `r`, screen) while a drag is held near its
/// edge: up to 12 points a frame, faster nearer the edge.
pub(crate) fn auto_scroll(ui: &Ui, r: Rect) {
    let ctx = ui.ctx();
    let dragging = DragAndDrop::has_any_payload(ctx) || ctx.input(|i| !i.raw.hovered_files.is_empty());
    if !dragging {
        return;
    }
    let Some(p) = ctx.input(|i| i.pointer.latest_pos()) else { return };
    let d = edge_delta(r, p);
    if d != egui::Vec2::ZERO {
        ui.scroll_with_delta(d);
        ctx.request_repaint();
    }
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

/// The label that follows the pointer during a drag.
fn ghost(ui: &Ui, id: Id, text: &str) {
    let Some(pos) = ui.ctx().pointer_interact_pos() else {
        return;
    };
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
