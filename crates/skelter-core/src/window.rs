//! Per-window state: the root, the content size and scale, queued layout work, focus, and what the
//! last commits sent (property-tree ids, layers, items), so the next commit is a delta.

use crate::hit::HitList;
use crate::record::NodePaint;
use skelter_base::anim::AnimationCmd;
use skelter_base::geom::{Scale, Size};
use skelter_base::ids::{LayerId, NodeId, PropNodeId, Revision, RunId, WindowId};
use skelter_base::layer::{ItemRef, LayerDesc};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// The queued layout work of one window.
#[derive(Default, Debug)]
pub(crate) struct RootDirty {
    /// Boundaries whose content changed (flag `IN_RELAYOUT`; stale or moved entries are skipped).
    pub relayout: Vec<NodeId>,
    /// Leaves whose content changed (flag `IN_REMEASURE`).
    pub remeasure: Vec<NodeId>,
    /// Lay out the whole window (new, resized, or its root's own style changed).
    pub full: bool,
}

/// Hands out small ids with reuse.
#[derive(Default, Debug)]
pub(crate) struct IdAlloc {
    next: u32,
    free: Vec<u32>,
}

impl IdAlloc {
    pub fn alloc(&mut self) -> u32 {
        self.free.pop().unwrap_or_else(|| {
            self.next += 1;
            self.next
        })
    }
    pub fn free(&mut self, id: u32) {
        self.free.push(id);
    }
}

/// What the next commit must remove, start or upload, gathered between frames.
#[derive(Default)]
pub(crate) struct Pending {
    pub removed_lists: Vec<NodeId>,
    pub removed_props: Vec<PropNodeId>,
    pub dropped_runs: Vec<RunId>,
    pub removed_layers: Vec<LayerId>,
    pub animations: Vec<AnimationCmd>,
}

impl Pending {
    pub fn is_empty(&self) -> bool {
        self.removed_lists.is_empty()
            && self.removed_props.is_empty()
            && self.dropped_runs.is_empty()
            && self.removed_layers.is_empty()
            && self.animations.is_empty()
    }
}

pub(crate) struct WindowState {
    pub id: WindowId,
    pub root: NodeId,
    /// Content size in points.
    pub size: Size,
    pub scale: Scale,
    pub dirty: RootDirty,
    /// Laid out at least once.
    pub laid_out: bool,
    pub focus: Option<NodeId>,
    pub focus_visible: bool,
    /// Focus to restore when a `dialog` scope closes: (scope, the node focused before it opened).
    pub restore: Vec<(NodeId, NodeId)>,
    pub props: IdAlloc,
    pub layer_ids: IdAlloc,
    /// The window's root transform node and layer.
    pub root_transform: Option<PropNodeId>,
    pub root_layer: Option<LayerId>,
    /// The layers last sent, and their items.
    pub layers: HashMap<LayerId, LayerDesc>,
    pub items: HashMap<LayerId, Vec<ItemRef>>,
    pub pending: Pending,
    /// The hit list of the last frame (what input hit-tests against).
    pub hits: Arc<HitList>,
    /// Scroll property nodes → their scroll container.
    pub scroll_nodes: HashMap<PropNodeId, NodeId>,
    /// The AccessKit ids sent so far (to know what an update must re-send after removals).
    pub a11y_sent: HashSet<NodeId>,
    /// The IME area the last commit carried.
    pub ime_sent: Option<skelter_base::geom::Rect>,
    /// Nodes whose transform relative to their parent changed at the last rebuild (their
    /// accessibility nodes are re-sent).
    pub a11y_moved: Vec<NodeId>,
    /// Structure, layout or properties changed: rebuild layer items and the hit list.
    pub rebuild: bool,
    /// The revision of the last commit built, and of the last one the compositor applied.
    pub built: Option<Revision>,
    pub applied: Option<Revision>,
}

impl WindowState {
    pub fn new(id: WindowId, root: NodeId, size: Size, scale: Scale) -> Self {
        WindowState {
            id,
            root,
            size,
            scale,
            dirty: RootDirty { full: true, ..Default::default() },
            laid_out: false,
            focus: None,
            focus_visible: false,
            restore: Vec::new(),
            props: IdAlloc::default(),
            layer_ids: IdAlloc::default(),
            root_transform: None,
            root_layer: None,
            layers: HashMap::new(),
            items: HashMap::new(),
            pending: Pending::default(),
            hits: Arc::new(HitList::default()),
            scroll_nodes: HashMap::new(),
            a11y_sent: HashSet::new(),
            ime_sent: None,
            a11y_moved: Vec::new(),
            rebuild: true,
            built: None,
            applied: None,
        }
    }

    /// A node left the window: its list, property nodes and layer go in the next commit's removals.
    pub fn retire(&mut self, node: NodeId, paint: NodePaint) {
        if paint.list.is_some() {
            self.pending.removed_lists.push(node);
        }
        if paint.fixed.is_some() {
            self.pending.removed_lists.push(crate::record::fixed_key(node));
        }
        if let Some(s) = paint.scroll {
            self.scroll_nodes.remove(&s);
        }
        for p in paint.prop_nodes() {
            self.pending.removed_props.push(p);
            self.props.free(p.0);
        }
        self.pending.dropped_runs.extend(paint.runs);
        if let Some(l) = paint.layer {
            self.pending.removed_layers.push(l);
            self.layers.remove(&l);
            self.items.remove(&l);
            self.layer_ids.free(l.0);
        }
        if self.focus == Some(node) {
            self.focus = None;
        }
        self.restore.retain(|&(s, n)| s != node && n != node);
        self.rebuild = true;
    }

    /// A commit is built and not yet applied by the compositor.
    pub fn commit_in_flight(&self) -> bool {
        match (self.built, self.applied) {
            (Some(b), Some(a)) => b > a,
            (Some(_), None) => true,
            _ => false,
        }
    }
}
