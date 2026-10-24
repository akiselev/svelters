//! CV1 (SV49): panes the host paints itself (cadatomic's board and schematic canvases, a 3D view)
//! answering SG3 hit tests like a node.
//!
//! ```ignore
//! let pane = embed.create_pane("board.canvas", BoardPicker::default()); // PaneHandle<BoardPicker>
//! // App::ui, where the canvas is painted:
//! let rect = ui.available_rect_before_wrap();
//! paint_board(ui, rect);
//! embed.show_pane(ui, pane.id(), rect);                                 // records it for hit tests
//! // anywhere after: SceneRef::hit_test / event_target / OverlayCx::hovered report
//! // Target::Virtual(VNodeId { host: pane.node(), key }) for the part `BoardPicker` names
//! ```
//!
//! A pane is a host-owned node at the top of its own root (a slot without a mount): it has an id
//! (`PaneId::node`), a viewport (`Embed::set_viewport(pane.slot(), …)`) and a visibility
//! (`Embed::set_visible`), and nothing in it runs JS. [`Embed::show_pane`] records it in the
//! viewport's hit list where the host draws it, so it is ordered among the slots and overlays by
//! draw order and layer; an overlay root drawn over it is above it. Its [`PaneHit`] answers what is
//! under a point: `Hit::Part(key)` reaches picking, overlays (`OverlayCx::rect`, `hovered`), event
//! filters (`FilterScope::Subtree(pane.node())`, the pane's root) and automation as a
//! `Target::Virtual` of the pane.
//!
//! Writer lane: CV1.

use super::{Embed, SlotId, SlotKey, SlotOptions};
use crate::scene::{NodeId, PaneHit};
use std::cell::RefCell;
use std::rc::Rc;

/// A pane (SV49): its node is the host of the parts its [`PaneHit`] reports.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct PaneId(pub(crate) SlotId);

impl PaneId {
    /// The pane's slot (for `Embed::set_viewport`, `Embed::set_visible`).
    pub fn slot(self) -> SlotId {
        self.0
    }
    /// The pane's node: the `host` of its `Target::Virtual` parts.
    pub fn node(self) -> NodeId {
        NodeId::from_raw(self.0.handle()).expect("a pane's node")
    }
}

/// A registered pane and its hit tester. Cloning shares it.
pub struct PaneHandle<P> {
    id: PaneId,
    p: Rc<RefCell<P>>,
}

impl<P> Clone for PaneHandle<P> {
    fn clone(&self) -> Self {
        PaneHandle { id: self.id, p: self.p.clone() }
    }
}

impl<P: PaneHit> PaneHandle<P> {
    pub fn id(&self) -> PaneId {
        self.id
    }
    pub fn node(&self) -> NodeId {
        self.id.node()
    }
    pub fn with<R>(&self, f: impl FnOnce(&P) -> R) -> R {
        f(&self.p.borrow())
    }
    /// Changes the hit tester (the canvas' geometry); hit tests see it at once.
    pub fn with_mut<R>(&self, f: impl FnOnce(&mut P) -> R) -> R {
        f(&mut self.p.borrow_mut())
    }
}

impl Embed {
    /// SV49: a pane the host paints itself, answering hit tests through `hit` (or the existing
    /// pane's node with a new tester, for a key in use).
    pub fn create_pane<P: PaneHit>(&self, key: impl Into<SlotKey>, hit: P) -> PaneHandle<P> {
        let key = key.into();
        let name = key.0.clone();
        let slot = self.create_slot_as(key, SlotOptions::fill(), None);
        let p = Rc::new(RefCell::new(hit));
        let id = PaneId(slot);
        let dynp: Rc<RefCell<dyn PaneHit>> = p.clone();
        self.inner.backend.0.scene.borrow_mut().panes.by_node.insert(id.node(), (name, dynp));
        PaneHandle { id, p }
    }

    /// SV49: the host drew `pane` at `rect` (screen points) in `ui`: hit tests from now on (this
    /// pass, once the hit list is this pass's, and the next pass's input) see it there, on `ui`'s
    /// layer and clip, above what was recorded before it in this pass. Call it after the slots the
    /// pane is over and before the overlays over it, every pass the pane shows. Never runs JS.
    pub fn show_pane(&self, ui: &egui::Ui, pane: PaneId, rect: egui::Rect) {
        let Ok(mut scene) = self.inner.backend.0.scene.try_borrow_mut() else {
            debug_assert!(false, "show_pane inside the walk");
            return;
        };
        let Some(i) = scene.idx(pane.node()) else { return };
        crate::scene::hit::record_pane(&mut scene, i, rect, ui);
    }

    /// SV49: removes `pane` (its tester and its slot).
    pub fn destroy_pane(&self, pane: PaneId) {
        self.inner.backend.0.scene.borrow_mut().panes.by_node.remove(&pane.node());
        self.destroy_slot(pane.0);
    }
}
