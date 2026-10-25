//! egui ids of scene nodes (ARCH-EMBED §4.4).
//!
//! | Node | `egui::Id` |
//! |---|---|
//! | slot | `Id::unique(("svelters", salt, "slot", slot_key))` |
//! | root base | `Id::unique(("svelters", salt, root_key))` |
//! | node of a root with `anchor="…"` | `root_base.with(("a", anchor))` |
//! | the same inside a keyed `{#each}` (SV53: an `anchor-scope` on it or an ancestor) | `root_base.with(("a", anchor, scope path))` |
//! | other node of a root | `root_base.with(("h", handle))` |
//! | node outside any root (desktop windows, detached) | `Id::unique(("svl", handle))`, as before EM1 |
//!
//! Never the slot, the pane, the viewport or `ui.id()`: drawing a slot from another `Ui` (a pane
//! moved to another window) keeps every id. A node takes its root's id when it enters a slot's
//! scene root (SG1's attach hook `Scene::attached`, from every insert); an `anchor` change re-derives
//! it (`Scene::set_value`). The derivation is deterministic, so nodes keep their ids across keyed
//! reorders (a move within the root) and across other roots mounting.
//!
//! **SV53.** An anchor repeated in the rows of a keyed `{#each}` would give every row one id. The
//! runtime sets `anchor-scope` (the item's keys) on each item's top nodes; an anchored node with a
//! scope on itself or an ancestor takes its id from its anchor and the scope path: from the root
//! down, each scope and the anchor of each anchored ancestor. Keys follow the item, so the id
//! survives keyed reorders; an anchor outside every keyed each keeps the plain derivation.
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
    if anchor.is_empty() { base.with(("h", h)) } else { base.with(("a", anchor)) }
}

/// SV53: the id of an anchored node under keyed each scopes: `path` is the scope path from the
/// root down (`("s", keys)` per scope, `("a", anchor)` per anchored ancestor).
pub(crate) fn scoped_id(base: egui::Id, anchor: &str, path: &[(&str, &str)]) -> egui::Id {
    base.with(("a", anchor, path))
}
