//! Accessibility: what AccessKit gets for a node (ARCH-EMBED §4.4, SV16 minimum, SVP3).
//!
//! - The accessible name of a leaf: `accessible-name`, else a text field's hint, a slider's caption
//!   or an icon's tooltip or name, else the tooltip of a node that shows no text of its own
//!   (egui_kittest 0.37 fails an input without a name; WG1).
//! - `invalid` (WG1): AccessKit `invalid`.
//! - The author id of an anchored node: its `anchor` (unique in its root, as AccessKit wants among
//!   siblings), and the slot's key for a slot (`slot_group`).
//!
//! Writer lane: EMBED.

use crate::scene::{Common, Node, NodeRef, W};

/// The accessible name a leaf's response gets (`None`: the widget's own, from its text).
pub(crate) fn name(n: &Node) -> Option<String> {
    name_of(&n.c, &n.w, n.text())
}

/// AU1: a node's accessible name as AccessKit gets it: [`name`], else the node's own text. The
/// automation selectors (`role=…[name=…]`, `label=`) read it, so tests and screen readers agree.
pub fn accessible_name(n: NodeRef<'_>) -> String {
    name_of(n.common(), n.builtin(), n.text()).unwrap_or_else(|| n.text().to_string())
}

fn name_of(c: &Common, w: &W, text: &str) -> Option<String> {
    let some = |s: &str| (!s.is_empty()).then(|| s.to_string());
    if !c.accessible_name.is_empty() {
        return Some(c.accessible_name.clone());
    }
    let own = match w {
        W::TextEdit { hint, .. } => some(hint),
        W::Slider { text, .. } => some(text),
        W::Icon { name, .. } => some(&c.tooltip).or_else(|| some(name)),
        _ => None,
    };
    // a node without text of its own (a combo box, a frameless icon button, a provider kind) is
    // named by its tooltip, as a `title` names an unlabelled HTML control
    // (a provider kind shows no text of its own: OW0's own widgets that replace a built-in do)
    own.or_else(|| if text.is_empty() || matches!(w, W::ComboBox { .. }) { some(&c.tooltip) } else { None })
}

/// Everything AccessKit gets for leaf `n` on top of the widget's own node: its name and, when it
/// has an anchor, the anchor as author id.
pub(crate) fn decorate(resp: egui::Response, n: &Node) -> egui::Response {
    let resp = match name(n) {
        Some(name) => resp.accessible_name(name),
        None => resp,
    };
    if !n.c.anchor.is_empty() {
        resp.ctx.accesskit_node_builder(resp.id, |b| b.set_author_id(n.c.anchor.clone()));
    }
    if n.c.invalid {
        resp.ctx.accesskit_node_builder(resp.id, |b| b.set_invalid(egui::accesskit::Invalid::True));
    }
    resp
}

/// The slot's `Ui` node: a group named by the slot's label, with the slot key as author id.
pub(crate) fn slot_group(ctx: &egui::Context, id: egui::Id, key: &str) {
    ctx.accesskit_node_builder(id, |b| b.set_author_id(key.to_string()));
}
