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
//! moved to another window) keeps every id. A node gets its root's ids when its subtree is inserted
//! under a tree that reaches a slot ([`adopt`], called by the insert op); nodes inside keep theirs
//! across keyed reorders (a move within the root) and across other roots mounting.
//!
//! Writer lane: EMBED.

use crate::scene::{Arena, Handle, Node};
use std::collections::HashMap;

/// The egui id of a node outside every embedded root.
pub(crate) fn node_id(h: Handle) -> egui::Id {
    egui::Id::unique(("svl", h))
}

pub(crate) fn slot_id(salt: &str, key: &str) -> egui::Id {
    egui::Id::unique(("svelters", salt, "slot", key))
}

pub(crate) fn root_base(salt: &str, key: &str) -> egui::Id {
    egui::Id::unique(("svelters", salt, key))
}

/// The id of node `n` in a root whose base is `base`.
pub(crate) fn rooted_id(base: egui::Id, n: &Node) -> egui::Id {
    if n.c.anchor.is_empty() { base.with(("h", n.handle)) } else { base.with(("a", n.c.anchor.as_str())) }
}

/// The id `n` should have now: from its root's base, or the legacy one.
pub(crate) fn id_of(bases: &HashMap<u32, egui::Id>, n: &Node) -> egui::Id {
    match (n.root, n.host_owned) {
        (_, true) => n.id, // slots keep the id the embed gave them
        (0, _) => node_id(n.handle),
        (r, _) => match bases.get(&r) {
            Some(&b) => rooted_id(b, n),
            None => node_id(n.handle),
        },
    }
}

/// `child`'s subtree was inserted under `parent`: it joins `parent`'s root (or none) and takes
/// that root's ids. A subtree already in that root (a reorder) is left alone.
pub(crate) fn adopt(scene: &mut Arena, bases: &HashMap<u32, egui::Id>, parent: Handle, child: Handle) {
    let root = scene.get(parent).map(|p| p.root).unwrap_or(0);
    if scene.get(child).is_none_or(|c| c.root == root) {
        return;
    }
    let mut stack = vec![child];
    while let Some(h) = stack.pop() {
        let Some(n) = scene.get_mut(h) else { continue };
        n.root = root;
        n.id = id_of(bases, n);
        stack.extend(n.children.iter().copied());
    }
}

/// The node's `anchor` changed: a new id (its egui memory under the old one is dropped).
pub(crate) fn reanchor(scene: &mut Arena, bases: &HashMap<u32, egui::Id>, h: Handle) {
    if let Some(n) = scene.get_mut(h) {
        n.id = id_of(bases, n);
    }
}
