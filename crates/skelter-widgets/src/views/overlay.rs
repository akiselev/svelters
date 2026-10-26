//! A window's overlay stack (architecture §14): the surfaces above its panes (popups and popovers,
//! menus, modals, inner windows, the toast host, tooltips, the drag image, designer adorners), each
//! one compositor layer of kind [`LayerKind::Overlay`] under the window's root layer.
//!
//! The stack computes:
//! - **paint order**: by band ([`OverlayClass::band`]: adorners, inner windows, then popups, menus
//!   and modals in the order they opened, then toasts, tooltips, the drag image), and by push order
//!   inside a band; [`OverlayStack::raise`] moves an entry to the top of its band;
//! - **hit routing** ([`OverlayStack::hit`]): topmost first; entries that take no input are
//!   click-through; an open modal swallows every press below it;
//! - **light dismiss** ([`OverlayStack::pointer_down`], [`OverlayStack::escape`]): a press outside
//!   closes the open non-modal surfaces that ask for it, except the chain the press landed in (a
//!   press inside a submenu keeps its parent menus; a press on a surface's anchor keeps it); Escape
//!   closes the topmost surface that takes it, one level per press (svelters-egui
//!   `interact/dismiss.rs`);
//! - **focus handoff**: closing a chain reports the opener of its outermost entry, where keyboard
//!   focus goes back;
//! - **layer deltas** ([`OverlayStack::take_deltas`]): `Add`, `Update` (moved, resized or reordered)
//!   and `Remove` for the commit.

use skelter_base::geom::{Point, Rect};
use skelter_base::ids::{LayerId, NodeId, PropNodeId};
use skelter_base::layer::{LayerDelta, LayerDesc, LayerKind};

/// An entry's id in its stack (not reused while the stack lives).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OverlayId(pub u32);

/// What an entry is; decides its band.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum OverlayClass {
    /// Designer adorners and the tour overlay: under everything else, usually click-through.
    Adorner,
    /// Inner windows (above the panes, below popups).
    Window,
    Popup,
    Menu,
    Modal,
    ToastHost,
    Tooltip,
    DragImage,
}

impl OverlayClass {
    /// Paint band: higher bands paint above lower ones whatever the push order.
    pub fn band(self) -> u8 {
        match self {
            OverlayClass::Adorner => 0,
            OverlayClass::Window => 1,
            OverlayClass::Popup | OverlayClass::Menu | OverlayClass::Modal => 2,
            OverlayClass::ToastHost => 3,
            OverlayClass::Tooltip => 4,
            OverlayClass::DragImage => 5,
        }
    }
}

/// Why a surface closed (the `close` signal's `reason`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum DismissReason {
    Escape,
    /// A pointer press outside the surface, its anchor and its open children.
    OutsidePress,
    /// Keyboard focus left it.
    FocusOut,
    /// Its anchor went away.
    AnchorHidden,
    /// Its parent in the chain closed.
    Parent,
    /// The app (`open = false`).
    Programmatic,
}

impl DismissReason {
    pub fn name(self) -> &'static str {
        match self {
            DismissReason::Escape => "escape",
            DismissReason::OutsidePress => "outside-press",
            DismissReason::FocusOut => "focus-out",
            DismissReason::AnchorHidden => "anchor-hidden",
            DismissReason::Parent => "parent",
            DismissReason::Programmatic => "programmatic",
        }
    }
}

/// What may close an entry without the app asking.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dismiss {
    /// A press outside closes it (`light-dismiss`, GTK `autohide`).
    pub outside: bool,
    /// Escape closes it when it is the topmost such entry.
    pub escape: bool,
}

impl Dismiss {
    pub const NONE: Dismiss = Dismiss { outside: false, escape: false };
    pub const LIGHT: Dismiss = Dismiss { outside: true, escape: true };
}

/// What [`OverlayStack::push`] is given.
#[derive(Clone, Debug, PartialEq)]
pub struct OverlaySpec {
    pub class: OverlayClass,
    /// The node whose subtree the surface shows.
    pub node: NodeId,
    /// The compositor layer the caller allocated for it, and the transform node it moves with.
    pub layer: LayerId,
    pub transform: PropNodeId,
    /// Window points.
    pub rect: Rect,
    /// The entry this one opened from (a submenu's menu, a popup opened inside a modal): closing
    /// the parent closes it, and a press inside it keeps the parent open.
    pub parent: Option<OverlayId>,
    /// Where keyboard focus goes back when the surface closes.
    pub opener: Option<NodeId>,
    /// Rects that count as inside for light dismiss (the anchor), window points.
    pub inside: Vec<Rect>,
    /// False: click-through (tooltips, adorners, the drag image).
    pub takes_input: bool,
    /// Swallows every press below it.
    pub modal: bool,
    pub dismiss: Dismiss,
}

impl OverlaySpec {
    /// A spec with the defaults of `class`: popups and menus light-dismiss and take input; modals
    /// block and close on Escape; tooltips, adorners and the drag image are click-through.
    pub fn new(class: OverlayClass, node: NodeId, layer: LayerId, transform: PropNodeId, rect: Rect) -> Self {
        let (takes_input, modal, dismiss) = match class {
            OverlayClass::Popup | OverlayClass::Menu => (true, false, Dismiss::LIGHT),
            OverlayClass::Modal => (true, true, Dismiss { outside: false, escape: true }),
            OverlayClass::Window | OverlayClass::ToastHost => (true, false, Dismiss::NONE),
            OverlayClass::Tooltip => (false, false, Dismiss { outside: true, escape: true }),
            OverlayClass::Adorner | OverlayClass::DragImage => (false, false, Dismiss::NONE),
        };
        OverlaySpec { class, node, layer, transform, rect, parent: None, opener: None, inside: Vec::new(), takes_input, modal, dismiss }
    }
}

/// One open surface.
#[derive(Clone, Debug, PartialEq)]
pub struct OverlayEntry {
    pub id: OverlayId,
    pub spec: OverlaySpec,
    /// Push (or raise) order.
    seq: u64,
    /// The z it was last reported with (`None`: not reported yet).
    reported: Option<(i32, Rect)>,
}

/// One entry the stack closed, in close order (children before their parents).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Closed {
    pub id: OverlayId,
    pub node: NodeId,
    pub reason: DismissReason,
    /// Set on the outermost entry of a closed chain: where keyboard focus goes back.
    pub restore_focus: Option<NodeId>,
}

/// Where a pointer position lands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hit {
    /// On this entry.
    Overlay(OverlayId),
    /// Below this modal: nothing below it gets the event.
    Blocked(OverlayId),
    /// Through every overlay, to the window's content.
    Content,
}

/// The overlay stack of one window.
#[derive(Debug)]
pub struct OverlayStack {
    /// The window's root layer (the parent of every overlay layer).
    root: LayerId,
    /// Paint order, bottom first.
    entries: Vec<OverlayEntry>,
    next_id: u32,
    next_seq: u64,
    deltas: Vec<LayerDelta>,
}

impl OverlayStack {
    pub fn new(root: LayerId) -> Self {
        OverlayStack { root, entries: Vec::new(), next_id: 1, next_seq: 1, deltas: Vec::new() }
    }

    /// Bottom first.
    pub fn entries(&self) -> &[OverlayEntry] {
        &self.entries
    }
    pub fn get(&self, id: OverlayId) -> Option<&OverlayEntry> {
        self.entries.iter().find(|e| e.id == id)
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    /// The topmost open modal.
    pub fn top_modal(&self) -> Option<OverlayId> {
        self.entries.iter().rev().find(|e| e.spec.modal).map(|e| e.id)
    }
    /// The entry showing `node`.
    pub fn find_node(&self, node: NodeId) -> Option<OverlayId> {
        self.entries.iter().find(|e| e.spec.node == node).map(|e| e.id)
    }

    /// Opens a surface on top of its band.
    pub fn push(&mut self, spec: OverlaySpec) -> OverlayId {
        let id = OverlayId(self.next_id);
        self.next_id += 1;
        let seq = self.bump();
        self.entries.push(OverlayEntry { id, spec, seq, reported: None });
        self.sort();
        id
    }

    /// Moves an entry to the top of its band (a press on an inner window).
    pub fn raise(&mut self, id: OverlayId) -> bool {
        let seq = self.bump();
        let Some(e) = self.entries.iter_mut().find(|e| e.id == id) else { return false };
        e.seq = seq;
        self.sort();
        true
    }

    /// The surface moved or resized (window points).
    pub fn set_rect(&mut self, id: OverlayId, rect: Rect) {
        if let Some(e) = self.entries.iter_mut().find(|e| e.id == id) {
            e.spec.rect = rect;
        }
    }

    /// The rects that count as inside it (its anchor moved).
    pub fn set_inside(&mut self, id: OverlayId, inside: Vec<Rect>) {
        if let Some(e) = self.entries.iter_mut().find(|e| e.id == id) {
            e.spec.inside = inside;
        }
    }

    /// Closes `id` and every entry opened from it.
    pub fn close(&mut self, id: OverlayId, reason: DismissReason) -> Vec<Closed> {
        let mut out = Vec::new();
        self.close_into(id, reason, &mut out);
        out
    }

    /// Where `p` (window points) lands.
    pub fn hit(&self, p: Point) -> Hit {
        for e in self.entries.iter().rev() {
            if e.spec.takes_input && e.spec.rect.contains(p) {
                return Hit::Overlay(e.id);
            }
            if e.spec.modal {
                return Hit::Blocked(e.id);
            }
        }
        Hit::Content
    }

    /// A pointer press at `p`: light-dismisses what it is outside of. Returns what closed and where
    /// the press goes.
    pub fn pointer_down(&mut self, p: Point) -> (Hit, Vec<Closed>) {
        let hit = self.hit(p);
        // the chain the press is in stays open
        let mut keep: Vec<OverlayId> = Vec::new();
        if let Hit::Overlay(id) = hit {
            let mut cur = Some(id);
            while let Some(c) = cur {
                keep.push(c);
                cur = self.get(c).and_then(|e| e.spec.parent);
            }
        }
        // entries below the topmost modal cannot be reached by this press: they stay
        let floor = self.entries.iter().rposition(|e| e.spec.modal).unwrap_or(0);
        let victims: Vec<OverlayId> = self.entries[floor..]
            .iter()
            .rev()
            .filter(|e| e.spec.dismiss.outside && !e.spec.modal)
            .filter(|e| !keep.contains(&e.id) && !e.spec.inside.iter().any(|r| r.contains(p)))
            .map(|e| e.id)
            .collect();
        let mut closed = Vec::new();
        for id in victims {
            if self.get(id).is_some() {
                self.close_into(id, DismissReason::OutsidePress, &mut closed);
            }
        }
        (self.hit(p), closed)
    }

    /// Escape: closes the topmost entry that takes it (and what it opened). Empty when nothing did:
    /// the key goes on to the focused node.
    pub fn escape(&mut self) -> Vec<Closed> {
        let Some(id) = self.entries.iter().rev().find(|e| e.spec.dismiss.escape).map(|e| e.id) else {
            return Vec::new();
        };
        self.close(id, DismissReason::Escape)
    }

    /// The layer changes since the last call, for the commit.
    pub fn take_deltas(&mut self) -> Vec<LayerDelta> {
        for (z, e) in self.entries.iter_mut().enumerate() {
            let z = z as i32;
            let desc = LayerDesc {
                id: e.spec.layer,
                parent: Some(self.root),
                kind: LayerKind::Overlay,
                transform: e.spec.transform,
                clip: None,
                effect: None,
                z,
                bounds: e.spec.rect,
                opaque: false,
            };
            match e.reported {
                None => self.deltas.push(LayerDelta::Add(desc)),
                Some(r) if r != (z, e.spec.rect) => self.deltas.push(LayerDelta::Update(desc)),
                Some(_) => {}
            }
            e.reported = Some((z, e.spec.rect));
        }
        std::mem::take(&mut self.deltas)
    }

    fn bump(&mut self) -> u64 {
        let s = self.next_seq;
        self.next_seq += 1;
        s
    }

    fn sort(&mut self) {
        self.entries.sort_by_key(|e| (e.spec.class.band(), e.seq));
    }

    fn close_into(&mut self, id: OverlayId, reason: DismissReason, out: &mut Vec<Closed>) {
        let Some(pos) = self.entries.iter().position(|e| e.id == id) else { return };
        // children first, newest first
        let kids: Vec<OverlayId> = self.entries.iter().rev().filter(|e| e.spec.parent == Some(id)).map(|e| e.id).collect();
        for k in kids {
            self.close_into(k, DismissReason::Parent, out);
        }
        let pos = self.entries.iter().position(|e| e.id == id).unwrap_or(pos);
        let e = self.entries.remove(pos);
        if e.reported.is_some() {
            self.deltas.push(LayerDelta::Remove(e.spec.layer));
        }
        // focus goes back to the outermost closed entry's opener: a child's opener is inside its
        // parent, which is closing too
        let parent_closing = reason == DismissReason::Parent;
        out.push(Closed { id, node: e.spec.node, reason, restore_focus: if parent_closing { None } else { e.spec.opener } });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use skelter_base::geom::Size;

    fn r(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::from_origin_size(Point::new(x, y), Size::new(w, h))
    }

    fn spec(class: OverlayClass, n: u64, rect: Rect) -> OverlaySpec {
        let mut s = OverlaySpec::new(class, NodeId(n), LayerId(n as u32), PropNodeId(n as u32), rect);
        s.opener = Some(NodeId(100 + n));
        s
    }

    #[test]
    fn bands_order_paint_and_hits() {
        let mut st = OverlayStack::new(LayerId(1));
        let tip = st.push(spec(OverlayClass::Tooltip, 2, r(0.0, 0.0, 50.0, 50.0)));
        let menu = st.push(spec(OverlayClass::Menu, 3, r(0.0, 0.0, 100.0, 100.0)));
        let win = st.push(spec(OverlayClass::Window, 4, r(0.0, 0.0, 200.0, 200.0)));
        let order: Vec<_> = st.entries().iter().map(|e| e.id).collect();
        assert_eq!(order, vec![win, menu, tip]);
        // the tooltip is click-through: the menu under it gets the press
        assert_eq!(st.hit(Point::new(10.0, 10.0)), Hit::Overlay(menu));
        assert_eq!(st.hit(Point::new(150.0, 150.0)), Hit::Overlay(win));
        assert_eq!(st.hit(Point::new(300.0, 300.0)), Hit::Content);
        let d = st.take_deltas();
        assert_eq!(d.len(), 3);
        assert!(matches!(&d[0], LayerDelta::Add(l) if l.id == LayerId(4) && l.z == 0 && l.kind == LayerKind::Overlay && l.parent == Some(LayerId(1))));
        assert!(st.take_deltas().is_empty(), "nothing changed");
    }

    #[test]
    fn outside_press_closes_the_chain_but_not_the_branch_pressed() {
        let mut st = OverlayStack::new(LayerId(1));
        let bar_anchor = r(0.0, 0.0, 40.0, 20.0);
        let mut m = spec(OverlayClass::Menu, 2, r(0.0, 20.0, 100.0, 200.0));
        m.inside = vec![bar_anchor];
        let menu = st.push(m);
        let mut s = spec(OverlayClass::Menu, 3, r(100.0, 40.0, 100.0, 100.0));
        s.parent = Some(menu);
        let sub = st.push(s);
        let mut s2 = spec(OverlayClass::Menu, 4, r(200.0, 60.0, 100.0, 100.0));
        s2.parent = Some(sub);
        let subsub = st.push(s2);
        st.take_deltas();

        // a press inside the submenu keeps it and its parent; its own child closes
        let (hit, closed) = st.pointer_down(Point::new(150.0, 50.0));
        assert_eq!(hit, Hit::Overlay(sub));
        assert_eq!(closed.iter().map(|c| c.id).collect::<Vec<_>>(), vec![subsub]);
        assert_eq!(closed[0].reason, DismissReason::OutsidePress);
        assert_eq!(closed[0].restore_focus, Some(NodeId(104)));

        // a press on the anchor keeps the menu (the anchor toggles it itself)
        let (hit, closed) = st.pointer_down(Point::new(10.0, 10.0));
        assert_eq!(hit, Hit::Content);
        assert_eq!(closed.iter().map(|c| c.id).collect::<Vec<_>>(), vec![sub], "the submenu is outside");

        // a press elsewhere closes the menu; focus goes back to its opener
        let (hit, closed) = st.pointer_down(Point::new(500.0, 500.0));
        assert_eq!(hit, Hit::Content);
        assert_eq!(closed, vec![Closed { id: menu, node: NodeId(2), reason: DismissReason::OutsidePress, restore_focus: Some(NodeId(102)) }]);
        assert!(st.is_empty());
        let d = st.take_deltas();
        assert!(d.contains(&LayerDelta::Remove(LayerId(2))) && d.contains(&LayerDelta::Remove(LayerId(3))) && d.contains(&LayerDelta::Remove(LayerId(4))));
    }

    #[test]
    fn escape_closes_one_level_and_the_children_of_it() {
        let mut st = OverlayStack::new(LayerId(1));
        let menu = st.push(spec(OverlayClass::Menu, 2, r(0.0, 0.0, 100.0, 100.0)));
        let mut s = spec(OverlayClass::Menu, 3, r(100.0, 0.0, 100.0, 100.0));
        s.parent = Some(menu);
        let sub = st.push(s);
        let closed = st.escape();
        assert_eq!(closed.len(), 1);
        assert_eq!((closed[0].id, closed[0].reason), (sub, DismissReason::Escape));
        assert_eq!(st.get(menu).map(|e| e.id), Some(menu));
        // the parent closing takes its children with it; only the outermost restores focus
        let mut s = spec(OverlayClass::Menu, 5, r(100.0, 0.0, 100.0, 100.0));
        s.parent = Some(menu);
        let sub2 = st.push(s);
        let closed = st.close(menu, DismissReason::Programmatic);
        assert_eq!(closed.iter().map(|c| (c.id, c.reason)).collect::<Vec<_>>(), vec![(sub2, DismissReason::Parent), (menu, DismissReason::Programmatic)]);
        assert_eq!(closed[0].restore_focus, None);
        assert_eq!(closed[1].restore_focus, Some(NodeId(102)));
        assert!(st.escape().is_empty(), "nothing left takes Escape");
    }

    #[test]
    fn a_modal_blocks_presses_below_it() {
        let mut st = OverlayStack::new(LayerId(1));
        let pop = st.push(spec(OverlayClass::Popup, 2, r(0.0, 0.0, 50.0, 50.0)));
        // a modal whose own rect is its dialog box (the backdrop is the block)
        let modal = st.push(spec(OverlayClass::Modal, 3, r(100.0, 100.0, 200.0, 100.0)));
        assert_eq!(st.hit(Point::new(10.0, 10.0)), Hit::Blocked(modal), "the popup under the modal is unreachable");
        assert_eq!(st.hit(Point::new(150.0, 150.0)), Hit::Overlay(modal));
        // a press below the modal closes nothing below it
        let (hit, closed) = st.pointer_down(Point::new(10.0, 10.0));
        assert_eq!(hit, Hit::Blocked(modal));
        assert!(closed.is_empty() && st.get(pop).is_some());
        // a popup opened inside the modal sits above it and light-dismisses
        let mut inner = spec(OverlayClass::Popup, 4, r(120.0, 120.0, 40.0, 40.0));
        inner.parent = Some(modal);
        let inner = st.push(inner);
        assert_eq!(st.hit(Point::new(130.0, 130.0)), Hit::Overlay(inner));
        let (_, closed) = st.pointer_down(Point::new(250.0, 150.0));
        assert_eq!(closed.iter().map(|c| c.id).collect::<Vec<_>>(), vec![inner]);
        // Escape closes the modal, not the popup below it
        assert_eq!(st.escape().iter().map(|c| c.id).collect::<Vec<_>>(), vec![modal]);
        assert!(st.get(pop).is_some());
    }

    #[test]
    fn raise_reorders_within_the_band_and_reports_it() {
        let mut st = OverlayStack::new(LayerId(1));
        let a = st.push(spec(OverlayClass::Window, 2, r(0.0, 0.0, 100.0, 100.0)));
        let b = st.push(spec(OverlayClass::Window, 3, r(50.0, 50.0, 100.0, 100.0)));
        let pop = st.push(spec(OverlayClass::Popup, 4, r(400.0, 0.0, 10.0, 10.0)));
        st.take_deltas();
        assert_eq!(st.hit(Point::new(60.0, 60.0)), Hit::Overlay(b));
        assert!(st.raise(a));
        assert_eq!(st.hit(Point::new(60.0, 60.0)), Hit::Overlay(a));
        assert_eq!(st.entries().last().map(|e| e.id), Some(pop), "a raised window stays below popups");
        let d = st.take_deltas();
        assert_eq!(d.len(), 2, "both windows changed z: {d:?}");
    }
}
