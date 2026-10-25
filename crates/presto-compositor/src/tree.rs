//! A window's committed state on the compositor thread (architecture §6.2–§6.4, §7.3): the layer tree,
//! the four property trees, the display lists and the CPU-side resources, with commits and fast commits
//! applied to them, and the flattening of all of it into paint-ordered draw items per surface.
//!
//! Ownership of native properties (architecture §4.5): a fast commit writes a scroll offset, a local
//! transform or an opacity with a revision newer than any commit in flight. A later-arriving commit
//! older than that revision carries a stale value for the same field, so the field keeps the fast
//! value; a fast commit not newer than the last applied commit is stale itself and is rejected.

use presto_base::anim::AnimationCmd;
use presto_base::commit::{Commit, FastCommit, ImageData, PathData};
use presto_base::display::{DisplayList, Prim, SurfacePrim};
use presto_base::geom::{Affine, Rect, Vec2};
use presto_base::ids::{CanvasId, ImageId, InputSeq, LayerId, NodeId, PathId, PropNodeId, Revision, RunId, SurfaceId};
use presto_base::layer::{ItemRef, LayerDelta, LayerDesc, LayerKind};
use presto_base::props::{ClipNode, ClipShape, EffectNode, OverscrollBehavior, PropOp, ScrollNode, TransformNode};
use presto_base::scene::SceneResources;
use presto_base::text::GlyphRun;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// Property chains deeper than this are treated as cyclic (a malformed delta) and cut.
const MAX_CHAIN: usize = 1024;

/// The four property trees, plus the values compositor animations currently drive.
#[derive(Default, Debug)]
pub struct PropTrees {
    pub transforms: HashMap<PropNodeId, TransformNode>,
    pub clips: HashMap<PropNodeId, ClipNode>,
    pub effects: HashMap<PropNodeId, EffectNode>,
    pub scrolls: HashMap<PropNodeId, ScrollNode>,
    /// A running transform animation's value, applied after the node's committed local transform.
    pub anim_transform: HashMap<PropNodeId, Affine>,
    /// A running opacity animation's value, replacing the node's committed opacity.
    pub anim_opacity: HashMap<PropNodeId, f32>,
    /// A running clip animation's value, replacing the node's committed shape.
    pub anim_clip: HashMap<PropNodeId, Rect>,
}

impl PropTrees {
    fn apply(&mut self, op: &PropOp) {
        match op {
            PropOp::Transform(id, n) => {
                self.transforms.insert(*id, *n);
            }
            PropOp::Clip(id, n) => {
                self.clips.insert(*id, *n);
            }
            PropOp::Effect(id, n) => {
                self.effects.insert(*id, *n);
            }
            PropOp::Scroll(id, n) => {
                self.scrolls.insert(*id, *n);
            }
            PropOp::Remove(id) => {
                self.transforms.remove(id);
                self.clips.remove(id);
                self.effects.remove(id);
                self.scrolls.remove(id);
                self.anim_transform.remove(id);
                self.anim_opacity.remove(id);
                self.anim_clip.remove(id);
            }
        }
    }

    /// The transform from `id`'s space to window points: the chain of local transforms, each space
    /// translated by its scroll node's offset. An unknown id is the window space.
    pub fn world_transform(&self, id: PropNodeId, memo: &mut HashMap<PropNodeId, Affine>) -> Affine {
        if let Some(t) = memo.get(&id) {
            return *t;
        }
        let mut chain = Vec::new();
        let mut cur = Some(id);
        let mut base = Affine::IDENTITY;
        while let Some(c) = cur {
            if let Some(t) = memo.get(&c) {
                base = *t;
                break;
            }
            let Some(node) = self.transforms.get(&c) else { break };
            chain.push(c);
            if chain.len() > MAX_CHAIN {
                break;
            }
            cur = node.parent;
        }
        let mut world = base;
        for c in chain.into_iter().rev() {
            let node = &self.transforms[&c];
            if let Some(s) = node.scroll.and_then(|s| self.scrolls.get(&s)) {
                world = world.then(&Affine::translate(-s.offset.x, -s.offset.y));
            }
            world = world.then(&node.local);
            if let Some(a) = self.anim_transform.get(&c) {
                world = world.then(a);
            }
            memo.insert(c, world);
        }
        world
    }

    /// The intersection of `id`'s clip chain in window points. A rounded shape survives only when the
    /// rest of the chain contains it and its space is a translation or scale; otherwise the result is
    /// the bounding rectangle (rotated clips clip to their bounds).
    pub fn world_clip(&self, id: Option<PropNodeId>, memo: &mut HashMap<PropNodeId, Affine>) -> Option<ClipShape> {
        let mut cur = id;
        let mut rect: Option<Rect> = None;
        let mut rounded: Option<presto_base::geom::RoundedRect> = None;
        let mut depth = 0;
        while let Some(c) = cur {
            let Some(node) = self.clips.get(&c) else { break };
            depth += 1;
            if depth > MAX_CHAIN {
                break;
            }
            let space = self.world_transform(node.space, memo);
            let axis_aligned = space.b == 0.0 && space.c == 0.0;
            let shape = match self.anim_clip.get(&c) {
                Some(r) => ClipShape::Rect(*r),
                None => node.shape,
            };
            let world = match shape {
                ClipShape::Rect(r) => space.apply_rect(&r),
                ClipShape::Rounded(rr) => {
                    let wr = space.apply_rect(&rr.rect);
                    if axis_aligned && rounded.is_none() {
                        let s = space.a.abs().min(space.d.abs());
                        let k = |r: f32| r * s;
                        rounded = Some(presto_base::geom::RoundedRect {
                            rect: wr,
                            radii: presto_base::geom::Corners {
                                tl: k(rr.radii.tl),
                                tr: k(rr.radii.tr),
                                br: k(rr.radii.br),
                                bl: k(rr.radii.bl),
                            },
                        });
                    }
                    wr
                }
            };
            rect = Some(match rect {
                Some(r) => r.intersect(&world),
                None => world,
            });
            cur = node.parent;
        }
        let rect = rect?;
        match rounded {
            Some(rr) if rr.rect == rect => Some(ClipShape::Rounded(rr)),
            _ => Some(ClipShape::Rect(rect)),
        }
    }

    /// The product of the effect chain's opacities.
    pub fn opacity(&self, id: Option<PropNodeId>) -> f32 {
        let mut cur = id;
        let mut o = 1.0;
        let mut depth = 0;
        while let Some(c) = cur {
            let Some(node) = self.effects.get(&c) else { break };
            depth += 1;
            if depth > MAX_CHAIN {
                break;
            }
            o *= self.anim_opacity.get(&c).copied().unwrap_or(node.opacity);
            cur = node.parent;
        }
        o
    }
}

/// The CPU-side resources lists reference. Kept across device loss: the renderer re-uploads from them.
#[derive(Default, Debug)]
pub struct Resources {
    pub runs: HashMap<RunId, Arc<GlyphRun>>,
    pub images: HashMap<ImageId, Arc<ImageData>>,
    pub paths: HashMap<PathId, Arc<PathData>>,
}

impl SceneResources for Resources {
    fn run(&self, id: RunId) -> Option<&GlyphRun> {
        self.runs.get(&id).map(|r| &**r)
    }
    fn image(&self, id: ImageId) -> Option<&ImageData> {
        self.images.get(&id).map(|r| &**r)
    }
    fn path(&self, id: PathId) -> Option<&PathData> {
        self.paths.get(&id).map(|r| &**r)
    }
}

#[derive(Clone, Debug)]
pub struct Layer {
    pub desc: LayerDesc,
    pub items: Vec<ItemRef>,
    /// Insertion order, the tie-break among siblings with equal `z`.
    seq: u64,
}

/// The fields a fast commit owns.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum FastField {
    Scroll,
    Transform,
    Opacity,
}

/// Why a fast commit was not applied.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FastReject {
    /// Its revision is not newer than the last applied commit, which already carries the tree's value.
    Stale,
    /// It names a property node this window does not have.
    UnknownNode,
}

/// Which surface a draw item goes to: the window's own, or one of its OS layers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SurfaceKey {
    Main,
    Os(SurfaceId),
}

/// One display list resolved for drawing, in its surface's points.
#[derive(Clone, Debug)]
pub struct FlatItem {
    pub layer: LayerId,
    pub node: NodeId,
    pub transform: Affine,
    pub clip: Option<ClipShape>,
    pub opacity: f32,
    pub list: Arc<DisplayList>,
    /// Ink bounds in surface points, clipped.
    pub bounds: Rect,
}

/// A window's committed scene.
#[derive(Default, Debug)]
pub struct WindowTree {
    pub layers: HashMap<LayerId, Layer>,
    pub props: PropTrees,
    pub lists: HashMap<NodeId, Arc<DisplayList>>,
    pub resources: Resources,
    /// The newest applied commit's revision and input.
    pub revision: Revision,
    pub input_seq: InputSeq,
    /// The canvas view transforms fast commits set (architecture §9.5).
    pub canvas_views: HashMap<CanvasId, Affine>,
    /// Scroll nodes whose offset a compositor scroll motion owns while it runs.
    pub compositor_scrolling: HashSet<PropNodeId>,
    fast_owned: HashMap<(PropNodeId, FastField), Revision>,
    /// `LayerDelta::Damage` rects not drawn yet, in layer space.
    pub layer_damage: Vec<(LayerId, Rect)>,
    /// Lists synthesised for texture layers, kept while their texture and bounds stay the same.
    texture_lists: HashMap<LayerId, Arc<DisplayList>>,
    layer_seq: u64,
}

impl WindowTree {
    /// Applies one commit over the current state (folding = applying pending commits in order) and
    /// returns its animation commands for the animation engine.
    pub fn apply_commit(&mut self, c: Commit) -> Vec<AnimationCmd> {
        for op in &c.props.ops {
            let op = self.reconcile(op, c.revision);
            self.props.apply(&op);
        }
        for d in c.layers {
            match d {
                LayerDelta::Add(desc) | LayerDelta::Update(desc) => {
                    let seq = self.layer_seq;
                    self.layer_seq += 1;
                    self.layers
                        .entry(desc.id)
                        .and_modify(|l| l.desc = desc)
                        .or_insert(Layer { desc, items: Vec::new(), seq });
                }
                LayerDelta::Remove(id) => {
                    self.layers.remove(&id);
                    self.texture_lists.remove(&id);
                }
                LayerDelta::Items { layer, items } => {
                    if let Some(l) = self.layers.get_mut(&layer) {
                        l.items = items;
                    }
                }
                LayerDelta::Damage { layer, rects } => {
                    self.layer_damage.extend(rects.into_iter().map(|r| (layer, r)));
                }
            }
        }
        for n in &c.removed {
            self.lists.remove(n);
        }
        for (n, l) in c.lists {
            self.lists.insert(n, l);
        }
        for (id, r) in c.runs {
            self.resources.runs.insert(id, r);
        }
        for id in &c.dropped_runs {
            self.resources.runs.remove(id);
        }
        let res = c.resources;
        for id in &res.removed_images {
            self.resources.images.remove(id);
        }
        for (id, i) in res.images {
            self.resources.images.insert(id, i);
        }
        for id in &res.removed_paths {
            self.resources.paths.remove(id);
        }
        for (id, p) in res.paths {
            self.resources.paths.insert(id, p);
        }
        if c.revision > self.revision {
            self.revision = c.revision;
        }
        if c.input_seq > self.input_seq {
            self.input_seq = c.input_seq;
        }
        c.animations
    }

    /// Keeps fast-owned fields when the commit is older than the fast write, and releases ownership once
    /// a commit has caught up with it.
    fn reconcile(&mut self, op: &PropOp, revision: Revision) -> PropOp {
        let keep = |id: PropNodeId, f: FastField, fast: &mut HashMap<(PropNodeId, FastField), Revision>| -> bool {
            match fast.get(&(id, f)) {
                Some(r) if revision < *r => true,
                Some(_) => {
                    fast.remove(&(id, f));
                    false
                }
                None => false,
            }
        };
        match op {
            PropOp::Transform(id, n) => {
                let mut n = *n;
                if keep(*id, FastField::Transform, &mut self.fast_owned) {
                    if let Some(cur) = self.props.transforms.get(id) {
                        n.local = cur.local;
                    }
                }
                PropOp::Transform(*id, n)
            }
            PropOp::Scroll(id, n) => {
                let mut n = *n;
                let owned = keep(*id, FastField::Scroll, &mut self.fast_owned) || self.compositor_scrolling.contains(id);
                if owned {
                    if let Some(cur) = self.props.scrolls.get(id) {
                        n.offset = cur.offset;
                    }
                }
                PropOp::Scroll(*id, n)
            }
            PropOp::Effect(id, n) => {
                let mut n = *n;
                if keep(*id, FastField::Opacity, &mut self.fast_owned) {
                    if let Some(cur) = self.props.effects.get(id) {
                        n.opacity = cur.opacity;
                    }
                }
                PropOp::Effect(*id, n)
            }
            PropOp::Remove(id) => {
                self.fast_owned.retain(|(n, _), _| n != id);
                self.compositor_scrolling.remove(id);
                PropOp::Remove(*id)
            }
            op => op.clone(),
        }
    }

    /// Applies a scroll, transform, opacity or canvas-view fast commit. The caret is the engine's
    /// (`crate::core`), not the tree's.
    pub fn apply_fast(&mut self, f: &FastCommit) -> Result<(), FastReject> {
        let check = |rev: Revision, last: Revision| if rev <= last { Err(FastReject::Stale) } else { Ok(()) };
        match *f {
            FastCommit::ScrollOffset { node, offset, revision, input_seq, .. } => {
                check(revision, self.revision)?;
                let s = self.props.scrolls.get_mut(&node).ok_or(FastReject::UnknownNode)?;
                s.offset = clamp_offset(s, offset);
                self.own(node, FastField::Scroll, revision);
                self.input_seq = self.input_seq.max(input_seq);
                Ok(())
            }
            FastCommit::Transform { node, local, revision, input_seq, .. } => {
                check(revision, self.revision)?;
                let t = self.props.transforms.get_mut(&node).ok_or(FastReject::UnknownNode)?;
                t.local = local;
                self.own(node, FastField::Transform, revision);
                self.input_seq = self.input_seq.max(input_seq);
                Ok(())
            }
            FastCommit::Opacity { node, opacity, revision, .. } => {
                check(revision, self.revision)?;
                let e = self.props.effects.get_mut(&node).ok_or(FastReject::UnknownNode)?;
                e.opacity = opacity;
                self.own(node, FastField::Opacity, revision);
                Ok(())
            }
            FastCommit::CanvasView { canvas, view, input_seq, .. } => {
                self.canvas_views.insert(canvas, view);
                self.input_seq = self.input_seq.max(input_seq);
                Ok(())
            }
            FastCommit::Caret { .. } => Ok(()),
        }
    }

    fn own(&mut self, node: PropNodeId, f: FastField, revision: Revision) {
        let e = self.fast_owned.entry((node, f)).or_insert(revision);
        *e = (*e).max(revision);
    }

    /// The window's root layer: the `Root` layer without a parent (lowest id if several).
    pub fn root(&self) -> Option<LayerId> {
        self.layers
            .values()
            .filter(|l| l.desc.parent.is_none() && l.desc.kind == LayerKind::Root)
            .map(|l| l.desc.id)
            .min()
    }

    /// Flattens the layer tree into paint-ordered items per surface: a layer's items, then its child
    /// layers by `(z, insertion)`. An OS layer's subtree goes to its own surface, in that layer's space
    /// (its bounds' origin at the surface origin). Layers not reachable from the root are not drawn.
    pub fn flatten(&mut self) -> HashMap<SurfaceKey, Vec<FlatItem>> {
        let mut children: HashMap<LayerId, Vec<(i32, u64, LayerId)>> = HashMap::new();
        for l in self.layers.values() {
            if let Some(p) = l.desc.parent {
                children.entry(p).or_default().push((l.desc.z, l.seq, l.desc.id));
            }
        }
        for c in children.values_mut() {
            c.sort();
        }
        let mut out: HashMap<SurfaceKey, Vec<FlatItem>> = HashMap::new();
        out.insert(SurfaceKey::Main, Vec::new());
        let Some(root) = self.root() else { return out };
        let mut memo = HashMap::new();
        let mut stack = vec![(root, SurfaceKey::Main, Affine::IDENTITY)];
        let mut seen = HashSet::new();
        // Explicit DFS that emits a layer's items before its children, children in order.
        while let Some((id, surface, base)) = stack.pop() {
            if !seen.insert(id) {
                continue;
            }
            let Some(layer) = self.layers.get(&id) else { continue };
            let (surface, base) = match layer.desc.kind {
                LayerKind::OsLayer(sid) => {
                    let w = self.props.world_transform(layer.desc.transform, &mut memo);
                    let to_layer = w.inverse().unwrap_or(Affine::IDENTITY);
                    let origin = Affine::translate(-layer.desc.bounds.min.x, -layer.desc.bounds.min.y);
                    (SurfaceKey::Os(sid), origin.then(&to_layer))
                }
                _ => (surface, base),
            };
            let items = out.entry(surface).or_default();
            if let LayerKind::Texture(tex) = layer.desc.kind {
                let list = texture_list(&mut self.texture_lists, id, tex, layer.desc.bounds);
                let item = ItemRef {
                    node: NodeId(0),
                    transform: layer.desc.transform,
                    clip: layer.desc.clip,
                    effect: layer.desc.effect,
                };
                push_item(items, &self.props, &mut memo, base, id, &item, list);
            }
            for item in &layer.items {
                if let Some(list) = self.lists.get(&item.node) {
                    push_item(items, &self.props, &mut memo, base, id, item, list.clone());
                }
            }
            if let Some(cs) = children.get(&id) {
                for &(_, _, c) in cs.iter().rev() {
                    stack.push((c, surface, base));
                }
            }
        }
        out
    }

    /// Takes the pending `LayerDelta::Damage` rects in surface points.
    pub fn take_layer_damage(&mut self) -> Vec<(SurfaceKey, Rect)> {
        let mut memo = HashMap::new();
        let mut out = Vec::new();
        for (layer, r) in std::mem::take(&mut self.layer_damage) {
            let Some(l) = self.layers.get(&layer) else { continue };
            let w = self.props.world_transform(l.desc.transform, &mut memo);
            let (key, base) = self.surface_of(layer, &mut memo);
            out.push((key, base.then(&w).apply_rect(&r)));
        }
        out
    }

    /// The surface a layer draws to and the transform from window points to that surface's points.
    pub fn surface_of(&self, layer: LayerId, memo: &mut HashMap<PropNodeId, Affine>) -> (SurfaceKey, Affine) {
        let mut cur = Some(layer);
        let mut depth = 0;
        while let Some(c) = cur {
            let Some(l) = self.layers.get(&c) else { break };
            if let LayerKind::OsLayer(sid) = l.desc.kind {
                let w = self.props.world_transform(l.desc.transform, memo);
                let to_layer = w.inverse().unwrap_or(Affine::IDENTITY);
                let origin = Affine::translate(-l.desc.bounds.min.x, -l.desc.bounds.min.y);
                return (SurfaceKey::Os(sid), origin.then(&to_layer));
            }
            depth += 1;
            if depth > MAX_CHAIN {
                break;
            }
            cur = l.desc.parent;
        }
        (SurfaceKey::Main, Affine::IDENTITY)
    }

    /// Sets a scroll node's offset from a compositor scroll motion: clamped to its range, except under
    /// elastic overscroll, where the motion's rubber band goes past the edges.
    pub fn set_scroll_offset(&mut self, node: PropNodeId, offset: Vec2) -> Option<Vec2> {
        let s = self.props.scrolls.get_mut(&node)?;
        s.offset = match s.overscroll {
            OverscrollBehavior::None => clamp_offset(s, offset),
            OverscrollBehavior::Elastic => offset,
        };
        Some(s.offset)
    }
}

fn push_item(
    items: &mut Vec<FlatItem>,
    props: &PropTrees,
    memo: &mut HashMap<PropNodeId, Affine>,
    base: Affine,
    layer: LayerId,
    item: &ItemRef,
    list: Arc<DisplayList>,
) {
    let transform = base.then(&props.world_transform(item.transform, memo));
    let clip = props.world_clip(item.clip, memo).map(|c| match c {
        ClipShape::Rect(r) => ClipShape::Rect(base.apply_rect(&r)),
        ClipShape::Rounded(mut rr) if base.is_translation() => {
            rr.rect = base.apply_rect(&rr.rect);
            ClipShape::Rounded(rr)
        }
        ClipShape::Rounded(rr) => ClipShape::Rect(base.apply_rect(&rr.rect)),
    });
    let opacity = props.opacity(item.effect);
    let mut bounds = transform.apply_rect(&list.bounds);
    if let Some(c) = &clip {
        bounds = bounds.intersect(&clip_rect(c));
    }
    items.push(FlatItem { layer, node: item.node, transform, clip, opacity, list, bounds });
}

pub fn clip_rect(c: &ClipShape) -> Rect {
    match c {
        ClipShape::Rect(r) => *r,
        ClipShape::Rounded(rr) => rr.rect,
    }
}

fn texture_list(
    cache: &mut HashMap<LayerId, Arc<DisplayList>>,
    layer: LayerId,
    texture: presto_base::ids::TextureId,
    bounds: Rect,
) -> Arc<DisplayList> {
    let prim = Prim::Surface(SurfacePrim { texture, rect: bounds, opacity: 1.0 });
    if let Some(l) = cache.get(&layer) {
        if l.items.len() == 1 && l.items[0] == prim {
            return l.clone();
        }
    }
    let mut list = DisplayList::new();
    list.push(prim, bounds);
    let list = Arc::new(list);
    cache.insert(layer, list.clone());
    list
}

/// Clamps an offset to `[0, content − viewport]` on each scrollable axis (0 on the others).
pub fn clamp_offset(s: &ScrollNode, o: Vec2) -> Vec2 {
    let max_x = (s.content.w - s.viewport.width()).max(0.0);
    let max_y = (s.content.h - s.viewport.height()).max(0.0);
    Vec2::new(
        if s.scroll_x { o.x.clamp(0.0, max_x) } else { 0.0 },
        if s.scroll_y { o.y.clamp(0.0, max_y) } else { 0.0 },
    )
}
