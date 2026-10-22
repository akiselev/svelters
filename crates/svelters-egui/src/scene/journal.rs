//! The change journal (ARCH-SCENE §6.5, S5(a)): [`Change`] records, pull [`Observer`]s with
//! [`ChangeFilter`]s, [`ChangeSet`] and [`Damage`].
//!
//! - **Pull, not push.** An observer's owner drains it when it chooses
//!   ([`SceneHandle::take_changes`](super::SceneHandle::take_changes) or [`Observer::take`]); nothing
//!   is called back from inside a mutation, so Rust mutation stays re-entrancy-free.
//! - **Pay only for interest.** Every mutation asks the journal first with a mask test against the
//!   union of the live observers' interests (kinds of change, extension keys, state bits). With no
//!   observer, a mutation costs that test and allocates nothing. A record is then matched against
//!   each observer's filter (scope, kinds, props, keys, states, origins) and pushed onto that
//!   observer's own bounded queue.
//! - **Replayable.** Records come in mutation order and name what changed, not the new values
//!   (MutationObserver records): an observer reads current values from the scene when it takes the
//!   records. Applying `Created`/`Inserted`/`Removed`/`Destroyed` and reading the props, state bits
//!   and extension keys named by `Prop`/`State`/`Ext` rebuilds the scene's dump (the SG2 acceptance
//!   test, `scene/tests.rs`).
//! - **Capacity.** A queue holds `ChangeFilter::capacity` records (default 64k). On overflow it is
//!   cleared, `overflowed` is set and recording for that observer stops until it takes: it re-reads
//!   what it needs (AccessKit's full-tree fallback).
//! - **Batches.** Records written inside one `SceneHandle::write`, deferred write or
//!   `SceneMut::batch` share a tick; an observer sees `Change::Batch { label, tick }` before the
//!   first record of each such batch. A runtime's host ops are not batched (one tick per op).
//!
//! Scope: a record's subject is the node it names, or the parent for `Inserted`/`Removed`; a
//! `Scope::Root(r)` observer sees records whose subject is under root `r` *at record time*, so a
//! node built detached and then inserted shows up as `Inserted` (read its subtree then), not as its
//! `Created` and prop records.
//!
//! Ticks are the cheaper alternative for polling: `NodeRef::changed_since`, `ExtTicks`,
//! `SceneRef::changed_since`.
//!
//! Writer lane: SCENE (SG2).

use super::access::Origin;
use super::id::{KindId, NodeId, PropId, ProviderId, RootIx, Tick};
use super::node::{NodeCore, Owner, bits};
use super::tree::RootTable;
use crate::kinds;
use crate::provider::{ExtId, ExtKey, IntoStateBits, StateBits};
use std::cell::RefCell;
use std::rc::Rc;

bits! {
    /// Kinds of change, for [`ChangeFilter::what`] and [`ChangeSet::by_node`].
    pub struct ChangeMask(u16) {
        /// `Created`, `Destroyed`.
        const CREATED = 1 << 0;
        /// `Inserted`, `Removed` (light children).
        const STRUCTURE = 1 << 1;
        /// `Prop`.
        const PROPS = 1 << 2;
        /// `State`.
        const STATE = 1 << 3;
        /// `Ext`.
        const EXT = 1 << 4;
        /// `Anchor`.
        const ANCHOR = 1 << 5;
        /// `Layout` records and `Damage::relayout`.
        const LAYOUT = 1 << 6;
        /// `Root`.
        const ROOT = 1 << 7;
    }
}

impl ChangeMask {
    pub const ALL: ChangeMask = ChangeMask(0xff);
    /// The kinds of change an [`Origin`] filter applies to (layout and root records have no origin).
    const MUTATIONS: ChangeMask = ChangeMask(0x3f);
}

bits! {
    /// Who wrote, for [`ChangeFilter::origins`].
    pub struct OriginMask(u8) {
        const JS = 1 << 0;
        const WIDGET = 1 << 1;
        const RUST = 1 << 2;
        const STYLE = 1 << 3;
        const ANIMATOR = 1 << 4;
        const HOST = 1 << 5;
    }
}

impl OriginMask {
    pub const ALL: OriginMask = OriginMask(0x3f);

    pub fn of(o: Origin) -> OriginMask {
        match o {
            Origin::Js => OriginMask::JS,
            Origin::Widget => OriginMask::WIDGET,
            Origin::Rust(_) => OriginMask::RUST,
            Origin::Style => OriginMask::STYLE,
            Origin::Animator => OriginMask::ANIMATOR,
            Origin::Host => OriginMask::HOST,
        }
    }
}

/// What happened to an extension entry.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum ExtChange {
    Added,
    Changed,
    /// Removed through `ExtEntry::remove`, or with its node (before `Destroyed`).
    Removed,
}

/// What happened to a root.
#[non_exhaustive]
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum RootChange {
    /// A window was created or a slot made (its top is in the record's root).
    Added,
    /// The root ended (its top destroyed, or the slot closed); its tree is detached or freed.
    Removed,
    /// Shown or hidden (the embed's `set_visible`).
    Visible(bool),
    /// Drawn in another viewport (the embed's `set_viewport`).
    Viewport(egui::ViewportId),
}

/// One change, in mutation order. Names what changed; read the new values from the scene.
#[non_exhaustive]
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum Change {
    Created { node: NodeId, kind: KindId, owner: Owner },
    /// `node` now is a light child of `parent`, before `before` (last if `None`). A move within or
    /// between parents is `Removed` then `Inserted`.
    Inserted { parent: NodeId, node: NodeId, before: Option<NodeId> },
    Removed { parent: NodeId, node: NodeId },
    /// The node is freed (its children, if alive, were `Removed` first).
    Destroyed { node: NodeId, kind: KindId },
    Prop { node: NodeId, prop: PropId, origin: Origin },
    State { node: NodeId, before: StateBits, after: StateBits },
    Ext { node: NodeId, ext: ExtId, what: ExtChange },
    /// The node's `anchor` changed (also a `Prop` record); `root` is its root then.
    Anchor { node: NodeId, root: RootIx },
    /// Its root-relative rect changed in a layout.
    Layout { node: NodeId, before: egui::Rect, after: egui::Rect },
    Root { root: RootIx, what: RootChange },
    /// The records after it, up to the next `Batch`, are one batch (one tick).
    Batch { label: &'static str, tick: Tick },
}

impl Change {
    /// The kind of change ([`ChangeMask::EMPTY`] for `Batch`).
    pub fn mask(&self) -> ChangeMask {
        match self {
            Change::Created { .. } | Change::Destroyed { .. } => ChangeMask::CREATED,
            Change::Inserted { .. } | Change::Removed { .. } => ChangeMask::STRUCTURE,
            Change::Prop { .. } => ChangeMask::PROPS,
            Change::State { .. } => ChangeMask::STATE,
            Change::Ext { .. } => ChangeMask::EXT,
            Change::Anchor { .. } => ChangeMask::ANCHOR,
            Change::Layout { .. } => ChangeMask::LAYOUT,
            Change::Root { .. } => ChangeMask::ROOT,
            Change::Batch { .. } => ChangeMask::EMPTY,
        }
    }

    /// The node the record is about (`None` for `Root` and `Batch`).
    pub fn node(&self) -> Option<NodeId> {
        match *self {
            Change::Created { node, .. }
            | Change::Inserted { node, .. }
            | Change::Removed { node, .. }
            | Change::Destroyed { node, .. }
            | Change::Prop { node, .. }
            | Change::State { node, .. }
            | Change::Ext { node, .. }
            | Change::Anchor { node, .. }
            | Change::Layout { node, .. } => Some(node),
            Change::Root { .. } | Change::Batch { .. } => None,
        }
    }

    /// The node scopes are checked against: the parent for structure records, else the node.
    fn subject(&self) -> Option<NodeId> {
        match *self {
            Change::Inserted { parent, .. } | Change::Removed { parent, .. } => Some(parent),
            _ => self.node(),
        }
    }
}

/// Where an observer looks: a root, or a node's subtree (light children, at record time).
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Scope {
    Root(RootIx),
    Subtree(NodeId),
}

impl From<RootIx> for Scope {
    fn from(r: RootIx) -> Scope {
        Scope::Root(r)
    }
}

impl From<NodeId> for Scope {
    fn from(n: NodeId) -> Scope {
        Scope::Subtree(n)
    }
}

/// What an observer wants. Empty lists and masks mean "any".
#[derive(Clone, Debug)]
pub struct ChangeFilter {
    pub scope: Option<Scope>,
    pub what: ChangeMask,
    /// Records about nodes of these kinds only.
    pub kinds: Vec<KindId>,
    /// `Ext` records of these keys only.
    pub ext: Vec<ExtId>,
    /// `Prop` records of these props only.
    pub props: Vec<PropId>,
    /// `State` records that change one of these bits only.
    pub states: StateBits,
    /// Records written by these origins only (mutations; `Layout` and `Root` records always pass).
    pub origins: OriginMask,
    /// Not the records written by this provider (one's own writes).
    pub ignore: Option<ProviderId>,
    /// Queue length before overflow.
    pub capacity: usize,
}

impl Default for ChangeFilter {
    /// Everything, from everyone, anywhere.
    fn default() -> Self {
        ChangeFilter {
            scope: None,
            what: ChangeMask::ALL,
            kinds: Vec::new(),
            ext: Vec::new(),
            props: Vec::new(),
            states: StateBits::NONE,
            origins: OriginMask::ALL,
            ignore: None,
            capacity: 1 << 16,
        }
    }
}

impl ChangeFilter {
    /// Every kind of change ([`ChangeFilter::default`]).
    pub fn all() -> Self {
        Self::default()
    }
    /// Only these kinds of change.
    pub fn only(what: ChangeMask) -> Self {
        ChangeFilter { what, ..Self::default() }
    }
    /// Only `Ext` records of `key` (add more with [`ChangeFilter::ext`]).
    pub fn ext_of<T: 'static>(key: &ExtKey<T>) -> Self {
        Self::only(ChangeMask::EXT).ext(key)
    }
    pub fn scope(mut self, s: impl Into<Scope>) -> Self {
        self.scope = Some(s.into());
        self
    }
    pub fn kind(mut self, k: KindId) -> Self {
        self.kinds.push(k);
        self
    }
    pub fn ext<T: 'static>(mut self, key: &ExtKey<T>) -> Self {
        self.ext.push(key.id());
        self
    }
    pub fn prop(mut self, p: PropId) -> Self {
        self.props.push(p);
        self
    }
    pub fn states(mut self, bits: impl IntoStateBits) -> Self {
        self.states = self.states | bits.state_bits();
        self
    }
    pub fn origins(mut self, o: OriginMask) -> Self {
        self.origins = o;
        self
    }
    /// Skips the records `p` writes.
    pub fn ignore(mut self, p: ProviderId) -> Self {
        self.ignore = Some(p);
        self
    }
    pub fn capacity(mut self, n: usize) -> Self {
        self.capacity = n.max(1);
        self
    }
}

/// What to redo since the observer's last take: a hint for caches outside the scene (AccessKit
/// trees, textures, a remote inspector).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Damage {
    /// Layout boundaries (or root tops, for a whole-root layout) laid out, in scope, if the observer
    /// wants `LAYOUT`.
    pub relayout: Vec<NodeId>,
    /// Per viewport, the union of the screen rects last drawn by nodes with records (old positions;
    /// new ones follow from the next frame).
    pub repaint: Vec<(egui::ViewportId, egui::Rect)>,
    /// Nodes whose accessibility data may have changed (props, state, children, layout), sorted.
    pub a11y: Vec<NodeId>,
}

impl Damage {
    pub fn is_empty(&self) -> bool {
        self.relayout.is_empty() && self.repaint.is_empty() && self.a11y.is_empty()
    }
}

/// What an observer gets from a take.
#[derive(Clone, Debug, Default)]
pub struct ChangeSet {
    /// In mutation order.
    pub records: Vec<Change>,
    /// The queue overflowed since the last take: records are missing, re-read what you need.
    pub overflowed: bool,
    pub damage: Damage,
}

impl ChangeSet {
    pub fn is_empty(&self) -> bool {
        self.records.is_empty() && !self.overflowed && self.damage.is_empty()
    }

    /// Coalesced per node: one entry per node with the union of what changed, in first-change
    /// order (MutationObserver records, reduced). `Root` and `Batch` records are left out.
    pub fn by_node(&self) -> impl Iterator<Item = (NodeId, ChangeMask)> + '_ {
        let mut out: Vec<(NodeId, ChangeMask)> = Vec::new();
        let mut at: std::collections::HashMap<NodeId, usize> = std::collections::HashMap::new();
        for r in &self.records {
            let Some(n) = r.node() else { continue };
            match at.get(&n) {
                Some(&k) => out[k].1 |= r.mask(),
                None => {
                    at.insert(n, out.len());
                    out.push((n, r.mask()));
                }
            }
        }
        out.into_iter()
    }
}

/// One observer's queue (shared by the [`Observer`] and the journal).
pub(crate) struct Queue {
    filter: ChangeFilter,
    /// `filter.kinds` as kind indexes.
    kinds: Vec<u32>,
    records: Vec<Change>,
    overflowed: bool,
    damage: Damage,
    last_batch: u64,
}

impl Queue {
    fn accepts(&self, core: &[NodeCore], c: &Change, mask: ChangeMask, origin: Origin) -> bool {
        let f = &self.filter;
        if !f.what.intersects(mask) {
            return false;
        }
        if ChangeMask::MUTATIONS.intersects(mask) {
            if !f.origins.intersects(OriginMask::of(origin)) {
                return false;
            }
            if let (Some(p), Origin::Rust(o)) = (f.ignore, origin)
                && p == o
            {
                return false;
            }
        }
        match *c {
            Change::Ext { ext, .. } if !f.ext.is_empty() && !f.ext.contains(&ext) => return false,
            Change::Prop { prop, .. } if !f.props.is_empty() && !f.props.contains(&prop) => return false,
            Change::State { before, after, .. } if !f.states.is_empty() && (before.0 ^ after.0) & f.states.0 == 0 => return false,
            _ => {}
        }
        if !self.kinds.is_empty()
            && let Some(n) = c.node()
            && !self.kinds.contains(&core[n.index() as usize].kind)
        {
            return false;
        }
        match (f.scope, c) {
            (None, _) => true,
            (Some(Scope::Root(r)), Change::Root { root, .. }) => r == *root,
            (Some(Scope::Subtree(_)), Change::Root { .. }) => false,
            (Some(s), _) => c.subject().is_some_and(|n| in_scope(core, s, n)),
        }
    }

    fn push(&mut self, core: &[NodeCore], roots: &RootTable, c: Change, tick: u64, batch: Option<&'static str>) {
        if self.overflowed {
            return;
        }
        if self.records.len() + 2 > self.filter.capacity {
            self.records.clear();
            self.damage = Damage::default();
            self.overflowed = true;
            return;
        }
        if let Some(label) = batch
            && self.last_batch != tick
        {
            self.last_batch = tick;
            self.records.push(Change::Batch { label, tick: Tick(tick) });
        }
        self.records.push(c);
        // damage: where the subject was drawn, and what accessibility must look at again
        let Some(s) = c.subject() else { return };
        let sc = &core[s.index() as usize];
        if sc.drawn != egui::Rect::NOTHING {
            let vp = roots.get(sc.root).map_or(egui::ViewportId::ROOT, |e| e.viewport);
            match self.damage.repaint.iter_mut().find(|(v, _)| *v == vp) {
                Some((_, r)) => *r = r.union(sc.drawn),
                None => self.damage.repaint.push((vp, sc.drawn)),
            }
        }
        if !matches!(c, Change::Created { .. } | Change::Destroyed { .. } | Change::Ext { .. }) && self.damage.a11y.last() != Some(&s) {
            self.damage.a11y.push(s);
        }
    }

    fn take(&mut self) -> ChangeSet {
        let mut damage = std::mem::take(&mut self.damage);
        damage.a11y.sort();
        damage.a11y.dedup();
        damage.relayout.sort();
        damage.relayout.dedup();
        let set = ChangeSet { records: std::mem::take(&mut self.records), overflowed: self.overflowed, damage };
        self.overflowed = false;
        set
    }
}

/// True if `n` is in scope `s` (`n` live).
fn in_scope(core: &[NodeCore], s: Scope, n: NodeId) -> bool {
    match s {
        Scope::Root(r) => core[n.index() as usize].root == r,
        Scope::Subtree(top) => {
            let mut a = Some(n);
            while let Some(x) = a {
                if x == top {
                    return true;
                }
                a = core[x.index() as usize].parent;
            }
            false
        }
    }
}

/// A pull observer of one scene's changes. Drop it to unsubscribe.
pub struct Observer {
    q: Rc<RefCell<Queue>>,
}

impl Observer {
    /// The records since the last take (also [`SceneHandle::take_changes`](super::SceneHandle::take_changes)).
    /// Works in any phase, including inside a `write`.
    pub fn take(&self) -> ChangeSet {
        self.q.borrow_mut().take()
    }

    /// Records waiting.
    pub fn pending(&self) -> usize {
        self.q.borrow().records.len()
    }

    pub fn filter(&self) -> ChangeFilter {
        self.q.borrow().filter.clone()
    }
}

impl std::fmt::Debug for Observer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Observer({} pending)", self.pending())
    }
}

/// The scene's observers and the union of their interests.
#[derive(Default)]
pub(crate) struct Journal {
    observers: Vec<Rc<RefCell<Queue>>>,
    want: ChangeMask,
    /// `Ext` interest: any key, or these.
    ext_any: bool,
    ext_ids: Vec<ExtId>,
    /// `State` interest: bits whose change some observer wants.
    states: u64,
    /// The label of the explicit batch running (`write`, `defer`, `SceneMut::batch`).
    pub batch: Option<&'static str>,
}

impl Journal {
    pub fn observe(&mut self, filter: ChangeFilter) -> Observer {
        let kinds = filter.kinds.iter().filter_map(|k| kinds::kind_of_id(k.0)).map(|k| k as u32).collect();
        let q = Rc::new(RefCell::new(Queue { filter, kinds, records: Vec::new(), overflowed: false, damage: Damage::default(), last_batch: 0 }));
        self.observers.push(q.clone());
        self.recompute();
        Observer { q }
    }

    /// Live observers.
    pub fn len(&self) -> usize {
        self.observers.iter().filter(|q| Rc::strong_count(q) > 1).count()
    }

    fn recompute(&mut self) {
        self.observers.retain(|q| Rc::strong_count(q) > 1);
        self.want = ChangeMask::EMPTY;
        self.ext_any = false;
        self.ext_ids.clear();
        self.states = 0;
        for q in &self.observers {
            let f = &q.borrow().filter;
            self.want |= f.what;
            if f.what.intersects(ChangeMask::EXT) {
                if f.ext.is_empty() {
                    self.ext_any = true;
                } else {
                    self.ext_ids.extend(f.ext.iter().copied());
                }
            }
            if f.what.intersects(ChangeMask::STATE) {
                self.states |= if f.states.is_empty() { u64::MAX } else { f.states.0 };
            }
        }
    }

    /// Could a record of kind `m` interest a live observer? (The mutation paths' one test.)
    #[inline]
    pub fn wants(&self, m: ChangeMask) -> bool {
        self.want.intersects(m)
    }

    #[inline]
    pub fn wants_ext(&self, ext: ExtId) -> bool {
        self.want.intersects(ChangeMask::EXT) && (self.ext_any || self.ext_ids.contains(&ext))
    }

    #[inline]
    pub fn wants_state(&self, before: StateBits, after: StateBits) -> bool {
        self.want.intersects(ChangeMask::STATE) && (before.0 ^ after.0) & self.states != 0
    }

    /// Pushes `c` (written by `origin` at `tick`) onto every observer whose filter takes it. `core`
    /// is the scene's hot column, with every node `c` names still live.
    pub fn record(&mut self, core: &[NodeCore], roots: &RootTable, c: Change, origin: Origin, tick: u64) {
        let mask = c.mask();
        if !self.want.intersects(mask) {
            return;
        }
        let mut dead = false;
        for q in &self.observers {
            if Rc::strong_count(q) == 1 {
                dead = true;
                continue;
            }
            let mut q = q.borrow_mut();
            if q.accepts(core, &c, mask, origin) {
                q.push(core, roots, c, tick, self.batch);
            }
        }
        if dead {
            self.recompute();
        }
    }

    /// Boundary `b` (or a root's top) was laid out: `Damage::relayout` of the observers that want
    /// `LAYOUT` in its scope.
    pub fn laid_out(&mut self, core: &[NodeCore], b: NodeId) {
        if !self.want.intersects(ChangeMask::LAYOUT) {
            return;
        }
        for q in &self.observers {
            if Rc::strong_count(q) == 1 {
                continue;
            }
            let mut q = q.borrow_mut();
            if q.overflowed || !q.filter.what.intersects(ChangeMask::LAYOUT) || !q.filter.scope.is_none_or(|s| in_scope(core, s, b)) {
                continue;
            }
            if !q.kinds.is_empty() && !q.kinds.contains(&core[b.index() as usize].kind) {
                continue;
            }
            q.damage.relayout.push(b);
        }
    }
}
