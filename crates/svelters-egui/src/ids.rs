//! egui ids of scene nodes. Today: `Id::unique(("svl", handle))` at creation, which survives keyed
//! reorders but not reloads, and clashes across two backends in one `Context`.
//!
//! EM1 replaces it with the salted derivation at attach (on SG1's attach hook), author ids and
//! anchors (ARCH-EMBED §4.4). Writer lane: EMBED.

use crate::scene::Handle;

/// The egui id of the node with handle `h`.
pub(crate) fn node_id(h: Handle) -> egui::Id {
    egui::Id::unique(("svl", h))
}
