//! Islands as presto-core nodes: the `egui-island` kind ([`EguiIslandKind`]) and the per-document table of
//! mounted islands ([`IslandHost`], a document service).
//!
//! A node of kind `egui-island` is an empty, focusable leaf until an [`EguiIsland`] is mounted on it
//! ([`IslandHost::mount`]). The kind then:
//! - records the island's texture over its border box (`record`);
//! - forwards its native input to the island (`event`: pointer with capture, wheel, keys, IME, focus,
//!   AccessKit actions), so egui sees the production dispatch path, hit testing and focus included;
//! - marks itself as the graft point of the island's AccessKit subtree (`a11y`), whose tree id follows the
//!   node ([`island_tree_id`] of its id).
//!
//! Detaching ([`IslandHost::detach`]) hands the island back with its egui state (memory, scroll offsets,
//! open windows), so a pane moving between dock slots or windows keeps it: mount it on the new node.
//!
//! The app runner drives the table each frame, outside the document's phases (an egui pass runs the host's
//! own UI code, which borrows app state the document can't hold):
//! 1. [`IslandHost::run`] before `Document::frame`: due passes, with the host's `ui` callback per node
//!    (cadatomic's `PaneHost::ui` for the pane mounted there). A pass re-records its node, so the frame's
//!    commit re-composites the new texture content.
//! 2. [`IslandHost::render`] before the commit is sent: renders the passes into the islands' textures and
//!    releases the islands of destroyed nodes.
//! 3. [`IslandHost::bridge_events`] with each `Dispatched`'s events: presto-core's in-app drags over an
//!    island reach egui's `DragAndDrop`.
//!
//! [`IslandHost::next_wakeup`] joins `Document::next_timer` in the runner's wait; on `ToUi::DeviceReset` the
//! runner calls [`IslandHost::device_reset`] next to `Document::device_reset`.

use crate::a11y::island_tree_id;
use crate::dnd::DragData;
use crate::island::{EguiIsland, IslandOutput, TextureRegistry};
use presto_base::change::ChangeClass;
use presto_base::display::{Prim, SurfacePrim};
use presto_base::event::{InputEvent, PointerEvent, PointerPhase, WheelEvent};
use presto_base::geom::{Point, Rect, Scale, Vec2};
use presto_base::ids::NodeId;
use presto_base::time::Time;
use presto_core::{
    A11yCx, Document, Event, EventCx, FocusReason, Handled, JsEvent, Kind, KindId, KindRegistry, KindSpec, ListenFlags,
    PointerInput, PropDefault, PropId, PropSpec, RecordCx, RegistryError, Value, ValueType, common,
};
use std::collections::HashMap;

pub const EGUI_ISLAND: KindId = KindId::of("egui-island");
/// An island is mounted (the node is a graft point).
pub const MOUNTED: PropId = PropId::of("egui-island", "mounted", "bool");
/// Counts the island's passes; each write re-records the node so its new texture content is composited.
pub const PASS: PropId = PropId::of("egui-island", "pass", "int");

pub static SPEC: KindSpec = KindSpec {
    props: &[
        PropSpec::new("mounted", ValueType::Bool, ChangeClass::Structure)
            .default(PropDefault::Bool(false))
            .doc("Set by the island host: an egui island is mounted on the node."),
        PropSpec::new("pass", ValueType::I64, ChangeClass::Paint)
            .default(PropDefault::I64(0))
            .doc("Set by the island host after each egui pass."),
    ],
    role: accesskit::Role::Pane,
    focusable: true,
    doc: "An egui 0.37 island (migration): a Rust egui UI mounted by the host app, composited as a texture.",
    ..KindSpec::new("egui-island")
};

/// The `egui-island` kind. Its state lives in the document's [`IslandHost`].
#[derive(Default)]
pub struct EguiIslandKind;

impl Kind for EguiIslandKind {
    fn spec() -> &'static KindSpec {
        &SPEC
    }

    fn record(&self, cx: &mut RecordCx) {
        let node = cx.node.id();
        let rect = Rect::from_origin_size(Point::ZERO, cx.node.size());
        let Some(texture) = cx.service::<IslandHost>().and_then(|h| h.islands.get(&node)).map(|i| i.texture_id()) else {
            return;
        };
        cx.push(Prim::Surface(SurfacePrim { texture, rect, opacity: 1.0 }), rect);
    }

    fn event(&mut self, cx: &mut EventCx, ev: &Event) -> Handled {
        let node = cx.node();
        let at = cx.time();
        let input = match ev {
            Event::PointerDown(p) => {
                cx.capture_pointer();
                cx.focus(FocusReason::Pointer);
                pointer(PointerPhase::Down, p)
            }
            Event::PointerUp(p) => {
                cx.release_pointer();
                pointer(PointerPhase::Up, p)
            }
            Event::PointerMove(p) => pointer(PointerPhase::Move, p),
            Event::HoverLeave => leave(PointerPhase::Leave),
            Event::PointerCancel => leave(PointerPhase::Cancel),
            Event::Wheel(w) => InputEvent::Wheel(WheelEvent {
                pos: w.pos,
                delta: w.delta,
                phase: None,
                momentum: None,
                modifiers: w.modifiers,
            }),
            Event::Key(k) => InputEvent::Key((*k).clone()),
            Event::Ime(i) => InputEvent::Ime((*i).clone()),
            Event::FocusIn(_) | Event::FocusOut => {
                let focused = matches!(ev, Event::FocusIn(_));
                if let Some(island) = cx.service::<IslandHost>().and_then(|h| h.islands.get_mut(&node)) {
                    island.set_focused(focused);
                }
                return Handled::No;
            }
            Event::A11y(req) => {
                let taken = cx
                    .service::<IslandHost>()
                    .and_then(|h| h.islands.get_mut(&node))
                    .is_some_and(|i| i.handle_a11y_action((*req).clone()));
                return if taken { Handled::Yes } else { Handled::No };
            }
            Event::Click(_) | Event::HoverEnter | Event::Timer => return Handled::No,
        };
        let Some(island) = cx.service::<IslandHost>().and_then(|h| h.islands.get_mut(&node)) else {
            return Handled::No;
        };
        let consumed = island.handle_event(&input, at);
        let cursor = island.cursor();
        if matches!(input, InputEvent::Pointer(_)) {
            cx.set_cursor(cursor);
        }
        if consumed { Handled::Yes } else { Handled::No }
    }

    fn a11y(&self, cx: &mut A11yCx) {
        cx.default_a11y();
        if cx.node.prop_bool(MOUNTED) {
            cx.out.set_tree_id(island_tree_id(cx.node.id().0));
        }
    }
}

fn pointer(phase: PointerPhase, p: &PointerInput) -> InputEvent {
    InputEvent::Pointer(PointerEvent {
        phase,
        pointer: p.pointer,
        kind: p.kind,
        button: p.button,
        buttons: p.buttons,
        pos: p.pos,
        modifiers: p.modifiers,
        pressure: p.pressure,
        tilt: Vec2::ZERO,
        history: Default::default(),
    })
}

/// A leave or cancel (the translator ignores their position).
fn leave(phase: PointerPhase) -> InputEvent {
    InputEvent::Pointer(PointerEvent {
        phase,
        pointer: 0,
        kind: presto_base::event::PointerType::Mouse,
        button: None,
        buttons: 0,
        pos: Point::ZERO,
        modifiers: Default::default(),
        pressure: 0.0,
        tilt: Vec2::ZERO,
        history: Default::default(),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MountError {
    /// The node doesn't exist or isn't of kind `egui-island`.
    NotAnIsland(NodeId),
    /// Another island is mounted there.
    Occupied(NodeId),
}

impl std::fmt::Display for MountError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MountError::NotAnIsland(n) => write!(f, "{n:?} is not an egui-island node"),
            MountError::Occupied(n) => write!(f, "{n:?} already has an island mounted"),
        }
    }
}

impl std::error::Error for MountError {}

/// A refused mount hands the island back.
pub struct MountRejected {
    pub reason: MountError,
    pub island: EguiIsland,
}

impl std::fmt::Debug for MountRejected {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MountRejected").field("reason", &self.reason).finish_non_exhaustive()
    }
}

/// The document's mounted islands, by node (a document service).
#[derive(Default)]
pub struct IslandHost {
    islands: HashMap<NodeId, EguiIsland>,
    /// The island a presto drag is over.
    drag_over: Option<NodeId>,
}

/// The signals a mounted island listens to for the drag-and-drop bridge.
const DND_SIGNALS: [presto_core::SignalId; 3] = [common::DRAG_ENTER, common::DRAG_LEAVE, common::DROP];

impl IslandHost {
    /// Registers the `egui-island` kind.
    pub fn register(registry: &mut KindRegistry) -> Result<KindId, RegistryError> {
        registry.register::<EguiIslandKind>()
    }

    fn get(doc: &mut Document) -> &mut IslandHost {
        if doc.service::<IslandHost>().is_none() {
            doc.set_service(IslandHost::default());
        }
        doc.service::<IslandHost>().expect("installed above")
    }

    /// Mounts `island` on `node` (an `egui-island` node): its AccessKit subtree moves to the node's tree
    /// id, accessibility turns on, and the node starts recording and routing input to it.
    pub fn mount(doc: &mut Document, node: NodeId, mut island: EguiIsland) -> Result<(), MountRejected> {
        if doc.kind_of(node) != Some(EGUI_ISLAND) {
            return Err(MountRejected { reason: MountError::NotAnIsland(node), island });
        }
        if Self::get(doc).islands.contains_key(&node) {
            return Err(MountRejected { reason: MountError::Occupied(node), island });
        }
        island.set_tree_id(island_tree_id(node.0));
        island.enable_accesskit();
        if let Some((rect, scale)) = placement(doc, node) {
            island.set_rect(rect, scale);
        }
        Self::get(doc).islands.insert(node, island);
        for s in DND_SIGNALS {
            doc.listen(node, s, ListenFlags::PASSIVE);
        }
        doc.set_prop(node, MOUNTED, Value::Bool(true)).expect("the spec's own prop");
        Ok(())
    }

    /// Takes the island off `node`, egui state and texture intact (mount it elsewhere, or
    /// [`EguiIsland::release`] it).
    pub fn detach(doc: &mut Document, node: NodeId) -> Option<EguiIsland> {
        let host = Self::get(doc);
        let mut island = host.islands.remove(&node)?;
        if host.drag_over == Some(node) {
            host.drag_over = None;
            island.drag_leave();
        }
        island.set_focused(false);
        if doc.contains(node) {
            for s in DND_SIGNALS {
                doc.unlisten(node, s, false);
            }
            doc.set_prop(node, MOUNTED, Value::Bool(false)).expect("the spec's own prop");
        }
        Some(island)
    }

    pub fn island_mut(doc: &mut Document, node: NodeId) -> Option<&mut EguiIsland> {
        doc.service::<IslandHost>()?.islands.get_mut(&node)
    }

    /// The nodes with an island mounted.
    pub fn mounted(doc: &mut Document) -> Vec<NodeId> {
        doc.service::<IslandHost>().map_or_else(Vec::new, |h| h.islands.keys().copied().collect())
    }

    /// Runs the passes due at `now`, `ui(node, ui)` drawing each node's island, after moving every island to
    /// its node's last layout. Returns each pass's output (cursor, IME area, the AccessKit subtree update to
    /// forward to the platform adapter after the commit that grafts it, copied text, links, drags out).
    pub fn run(doc: &mut Document, now: Time, mut ui: impl FnMut(NodeId, &mut egui::Ui)) -> Vec<(NodeId, IslandOutput)> {
        let nodes = Self::mounted(doc);
        let places: Vec<(NodeId, Option<(Rect, Scale)>)> = nodes.iter().map(|&n| (n, placement(doc, n))).collect();
        let host = Self::get(doc);
        let mut outputs = Vec::new();
        for (node, place) in places {
            let Some(island) = host.islands.get_mut(&node) else { continue };
            let Some((rect, scale)) = place else { continue };
            island.set_rect(rect, scale);
            if island.needs_pass(now) {
                outputs.push((node, island.run(now, |u| ui(node, u))));
            }
        }
        for (node, _) in &outputs {
            let pass = match doc.get_prop(*node, PASS) {
                Some(Value::I64(n)) => n,
                _ => 0,
            };
            doc.set_prop(*node, PASS, Value::I64(pass.wrapping_add(1))).expect("the spec's own prop");
        }
        outputs
    }

    /// Renders the passes [`run`](Self::run) produced into the islands' textures (submit the returned paint
    /// callback buffers before `encoder`'s), and releases the islands of destroyed nodes.
    pub fn render(
        doc: &mut Document,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        registry: &dyn TextureRegistry,
    ) -> Vec<wgpu::CommandBuffer> {
        let dead: Vec<NodeId> = Self::mounted(doc).into_iter().filter(|&n| !doc.contains(n)).collect();
        let host = Self::get(doc);
        for node in dead {
            if let Some(mut island) = host.islands.remove(&node) {
                island.release(registry);
            }
            if host.drag_over == Some(node) {
                host.drag_over = None;
            }
        }
        let mut buffers = Vec::new();
        for island in host.islands.values_mut() {
            buffers.extend(island.render(device, queue, encoder, registry));
        }
        buffers
    }

    /// The GPU device was lost and recreated (`ToUi::DeviceReset`; call it with `Document::device_reset`):
    /// every island re-renders on the new device at its next pass ([`EguiIsland::device_reset`]).
    pub fn device_reset(doc: &mut Document) {
        if let Some(host) = doc.service::<IslandHost>() {
            for island in host.islands.values_mut() {
                island.device_reset();
            }
        }
    }

    /// The earliest time an island needs a pass without input.
    pub fn next_wakeup(doc: &mut Document) -> Option<Time> {
        doc.service::<IslandHost>()?.islands.values().filter_map(EguiIsland::next_wakeup).min()
    }

    /// presto-core's in-app drag-and-drop events (`drag-enter`, `drag-leave`, `drop` on island nodes, from
    /// `Dispatched::events`) into the islands' `egui::DragAndDrop`. A drop carries the data and the point, so
    /// egui drop zones under it take the payload in the island's next pass.
    pub fn bridge_events(doc: &mut Document, events: &[JsEvent]) {
        for e in events {
            if !doc.service::<IslandHost>().is_some_and(|h| h.islands.contains_key(&e.target)) {
                continue;
            }
            let origin = doc.world_rect(e.target).map_or(Point::ZERO, |r| r.min);
            let host = Self::get(doc);
            let Some(island) = host.islands.get_mut(&e.target) else { continue };
            if e.signal == common::DRAG_ENTER {
                let Some(Value::Str(ty)) = &e.arg else { continue };
                let r = island.rect();
                let center = Point::new((r.min.x + r.max.x) / 2.0, (r.min.y + r.max.y) / 2.0);
                island.drag_enter(&DragData::new(ty.clone(), ""), center);
                host.drag_over = Some(e.target);
            } else if e.signal == common::DRAG_LEAVE {
                island.drag_leave();
                if host.drag_over == Some(e.target) {
                    host.drag_over = None;
                }
            } else if e.signal == common::DROP {
                let Some(Value::Record(fields)) = &e.arg else { continue };
                let field = |name: &str| fields.iter().find(|(k, _)| &**k == name).map(|(_, v)| v);
                let (Some(Value::Str(ty)), Some(Value::Str(data))) = (field("type"), field("data")) else { continue };
                let num = |name: &str| match field(name) {
                    Some(Value::F64(v)) => *v as f32,
                    _ => 0.0,
                };
                let at = Point::new(origin.x + num("x"), origin.y + num("y"));
                island.drag_enter(&DragData::new(ty.clone(), data.clone()), at);
                island.drag_drop(at);
            }
        }
    }

    /// The pointer moved (window points) during a presto drag: the island it is over follows it, so egui
    /// drop zones highlight under the pointer.
    pub fn drag_moved(doc: &mut Document, pos: Point) {
        let Some(host) = doc.service::<IslandHost>() else { return };
        if let Some(island) = host.drag_over.and_then(|n| host.islands.get_mut(&n)) {
            island.drag_move(pos);
        }
    }
}

/// A node's window-space border box from the last frame, and its window's scale.
fn placement(doc: &Document, node: NodeId) -> Option<(Rect, Scale)> {
    let rect = doc.world_rect(node)?;
    let scale = doc.view(node)?.scale();
    Some((rect, Scale(scale)))
}
