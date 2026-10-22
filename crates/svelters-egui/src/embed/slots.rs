//! The slot table: Rust-created free-standing arena roots, drawn wherever the host calls
//! `show_slot` (ARCH-EMBED §0.1, §4.1, §5.3). Visibility and viewport come from the host
//! (D7(a)); what the last draw took is recorded for `SlotResponse`, root info and anchors.
//! Writer lane: EMBED.

use super::roots::{RootError, RootId, RootState};
use crate::scene::Handle;
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
    pub visible: bool,
    pub viewport: egui::ViewportId,
    /// The last rect `show_slot` gave it (screen coordinates) and its content size.
    pub rect: Option<egui::Rect>,
    pub content: egui::Vec2,
}
