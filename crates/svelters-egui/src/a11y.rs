//! Accessibility: what AccessKit gets for a node. Today only the accessible name of a leaf.
//!
//! EM1 adds names from author ids and the role projection (ARCH-EMBED §4.4, SV16 minimum, SVP3).
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
