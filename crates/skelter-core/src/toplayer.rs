//! Each window's top layer at frame and dispatch time: the overlay parts kinds record
//! ([`crate::RecordCx::overlay`]) entered in the window's [`OverlayStack`], their layers above every
//! content layer, input on them before the content (hit test, light dismiss, Escape), and the drag
//! layer that shows an in-app drag's [`DragPreview`] at the pointer.

use crate::cx::Emitted;
use crate::document::Document;
use crate::kind::{DragPreview, FocusReason};
use crate::overlay::{Closed, DismissReason, Hit, OverlayId, OverlayPart, OverlaySpec, OverlayStack};
use crate::record::{app_overlay_key, drag_key, overlay_key, tip_key};
use crate::window::{DragLayer, OverlayNode};
use skelter_base::commit::Commit;
use skelter_base::display::Prim;
use skelter_base::geom::{Affine, Point, Rect};
use skelter_base::ids::{LayerId, NodeId, PropNodeId, RunId, WindowId};
use skelter_base::layer::{ItemRef, LayerDelta, LayerDesc, LayerKind};
use skelter_base::props::{PropOp, TransformNode};
use std::collections::HashMap;
use std::sync::Arc;

/// The z of the overlay stack's first layer among the root layer's children (content layers count up
/// from 1).
pub(crate) const OVERLAY_Z: i32 = 1 << 20;
/// The app overlay layer's z: above the content, below the overlay stack's surfaces.
pub(crate) const APP_OVERLAY_Z: i32 = OVERLAY_Z - 1;
/// The drag layer's z: above every overlay.
pub(crate) const DRAG_Z: i32 = i32::MAX;
/// The `title` tooltip's z: above the overlay stack, below the drag image.
pub(crate) const TIP_Z: i32 = DRAG_Z - 1;

/// A document-owned list layer of a window: the app's overlay or the `title` tooltip.
#[derive(Clone, Copy)]
pub(crate) enum ListLayer {
    App,
    Tip,
}

fn slot(ws: &mut crate::window::WindowState, which: ListLayer) -> &mut crate::window::AppOverlay {
    match which {
        ListLayer::App => &mut ws.app_overlay,
        ListLayer::Tip => &mut ws.tip,
    }
}

/// An overlay part the rebuild walk found drawn: its node, the node's window transform and transform
/// node, and the part.
pub(crate) struct DrawnPart {
    pub node: NodeId,
    pub world: Affine,
    pub transform: PropNodeId,
    pub part: OverlayPart,
}

impl Document {
    /// Enters the parts drawn in `window` (tree order) in its overlay stack and sends their layers:
    /// new parts open (a part under another node's part opens from it), moved ones follow, and the
    /// entries whose node records no part any more close.
    ///
    /// `tops` are the window's top-layer nodes this rebuild entered (`Document::enter_top`): their
    /// entries stay, and a part drawn inside one opens from it. A top-layer node's own part is not
    /// shown.
    pub(crate) fn sync_overlays(&mut self, window: WindowId, root_t: PropNodeId, root_layer: LayerId, mut drawn: Vec<DrawnPart>, tops: &[NodeId], commit: &mut Commit) {
        drawn.retain(|d| !tops.contains(&d.node));
        let nodes: Vec<NodeId> = drawn.iter().map(|d| d.node).collect();
        let parents: Vec<Option<NodeId>> = drawn
            .iter()
            .map(|d| {
                let mut a = self.parent(d.node);
                while let Some(n) = a {
                    if nodes.contains(&n) || tops.contains(&n) {
                        return Some(n);
                    }
                    a = self.parent(n);
                }
                None
            })
            .collect();
        let boxes: Vec<Rect> = nodes.iter().map(|&n| self.world_rect(n).unwrap_or(Rect::ZERO)).collect();
        let ws = self.window_mut(window).expect("window");
        let stack = ws.overlays.get_or_insert_with(|| OverlayStack::with_z_base(root_layer, OVERLAY_Z));
        let gone: Vec<OverlayId> = stack.entries().iter().filter(|e| !nodes.contains(&e.spec.node) && !tops.contains(&e.spec.node)).map(|e| e.id).collect();
        for id in gone {
            // a child still drawn under a closing parent opens again below
            stack.close(id, DismissReason::Programmatic);
        }
        for (k, d) in drawn.iter().enumerate() {
            let rect = d.world.apply_rect(&d.part.rect);
            let mut inside = vec![boxes[k]];
            inside.extend(d.part.inside.iter().map(|r| d.world.apply_rect(r)));
            match stack.find_node(d.node) {
                Some(id) => {
                    stack.set_rect(id, rect);
                    stack.set_inside(id, inside);
                }
                None => {
                    let layer = LayerId(ws.layer_ids.alloc());
                    let mut spec = OverlaySpec::new(d.part.class, d.node, layer, root_t, rect);
                    spec.parent = parents[k].and_then(|p| stack.find_node(p));
                    spec.opener = d.part.opener.or(Some(d.node));
                    spec.inside = inside;
                    spec.takes_input = d.part.takes_input;
                    spec.modal = d.part.modal;
                    spec.dismiss = d.part.dismiss;
                    stack.push(spec);
                }
            }
        }
        commit.layers.extend(stack.take_deltas());
        // each entry's layer draws its node's part list in the node's space
        let mut open = HashMap::new();
        for e in stack.entries() {
            if tops.contains(&e.spec.node) {
                open.insert(e.spec.node, OverlayNode { layer: e.spec.layer, transform: root_t, inverse: Affine::IDENTITY, subtree: true });
                continue;
            }
            let Some(d) = drawn.iter().find(|d| d.node == e.spec.node) else { continue };
            let prev = ws.overlay_nodes.get(&d.node).map(|o| (o.layer, o.transform));
            if prev != Some((e.spec.layer, d.transform)) {
                let item = ItemRef { node: overlay_key(d.node), transform: d.transform, clip: None, effect: None };
                commit.layers.push(LayerDelta::Items { layer: e.spec.layer, items: vec![item] });
            }
            open.insert(d.node, OverlayNode { layer: e.spec.layer, transform: d.transform, inverse: d.world.inverse().unwrap_or(Affine::IDENTITY), subtree: false });
        }
        let old = std::mem::replace(&mut ws.overlay_nodes, open);
        for o in old.values() {
            if !ws.overlay_nodes.values().any(|n| n.layer == o.layer) {
                ws.layer_ids.free(o.layer.0);
                ws.items.remove(&o.layer);
            }
        }
    }

    /// Where `p` (window points) lands on `window`'s overlay stack: `Some(Some(node))` on an open
    /// surface that takes input, `Some(None)` under a modal (nothing gets it), `None` through to the
    /// content.
    /// On a top-layer node's entry, the node of its subtree the hit list finds at `p` (the node itself
    /// when none does).
    pub(crate) fn overlay_hit(&self, window: WindowId, p: Point, scroll: bool) -> Option<Option<NodeId>> {
        let w = self.window(window)?;
        let st = w.overlays.as_ref()?;
        match st.hit(p) {
            Hit::Overlay(id) => {
                let node = st.get(id).map(|e| e.spec.node).filter(|&n| self.contains(n));
                match node {
                    Some(top) if w.overlay_nodes.get(&top).is_some_and(|o| o.subtree) => Some(self.hit_in(window, p, scroll, Some(top)).or(Some(top))),
                    n => Some(n),
                }
            }
            Hit::Blocked(_) => Some(None),
            Hit::Content => None,
        }
    }

    /// `node`'s window → local transform when it shows an overlay part.
    pub(crate) fn overlay_inverse(&self, window: WindowId, node: NodeId) -> Option<Affine> {
        self.window(window)?.overlay_nodes.get(&node).filter(|o| !o.subtree).map(|o| o.inverse)
    }

    /// A press at `p`: closes the surfaces of `window`'s stack it is outside of (light dismiss).
    pub(crate) fn overlay_press(&mut self, window: WindowId, p: Point, emitted: &mut Vec<Emitted>) {
        let Some(st) = self.window_mut(window).and_then(|w| w.overlays.as_mut()) else { return };
        if st.is_empty() {
            return;
        }
        let (_, closed) = st.pointer_down(p);
        self.overlays_closed(window, &closed, false, emitted);
    }

    /// Escape: closes the topmost surface of `window`'s stack that takes it. True when one closed.
    pub(crate) fn overlay_escape(&mut self, window: WindowId, emitted: &mut Vec<Emitted>) -> bool {
        let Some(st) = self.window_mut(window).and_then(|w| w.overlays.as_mut()) else { return false };
        let closed = st.escape();
        self.overlays_closed(window, &closed, true, emitted);
        !closed.is_empty()
    }

    /// Closes the top-layer entries whose kinds asked to close at this rebuild
    /// ([`crate::overlay::TopLayer::close`]) and tells the kinds, after the frame's walk.
    pub(crate) fn close_asked_tops(&mut self, window: WindowId) {
        let Some(w) = self.window_mut(window) else { return };
        let asked = std::mem::take(&mut w.top_close);
        if asked.is_empty() {
            return;
        }
        let mut closed = Vec::new();
        if let Some(st) = w.overlays.as_mut() {
            for (node, reason) in asked {
                if let Some(id) = st.find_node(node) {
                    closed.extend(st.close(id, reason));
                }
            }
        }
        let mut emitted = Vec::new();
        self.overlays_closed(window, &closed, false, &mut emitted);
        for e in emitted {
            crate::input::dispatch::queue_emitted(self, e);
        }
    }

    /// Tells the kinds whose surfaces closed (children first); with `restore`, keyboard focus inside
    /// a closed surface's node goes back to its opener.
    fn overlays_closed(&mut self, window: WindowId, closed: &[Closed], restore: bool, emitted: &mut Vec<Emitted>) {
        if closed.is_empty() {
            return;
        }
        if let Some(w) = self.window_mut(window) {
            // the next frame re-enters what the kinds keep recording
            w.rebuild = true;
        }
        for c in closed {
            if !self.contains(c.node) {
                continue;
            }
            if let Some(i) = self.idx(c.node)
                && let Some(mut k) = self.kinds[i].take()
            {
                {
                    let mut cx = crate::cx::EventCx { doc: self, node: c.node, emitted, pointer: None };
                    k.overlay_closed(&mut cx, c.reason);
                }
                self.restore_kind(c.node, k);
            }
            if restore
                && let Some(to) = c.restore_focus.filter(|&n| self.contains(n))
            {
                let focus = self.focused(window);
                let inside = focus.is_none_or(|f| self.is_ancestor_or_self(c.node, f));
                if inside && focus != Some(to) && crate::input::focus::can_focus(self, to) {
                    crate::input::focus::set_focus(self, Some(to), FocusReason::Keyboard);
                }
            }
        }
    }

    /// The drag layer of `window` follows the drag in progress: added with the drag's preview at the
    /// pointer, moved (its transform node only) as the pointer moves, re-recorded when the preview
    /// is replaced, removed when the drag ends.
    pub(crate) fn sync_drag_layer(&mut self, window: WindowId, commit: &mut Commit) {
        let Some(ws) = self.window(window) else { return };
        let (Some(root_t), Some(root_layer)) = (ws.root_transform, ws.root_layer) else { return };
        let want = self.input.drag.as_ref().filter(|d| d.window == window && d.preview.is_some()).map(|d| (d.generation, d.pos));
        let have = ws.drag_layer.as_ref().map(|l| l.generation);
        if have.is_some() && want.map(|w| w.0) != have {
            let ws = self.window_mut(window).expect("window");
            let l = ws.drag_layer.take().expect("checked");
            commit.layers.push(LayerDelta::Remove(l.layer));
            commit.props.ops.push(PropOp::Remove(l.transform));
            if l.list == drag_key(window) {
                commit.removed.push(l.list);
            }
            commit.dropped_runs.extend(l.runs);
            ws.layer_ids.free(l.layer.0);
            ws.props.free(l.transform.0);
        }
        let Some((generation, pos)) = want else { return };
        let at = |pos: Point, l: &DragLayer| Affine::translate(pos.x + l.offset.x, pos.y + l.offset.y);
        if let Some(l) = self.window_mut(window).and_then(|w| w.drag_layer.as_mut()) {
            if let Some(pos) = pos.filter(|&p| p != l.pos) {
                l.pos = pos;
                commit.props.ops.push(PropOp::Transform(l.transform, TransformNode { parent: Some(root_t), local: at(pos, l), scroll: None }));
            }
            return;
        }
        let Some(pos) = pos else { return };
        let preview = self.input.drag.as_ref().and_then(|d| d.preview.clone()).expect("checked");
        let (list, offset, bounds, runs) = match preview {
            DragPreview::List { mut list, runs, offset } => {
                // the preview's run indices become run ids for the compositor
                let ids: Vec<RunId> = runs
                    .into_iter()
                    .map(|r| {
                        self.run_counter += 1;
                        let id = RunId(self.run_counter);
                        commit.runs.push((id, r));
                        id
                    })
                    .collect();
                for p in list.items.iter_mut() {
                    if let Prim::Glyphs(g) = p
                        && let Some(id) = (g.run.0 as usize).checked_sub(1).and_then(|i| ids.get(i))
                    {
                        g.run = *id;
                    }
                }
                let bounds = list.bounds;
                commit.lists.push((drag_key(window), Arc::new(list)));
                (drag_key(window), offset, bounds, ids)
            }
            DragPreview::Node { node, offset } => {
                let bounds = self.idx(node).and_then(|i| self.record[i].list.as_ref().map(|l| l.bounds)).unwrap_or(Rect::ZERO);
                (node, offset, bounds, Vec::new())
            }
        };
        let ws = self.window_mut(window).expect("window");
        let layer = LayerId(ws.layer_ids.alloc());
        let transform = PropNodeId(ws.props.alloc());
        let l = DragLayer { layer, transform, list, offset, pos, generation, runs };
        commit.props.ops.push(PropOp::Transform(transform, TransformNode { parent: Some(root_t), local: at(pos, &l), scroll: None }));
        commit.layers.push(LayerDelta::Add(LayerDesc { id: layer, parent: Some(root_layer), kind: LayerKind::Overlay, transform, clip: None, effect: None, z: DRAG_Z, bounds, opaque: false }));
        commit.layers.push(LayerDelta::Items { layer, items: vec![ItemRef { node: list, transform, clip: None, effect: None }] });
        ws.drag_layer = Some(l);
    }

    /// Sets `window`'s app overlay: a display list in window points (with the glyph runs it draws:
    /// its `Prim::Glyphs` name them by position in `runs` plus one, as a [`DragPreview::List`]'s),
    /// drawn in an overlay layer above the window's content and below its open overlay surfaces.
    /// It takes no input (presses go through to what is under it). `None` removes it. The next
    /// frame sends it (a pick highlight, a rubber band, a snap marker the app draws itself).
    pub fn set_window_overlay(&mut self, window: WindowId, overlay: Option<(skelter_base::display::DisplayList, Vec<Arc<skelter_base::text::GlyphRun>>)>) {
        if let Some(w) = self.window_mut(window) {
            w.app_overlay.pending = Some(overlay);
            w.rebuild = true;
        }
    }

    /// The app overlay layer of `window` while it has a list.
    pub fn window_overlay_layer(&self, window: WindowId) -> Option<LayerId> {
        self.window(window)?.app_overlay.layer
    }

    /// Sends `window`'s app overlay or tooltip list when it changed.
    pub(crate) fn sync_list_layer(&mut self, window: WindowId, which: ListLayer, commit: &mut Commit) {
        let Some(ws) = self.window(window) else { return };
        let (Some(root_t), Some(root_layer)) = (ws.root_transform, ws.root_layer) else { return };
        let (key, z) = match which {
            ListLayer::App => (app_overlay_key(window), APP_OVERLAY_Z),
            ListLayer::Tip => (tip_key(window), TIP_Z),
        };
        let Some(pending) = self.window_mut(window).and_then(|w| slot(w, which).pending.take()) else { return };
        let ws = self.window_mut(window).expect("window");
        commit.dropped_runs.extend(std::mem::take(&mut slot(ws, which).runs));
        let Some((mut list, runs)) = pending else {
            if let Some(l) = slot(ws, which).layer.take() {
                commit.layers.push(LayerDelta::Remove(l));
                commit.removed.push(key);
                ws.layer_ids.free(l.0);
            }
            return;
        };
        let ids: Vec<RunId> = runs
            .into_iter()
            .map(|r| {
                self.run_counter += 1;
                let id = RunId(self.run_counter);
                commit.runs.push((id, r));
                id
            })
            .collect();
        for p in list.items.iter_mut() {
            if let Prim::Glyphs(g) = p
                && let Some(id) = (g.run.0 as usize).checked_sub(1).and_then(|i| ids.get(i))
            {
                g.run = *id;
            }
        }
        let ws = self.window_mut(window).expect("window");
        slot(ws, which).runs = ids;
        let bounds = list.bounds;
        commit.lists.push((key, Arc::new(list)));
        let (layer, new) = match slot(ws, which).layer {
            Some(l) => (l, false),
            None => (LayerId(ws.layer_ids.alloc()), true),
        };
        slot(ws, which).layer = Some(layer);
        let desc = LayerDesc { id: layer, parent: Some(root_layer), kind: LayerKind::Overlay, transform: root_t, clip: None, effect: None, z, bounds, opaque: false };
        commit.layers.push(if new { LayerDelta::Add(desc) } else { LayerDelta::Update(desc) });
        if new {
            commit.layers.push(LayerDelta::Items { layer, items: vec![ItemRef { node: key, transform: root_t, clip: None, effect: None }] });
        }
    }

    /// The overlay stack of `window` (its open surfaces, bottom first), as the last frame left it.
    pub fn overlay_stack(&self, window: WindowId) -> Option<&OverlayStack> {
        self.window(window)?.overlays.as_ref()
    }

    /// The drag layer of `window` while it shows a drag image: the layer and its transform node.
    pub fn drag_layer(&self, window: WindowId) -> Option<(LayerId, PropNodeId)> {
        self.window(window)?.drag_layer.as_ref().map(|l| (l.layer, l.transform))
    }
}
