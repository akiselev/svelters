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
//! **Panes (SV49).** A pane the host paints itself (a canvas, a 3D view: `Embed::create_pane`) is
//! a host-owned node at the top of its own root, recorded in the hit list where the host draws it
//! (`Embed::show_pane`) and answered by its [`PaneHit`]: a `Hit::Part` is a `Target::Virtual` of
//! the pane, as a provider's part is, so picking, overlays and event filters treat it as a node.
//!
//! Layers: entries of a higher `egui::Order` are on top; within one order, later-drawn entries
//! are. Two overlapping areas of the same order (two floating windows) are ordered by draw order,
//! not by egui's area z-order.
//!
//! [`Widget::hit_test`]: crate::provider::Widget::hit_test
//!
//! Writer lane: SCENE.

use super::Scene;
use super::access::{NodeRef, SceneRef};
use super::id::{NodeId, RootIx, Target, VNodeId};
use super::node::{NodeFlags, W};

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

/// SV49: what a pane the host paints itself answers for SG3 hit tests (`Embed::create_pane`). Its
/// parts are its own (a pick id, a handle index): `Hit::Part(key)` is reported as
/// `Target::Virtual(VNodeId { host: pane node, key })`.
pub trait PaneHit: 'static {
    /// What is at `local` (relative to the pane's rect drawn last pass, `cx.rect`).
    fn hit_test(&self, cx: &HitCx, local: egui::Pos2) -> Hit;
    /// Part `key`'s rect, relative to the pane's rect (overlays, `SceneRef::target_rect`,
    /// automation); `None` when it has none.
    fn part_rect(&self, _key: u64) -> Option<egui::Rect> {
        None
    }
    /// Part `key`'s name (`SceneRef::target_anchor`: `"<pane key>/<name>"`); `None` when unnamed.
    fn part_anchor(&self, _key: u64) -> Option<String> {
        None
    }
}

/// The panes' hit testers by node (SV49).
#[derive(Default)]
pub(crate) struct Panes {
    pub(crate) by_node: std::collections::HashMap<NodeId, (String, std::rc::Rc<std::cell::RefCell<dyn PaneHit>>)>,
}

impl Panes {
    fn get(&self, n: NodeId) -> Option<&std::rc::Rc<std::cell::RefCell<dyn PaneHit>>> {
        self.by_node.get(&n).map(|p| &p.1)
    }
}

#[derive(Copy, Clone, Debug)]
struct Entry {
    node: NodeId,
    /// Its root when drawn.
    root: RootIx,
    rect: egui::Rect,
    clip: egui::Rect,
    order: egui::Order,
    /// EV1: a container box (recorded by [`record_box`] before its children): never reported by
    /// [`SceneRef::hit_test`], only by the event dispatch's [`SceneRef::event_target`].
    container: bool,
}

#[derive(Default)]
struct HitList {
    vp: Option<egui::ViewportId>,
    /// The viewport's cumulative pass number the entries come from.
    pass: u64,
    entries: Vec<Entry>,
    /// AX1: the containers (and virtual-list rows) the walk culled in that pass: nothing below
    /// them was drawn, whatever rect they kept from an earlier pass.
    culled: Vec<NodeId>,
}

/// AX1: where a node showed in a viewport's last drawn pass ([`SceneRef::shown`]).
#[derive(Copy, Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct Shown {
    /// Its rect on screen, as drawn.
    pub rect: egui::Rect,
    /// The part of `rect` inside the clip it was drawn with: what a user sees of it (empty when
    /// it is scrolled out of a `scroll-area` or a list, partly when partly scrolled out). Equal to
    /// `rect` for a node drawn outside the walk (its clip is not known).
    pub visible: egui::Rect,
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

    /// Leaf entries in all lists (stats; EV1's container entries are not counted).
    pub fn len(&self) -> usize {
        self.lists.iter().map(|l| l.entries.iter().filter(|e| !e.container).count()).sum()
    }
}

/// The walk drew node `i` in `ui` (its response rect is `core.drawn`): appends it to the viewport's
/// hit list, starting the list over if this is a new pass of that viewport.
pub(crate) fn record(s: &mut Scene, i: usize, ui: &egui::Ui) {
    let rect = s.core[i].drawn;
    push(s, i, rect, ui, false);
}

/// EV1: the walk is about to draw container `i` at `rect` (screen) in `ui`: appends it to the
/// viewport's hit list as a container entry, under the children drawn after it. Events whose point
/// hits no leaf go to the innermost box under it (a click on a box's padding targets the box).
pub(crate) fn record_box(s: &mut Scene, i: usize, rect: egui::Rect, ui: &egui::Ui) {
    push(s, i, rect, ui, true);
}

/// SV49: the host drew pane `i` at `rect` (screen) in `ui`: its drawn rect, and an entry of the
/// viewport's hit list in the host's draw order.
pub(crate) fn record_pane(s: &mut Scene, i: usize, rect: egui::Rect, ui: &egui::Ui) {
    s.core[i].drawn = rect;
    push(s, i, rect, ui, false);
}

/// AX1: the walk culled node `i` in `ui` (a container outside the clip, a virtual-list row out of
/// view): nothing in its subtree was drawn this pass ([`SceneRef::shown`] answers `None` for it).
pub(crate) fn record_culled(s: &mut Scene, i: usize, ui: &egui::Ui) {
    let Some(node) = s.slots.id_at(i) else { return };
    current(s, ui).culled.push(node);
}

/// The hit list of `ui`'s viewport for this pass (started over at a new pass).
fn current<'s>(s: &'s mut Scene, ui: &egui::Ui) -> &'s mut HitList {
    let ctx = ui.ctx();
    let vp = ctx.viewport_id();
    let pass = ctx.cumulative_pass_nr();
    let lists = &mut s.hits.lists;
    let k = match lists.iter().position(|l| l.vp == Some(vp)) {
        Some(k) => k,
        None => {
            lists.push(HitList { vp: Some(vp), pass, ..HitList::default() });
            lists.len() - 1
        }
    };
    let l = &mut lists[k];
    if l.pass != pass {
        l.pass = pass;
        l.entries.clear();
        l.culled.clear();
    }
    l
}

fn push(s: &mut Scene, i: usize, rect: egui::Rect, ui: &egui::Ui, container: bool) {
    let Some(node) = s.slots.id_at(i) else { return };
    if rect == egui::Rect::NOTHING || !rect.is_positive() {
        return;
    }
    let root = s.core[i].root;
    current(s, ui).entries.push(Entry { node, root, rect, clip: ui.clip_rect(), order: ui.layer_id().order, container });
}

impl<'a> SceneRef<'a> {
    /// What is under `pos` in `viewport`, from the topmost down (ARCH-SCENE §7.4): `Target::Node`
    /// for a node, `Target::Virtual` for a part a `Caps::HIT_TEST` provider reported. From the
    /// viewport's last drawn pass (or the current one, after the walk drew it). O(entries of that
    /// viewport).
    pub fn hit_test(self, viewport: egui::ViewportId, pos: egui::Pos2) -> Vec<Target> {
        let s = self.s;
        let Some(l) = s.hits.list(viewport) else { return Vec::new() };
        let mut cands: Vec<&Entry> = l.entries.iter().rev().filter(|e| !e.container && e.rect.contains(pos) && e.clip.contains(pos)).collect();
        // topmost first: higher layer order, then later drawn (stable over the reversed list)
        cands.sort_by(|a, b| b.order.cmp(&a.order));
        let mut out: Vec<Target> = Vec::new();
        for e in cands {
            let Some(i) = s.idx(e.node) else { continue };
            if s.core[i].root != e.root || !self.live_in(e.root, viewport) || hit_transparent(s, i) || exiting(s, i) {
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

    /// EV1: the target of a pointer event at `pos` in `viewport`: the topmost leaf or provider node
    /// as [`SceneRef::hit_test`] finds it, or the innermost container box drawn under the point
    /// when it is above every leaf there (a box's padding, an empty box over content). The same
    /// skips apply (freed, hidden, other viewport, `HIT_TRANSPARENT`, exiting). A provider part is
    /// reported as `Target::Virtual`.
    pub fn event_target(self, viewport: egui::ViewportId, pos: egui::Pos2) -> Option<Target> {
        let s = self.s;
        let l = s.hits.list(viewport)?;
        // topmost first: higher layer order, then later drawn (stable sort over the reversed list)
        let mut cands: Vec<&Entry> = l.entries.iter().rev().filter(|e| e.rect.contains(pos) && e.clip.contains(pos)).collect();
        cands.sort_by(|a, b| b.order.cmp(&a.order));
        let mut hits: Vec<(Target, egui::Order)> = Vec::new();
        for e in cands {
            let Some(i) = s.idx(e.node) else { continue };
            if s.core[i].root != e.root || !self.live_in(e.root, viewport) || hit_transparent(s, i) || exiting(s, i) {
                continue;
            }
            let t = if e.container {
                Target::Node(e.node)
            } else {
                match ask_provider(s, i, e, pos) {
                    Hit::Node => Target::Node(e.node),
                    Hit::Part(key) => Target::Virtual(VNodeId { host: e.node, key }),
                    Hit::Miss => continue,
                }
            };
            hits.push((t, e.order));
        }
        // a node recorded after its own children (a provider that draws its shadow children in its
        // `ui` is recorded when it returns) is under them: the innermost node of the top layer wins
        let (mut best, order) = *hits.first()?;
        while let Some(&(t, _)) = hits.iter().find(|(t, o)| *o == order && t.node() != best.node() && is_ancestor(s, best.node(), t.node())) {
            best = t;
        }
        Some(best)
    }

    /// DZ0: every node drawn under `pos` in `viewport`, containers included, topmost first and,
    /// within a layer, innermost first: what a designer's pick cycles through (a click selects the
    /// first, a modifier click walks out to its boxes). Unlike [`SceneRef::hit_test`] it lists
    /// container boxes and ignores `HIT_TRANSPARENT` (`pointer-events: none` hides a node from
    /// input, not from the tool that edits it); freed nodes, hidden roots, other viewports and
    /// exiting nodes are skipped as there, and a provider still answers for its own rect (a part is
    /// a `Target::Virtual`, `Hit::Miss` leaves the node out). Each target once. From the last drawn
    /// pass; O(entries of that viewport), for tools.
    pub fn hit_stack(self, viewport: egui::ViewportId, pos: egui::Pos2) -> Vec<Target> {
        let s = self.s;
        let Some(l) = s.hits.list(viewport) else { return Vec::new() };
        // a box is recorded before its children, so the reversed list has them first
        let mut cands: Vec<&Entry> = l.entries.iter().rev().filter(|e| e.rect.contains(pos) && e.clip.contains(pos)).collect();
        cands.sort_by(|a, b| b.order.cmp(&a.order));
        let mut out: Vec<(Target, egui::Order)> = Vec::new();
        for e in cands {
            let Some(i) = s.idx(e.node) else { continue };
            if s.core[i].root != e.root || !self.live_in(e.root, viewport) || exiting(s, i) {
                continue;
            }
            let t = if e.container {
                Target::Node(e.node)
            } else {
                match ask_provider(s, i, e, pos) {
                    Hit::Node => Target::Node(e.node),
                    Hit::Part(key) => Target::Virtual(VNodeId { host: e.node, key }),
                    Hit::Miss => continue,
                }
            };
            if !out.iter().any(|(x, _)| *x == t) {
                out.push((t, e.order));
            }
        }
        // a provider is recorded after the shadow children it draws, so it can come before them:
        // within a layer, a node goes before the first node already placed that is its ancestor
        let mut sorted: Vec<(Target, egui::Order)> = Vec::with_capacity(out.len());
        for (t, o) in out {
            match sorted.iter().position(|(x, xo)| *xo == o && x.node() != t.node() && is_ancestor(s, x.node(), t.node())) {
                Some(k) => sorted.insert(k, (t, o)),
                None => sorted.push((t, o)),
            }
        }
        sorted.into_iter().map(|(t, _)| t).collect()
    }

    /// EV1: where container `node` was drawn last pass in `viewport` (its layout rect on screen),
    /// from the hit list; for overlays (SV7) when the node has no widget rect of its own.
    pub fn box_rect(self, viewport: egui::ViewportId, node: NodeId) -> Option<egui::Rect> {
        let l = self.s.hits.list(viewport)?;
        l.entries.iter().find(|e| e.container && e.node == node).map(|e| e.rect)
    }

    /// AX1: where `node` showed in `viewport`'s last drawn pass: its rect and the part of it inside
    /// its clip. `None` when it was not drawn in that pass: below a culled container (scrolled out
    /// of a `scroll-area`), a virtual-list row out of view, an entry of a closed menu, a node that
    /// was never drawn. A node the walk does not record (an overlay's surface, a window) answers
    /// the rect it was last drawn at, unclipped. O(entries + depth · culled), for tools.
    pub fn shown(self, viewport: egui::ViewportId, node: NodeId) -> Option<Shown> {
        let s = self.s;
        let l = s.hits.list(viewport);
        let entry = l.and_then(|l| l.entries.iter().rev().find(|e| e.node == node && !e.container).or_else(|| l.entries.iter().find(|e| e.node == node)));
        if let Some(e) = entry {
            let v = e.rect.intersect(e.clip);
            return Some(Shown { rect: e.rect, visible: if v.is_positive() { v } else { egui::Rect::from_min_size(e.rect.min, egui::Vec2::ZERO) } });
        }
        let i = s.idx(node)?;
        // culled with an ancestor, or inside a closed overlay (one not drawn this pass)
        let culled = l.map_or(&[][..], |l| &l.culled[..]);
        let mut x = Some(node);
        while let Some(id) = x {
            let Some(k) = s.idx(id) else { break };
            if culled.contains(&id) || k != i && matches!(s.w(k), W::Overlay(_)) && s.core[k].drawn == egui::Rect::NOTHING {
                return None;
            }
            x = s.core[k].parent;
        }
        let r = s.core[i].drawn;
        (r != egui::Rect::NOTHING && r.is_positive()).then_some(Shown { rect: r, visible: r })
    }

    /// AX1: `node` was not drawn in `viewport`'s last pass because it or an ancestor was culled
    /// outside the clip (scrolled out of a `scroll-area`, a realised `virtual-list` row out of
    /// view): it is there, out of view, and `scroll-into-view` brings it in. False for a node that
    /// was drawn, or is hidden some other way (a closed menu or overlay).
    pub fn scrolled_out(self, viewport: egui::ViewportId, node: NodeId) -> bool {
        let s = self.s;
        let Some(l) = s.hits.list(viewport) else { return false };
        if l.culled.is_empty() || l.entries.iter().any(|e| e.node == node) {
            return false;
        }
        let mut x = Some(node);
        while let Some(id) = x {
            if l.culled.contains(&id) {
                return true;
            }
            x = s.idx(id).and_then(|k| s.core[k].parent);
        }
        false
    }

    /// Root `r` is shown and, for a slot root, drawn in `vp` now (R20).
    fn live_in(self, r: RootIx, vp: egui::ViewportId) -> bool {
        match self.s.roots.get(r) {
            Some(e) => e.visible && (e.kind.slot_node().is_none() || e.viewport == vp),
            None => false,
        }
    }
}

/// Whether `a` is a strict ancestor of `n`.
fn is_ancestor(s: &Scene, a: NodeId, n: NodeId) -> bool {
    let mut x = s.idx(n).and_then(|i| s.core[i].parent);
    while let Some(p) = x {
        if p == a {
            return true;
        }
        x = s.idx(p).and_then(|i| s.core[i].parent);
    }
    false
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

/// TR2: node `i` or an ancestor is inert, i.e. an exiting node (its outro runs): not pickable.
fn exiting(s: &Scene, i: usize) -> bool {
    let mut x = Some(i);
    while let Some(k) = x {
        if s.core[k].flags.contains(NodeFlags::HAS_VISUAL) && s.vis(k).inert {
            return true;
        }
        x = s.core[k].parent.and_then(|p| s.idx(p));
    }
    false
}

/// Asks a `Caps::HIT_TEST` provider kind (or a pane's `PaneHit`) about `pos`; every other node is
/// hit as itself.
fn ask_provider(s: &Scene, i: usize, e: &Entry, pos: egui::Pos2) -> Hit {
    if let Some(p) = s.panes.get(e.node) {
        // borrowed (the host is changing it): the pane as a whole
        let Ok(p) = p.try_borrow() else { return Hit::Node };
        return p.hit_test(&HitCx { node: e.node, rect: e.rect, pos }, (pos - e.rect.min).to_pos2());
    }
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

impl<'a> SceneRef<'a> {
    /// SV49: part `key` of pane `pane`: its rect on screen (relative to the pane's drawn rect,
    /// offset), if the pane is drawn and its `PaneHit` has one.
    pub(crate) fn pane_part_rect(self, pane: NodeId, key: u64) -> Option<Option<egui::Rect>> {
        let p = self.s.panes.get(pane)?;
        let drawn = self.get(pane)?.drawn();
        Some(drawn.and_then(|d| p.try_borrow().ok()?.part_rect(key).map(|r| r.translate(d.min.to_vec2()))))
    }

    /// SV49: part `key` of pane `pane`: `"<pane key>/<name>"` if its `PaneHit` names it.
    pub(crate) fn pane_part_anchor(self, pane: NodeId, key: u64) -> Option<Option<String>> {
        let (k, p) = self.s.panes.by_node.get(&pane)?;
        Some(p.try_borrow().ok().and_then(|p| p.part_anchor(key)).map(|a| format!("{k}/{a}")))
    }
}

impl NodeRef<'_> {
    /// AX1: where this node showed in its root's viewport last pass ([`SceneRef::shown`]).
    pub fn shown(self) -> Option<Shown> {
        let sr = SceneRef { s: self.s };
        sr.shown(sr.root_viewport(self.root()).unwrap_or(egui::ViewportId::ROOT), self.id())
    }

    /// AX1: visible to a user, as automation's `visible` means it (Playwright's sense): shown
    /// ([`NodeRef::is_visible`]: its props, stack page and root) and either drawn last pass with a
    /// non-empty rect, or only scrolled out of view ([`SceneRef::scrolled_out`]; an act scrolls it
    /// in). Not visible: an entry of a closed menu, the content of a closed overlay, a node not
    /// drawn yet. A root's top (a window) needs no drawn rect.
    pub fn is_shown(self) -> bool {
        if !self.is_visible() {
            return false;
        }
        if self.s.core[self.i].flags.contains(NodeFlags::ROOT) || matches!(self.s.w(self.i), W::Window { .. }) {
            return true;
        }
        match self.shown() {
            Some(sh) => sh.rect.area() > 0.0,
            None => {
                let sr = SceneRef { s: self.s };
                sr.scrolled_out(sr.root_viewport(self.root()).unwrap_or(egui::ViewportId::ROOT), self.id())
            }
        }
    }
}
