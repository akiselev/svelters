//! Indexes, traversal, cursors and [`Query`] (ARCH-SCENE §6.2, §6.3, with R14's
//! [`Query::changed_since`]). Written by SCENE (SG3); STYLE adds `select` (CS2).
//!
//! Indexes, kept by the one edit path (`tree.rs`, `Scene::set_value`):
//! - **kind sets**: per kind, the live nodes of that kind (a packed vector with a position per
//!   slot, swap-removed when a node is freed). [`SceneRef::of_kind`] and [`SceneRef::of_tag`] are
//!   O(nodes of that kind).
//! - **anchor maps**: per root (`RootEntry::anchors`), anchor → the nodes of that root carrying it
//!   (the first one wins in [`SceneRef::by_anchor`]; duplicates are kept so that removing one
//!   promotes the next). Updated when a node enters or leaves a root and when its `anchor` prop
//!   changes. Detached nodes are in no anchor map.
//!
//! [`Query`] chains predicates (all must hold) and drives from the smallest index it has: an
//! extension column, the kind set, a scope's subtree, or every node. Results come in the driver's
//! order (an extension column's packed order, a kind set's order, pre-order for a scope): it is
//! deterministic for a given history but not tree order unless the scope drives.
//!
//! Writer lane: SCENE.

use super::Scene;
use super::access::{NodeRef, SceneRef};
use super::ext::ExtKey;
use super::id::{KindId, NodeId, RootIx, Target, Tick};
use super::journal::Scope;
use super::tree::RootTable;
use crate::kinds;
use crate::provider::{IntoStateBits, StateBits};

const NO_POS: u32 = u32::MAX;

/// The kind sets: per kind index, its live nodes.
#[derive(Default)]
pub(crate) struct Indexes {
    by_kind: Vec<Vec<NodeId>>,
    /// Slot → position in its kind's vector (`NO_POS`: free slot).
    pos: Vec<u32>,
}

impl Indexes {
    pub fn add(&mut self, kind: u32, id: NodeId) {
        let k = kind as usize;
        if self.by_kind.len() <= k {
            self.by_kind.resize_with(k + 1, Vec::new);
        }
        let i = id.index() as usize;
        if self.pos.len() <= i {
            self.pos.resize(i + 1, NO_POS);
        }
        self.pos[i] = self.by_kind[k].len() as u32;
        self.by_kind[k].push(id);
    }

    pub fn remove(&mut self, kind: u32, id: NodeId) {
        let i = id.index() as usize;
        let p = match self.pos.get(i) {
            Some(&p) if p != NO_POS => p as usize,
            _ => return,
        };
        let v = &mut self.by_kind[kind as usize];
        v.swap_remove(p);
        if let Some(&moved) = v.get(p) {
            self.pos[moved.index() as usize] = p as u32;
        }
        self.pos[i] = NO_POS;
    }

    pub fn of_kind(&self, kind: usize) -> &[NodeId] {
        self.by_kind.get(kind).map_or(&[], |v| v.as_slice())
    }

    /// Checks the kind sets against the live nodes (`Scene::validate`).
    pub fn check(&self, s: &Scene, errs: &mut Vec<String>) {
        let mut n = 0;
        for (k, v) in self.by_kind.iter().enumerate() {
            n += v.len();
            for (p, &id) in v.iter().enumerate() {
                match s.idx(id) {
                    None => errs.push(format!("kind set {k}: freed node {id}")),
                    Some(i) => {
                        if s.core[i].kind as usize != k {
                            errs.push(format!("kind set {k}: {id} is of kind {}", s.core[i].kind));
                        }
                        if self.pos.get(i) != Some(&(p as u32)) {
                            errs.push(format!("kind set {k}: {id} at {p}, position {:?}", self.pos.get(i)));
                        }
                    }
                }
            }
        }
        if n != s.len() {
            errs.push(format!("kind sets hold {n} nodes, {} live", s.len()));
        }
    }
}

/// Adds (`on`) or drops `id` under `anchor` in root `r`'s anchor map.
pub(crate) fn index_anchor(roots: &mut RootTable, r: RootIx, id: NodeId, anchor: &str, on: bool) {
    if anchor.is_empty() {
        return;
    }
    let Some(e) = roots.get_mut(r) else { return };
    if on {
        let v = e.anchors.entry(Box::from(anchor)).or_default();
        if !v.contains(&id) {
            v.push(id);
        }
    } else if let Some(v) = e.anchors.get_mut(anchor) {
        v.retain(|&x| x != id);
        if v.is_empty() {
            e.anchors.remove(anchor);
        }
    }
}

impl Scene {
    /// The node's `anchor` prop, if set.
    pub(crate) fn anchor_at(&self, i: usize) -> Option<&str> {
        self.cold[i].as_deref().map(|c| c.common.anchor.as_str()).filter(|a| !a.is_empty())
    }

    /// Checks the anchor maps against the nodes (`Scene::validate`).
    pub(crate) fn check_anchors(&self, errs: &mut Vec<String>) {
        for (r, e) in self.roots.iter() {
            for (a, ids) in &e.anchors {
                if ids.is_empty() {
                    errs.push(format!("root {r:?}: empty anchor entry {a:?}"));
                }
                for &id in ids {
                    match self.idx(id) {
                        None => errs.push(format!("root {r:?}: anchor {a:?} names freed {id}")),
                        Some(i) => {
                            if self.core[i].root != r || self.anchor_at(i) != Some(&**a) {
                                errs.push(format!("root {r:?}: anchor {a:?} names {id} (root {:?}, anchor {:?})", self.core[i].root, self.anchor_at(i)));
                            }
                        }
                    }
                }
            }
        }
        for id in self.ids() {
            let i = self.ix(id);
            let r = self.core[i].root;
            if let Some(a) = self.anchor_at(i)
                && !r.is_detached()
                && !self.roots.get(r).is_some_and(|e| e.anchors.get(a).is_some_and(|v| v.contains(&id)))
            {
                errs.push(format!("{id}: anchor {a:?} missing from root {r:?}'s map"));
            }
        }
    }
}

// ---- SceneRef: indexed reads ----

impl<'a> SceneRef<'a> {
    /// The node with `anchor="…"` in root `r` (O(1); the first one indexed if several share it).
    pub fn by_anchor(self, r: RootIx, anchor: &str) -> Option<NodeRef<'a>> {
        let id = *self.s.roots.get(r)?.anchors.get(anchor)?.first()?;
        self.get(id)
    }

    /// Every node with `anchor="…"` in root `r` (more than one is an authoring error).
    pub fn all_by_anchor(self, r: RootIx, anchor: &str) -> impl Iterator<Item = NodeRef<'a>> + 'a {
        let s = self.s;
        s.roots.get(r).and_then(|e| e.anchors.get(anchor)).into_iter().flatten().map(move |&id| NodeRef::at(s, id))
    }

    /// The root mounted under key `key` (the embed's `RootKey`; set while a root is mounted in a
    /// slot).
    pub fn root_by_key(self, key: &str) -> Option<RootIx> {
        self.s.roots.iter().find(|(_, e)| e.key.as_deref() == Some(key)).map(|(r, _)| r)
    }

    /// A node by id path: `"<root key>/<anchor>"`, e.g. `"panel.task/slot/extents[0]"` (the anchor
    /// may itself contain `/`). SG5 extends it to painted rows (`VirtualChildren::find_anchor`).
    pub fn by_path(self, path: &str) -> Option<NodeRef<'a>> {
        let (key, anchor) = path.split_once('/')?;
        self.by_anchor(self.root_by_key(key)?, anchor)
    }

    /// The live nodes of kind `k` (O(nodes of that kind), index order).
    pub fn of_kind(self, k: KindId) -> impl Iterator<Item = NodeRef<'a>> + 'a {
        let s = self.s;
        let v = kinds::kind_of_id(k.0).map_or(&[][..], |k| s.index.of_kind(k));
        v.iter().map(move |&id| NodeRef::at(s, id))
    }

    /// The live nodes of element tag `tag` (O(nodes of that kind), index order).
    pub fn of_tag(self, tag: &str) -> impl Iterator<Item = NodeRef<'a>> + 'a {
        let s = self.s;
        let v = kinds::tag_index(tag).map_or(&[][..], |k| s.index.of_kind(k));
        v.iter().map(move |&id| NodeRef::at(s, id))
    }

    /// The nodes with every bit of `bits` set (a scan of the hot column).
    pub fn with_state(self, bits: impl IntoStateBits) -> impl Iterator<Item = NodeRef<'a>> + 'a {
        let b = bits.state_bits();
        self.nodes().filter(move |n| n.state().contains(b))
    }

    /// A query over the whole scene (ARCH-SCENE §6.3).
    pub fn query(self) -> Query<'a> {
        Query::new(self.s)
    }

    /// Whether root `r` is shown (the embed's `set_visible`; windows are always).
    pub fn root_visible(self, r: RootIx) -> bool {
        self.s.roots.get(r).is_some_and(|e| e.visible)
    }

    /// The viewport root `r` is drawn in (the embed's `set_viewport`).
    pub fn root_viewport(self, r: RootIx) -> Option<egui::ViewportId> {
        self.s.roots.get(r).map(|e| e.viewport)
    }
}

// ---- traversal ----

/// What [`NodeRef::visit`]'s callback asks for next.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Visit {
    Continue,
    /// Do not enter this node's children.
    SkipChildren,
    /// End the walk.
    Stop,
}

impl<'a> NodeRef<'a> {
    /// The `anchor` prop, if set.
    pub fn anchor(self) -> Option<&'a str> {
        self.s.anchor_at(self.i)
    }

    /// The screen rect drawn last frame and its root's viewport; `None` if not drawn.
    pub fn screen_rect(self) -> Option<(egui::ViewportId, egui::Rect)> {
        let r = self.drawn()?;
        let vp = self.s.roots.get(self.root()).map_or(egui::ViewportId::ROOT, |e| e.viewport);
        Some((vp, r))
    }

    /// A cursor at this node.
    pub fn cursor(self) -> Cursor<'a> {
        Cursor { s: self.s, path: vec![(self.id(), self.index_in_parent())] }
    }

    /// The position among its parent's light children.
    pub fn index_in_parent(self) -> usize {
        self.parent().and_then(|p| self.s.children[p.i].iter().position(|&c| c == self.id())).unwrap_or(0)
    }

    /// Pre-order walk of the subtree, this node first; `f` decides whether to go on, skip the
    /// node's children, or stop. Returns `Stop` if `f` stopped it.
    pub fn visit(self, mut f: impl FnMut(NodeRef<'a>) -> Visit) -> Visit {
        let mut d = self.descendants();
        while let Some(n) = d.next() {
            match f(n) {
                Visit::Continue => {}
                Visit::SkipChildren => d.skip_subtree(),
                Visit::Stop => return Visit::Stop,
            }
        }
        Visit::Continue
    }
}

/// A movable position in the tree (tree-sitter's `TreeCursor`): parent, first and last child,
/// next and previous sibling, over light children. It may move above the node it started at.
#[derive(Clone)]
pub struct Cursor<'a> {
    s: &'a Scene,
    /// From the topmost visited ancestor down: (node, its index in its parent).
    path: Vec<(NodeId, usize)>,
}

impl<'a> Cursor<'a> {
    pub fn node(&self) -> NodeRef<'a> {
        NodeRef::at(self.s, self.path.last().expect("a cursor has a node").0)
    }

    /// How many `goto_parent` calls lead back above the start (0 at the start node or above).
    pub fn depth(&self) -> usize {
        self.path.len() - 1
    }

    pub fn goto_parent(&mut self) -> bool {
        if self.path.len() > 1 {
            self.path.pop();
            return true;
        }
        match self.node().parent() {
            Some(p) => {
                self.path[0] = (p.id(), p.index_in_parent());
                true
            }
            None => false,
        }
    }

    pub fn goto_first_child(&mut self) -> bool {
        let i = self.node().i;
        match self.s.children[i].first() {
            Some(&c) => {
                self.path.push((c, 0));
                true
            }
            None => false,
        }
    }

    pub fn goto_last_child(&mut self) -> bool {
        let i = self.node().i;
        let kids = &self.s.children[i];
        match kids.last() {
            Some(&c) => {
                self.path.push((c, kids.len() - 1));
                true
            }
            None => false,
        }
    }

    fn sibling(&mut self, at: impl FnOnce(usize) -> Option<usize>) -> bool {
        let Some(p) = self.node().parent() else { return false };
        let kids = &self.s.children[p.i];
        let last = self.path.len() - 1;
        let Some(k) = at(self.path[last].1) else { return false };
        match kids.get(k) {
            Some(&c) => {
                self.path[last] = (c, k);
                true
            }
            None => false,
        }
    }

    pub fn goto_next_sibling(&mut self) -> bool {
        self.sibling(|k| k.checked_add(1))
    }

    pub fn goto_prev_sibling(&mut self) -> bool {
        self.sibling(|k| k.checked_sub(1))
    }
}

// ---- Query ----

/// One extension predicate: the column's length (for the planner), its nodes (as a driver), and
/// the test by slot.
struct ExtPred<'a> {
    len: usize,
    ids: Option<Box<dyn Iterator<Item = NodeId> + 'a>>,
    test: Box<dyn Fn(u32) -> bool + 'a>,
}

/// A chain of predicates over the scene's nodes, all of which must hold (ARCH-SCENE §6.3). Build
/// it with [`SceneRef::query`]; run it with [`Query::iter`], [`Query::first`], [`Query::count`].
///
/// ```ignore
/// scene.read(|s| s.query().within(root).tag("label").has(&PRESENTS).state(StateBits::HOVERED).first());
/// ```
pub struct Query<'a> {
    s: &'a Scene,
    scope: Option<Scope>,
    kind: Option<usize>,
    /// A predicate that can never hold (an unknown tag, two kinds, a missing column).
    empty: bool,
    exts: Vec<ExtPred<'a>>,
    all: StateBits,
    none: StateBits,
    visible: bool,
    since: Option<Tick>,
    filters: Vec<Box<dyn Fn(NodeRef<'a>) -> bool + 'a>>,
}

impl<'a> Query<'a> {
    fn new(s: &'a Scene) -> Query<'a> {
        Query { s, scope: None, kind: None, empty: false, exts: Vec::new(), all: StateBits::NONE, none: StateBits::NONE, visible: false, since: None, filters: Vec::new() }
    }

    /// Only nodes in root `r` (`RootIx`), or in the subtree of a node (`NodeId`, itself included).
    /// A second `within` replaces the first.
    pub fn within(mut self, scope: impl Into<Scope>) -> Self {
        self.scope = Some(scope.into());
        self
    }

    fn set_kind(mut self, k: Option<usize>) -> Self {
        match (k, self.kind) {
            (None, _) => self.empty = true,
            (Some(k), Some(old)) if k != old => self.empty = true,
            (Some(k), _) => self.kind = Some(k),
        }
        self
    }

    /// Only nodes of kind `k`.
    pub fn kind(self, k: KindId) -> Self {
        self.set_kind(kinds::kind_of_id(k.0))
    }

    /// Only nodes of element tag `tag` (`"label"`, `"quantity-field"`).
    pub fn tag(self, tag: &str) -> Self {
        self.set_kind(kinds::tag_index(tag))
    }

    fn ext_pred<T: 'static>(mut self, key: &ExtKey<T>, test: impl Fn(&super::ext::Column<T>, u32) -> bool + 'a) -> Self {
        use super::ext::ErasedColumn;
        match self.s.ext.col(key) {
            Some(col) => self.exts.push(ExtPred { len: col.len(), ids: Some(Box::new(col.values().map(|(id, _)| id))), test: Box::new(move |i| test(col, i)) }),
            None => self.empty = true,
        }
        self
    }

    /// Only nodes with a value for `key`.
    pub fn has<T: 'static>(self, key: &ExtKey<T>) -> Self {
        self.ext_pred(key, |c, i| c.get(i).is_some())
    }

    /// Only nodes whose value for `key` satisfies `pred`.
    pub fn ext<T: 'static>(self, key: &ExtKey<T>, pred: impl Fn(&T) -> bool + 'a) -> Self {
        self.ext_pred(key, move |c, i| c.get(i).is_some_and(&pred))
    }

    /// Only nodes whose value for `key` was added or changed after tick `t` (the entry's own
    /// ticks, finer than [`Query::changed_since`]).
    pub fn ext_changed_since<T: 'static>(self, key: &ExtKey<T>, t: Tick) -> Self {
        self.ext_pred(key, move |c, i| c.ticks(i).is_some_and(|x| x.is_changed_since(t)))
    }

    /// Only nodes with every bit of `bits` set (built-in or provider bits).
    pub fn state(mut self, bits: impl IntoStateBits) -> Self {
        self.all = self.all | bits.state_bits();
        self
    }

    /// Only nodes with none of `bits` set.
    pub fn not_state(mut self, bits: impl IntoStateBits) -> Self {
        self.none = self.none | bits.state_bits();
        self
    }

    /// Only nodes that are shown: their `visible` prop and their ancestors', and their root's
    /// visibility ([`NodeRef::is_visible`]).
    pub fn visible(mut self) -> Self {
        self.visible = true;
        self
    }

    /// R14: only nodes changed (a prop, a state bit, an extension entry, their children) after
    /// tick `t`: Bevy's change detection over `NodeRef::changed`. Take `t` from
    /// [`SceneRef::tick`] after reading.
    pub fn changed_since(mut self, t: Tick) -> Self {
        self.since = Some(match self.since {
            Some(old) => old.max(t),
            None => t,
        });
        self
    }

    /// Any other test.
    pub fn filter(mut self, f: impl Fn(NodeRef<'a>) -> bool + 'a) -> Self {
        self.filters.push(Box::new(f));
        self
    }

    /// The matching nodes, from the smallest driving index.
    pub fn iter(mut self) -> impl Iterator<Item = NodeRef<'a>> + 'a {
        let s = self.s;
        let driver: Box<dyn Iterator<Item = NodeId> + 'a> = if self.empty {
            Box::new(std::iter::empty())
        } else {
            // the smallest of: the extension columns, the kind set
            let kind_len = self.kind.map(|k| s.index.of_kind(k).len());
            let best_ext = self.exts.iter().enumerate().min_by_key(|(_, e)| e.len).map(|(k, e)| (k, e.len));
            match (best_ext, kind_len) {
                (Some((k, n)), kl) if kl.is_none_or(|kl| n <= kl) => self.exts[k].ids.take().expect("one driver"),
                (_, Some(_)) => Box::new(s.index.of_kind(self.kind.expect("kind")).iter().copied()),
                _ => match self.scope {
                    Some(Scope::Subtree(n)) => match SceneRef::new(s).get(n) {
                        Some(n) => Box::new(n.descendants().map(|n| n.id())),
                        None => Box::new(std::iter::empty()),
                    },
                    Some(Scope::Root(r)) => match SceneRef::new(s).root_top(r) {
                        Some(top) => Box::new(top.descendants().map(|n| n.id())),
                        None => Box::new(std::iter::empty()),
                    },
                    None => Box::new(s.ids()),
                },
            }
        };
        driver.filter_map(move |id| {
            let i = s.idx(id)?;
            self.matches(id, i).then(|| NodeRef::at(s, id))
        })
    }

    fn matches(&self, id: NodeId, i: usize) -> bool {
        let s = self.s;
        let c = &s.core[i];
        if self.kind.is_some_and(|k| c.kind as usize != k) || !c.state.contains(self.all) || c.state.intersects(self.none) {
            return false;
        }
        if self.since.is_some_and(|t| c.changed <= t) {
            return false;
        }
        match self.scope {
            Some(Scope::Root(r)) if c.root != r => return false,
            Some(Scope::Subtree(top)) => {
                let mut a = Some(id);
                let mut inside = false;
                while let Some(x) = a {
                    if x == top {
                        inside = true;
                        break;
                    }
                    a = s.idx(x).and_then(|xi| s.core[xi].parent);
                }
                if !inside {
                    return false;
                }
            }
            _ => {}
        }
        if !self.exts.iter().all(|e| (e.test)(i as u32)) {
            return false;
        }
        let n = NodeRef::at(s, id);
        (!self.visible || n.is_visible()) && self.filters.iter().all(|f| f(n))
    }

    /// The matches as hit-test targets (`Target::Node`; painted rows join in SG5 with
    /// `include_virtual`).
    pub fn targets(self) -> impl Iterator<Item = Target> + 'a {
        self.iter().map(|n| Target::Node(n.id()))
    }

    pub fn first(self) -> Option<NodeRef<'a>> {
        self.iter().next()
    }

    pub fn count(self) -> usize {
        self.iter().count()
    }

    /// The matching ids.
    pub fn ids(self) -> Vec<NodeId> {
        self.iter().map(|n| n.id()).collect()
    }
}
