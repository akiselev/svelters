//! egui ids of scene nodes (ARCH-EMBED §4.4).
//!
//! | Node | `egui::Id` |
//! |---|---|
//! | slot | `Id::unique(("svelters", salt, "slot", slot_key))` |
//! | root base | `Id::unique(("svelters", salt, root_key))` |
//! | node of a root with `anchor="…"` | `root_base.with(("a", anchor))` |
//! | other node of a root | `root_base.with(("h", handle))` |
//! | node outside any root (desktop windows, detached) | `Id::unique(("svl", handle))`, as before EM1 |
//!
//! Never the slot, the pane, the viewport or `ui.id()`: drawing a slot from another `Ui` (a pane
//! moved to another window) keeps every id. A node takes its root's id when it enters a slot's
//! scene root (SG1's attach hook `Scene::attached`, from every insert); an `anchor` change re-derives
//! it (`Scene::set_value`). The derivation is deterministic, so nodes keep their ids across keyed
//! reorders (a move within the root) and across other roots mounting.
//!
//! Writer lane: EMBED.

/// The egui id of a node outside every embedded root.
pub(crate) fn node_id(h: u64) -> egui::Id {
    egui::Id::unique(("svl", h))
}

pub(crate) fn slot_id(salt: &str, key: &str) -> egui::Id {
    egui::Id::unique(("svelters", salt, "slot", key))
}

pub(crate) fn root_base(salt: &str, key: &str) -> egui::Id {
    egui::Id::unique(("svelters", salt, key))
}

/// The id of node `h` (with `anchor`, empty if none) in a root whose base is `base`.
pub(crate) fn rooted_id(base: egui::Id, h: u64, anchor: &str) -> egui::Id {
    if anchor.is_empty() {
        base.with(("h", h))
    } else {
        base.with(("a", anchor))
    }
}
