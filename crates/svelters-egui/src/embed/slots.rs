//! The slot table: Rust-created free-standing arena roots, drawn wherever the host calls
//! `show_slot` (ARCH-EMBED §0.1, §4.1, §5.3). Visibility and viewport come from the host
//! (D7(a)); what the last draw took is recorded for `SlotResponse`, root info and anchors.
//! Writer lane: EMBED.

use super::roots::{RootError, RootId, RootState};
use crate::scene::{Handle, RootIx};
use std::rc::Rc;

/// A slot: a node of the embed's arena with no parent, created by Rust. Its handle is
/// generational, so a destroyed slot's id never names another.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SlotId(pub(crate) Handle);

impl SlotId {
    /// The slot node's handle (the mount target JS sees).
    pub fn handle(self) -> Handle {
        self.0
    }
}

/// How a slot is sized by the `Ui` it is drawn in.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum SlotSizing {
    /// Takes the whole available rect (a pane body).
    #[default]
    Fill,
}

/// What a faulted slot draws.
#[derive(Clone, Default)]
pub enum FaultUi {
    /// The root's key and the error message, and a "Reload" button (restarts the root).
    #[default]
    Default,
    /// The host's own; returning true restarts the root.
    Custom(Rc<dyn Fn(&mut egui::Ui, &RootError) -> bool>),
}

#[derive(Clone, Default)]
pub struct SlotOptions {
    pub sizing: SlotSizing,
    /// AccessKit name of the slot's group node.
    pub label: Option<String>,
    pub fault_ui: FaultUi,
}

impl SlotOptions {
    /// A pane body: fills the rect it is given.
    pub fn fill() -> SlotOptions {
        SlotOptions::default()
    }
    pub fn label(mut self, label: impl Into<String>) -> SlotOptions {
        self.label = Some(label.into());
        self
    }
}

/// What a `show_slot` call took.
#[derive(Clone, Debug)]
pub struct SlotResponse {
    pub rect: egui::Rect,
    /// taffy's content size of the slot's subtree.
    pub content_size: egui::Vec2,
    pub state: RootState,
}

pub(crate) struct Slot {
    pub key: String,
    pub opts: SlotOptions,
    pub root: Option<RootId>,
    /// Its scene root (`TreeRoot::Slot`), the key of the backend's repaint routing.
    pub rix: RootIx,
    pub visible: bool,
    pub viewport: egui::ViewportId,
    /// The last rect `show_slot` gave it (screen coordinates) and its content size.
    pub rect: Option<egui::Rect>,
    pub content: egui::Vec2,
    /// `pixels_per_point` of the viewport it was last drawn in (`root().scale`).
    pub scale: f32,
    /// EM2 focus restore (focus is per viewport in egui): the node id inside this slot that had
    /// keyboard focus when it was last drawn, the last focused id looked at (inside or not), and
    /// the id to focus at its next draw (after `set_viewport` or `move_root`).
    pub focus: Option<egui::Id>,
    pub focus_seen: Option<egui::Id>,
    pub pending_focus: Option<egui::Id>,
    /// Debug checks (§5.3): drawn in the current pass, passes of its viewport without a draw, and
    /// the warnings already given (each once until the slot behaves again).
    pub drawn: bool,
    pub undrawn: u32,
    pub warned: u8,
}

impl Slot {
    pub fn new(key: String, opts: SlotOptions, rix: RootIx, scale: f32) -> Slot {
        Slot {
            key,
            opts,
            root: None,
            rix,
            visible: true,
            viewport: egui::ViewportId::ROOT,
            rect: None,
            content: egui::Vec2::ZERO,
            scale,
            focus: None,
            focus_seen: None,
            pending_focus: None,
            drawn: false,
            undrawn: 0,
            warned: 0,
        }
    }
}

/// What the debug checks found (ARCH-EMBED §5.3), from [`super::Embed::take_warnings`]. Each is
/// reported once until the slot behaves again; debug builds also print it (with `echo_errors`).
#[derive(Clone, Debug, PartialEq)]
pub enum SlotWarning {
    /// `show_slot` was called for a slot set hidden (`set_visible(false)`): nothing was drawn.
    DrawnWhileHidden { slot: SlotId, key: String },
    /// A visible slot with a root was not drawn for this many passes of its viewport (the host
    /// forgot `set_visible(false)`, or draws it in another viewport).
    NotDrawn { slot: SlotId, key: String, passes: u32 },
    /// Drawn in a viewport other than the one set (`set_viewport`): repaints go to the wrong window.
    WrongViewport { slot: SlotId, key: String, set: egui::ViewportId, drawn: egui::ViewportId },
}

pub(crate) const WARN_HIDDEN: u8 = 1;
pub(crate) const WARN_UNDRAWN: u8 = 2;
pub(crate) const WARN_VIEWPORT: u8 = 4;
/// Passes of its viewport a visible slot may go undrawn before `NotDrawn`.
pub(crate) const UNDRAWN_PASSES: u32 = 60;
