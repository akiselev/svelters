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
use skelter_base::anim::{AnimProperty, AnimationCmd};
use skelter_base::commit::{Commit, ResourceDelta};
use skelter_base::display::DisplayList;
use skelter_base::geom::{Affine, Point, Rect, RoundedRect, Size, Vec2};
use skelter_base::ids::{LayerId, NodeId, PropNodeId, WindowId};
use skelter_base::layer::{ItemRef, LayerDelta, LayerDesc, LayerKind};
use skelter_base::props::{BlendMode, ClipNode, ClipShape, EffectNode, OverscrollBehavior, PropOp, ScrollNode, TransformNode};
use skelter_base::time::Time;
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

fn rounded_inner(r: Rect, c: skelter_base::geom::Corners, b: skelter_base::geom::Insets) -> RoundedRect {
    let inner = r.inset(b);
    let k = |rad: f32, a: f32, bb: f32| (rad - a.max(bb)).max(0.0);
    RoundedRect {
        rect: inner,
        radii: skelter_base::geom::Corners { tl: k(c.tl, b.left, b.top), tr: k(c.tr, b.right, b.top), br: k(c.br, b.right, b.bottom), bl: k(c.bl, b.left, b.bottom) },
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
    pub fn frame_with_stats(&mut self, window: WindowId, target: Time) -> (Option<Commit>, FrameStats) {
        let mut stats = FrameStats::default();
        let Some(root) = self.window_root(window) else { return (None, stats) };
        if !self.contains(root) {
            return (None, stats);
        }
        self.restyle(window);
        self.sample_ui_transitions(window, target);
        // virtualising nodes realise their rows before layout, and again when layout moved their
        // viewport (new rows are styled before they are laid out)
        if self.drive_virtual(window) {
            self.restyle(window);
            self.drive_virtual(window);
        }
        self.keep_rows_parked(window);
        stats.layout = self.run_layout(window);
        if self.drive_virtual(window) {
            self.restyle(window);
            self.drive_virtual(window);
            self.keep_rows_parked(window);
            let again = self.run_layout(window);
            stats.layout.boundaries += again.boundaries;
            stats.layout.full |= again.full;
            stats.layout.leaf_layouts += again.leaf_layouts;
        }
        let mut commit = Commit { window, revision: self.revision, input_seq: self.input_seq, ..Default::default() };
        let mut runs = RunSink { next: self.run_counter, added: Vec::new() };
        let mut images = std::mem::take(&mut self.images);
        let mut a11y_dirty = Vec::new();
        self.ensure_root_props(window, &mut commit);
        self.dirty_walk(window, root, &mut commit, &mut runs, &mut images, &mut a11y_dirty, &mut stats);
        self.run_counter = runs.next;
        commit.runs = runs.added;
        commit.resources = ResourceDelta {
            images: std::mem::take(&mut images.added),
            removed_images: std::mem::take(&mut images.removed),
            paths: std::mem::take(&mut images.paths),
            removed_paths: std::mem::take(&mut images.removed_paths),
        };
        self.images = images;
        let rebuild = self.window(window).is_some_and(|w| w.rebuild);
        if rebuild {
            self.rebuild_walk(window, root, &mut commit);
            stats.rebuilt = true;
        }
        if rebuild {
            a11y_dirty.extend(std::mem::take(&mut self.window_mut(window).expect("window").a11y_moved));
        }
        commit.a11y = self.a11y_update(window, root, &a11y_dirty);
        // what left the window since the last frame
        let orphaned = std::mem::take(&mut self.anim.orphaned);
        let ws = self.window_mut(window).expect("window");
        let p = std::mem::take(&mut ws.pending);
        commit.removed.extend(p.removed_lists);
        commit.dropped_runs.extend(p.dropped_runs);
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
        commit.ime_area = self.ime_area(window);
        let empty = commit.layers.is_empty()
            && commit.props.ops.is_empty()
            && commit.lists.is_empty()
            && commit.removed.is_empty()
            && commit.runs.is_empty()
            && commit.dropped_runs.is_empty()
            && commit.resources.images.is_empty()
            && commit.resources.removed_images.is_empty()
            && commit.resources.paths.is_empty()
            && commit.resources.removed_paths.is_empty()
            && commit.animations.is_empty()
            && commit.a11y.is_none()
            && commit.cursor.is_none()
            && commit.ime_area.is_none();
        if empty {
            return (None, stats);
        }
        let ws = self.window_mut(window).expect("window");
        ws.built = Some(commit.revision);
        (Some(commit), stats)
    }

    /// The IME candidate area when it moved: the focused text kind's composition (or selection)
    /// bounds, else its box. `None` when unchanged or nothing editable has focus.
    fn ime_area(&mut self, window: WindowId) -> Option<Rect> {
        let node = self.focused(window)?;
        let i = self.idx(node)?;
        let mut k = self.kinds[i].take()?;
        let r = k.text_input().map(|c| c.marked_range().or_else(|| c.selected_range()).and_then(|r| c.bounds_for_range(r)));
        self.kinds[i] = Some(k);
        let r = r?.unwrap_or(self.record[i].world);
        if self.window(window).and_then(|w| w.ime_sent) == Some(r) {
            return None;
        }
        self.window_mut(window)?.ime_sent = Some(r);
        Some(r)
    }

    /// The compositor applied the commit with `revision` (back-pressure release; `settle`).
    pub fn commit_applied(&mut self, window: WindowId, revision: skelter_base::ids::Revision) {
        if let Some(w) = self.window_mut(window) {
            w.applied = Some(w.applied.map_or(revision, |a| a.max(revision)));
        }
    }

    /// The compositor reported an animation event: finished or aborted animations stop counting as
    /// running (they kept their node's layer and effect node while they ran).
    pub fn animation_event(&mut self, ev: skelter_base::anim::AnimationEvent) {
        use skelter_base::anim::AnimationEvent as E;
        let id = match ev {
            E::Started { .. } => return,
            E::Finished { id, .. } | E::Aborted { id } => id,
        };
        let mut owner = None;
        let mut finished_prop = None;
        for (n, v) in self.anim.running.iter_mut() {
            if let Some(k) = v.iter().position(|(a, _)| *a == id) {
                let (_, p) = v.remove(k);
                finished_prop = match p {
                    AnimProperty::Opacity => Some("opacity"),
                    AnimProperty::Transform => Some("transform"),
                    AnimProperty::Palette(0) => Some("color"),
                    _ => None,
                };
                owner = Some(*n);
                break;
            }
        }
        if let Some(n) = owner {
            // CSS transitions of compositor properties end on the compositor
            if let E::Finished { .. } = ev
                && let Some(prop) = finished_prop
            {
                crate::style::cascade::transition_end(self, n, prop, 0.0);
            }
            if self.anim.running.get(&n).is_some_and(Vec::is_empty) {
                self.anim.running.remove(&n);
            }
            self.mark(n, Dirty::PROPERTY);
            if let Some(w) = self.window_of(n).and_then(|w| self.window_mut(w)) {
                w.rebuild = true;
            }
        }
    }

    /// Starts a compositor animation of `property` on `node`, driven by `generator` (svelters-skelter's
    /// `OP_ANIMATE`: Svelte's native transitions and `flip`, skelter-motion's plans). `None` when the
    /// node is in no window. It is cancelled with the node, keeps the node's property nodes while it
    /// runs, and ends with an `AnimationEvent` like a CSS transition's.
    pub fn start_animation(&mut self, node: NodeId, property: AnimProperty, generator: skelter_base::anim::Generator) -> Option<skelter_base::ids::AnimationId> {
        let window = self.window_of(node)?;
        let id = self.anim.alloc();
        self.anim.running.entry(node).or_default().push((id, property));
        let spec = skelter_base::anim::AnimationSpec { id, node, target: None, property, generator, start: skelter_base::anim::StartTime::NextFrame };
        if let Some(ws) = self.window_mut(window) {
            ws.pending.animations.push(AnimationCmd::Start(spec));
            ws.rebuild = true;
        }
        self.mark(node, Dirty::PROPERTY);
        Some(id)
    }

    /// Cancels an animation [`Document::start_animation`] started (no-op once it ended).
    pub fn cancel_animation(&mut self, id: skelter_base::ids::AnimationId) {
        let owner = self.anim.running.iter_mut().find_map(|(n, v)| {
            let k = v.iter().position(|(a, _)| *a == id)?;
            v.remove(k);
            Some(*n)
        });
        let Some(n) = owner else { return };
        if self.anim.running.get(&n).is_some_and(Vec::is_empty) {
            self.anim.running.remove(&n);
        }
        if let Some(ws) = self.window_of(n).and_then(|w| self.window_mut(w)) {
            ws.pending.animations.push(AnimationCmd::Cancel(id));
            ws.rebuild = true;
        }
        self.mark(n, Dirty::PROPERTY);
    }

    /// Animations started this frame drive their node's effect or transform node.
    fn bind_animation_targets(&self, commit: &mut Commit) {
        for cmd in &mut commit.animations {
            if let AnimationCmd::Start(spec) = cmd
                && let Some(i) = self.idx(spec.node)
            {
                let p = &self.record[i];
                spec.target = match spec.property {
                    AnimProperty::Opacity | AnimProperty::Palette(_) => p.effect,
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
    pub(crate) fn animates(&self, node: NodeId, prop: AnimProperty) -> bool {
        self.anim.running.get(&node).is_some_and(|v| v.iter().any(|&(_, p)| p == prop))
    }

    /// Node `i` animates a palette slot on the compositor now (its effect node carries the palette).
    fn animates_palette(&self, node: NodeId) -> bool {
        self.anim.running.get(&node).is_some_and(|v| v.iter().any(|&(_, p)| matches!(p, AnimProperty::Palette(_))))
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
            // recording first: a kind that scrolls itself says so in `prepare`, which its property
            // nodes follow
            if new || d.contains(Dirty::PAINT) {
                self.record_node(i, commit, runs, images);
                stats.recorded += 1;
            }
            if new || force || self.core[i].dirty.contains(Dirty::PROPERTY) {
                force_children |= self.refresh_props(window, i, ctx, commit);
                stats.props += 1;
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
            let p = &mut self.record[i];
            let child = Ctx { space: p.content.or(p.transform), clip: p.clip.or(ctx.clip), effect: p.effect.or(ctx.effect) };
            p.child_clip = child.clip;
            for &c in self.layout_children(i).to_vec().iter().rev() {
                let ci = self.ix(c);
                // absolute boxes are placed in their containing block's space and clipped as its
                // children are (a clip between the two does not clip them, as in CSS)
                let cctx = match crate::layout::positional_parent(self, ci) {
                    Some(pp) if pp != i => {
                        let q = &self.record[pp];
                        Ctx { space: q.content.or(q.transform), clip: q.child_clip, ..child }
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
        let axes = self.scroll_axes(i);
        let wants_clip = s.overflow_x.clips() || s.overflow_y.clips() || axes != (false, false);
        let wants_effect = s.opacity < 1.0 || s.will_change || self.animates(node, AnimProperty::Opacity) || self.animates_palette(node);
        // a non-passive listener on the path or inside the container may cancel the scroll
        let passive = !self.nonpassive_scroll_path(i) && !self.core[i].flags.contains(crate::tree::NodeFlags::DESC_NONPASSIVE);
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
                    scroll_x: axes.0,
                    scroll_y: axes.1,
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
        let mut services = std::mem::take(&mut self.services);
        let request = {
            let mut cx = PrepareCx { node: self.view_at(i), shaper: shaper.as_deref_mut(), images, request: Dirty::EMPTY, services: &mut services };
            k.prepare(&mut cx);
            cx.request
        };
        // a kind that scrolls itself may have grown or shrunk its content
        let own = k.self_scroll(&self.view_at(i));
        if self.self_scroll.get(&node) != own.as_ref() {
            match own {
                Some(o) => self.self_scroll.insert(node, o),
                None => self.self_scroll.remove(&node),
            };
            self.core[i].dirty.insert(Dirty::PROPERTY);
            if let Some(w) = self.core[i].window.and_then(|w| self.window_mut(w)) {
                w.rebuild = true;
            }
        }
        let mut list = DisplayList::new();
        let mut node_runs = Vec::new();
        let hidden = self.style.computed(i).visibility_hidden;
        if !hidden {
            let mut cx = RecordCx { node: self.view_at(i), list: &mut list, runs, node_runs: &mut node_runs, shaper: shaper.as_deref_mut(), services: &mut services };
            k.record(&mut cx);
        }
        self.shaper = shaper;
        self.services = services;
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
        // a layer's `opaque` follows its owner's list
        let owns_layer = self.record[i].layer.is_some() || self.core[i].flags.contains(crate::tree::NodeFlags::ROOT);
        if (had != self.record[i].list.is_some() || owns_layer)
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

    /// Node `i` paints every pixel of its bounds opaquely: its display list starts with a square,
    /// solid, opaque quad covering its box (`draw_box` of an opaque background), at full opacity.
    fn opaque_box(&self, i: usize) -> bool {
        let s = self.style.computed(i);
        let l = &self.layout[i].layout;
        let bounds = Rect::new(0.0, 0.0, l.size.width, l.size.height);
        let covers = self.record[i].list.as_ref().and_then(|list| list.items.first()).is_some_and(|p| {
            matches!(p, skelter_base::display::Prim::Quad(q)
                if q.radii.is_zero()
                    && matches!(&q.fill, skelter_base::color::Fill::Solid(c) if c.a >= 1.0)
                    && q.rect.min.x <= bounds.min.x && q.rect.min.y <= bounds.min.y
                    && q.rect.max.x >= bounds.max.x && q.rect.max.y >= bounds.max.y)
        });
        covers && s.opacity >= 1.0 && !s.visibility_hidden
    }

    /// Children of node `i` in paint order: tree order, stably sorted by `z-index`.
    fn paint_order(&self, i: usize) -> Vec<NodeId> {
        let mut kids = self.layout_children(i).to_vec();
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
            opaque: self.idx(root).is_some_and(|ri| self.opaque_box(ri)),
        }];
        let mut items: std::collections::HashMap<LayerId, Vec<ItemRef>> = Default::default();
        items.insert(root_layer, Vec::new());
        let mut hits = Vec::new();
        // (slot, world transform of the space its children are placed in, world clip, layer, ctx)
        #[derive(Clone, Copy)]
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
        let mut moved = Vec::new();
        // what each visited node passes its children: absolute boxes take their containing block's
        let mut cb_frames: std::collections::HashMap<usize, Frame> = Default::default();
        while let Some(f) = stack.pop() {
            let i = f.i;
            let node = self.slots.id_at(i).expect("live");
            let s = self.style.nodes[i].computed.clone();
            let hidden = self.layout[i].style.display == taffy::Display::None || self.core[i].flags.contains(crate::tree::NodeFlags::CONCEALED);
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
                // a content layer whose own box paints all of it, unblended, hides what is below
                let opaque = matches!(kind, LayerKind::Content) && own_effect.is_none() && self.opaque_box(i);
                layers.push(LayerDesc { id, parent: Some(f.layer), kind, transform, clip, effect: own_effect, z: z_next, bounds: lbounds, opaque });
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
                // a kind that scrolls itself draws in its content space, in its scroll layer
                let item = match (self.self_scroll.contains_key(&node), content_layer, p.content) {
                    (true, Some(cl), Some(ct)) => (cl, ItemRef { node, transform: ct, clip: p.clip.or(f.clip), effect: own_effect }),
                    _ => (layer, ItemRef { node, transform: p.transform.expect("transform"), clip: f.clip, effect: own_effect }),
                };
                items.entry(item.0).or_default().push(item.1);
            }
            let world_rect = world.apply_rect(&bounds);
            if drawn && !s.pointer_events_none && let Some(inverse) = world.inverse() {
                hits.push(HitEntry { node, inverse, world: world_rect, clip: f.clip_world });
            }
            self.record[i].world = world_rect;
            self.record[i].world_tf = world;
            // the accessibility node carries its transform relative to its parent
            let rel = match self.core[i].parent {
                Some(p) => self.record[self.ix(p)].world_tf.inverse().map_or(Affine::IDENTITY, |pi| pi.then(&world)),
                None => world,
            };
            if self.record[i].a11y_tf != Some(rel) {
                moved.push(node);
            }
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
            cb_frames.insert(i, child);
            for c in self.paint_order(i).into_iter().rev() {
                let ci = self.ix(c);
                // an absolute box is placed in its containing block's space and clipped as its
                // children are (as in the dirty walk); effects and layers follow the tree
                let frame = match crate::layout::positional_parent(self, ci) {
                    Some(pp) if pp != i => match cb_frames.get(&pp) {
                        Some(cb) => Frame { i: ci, parent_world: cb.parent_world, clip_world: cb.clip_world, clip: cb.clip, ..child },
                        None => Frame { i: ci, ..child },
                    },
                    _ => Frame { i: ci, ..child },
                };
                stack.push(frame);
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
        ws.a11y_moved = moved;
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
