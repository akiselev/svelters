//! The caret's fast commits (architecture §7.3): a kind places the caret with
//! [`EventCx::set_caret`](crate::EventCx::set_caret) in its local space; the document sends it in
//! the space of the node's own transform node, with the node's colour, so the compositor moves it
//! with scrolls and transform animations of that chain.
//!
//! An editor that is not rendered (detached, `display: none` on it or an ancestor, concealed,
//! `visibility: hidden`, zero-size, or never recorded) gets its caret hidden once and sends no caret
//! while it stays so; each frame checks, and shows the last placed caret again once it is rendered.

use crate::document::Document;
use skelter_base::commit::FastCommit;
use skelter_base::geom::Rect;
use skelter_base::ids::{NodeId, WindowId};

/// The last caret a kind placed, and whether the document hid it for an editor that isn't rendered.
#[derive(Clone, Copy, Debug)]
pub(crate) struct CaretSlot {
    pub node: NodeId,
    pub window: WindowId,
    /// In the node's local space; `None` = hidden by the kind.
    pub rect: Option<Rect>,
    /// `rect: None` went out because the node isn't rendered; nothing more until it is.
    pub suppressed: bool,
}

impl Document {
    /// Node `i` is drawn in its window: attached, recorded, not `display: none` or concealed
    /// (itself or an ancestor), not `visibility: hidden`, and not zero-size.
    pub(crate) fn caret_rendered(&self, i: usize) -> bool {
        if self.core[i].window.is_none() || self.record[i].transform.is_none() {
            return false;
        }
        if self.style.computed(i).visibility_hidden {
            return false;
        }
        let size = self.layout[i].layout.size;
        if size.width <= 0.0 || size.height <= 0.0 {
            return false;
        }
        let mut j = i;
        loop {
            if self.layout[j].style.display == taffy::Display::None
                || self.core[j].flags.contains(crate::tree::NodeFlags::CONCEALED)
            {
                return false;
            }
            match self.core[j].parent {
                Some(p) => j = self.ix(p),
                None => return true,
            }
        }
    }

    /// Places (or with `None` hides) node `node`'s caret at `rect` in its local space.
    pub(crate) fn set_caret(&mut self, node: NodeId, rect: Option<Rect>) {
        let Some(i) = self.idx(node) else { return };
        let Some(window) = self.core[i].window else { return };
        let was = self.caret.filter(|s| s.node == node && s.window == window);
        let mut slot = CaretSlot { node, window, rect, suppressed: false };
        if !self.caret_rendered(i) {
            // hide once; nothing more while it stays unrendered
            if !was.is_some_and(|s| s.suppressed) {
                self.push_caret(i, window, None);
            }
            slot.suppressed = true;
        } else {
            self.push_caret(i, window, rect);
        }
        self.caret = Some(slot);
    }

    /// After a frame of `window`: hides the caret of an editor that stopped being rendered, and
    /// shows it again once it is.
    pub(crate) fn check_caret(&mut self, window: WindowId) {
        let Some(mut slot) = self.caret.filter(|s| s.window == window && s.rect.is_some()) else { return };
        let i = self.idx(slot.node).filter(|&i| self.core[i].window == Some(window));
        let rendered = i.is_some_and(|i| self.caret_rendered(i));
        match (rendered, slot.suppressed, i) {
            (false, false, _) => {
                self.fast.push(FastCommit::Caret {
                    window,
                    rect: None,
                    color: Default::default(),
                    transform: Default::default(),
                    revision: self.revision,
                });
                slot.suppressed = true;
            }
            (true, true, Some(i)) => {
                self.push_caret(i, window, slot.rect);
                slot.suppressed = false;
            }
            _ => {}
        }
        // a removed node's caret is gone for good
        self.caret = if i.is_none() { None } else { Some(slot) };
    }

    fn push_caret(&mut self, i: usize, window: WindowId, rect: Option<Rect>) {
        let color = self.style.computed(i).color;
        let transform = self.record[i].transform.unwrap_or_default();
        self.fast.push(FastCommit::Caret { window, rect, color, transform, revision: self.revision });
    }
}
