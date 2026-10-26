//! Input (architecture §11): hit testing on the last frame's hit list, native defaults, DOM-style
//! capture/target/bubble propagation to JS listeners, pointer capture, hover, and the focus model.
//!
//! | File | Content |
//! |---|---|
//! | `mod.rs` | [`InputState`] (hover, capture, press), [`Dispatched`] and its events |
//! | `dispatch.rs` | [`Document::dispatch`](crate::Document::dispatch): routing per event type |
//! | `focus.rs` | focus moves, tab order, `contain`/`dialog` scopes, `:focus-visible` |
//! | `scope.rs` | Enter/Escape to a scope's default and cancel buttons, `:default` |
//! | `peers.rs` | peer groups: one Tab stop, peer requests, roving focus |
//! | [`dnd`] | in-app drag and drop (`drag-type`, `drop-types`) |

pub(crate) mod dispatch;
pub mod dnd;
pub(crate) mod focus;
pub(crate) mod peers;
pub(crate) mod scope;

use crate::ids::SignalId;
use crate::tree::ListenFlags;
use crate::value::Value;
use skelter_base::cursor::CursorIcon;
use skelter_base::geom::Point;
use skelter_base::ids::{InputSeq, NodeId, Revision, WindowId};
use skelter_base::time::Time;
use smallvec::SmallVec;
use std::collections::HashMap;

/// Where in propagation a listener is called.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Capture,
    Target,
    Bubble,
}

/// One listener call of a JS-facing event, in delivery order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Delivery {
    pub node: NodeId,
    pub phase: Phase,
    pub flags: ListenFlags,
}

/// An event for JS: svelters-skelter calls `path` in order, stopping where a listener stops
/// propagation. Events with no listener on their path are not listed.
#[derive(Clone, Debug, PartialEq)]
pub struct JsEvent {
    pub signal: SignalId,
    pub target: NodeId,
    pub arg: Option<Value>,
    pub path: SmallVec<[Delivery; 4]>,
    /// A listener may call `preventDefault` (some listener on the path is not passive).
    pub cancelable: bool,
    /// Deliver at event time (dispatch and drain), not in the next frame's input flush.
    pub sync: bool,
}

/// What [`Document::dispatch`](crate::Document::dispatch) did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Dispatched {
    pub seq: InputSeq,
    pub window: WindowId,
    /// The hit target (pointer events) or the focused node (keys, IME).
    pub target: Option<NodeId>,
    /// A native default handled the event.
    pub handled: bool,
    /// The JS-facing events, in delivery order.
    pub events: Vec<JsEvent>,
    /// The revision the events were dispatched at (JS writes from their listeners pass it to
    /// [`Document::js_transaction`](crate::Document::js_transaction)).
    pub revision: Revision,
    /// The tree changed: the caller requests an urgent frame (§10).
    pub changed: bool,
    /// Printable keys went to the IME first (§11.2): keybindings must not match this key.
    pub to_ime: bool,
}

/// Pointer and hover state across events.
#[derive(Default)]
pub(crate) struct InputState {
    /// Per pointer: the hovered path, target first.
    pub hover: HashMap<u32, Vec<NodeId>>,
    /// Per pointer: the node holding the pointer until release.
    pub capture: HashMap<u32, NodeId>,
    /// Per pointer: the node the press started on (click = release on the same node).
    pub press: HashMap<u32, NodeId>,
    /// The last click, for double-click counting: (node, time, position, count).
    pub last_click: Option<(NodeId, Time, Point, u32)>,
    pub cursor: Option<CursorIcon>,
    /// The last pointer position per window (for re-hovering after a frame).
    pub last_pos: HashMap<WindowId, Point>,
    /// An IME composition is active (keys go to the IME first).
    pub composing: bool,
    /// Windows with an input method source active (printable keys go to the IME first).
    pub ime_source: std::collections::HashSet<WindowId>,
    /// The time of the event being handled (`EventCx::time`).
    pub now: Time,
    /// A press on a drag source that may become a drag, and the drag in progress.
    pub drag_press: Option<dnd::DragPress>,
    pub drag: Option<dnd::Drag>,
    /// Counts drag previews set (a drag's [`dnd::Drag::generation`]: the drag layer re-records on a new one).
    pub drag_generation: u64,
}

impl InputState {
    pub fn capture(&mut self, pointer: u32, node: NodeId) {
        self.capture.insert(pointer, node);
    }
    pub fn release(&mut self, pointer: u32) {
        self.capture.remove(&pointer);
    }
    /// A node was destroyed.
    pub fn forget(&mut self, node: NodeId) {
        self.leave_subtree(&[node]);
    }
    /// Nodes left their window: drop captures, presses and hover entries on them.
    pub fn leave_subtree(&mut self, nodes: &[NodeId]) {
        if nodes.is_empty() {
            return;
        }
        self.capture.retain(|_, n| !nodes.contains(n));
        self.press.retain(|_, n| !nodes.contains(n));
        for path in self.hover.values_mut() {
            path.retain(|n| !nodes.contains(n));
        }
        if self.last_click.is_some_and(|(n, ..)| nodes.contains(&n)) {
            self.last_click = None;
        }
        if self.drag_press.as_ref().is_some_and(|d| nodes.contains(&d.source)) {
            self.drag_press = None;
        }
        if let Some(d) = self.drag.as_mut() {
            if d.target.is_some_and(|t| nodes.contains(&t)) {
                d.target = None;
            }
            if nodes.contains(&d.source) {
                self.drag = None;
            }
        }
    }
}
