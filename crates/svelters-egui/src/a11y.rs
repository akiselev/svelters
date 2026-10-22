//! Accessibility: what AccessKit gets for a node (ARCH-EMBED §4.4, SV16 minimum, SVP3).
//!
//! - The accessible name of a leaf: `accessible-name`, else a text field's hint.
//! - The author id of an anchored node: its `anchor` (unique in its root, as AccessKit wants among
//!   siblings), and the slot's key for a slot (`slot_group`).
//!
//! Writer lane: EMBED.

use crate::scene::{Node, W};

/// The accessible name a leaf's response gets: `accessible-name`, else a text field's hint.
pub(crate) fn name(n: &Node) -> Option<String> {
    if !n.c.accessible_name.is_empty() {
        Some(n.c.accessible_name.clone())
    } else if let W::TextEdit { hint, .. } = &n.w
        && !hint.is_empty()
    {
        Some(hint.clone())
    } else {
        None
    }
}

/// Everything AccessKit gets for leaf `n` on top of the widget's own node: its name and, when it
/// has an anchor, the anchor as author id.
pub(crate) fn decorate(resp: egui::Response, n: &Node) -> egui::Response {
    let resp = match name(n) {
        Some(name) => resp.accessible_name(name),
        None => resp,
    };
    if !n.c.anchor.is_empty() {
        resp.ctx
            .accesskit_node_builder(resp.id, |b| b.set_author_id(n.c.anchor.clone()));
    }
    resp
}

/// The slot's `Ui` node: a group named by the slot's label, with the slot key as author id.
pub(crate) fn slot_group(ctx: &egui::Context, id: egui::Id, key: &str) {
    ctx.accesskit_node_builder(id, |b| b.set_author_id(key.to_string()));
}
