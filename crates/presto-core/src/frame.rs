//! The UI phases of one frame for one window (architecture §2, phase 3): style, layout, prepare,
//! record, property trees, layers, hit list, accessibility, and the [`Commit`] delta.
//!
//! Two walks after layout:
//! - **The dirty walk** visits only nodes with record work (`PAINT`, `PROPERTY`, `A11Y`, `CHILDREN`)
//!   and their ancestors' paths (`DESC_RECORD`). It prepares and re-records content-dirty nodes and
//!   refreshes the property-tree entries of property-dirty ones. A layout move is a transform-node
//!   update, never a re-record.
//! - **The rebuild walk** runs only when structure, layout or properties changed (`rebuild`): it
//!   computes world transforms and clips, assigns layers (§6.3), lists each layer's items in paint
//!   order and rebuilds the hit list input uses until the next one.
//!
//! Every node in a window has a transform node (the display list's space: its layout offset and CSS
//! transform). Clip, effect and scroll nodes exist only for nodes that clip, are translucent or
//! animate opacity, or scroll.

use crate::cx::{ImageSink, PrepareCx, RecordCx, RunSink};
use crate::document::Document;
use crate::hit::{HitEntry, HitList};
use crate::tree::Dirty;
use presto_base::anim::{AnimProperty, AnimationCmd};
use presto_base::commit::{Commit, ResourceDelta};
use presto_base::display::DisplayList;
use presto_base::geom::{Affine, Point, Rect, RoundedRect, Size, Vec2};
use presto_base::ids::{LayerId, NodeId, PropNodeId, WindowId};
use presto_base::layer::{ItemRef, LayerDelta, LayerDesc, LayerKind};
use presto_base::props::{BlendMode, ClipNode, ClipShape, EffectNode, OverscrollBehavior, PropOp, ScrollNode, TransformNode};
use presto_base::time::Time;
use std::sync::Arc;

/// Counters of one frame (tests and the inspector read them).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FrameStats {
    pub layout: crate::layout::LayoutRun,
    /// Nodes whose display list was re-recorded.
    pub recorded: u64,
    /// Nodes whose property-tree entries were refreshed.
    pub props: u64,
    /// The rebuild walk ran (layers, items, hit list).
    pub rebuilt: bool,
}

/// What the walks pass down to children.
#[derive(Clone, Copy)]
struct Ctx {
    /// The transform space children are placed in (the parent's content or own transform).
    space: Option<PropNodeId>,
    clip: Option<PropNodeId>,
    effect: Option<PropNodeId>,
}

fn rounded_inner(r: Rect, c: presto_base::geom::Corners, b: presto_base::geom::Insets) -> RoundedRect {
    let inner = r.inset(b);
    let k = |rad: f32, a: f32, bb: f32| (rad - a.max(bb)).max(0.0);
    RoundedRect {
        rect: inner,
        radii: presto_base::geom::Corners { tl: k(c.tl, b.left, b.top), tr: k(c.tr, b.right, b.top), br: k(c.br, b.right, b.bottom), bl: k(c.bl, b.left, b.bottom) },
    }
}

impl Document {
    /// Runs the UI phases for `window` for a frame presenting at `target` and returns the commit, or
    /// `None` when nothing changed. `target` is the frame's presentation time (UI-side animations
    /// sample at it).
    pub fn frame(&mut self, window: WindowId, target: Time) -> Option<Commit> {
        self.frame_with_stats(window, target).0
    }

    /// [`Document::frame`] with its counters.
    pub fn frame_with_stats(&mut self, window: WindowId, _target: Time) -> (Option<Commit>, FrameStats) {
        let mut stats = FrameStats::default();
        let Some(root) = self.window_root(window) else { return (None, stats) };
        if !self.contains(root) {
            return (None, stats);
        }
        self.restyle(window);
        stats.layout = self.run_layout(window);
        let mut commit = Commit { window, revision: self.revision, input_seq: self.input_seq, ..Default::default() };
        let mut runs = RunSink { next: self.run_counter, added: Vec::new() };
        let mut images = std::mem::take(&mut self.images);
        let mut a11y_dirty = Vec::new();
        self.ensure_root_props(window, &mut commit);
        self.dirty_walk(window, root, &mut commit, &mut runs, &mut images, &mut a11y_dirty, &mut stats);
        self.run_counter = runs.next;
        commit.runs = runs.added;
        commit.resources = ResourceDelta { images: std::mem::take(&mut images.added), removed_images: std::mem::take(&mut images.removed), ..Default::default() };
        self.images = images;
        let rebuild = self.window(window).is_some_and(|w| w.rebuild);
        if rebuild {
            self.rebuild_walk(window, root, &mut commit);
            stats.rebuilt = true;
        }
        commit.a11y = self.a11y_update(window, root, rebuild, &a11y_dirty);
        // what left the window since the last frame
        let orphaned = std::mem::take(&mut self.anim.orphaned);
        let ws = self.window_mut(window).expect("window");
        let p = std::mem::take(&mut ws.pending);
        commit.removed = p.removed_lists;
        commit.dropped_runs = p.dropped_runs;
        for id in p.removed_props {
            commit.props.ops.push(PropOp::Remove(id));
        }
        for l in p.removed_layers {
            commit.layers.push(LayerDelta::Remove(l));
        }
        commit.animations = p.animations;
        commit.animations.extend(orphaned.into_iter().map(AnimationCmd::Cancel));
        self.bind_animation_targets(&mut commit);
        commit.cursor = self.input.cursor.take();
        let empty = commit.layers.is_empty()
            && commit.props.ops.is_empty()
            && commit.lists.is_empty()
            && commit.removed.is_empty()
            && commit.runs.is_empty()
            && commit.dropped_runs.is_empty()
            && commit.resources.images.is_empty()
            && commit.resources.removed_images.is_empty()
            && commit.animations.is_empty()
            && commit.a11y.is_none()
            && commit.cursor.is_none();
        if empty {
            return (None, stats);
        }
        let ws = self.window_mut(window).expect("window");
        ws.built = Some(commit.revision);
        (Some(commit), stats)
    }

    /// The compositor applied the commit with `revision` (back-pressure release; `settle`).
    pub fn commit_applied(&mut self, window: WindowId, revision: presto_base::ids::Revision) {
        if let Some(w) = self.window_mut(window) {
            w.applied = Some(w.applied.map_or(revision, |a| a.max(revision)));
        }
    }

    /// The compositor reported an animation event: finished or aborted animations stop counting as
    /// running (they kept their node's layer and effect node while they ran).
    pub fn animation_event(&mut self, ev: presto_base::anim::AnimationEvent) {
        use presto_base::anim::AnimationEvent as E;
        let id = match ev {
            E::Started { .. } => return,
            E::Finished { id, .. } | E::Aborted { id } => id,
        };
        let mut owner = None;
        for (n, v) in self.anim.running.iter_mut() {
            if let Some(k) = v.iter().position(|(a, _)| *a == id) {
                v.remove(k);
                owner = Some(*n);
                break;
            }
        }
        if let Some(n) = owner {
            if self.anim.running.get(&n).is_some_and(Vec::is_empty) {
                self.anim.running.remove(&n);
            }
            self.mark(n, Dirty::PROPERTY);
            if let Some(w) = self.window_of(n).and_then(|w| self.window_mut(w)) {
                w.rebuild = true;
            }
        }
    }

    /// Animations started this frame drive their node's effect or transform node.
    fn bind_animation_targets(&self, commit: &mut Commit) {
        for cmd in &mut commit.animations {
            if let AnimationCmd::Start(spec) = cmd
                && let Some(i) = self.idx(spec.node)
            {
                let p = &self.record[i];
                spec.target = match spec.property {
                    AnimProperty::Opacity => p.effect,
                    AnimProperty::Transform => p.transform,
                    AnimProperty::Clip => p.clip,
                    AnimProperty::ScrollOffset => p.scroll,
                    _ => None,
                };
            }
        }
    }

    fn ensure_root_props(&mut self, window: WindowId, commit: &mut Commit) {
        let ws = self.window_mut(window).expect("window");
        if ws.root_transform.is_none() {
            let id = PropNodeId(ws.props.alloc());
            ws.root_transform = Some(id);
            commit.props.ops.push(PropOp::Transform(id, TransformNode { parent: None, local: Affine::IDENTITY, scroll: None }));
        }
    }

    /// Node `i` animates a compositor property now.
    fn animates(&self, node: NodeId, prop: AnimProperty) -> bool {
        self.anim.running.get(&node).is_some_and(|v| v.iter().any(|&(_, p)| p == prop))
    }

    /// The local transform of node `i` relative to its transform parent: layout offset, then the CSS
    /// transform about its origin.
    fn local_transform(&self, i: usize) -> Affine {
        let l = &self.layout[i].layout;
        let s = self.style.computed(i);
        let offset = Affine::translate(l.location.x, l.location.y);
        if s.transform == Affine::IDENTITY {
            return offset;
        }
        let o = Point::new(s.transform_origin.x * l.size.width, s.transform_origin.y * l.size.height);
        offset.then(&Affine::translate(o.x, o.y)).then(&s.transform).then(&Affine::translate(-o.x, -o.y))
    }

    fn dirty_walk(
        &mut self,
        window: WindowId,
        root: NodeId,
        commit: &mut Commit,
        runs: &mut RunSink,
        images: &mut ImageSink,
        a11y: &mut Vec<NodeId>,
        stats: &mut crate::frame::FrameStats,
    ) {
        let root_space = self.window(window).and_then(|w| w.root_transform);
        let ri = self.ix(root);
        let mut stack = vec![(ri, Ctx { space: root_space, clip: None, effect: None }, false)];
        while let Some((i, ctx, force)) = stack.pop() {
            let d = self.core[i].dirty;
            if !force && !d.intersects(Dirty::RECORD.union(Dirty::DESC_RECORD)) {
                continue;
            }
            let node = self.slots.id_at(i).expect("live");
            let new = self.record[i].transform.is_none();
            let mut force_children = force;
            if new || force || d.contains(Dirty::PROPERTY) {
                force_children |= self.refresh_props(window, i, ctx, commit);
                stats.props += 1;
            }
            if new || d.contains(Dirty::PAINT) {
                self.record_node(i, commit, runs, images);
                stats.recorded += 1;
            }
            if new || d.intersects(Dirty::A11Y | Dirty::CHILDREN) {
                a11y.push(node);
            }
            if d.contains(Dirty::CHILDREN) || new {
                if let Some(w) = self.window_mut(window) {
                    w.rebuild = true;
                }
            }
            self.core[i].dirty.remove(Dirty::RECORD.union(Dirty::DESC_RECORD).union(Dirty::LAYOUT).union(Dirty::DESC_LAYOUT));
            let p = &self.record[i];
            let child = Ctx { space: p.content.or(p.transform), clip: p.clip.or(ctx.clip), effect: p.effect.or(ctx.effect) };
            for &c in self.children[i].iter().rev() {
                let ci = self.ix(c);
                // absolute boxes are placed in their containing block's space
                let cctx = match crate::layout::positional_parent(self, ci) {
                    Some(pp) if pp != i => {
                        let q = &self.record[pp];
                        Ctx { space: q.content.or(q.transform), ..child }
                    }
                    _ => child,
                };
                stack.push((ci, cctx, force_children));
            }
        }
    }

    /// Refreshes node `i`'s property-tree entries; true when the set of nodes it owns changed (its
    /// descendants' parent links change with it).
    fn refresh_props(&mut self, window: WindowId, i: usize, ctx: Ctx, commit: &mut Commit) -> bool {
        let node = self.slots.id_at(i).expect("live");
        let s = self.style.computed(i).clone();
        let size = {
            let l = &self.layout[i].layout;
            Size::new(l.size.width, l.size.height)
        };
        let local = self.local_transform(i);
        let scroll_view = self.scroll_viewport(i);
        let content = self.scroll_content(i);
        let offset = self.scroll_offset(node);
        let wants_clip = s.overflow_x.clips() || s.overflow_y.clips();
        let wants_effect = s.opacity < 1.0 || s.will_change || self.animates(node, AnimProperty::Opacity);
        let passive = !self.nonpassive_scroll_path(i);
        let mut ids = std::mem::take(&mut self.window_mut(window).expect("window").props);
        let mut scroll_nodes: Option<(PropNodeId, Option<PropNodeId>)> = None;
        let p = &mut self.record[i];
        let before = (p.clip.is_some(), p.effect.is_some(), p.scroll.is_some());
        let mut alloc = |slot: &mut Option<PropNodeId>, want: bool, ops: &mut Vec<PropOp>| -> Option<PropNodeId> {
            match (*slot, want) {
                (None, true) => *slot = Some(PropNodeId(ids.alloc())),
                (Some(id), false) => {
                    ops.push(PropOp::Remove(id));
                    ids.free(id.0);
                    *slot = None;
                }
                _ => {}
            }
            *slot
        };
        let ops = &mut commit.props.ops;
        let t = alloc(&mut p.transform, true, ops).expect("allocated");
        ops.push(PropOp::Transform(t, TransformNode { parent: ctx.space, local, scroll: None }));
        let bounds = Rect::from_origin_size(Point::ZERO, size);
        if let Some(c) = alloc(&mut p.clip, wants_clip, ops) {
            let rr = rounded_inner(bounds, s.radii, s.border_widths);
            let shape = if rr.radii.is_zero() { ClipShape::Rect(rr.rect) } else { ClipShape::Rounded(rr) };
            ops.push(PropOp::Clip(c, ClipNode { parent: ctx.clip, space: t, shape }));
        }
        if let Some(e) = alloc(&mut p.effect, wants_effect, ops) {
            ops.push(PropOp::Effect(e, EffectNode { parent: ctx.effect, opacity: s.opacity, blend: BlendMode::Normal }));
        }
        let had_scroll = p.scroll;
        let sc = alloc(&mut p.scroll, scroll_view.is_some(), ops);
        let ct = alloc(&mut p.content, scroll_view.is_some(), ops);
        if let (Some(sc), Some(ct), Some(view)) = (sc, ct, scroll_view) {
            let b = s.border_widths;
            let viewport = Rect::new(b.left, b.top, b.left + view.w, b.top + view.h);
            ops.push(PropOp::Scroll(
                sc,
                ScrollNode {
                    space: t,
                    offset,
                    content,
                    viewport,
                    scroll_x: s.overflow_x.scrolls(),
                    scroll_y: s.overflow_y.scrolls(),
                    overscroll: OverscrollBehavior::None,
                    passive,
                },
            ));
            // children are laid out from the border box origin: the content space is the node's
            // own, translated by the scroll offset
            ops.push(PropOp::Transform(ct, TransformNode { parent: Some(t), local: Affine::IDENTITY, scroll: Some(sc) }));
            scroll_nodes = Some((sc, had_scroll));
        } else if let Some(old) = had_scroll {
            scroll_nodes = Some((old, None));
        }
        let after = (p.clip.is_some(), p.effect.is_some(), p.scroll.is_some());
        let ws = self.window_mut(window).expect("window");
        ws.props = ids;
        match scroll_nodes {
            Some((sc, _)) if after.2 => {
                ws.scroll_nodes.insert(sc, node);
            }
            Some((old, None)) => {
                ws.scroll_nodes.remove(&old);
            }
            _ => {}
        }
        if before != after {
            ws.rebuild = true;
        }
        before != after
    }

    /// A listener that is not passive on node `i` or an ancestor makes scrolling here wait for JS.
    fn nonpassive_scroll_path(&self, mut i: usize) -> bool {
        loop {
            if self.core[i].flags.contains(crate::tree::NodeFlags::NONPASSIVE) {
                return true;
            }
            match self.core[i].parent {
                Some(p) => i = self.ix(p),
                None => return false,
            }
        }
    }

    fn record_node(&mut self, i: usize, commit: &mut Commit, runs: &mut RunSink, images: &mut ImageSink) {
        let Some(mut k) = self.kinds[i].take() else { return };
        let node = self.slots.id_at(i).expect("live");
        let mut shaper = self.shaper.take();
        let request = {
            let mut cx = PrepareCx { node: self.view_at(i), shaper: shaper.as_deref_mut(), images, request: Dirty::EMPTY };
            k.prepare(&mut cx);
            cx.request
        };
        let mut list = DisplayList::new();
        let mut node_runs = Vec::new();
        let hidden = self.style.computed(i).visibility_hidden;
        if !hidden {
            let mut cx = RecordCx { node: self.view_at(i), list: &mut list, runs, node_runs: &mut node_runs, shaper: shaper.as_deref_mut() };
            k.record(&mut cx);
        }
        self.shaper = shaper;
        self.kinds[i] = Some(k);
        let _ = request;
        let p = &mut self.record[i];
        let old_runs = std::mem::replace(&mut p.runs, node_runs);
        commit.dropped_runs.extend(old_runs);
        let had = p.list.is_some();
        if list.is_empty() {
            if had {
                p.list = None;
                commit.removed.push(node);
            }
        } else {
            let list = Arc::new(list);
            p.list = Some(list.clone());
            commit.lists.push((node, list));
        }
        if had != self.record[i].list.is_some()
            && let Some(w) = self.core[i].window.and_then(|w| self.window_mut(w))
        {
            w.rebuild = true;
        }
    }

    /// The node's own layer, when it needs one (§6.3): scroll containers (their content), and
    /// subtrees with `will-change` or a running compositor animation.
    fn layer_kind(&self, i: usize, node: NodeId) -> Option<LayerKind> {
        if self.record[i].scroll.is_some() {
            return Some(LayerKind::Scroll { tiles: false });
        }
        let s = self.style.computed(i);
        if s.will_change || self.animates(node, AnimProperty::Opacity) || self.animates(node, AnimProperty::Transform) {
            return Some(LayerKind::Content);
        }
        None
    }

    /// Children of node `i` in paint order: tree order, stably sorted by `z-index`.
    fn paint_order(&self, i: usize) -> Vec<NodeId> {
        let mut kids = self.children[i].clone();
        if kids.iter().any(|&c| self.style.computed(self.ix(c)).z_index.is_some()) {
            kids.sort_by_key(|&c| self.style.computed(self.ix(c)).z_index.unwrap_or(0));
        }
        kids
    }

    /// Layers, their items, world transforms and the hit list.
    fn rebuild_walk(&mut self, window: WindowId, root: NodeId, commit: &mut Commit) {
        let ws = self.window_mut(window).expect("window");
        ws.rebuild = false;
        let root_t = ws.root_transform.expect("root transform");
        let size = ws.size;
        let root_layer = match ws.root_layer {
            Some(l) => l,
            None => {
                let l = LayerId(ws.layer_ids.alloc());
                ws.root_layer = Some(l);
                l
            }
        };
        let mut layers: Vec<LayerDesc> = vec![LayerDesc {
            id: root_layer,
            parent: None,
            kind: LayerKind::Root,
            transform: root_t,
            clip: None,
            effect: None,
            z: 0,
            bounds: Rect::from_origin_size(Point::ZERO, size),
            opaque: false,
        }];
        let mut items: std::collections::HashMap<LayerId, Vec<ItemRef>> = Default::default();
        items.insert(root_layer, Vec::new());
        let mut hits = Vec::new();
        // (slot, world transform of the space its children are placed in, world clip, layer, ctx)
        struct Frame {
            i: usize,
            parent_world: Affine,
            clip_world: Option<Rect>,
            layer: LayerId,
            clip: Option<PropNodeId>,
            effect: Option<PropNodeId>,
        }
        let ri = self.ix(root);
        let mut stack = vec![Frame { i: ri, parent_world: Affine::IDENTITY, clip_world: None, layer: root_layer, clip: None, effect: None }];
        let mut z_next = 1;
        let mut used_layers = vec![root_layer];
        while let Some(f) = stack.pop() {
            let i = f.i;
            let node = self.slots.id_at(i).expect("live");
            let s = self.style.nodes[i].computed.clone();
            let hidden = self.layout[i].style.display == taffy::Display::None;
            if hidden {
                continue;
            }
            let local = self.local_transform(i);
            let world = f.parent_world.then(&local);
            let l = self.layout[i].layout;
            let bounds = Rect::new(0.0, 0.0, l.size.width, l.size.height);
            let p = self.record[i].clone();
            let own_effect = p.effect.or(f.effect);
            // a new layer for this node's subtree (its own box is in it, except a scroll
            // container's, whose box stays below its scrolled content)
            let mut layer = f.layer;
            let kind = self.layer_kind(i, node);
            let mut content_layer = None;
            if let Some(kind) = kind {
                let id = match p.layer {
                    Some(l) => l,
                    None => {
                        let ws = self.window_mut(window).expect("window");
                        let l = LayerId(ws.layer_ids.alloc());
                        self.record[i].layer = Some(l);
                        l
                    }
                };
                used_layers.push(id);
                let (transform, clip, lbounds) = match kind {
                    LayerKind::Scroll { .. } => {
                        let c = self.scroll_content(i);
                        (p.content.expect("scroll content node"), p.clip, Rect::from_origin_size(Point::ZERO, c))
                    }
                    _ => (p.transform.expect("transform node"), f.clip, bounds),
                };
                layers.push(LayerDesc { id, parent: Some(f.layer), kind, transform, clip, effect: own_effect, z: z_next, bounds: lbounds, opaque: false });
                z_next += 1;
                items.entry(id).or_default();
                if matches!(kind, LayerKind::Scroll { .. }) {
                    content_layer = Some(id);
                } else {
                    layer = id;
                }
            } else if let Some(old) = p.layer {
                // it had a layer and needs none now
                self.record[i].layer = None;
                let ws = self.window_mut(window).expect("window");
                ws.layer_ids.free(old.0);
            }
            let drawn = !s.visibility_hidden;
            if drawn && p.list.is_some() {
                items.entry(layer).or_default().push(ItemRef { node, transform: p.transform.expect("transform"), clip: f.clip, effect: own_effect });
            }
            let world_rect = world.apply_rect(&bounds);
            if drawn && !s.pointer_events_none && let Some(inverse) = world.inverse() {
                hits.push(HitEntry { node, inverse, world: world_rect, clip: f.clip_world });
            }
            self.record[i].world = world_rect;
            // what children see
            let mut child_world = world;
            if let Some(_sc) = p.scroll {
                let off = self.scroll_offset(node);
                child_world = world.then(&Affine::translate(-off.x, -off.y));
            }
            let child_clip_world = if p.clip.is_some() {
                let b = s.border_widths;
                let inner = world.apply_rect(&bounds.inset(b));
                Some(f.clip_world.map_or(inner, |c| c.intersect(&inner)))
            } else {
                f.clip_world
            };
            let child = Frame {
                i: 0,
                parent_world: child_world,
                clip_world: child_clip_world,
                layer: content_layer.unwrap_or(layer),
                clip: p.clip.or(f.clip),
                effect: own_effect,
            };
            for c in self.paint_order(i).into_iter().rev() {
                let ci = self.ix(c);
                stack.push(Frame { i: ci, ..child });
            }
        }
        // diff the layers and items against what was sent
        let ws = self.window_mut(window).expect("window");
        for desc in &layers {
            match ws.layers.get(&desc.id) {
                None => commit.layers.push(LayerDelta::Add(*desc)),
                Some(old) if old != desc => commit.layers.push(LayerDelta::Update(*desc)),
                _ => {}
            }
        }
        let gone: Vec<LayerId> = ws.layers.keys().copied().filter(|l| !used_layers.contains(l)).collect();
        for l in gone {
            commit.layers.push(LayerDelta::Remove(l));
            ws.layers.remove(&l);
            ws.items.remove(&l);
        }
        for desc in layers {
            ws.layers.insert(desc.id, desc);
        }
        for (l, list) in items {
            if ws.items.get(&l) != Some(&list) {
                commit.layers.push(LayerDelta::Items { layer: l, items: list.clone() });
                ws.items.insert(l, list);
            }
        }
        ws.hits = Arc::new(HitList { entries: hits });
    }

    /// The last frame's hit list of `window` (for the compositor's scroll hit tests and tests).
    pub fn hit_list(&self, window: WindowId) -> Option<Arc<HitList>> {
        self.window(window).map(|w| w.hits.clone())
    }

    /// A node's border box in window space, from the last rebuild.
    pub fn world_rect(&self, node: NodeId) -> Option<Rect> {
        self.idx(node).map(|i| self.record[i].world)
    }

    /// The scroll offset delta a `scrollIntoView` of `node` needs in each scroll ancestor.
    pub fn scroll_into_view(&mut self, node: NodeId) {
        let Some(mut i) = self.idx(node) else { return };
        let target = self.record[i].world;
        while let Some(p) = self.core[i].parent {
            let pi = self.ix(p);
            if let Some(view) = self.scroll_viewport(pi) {
                let b = self.style.computed(pi).border_widths;
                let w = self.record[pi].world;
                let vis = Rect::new(w.min.x + b.left, w.min.y + b.top, w.min.x + b.left + view.w, w.min.y + b.top + view.h);
                let mut d = Vec2::ZERO;
                if target.min.y < vis.min.y {
                    d.y = target.min.y - vis.min.y;
                } else if target.max.y > vis.max.y {
                    d.y = (target.max.y - vis.max.y).min(target.min.y - vis.min.y);
                }
                if target.min.x < vis.min.x {
                    d.x = target.min.x - vis.min.x;
                } else if target.max.x > vis.max.x {
                    d.x = (target.max.x - vis.max.x).min(target.min.x - vis.min.x);
                }
                if d != Vec2::ZERO {
                    let off = self.scroll_offset(p);
                    self.set_scroll_offset(p, off + d);
                }
            }
            i = pi;
        }
    }
}
