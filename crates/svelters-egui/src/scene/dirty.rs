//! Layout invalidation: [`invalidate`] (clears the layout cache up to the window root) and
//! [`DirtyFlags`], today's backend-global layout flags.
//!
//! SG1 adds dirty bits, layout boundaries (invalidation stops at the nearest one) and per-root
//! `RootDirty` queues that replace these global flags (ARCH-SCENE §7.1, §12). Writer lane: SCENE.

use super::{Arena, Handle};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;

/// Clears the layout cache of `h` and its ancestors (a style or structure change below them).
pub fn invalidate(arena: &mut Arena, mut h: Handle) {
    while let Some(n) = arena.get_mut(h) {
        n.cache.clear();
        match n.parent {
            Some(p) => h = p,
            None => break,
        }
    }
}

/// The layout flags the backend keeps (one set for all windows).
pub(crate) struct DirtyFlags {
    /// Leaves whose content changed since the last layout.
    pub remeasure: RefCell<Vec<Handle>>,
    /// Some window's layout must run again (style or structure changed).
    pub layout_dirty: Cell<bool>,
    /// Content size of each window's last layout.
    pub layout_size: RefCell<HashMap<Handle, egui::Vec2>>,
}

impl DirtyFlags {
    /// Dirty: nothing has been laid out yet.
    pub fn new() -> DirtyFlags {
        DirtyFlags { remeasure: RefCell::new(Vec::new()), layout_dirty: Cell::new(true), layout_size: RefCell::new(Default::default()) }
    }
}
