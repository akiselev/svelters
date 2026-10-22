//! Hit lists and `hit_test` (ARCH-SCENE §7.4, S12(a); R13, R20). Written by SCENE (SG3); EVENTS
//! reads it.
//!
//! egui keeps interaction (picking, drag capture, focus). The scene keeps, per viewport, the list
//! of nodes the walk drew in its last pass, in paint order: each drawn leaf or provider node with
//! its response rect, clip rect and layer order (`walk.rs` calls [`record`]). A viewport's list
//! starts over at its next pass, so it always holds that viewport's last (or current) pass.
//!
//! [`SceneRef::hit_test`] walks it from the top down and answers what is under a point:
//! - entries of nodes freed, moved to another root, or whose root is now hidden are skipped;
//! - **R20**: entries of a slot root whose current viewport is not the one asked are skipped (the
//!   frame after `set_viewport`, before the old viewport draws again);
//! - `HIT_TRANSPARENT` on a node or any of its ancestors (CSS `pointer-events: none`) lets the
//!   point through;
//! - a provider kind with `Caps::HIT_TEST` is asked ([`Widget::hit_test`]): **R13** `Hit::Node`
//!   (the node), `Hit::Part(key)` (a part only it interprets: a row key, a handle, a pick id;
//!   reported as `Target::Virtual(VNodeId { host, key })`) or `Hit::Miss` (the point goes
//!   through).
//!
//! Layers: entries of a higher `egui::Order` are on top; within one order, later-drawn entries
//! are. Two overlapping areas of the same order (two floating windows) are ordered by draw order,
//! not by egui's area z-order.
//!
//! [`Widget::hit_test`]: crate::provider::Widget::hit_test
//!
//! Writer lane: SCENE.

use super::Scene;
use super::access::SceneRef;
use super::id::{NodeId, RootIx, Target, VNodeId};
use super::node::{NodeFlags, W};
use super::tree::TreeRoot;

/// What a provider widget answers for a point inside its drawn rect (R13).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Hit {
    /// The node itself.
    Node,
    /// A part of the node only the provider interprets (a row key, a handle index, a pick id):
    /// reported as `Target::Virtual(VNodeId { host: node, key })`.
    Part(u64),
    /// Nothing of this node is here: the point goes through to what is below.
    Miss,
}

/// What [`Widget::hit_test`](crate::provider::Widget::hit_test) gets besides the point.
#[derive(Copy, Clone, Debug)]
pub struct HitCx {
    /// The node asked.
    pub node: NodeId,
    /// Its rect drawn last pass (screen points; the `local` point is relative to `rect.min`).
    pub rect: egui::Rect,
    /// The point in screen points.
    pub pos: egui::Pos2,
}

#[derive(Copy, Clone, Debug)]
struct Entry {
    node: NodeId,
    /// Its root when drawn.
    root: RootIx,
    rect: egui::Rect,
    clip: egui::Rect,
    order: egui::Order,
}

#[derive(Default)]
struct HitList {
    vp: Option<egui::ViewportId>,
    /// The viewport's cumulative pass number the entries come from.
    pass: u64,
    entries: Vec<Entry>,
}

/// One hit list per viewport (a handful of viewports: a vector).
#[derive(Default)]
pub(crate) struct HitLists {
    lists: Vec<HitList>,
}

impl HitLists {
    fn list(&self, vp: egui::ViewportId) -> Option<&HitList> {
        self.lists.iter().find(|l| l.vp == Some(vp))
    }

    /// Entries in all lists (stats).
    pub fn len(&self) -> usize {
        self.lists.iter().map(|l| l.entries.len()).sum()
    }
}

/// The walk drew node `i` in `ui` (its response rect is `core.drawn`): appends it to the viewport's
/// hit list, starting the list over if this is a new pass of that viewport.
pub(crate) fn record(s: &mut Scene, i: usize, ui: &egui::Ui) {
    let rect = s.core[i].drawn;
    let Some(node) = s.slots.id_at(i) else { return };
    if rect == egui::Rect::NOTHING || !rect.is_positive() {
        return;
    }
    let ctx = ui.ctx();
    let vp = ctx.viewport_id();
    let pass = ctx.cumulative_pass_nr();
    let lists = &mut s.hits.lists;
    let k = match lists.iter().position(|l| l.vp == Some(vp)) {
        Some(k) => k,
        None => {
            lists.push(HitList { vp: Some(vp), pass, entries: Vec::new() });
            lists.len() - 1
        }
    };
    let l = &mut lists[k];
    if l.pass != pass {
        l.pass = pass;
        l.entries.clear();
    }
    l.entries.push(Entry { node, root: s.core[i].root, rect, clip: ui.clip_rect(), order: ui.layer_id().order });
}

impl<'a> SceneRef<'a> {
    /// What is under `pos` in `viewport`, from the topmost down (ARCH-SCENE §7.4): `Target::Node`
    /// for a node, `Target::Virtual` for a part a `Caps::HIT_TEST` provider reported. From the
    /// viewport's last drawn pass (or the current one, after the walk drew it). O(entries of that
    /// viewport).
    pub fn hit_test(self, viewport: egui::ViewportId, pos: egui::Pos2) -> Vec<Target> {
        let s = self.s;
        let Some(l) = s.hits.list(viewport) else { return Vec::new() };
        let mut cands: Vec<&Entry> = l.entries.iter().rev().filter(|e| e.rect.contains(pos) && e.clip.contains(pos)).collect();
        // topmost first: higher layer order, then later drawn (stable over the reversed list)
        cands.sort_by(|a, b| b.order.cmp(&a.order));
        let mut out: Vec<Target> = Vec::new();
        for e in cands {
            let Some(i) = s.idx(e.node) else { continue };
            if s.core[i].root != e.root || !self.live_in(e.root, viewport) || hit_transparent(s, i) {
                continue;
            }
            let t = match ask_provider(s, i, e, pos) {
                Hit::Node => Target::Node(e.node),
                Hit::Part(key) => Target::Virtual(VNodeId { host: e.node, key }),
                Hit::Miss => continue,
            };
            if !out.contains(&t) {
                out.push(t);
            }
        }
        out
    }

    /// The topmost node under `pos` in `viewport` (the first `Target::Node`, or the host of the
    /// first `Target::Virtual`).
    pub fn hit_node(self, viewport: egui::ViewportId, pos: egui::Pos2) -> Option<super::access::NodeRef<'a>> {
        let t = *self.hit_test(viewport, pos).first()?;
        self.get(match t {
            Target::Node(n) => n,
            Target::Virtual(v) => v.host,
        })
    }

    /// Root `r` is shown and, for a slot root, drawn in `vp` now (R20).
    fn live_in(self, r: RootIx, vp: egui::ViewportId) -> bool {
        match self.s.roots.get(r) {
            Some(e) => e.visible && (!matches!(e.kind, TreeRoot::Slot { .. }) || e.viewport == vp),
            None => false,
        }
    }
}

/// `HIT_TRANSPARENT` on node `i` or an ancestor.
fn hit_transparent(s: &Scene, i: usize) -> bool {
    let mut x = Some(i);
    while let Some(k) = x {
        if s.core[k].flags.contains(NodeFlags::HIT_TRANSPARENT) {
            return true;
        }
        x = s.core[k].parent.and_then(|p| s.idx(p));
    }
    false
}

/// Asks a `Caps::HIT_TEST` provider kind about `pos`; every other node is hit as itself.
fn ask_provider(s: &Scene, i: usize, e: &Entry, pos: egui::Pos2) -> Hit {
    let W::Ext(x) = s.w(i) else { return Hit::Node };
    let kind = s.core[i].kind as usize;
    let caps = crate::kinds::KindRegistry::global().provider(kind).map(|p| p.entry.caps);
    if !caps.is_some_and(|c| c.contains(crate::provider::Caps::HIT_TEST)) {
        return Hit::Node;
    }
    // the widget is borrowed while it draws: hit-testing from its own `ui` sees the node
    let Ok(w) = x.0.try_borrow() else { return Hit::Node };
    w.hit_test(&HitCx { node: e.node, rect: e.rect, pos }, (pos - e.rect.min).to_pos2())
}
