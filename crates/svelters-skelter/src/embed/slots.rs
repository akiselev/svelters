//! The slot table: Rust-created, host-owned `box` nodes with no parent, which JS mounts roots into
//! (ARCH-EMBED §0.1, §4.1). Where a slot shows (its window, size and scale) and whether it is visible
//! come from the host (D7(a)): skelter's layout places the slot node, and the app tells the embed.

use super::roots::RootId;
use skelter_base::ids::WindowId;
use svelters_abi::svl_handle;

/// A slot: its node's handle (the mount target JS sees). Handles are never reused, so a destroyed
/// slot's id never names another.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SlotId(pub(crate) svl_handle);

impl SlotId {
    pub fn handle(self) -> svl_handle {
        self.0
    }
}

#[derive(Clone, Debug, Default)]
pub struct SlotOptions {
    /// AccessKit name of the slot's group node.
    pub label: Option<String>,
}

impl SlotOptions {
    pub fn label(mut self, label: impl Into<String>) -> SlotOptions {
        self.label = Some(label.into());
        self
    }
}

pub(crate) struct Slot {
    pub key: String,
    pub root: Option<RootId>,
    pub visible: bool,
    pub window: Option<WindowId>,
    /// The size skelter's layout last gave it (`None`: not laid out yet).
    pub size: Option<[f32; 2]>,
    pub scale: f32,
    /// Keyboard focus is inside the slot.
    pub focused: bool,
}

impl Slot {
    pub fn new(key: String) -> Slot {
        Slot { key, root: None, visible: true, window: None, size: None, scale: 1.0, focused: false }
    }
}
