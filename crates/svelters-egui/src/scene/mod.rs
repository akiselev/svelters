//! The retained scene (ARCH-SCENE): one arena of nodes stored as parallel columns per slot, a root
//! table, per-root dirty queues, and the access API cadatomic uses from Rust.
//!
//! Host ops only edit this scene; they never touch egui. Every edit, from a runtime (`ops.rs`,
//! [`Origin::Js`]) or from Rust ([`SceneMut`]), goes through the same crate-private functions in
//! [`tree`], [`dirty`] and `props.rs`, which keep the invariants [`Scene::validate`] checks.
//!
//! | File | Content |
//! |---|---|
//! | `mod.rs` | [`Scene`] (columns, roots, counters), [`Phase`], `validate` |
//! | `id.rs` | [`NodeId`] (the ABI handle), the slot table, [`VNodeId`], [`Target`], small ids |
//! | `node.rs` | `NodeCore`, `LayoutBox`, `Payload`, `NodeCold`, [`NodeFlags`], [`Dirty`], [`StateBits`], [`Owner`], [`W`], [`Common`], [`Visual`] |
//! | `tree.rs` | create, insert, remove, destroy, free; the root cache and the root table ([`TreeRoot`]) |
//! | `props.rs` | built-in prop values (writer: WIDGETS) |
//! | `keys.rs` | [`PropKey`] and the built-in keys ([`keys`]) |
//! | `dirty.rs` | dirty bits, layout boundaries, per-root queues |
//! | `access.rs` | [`SceneHandle`], [`SceneRef`], [`NodeRef`], [`SceneMut`], [`NodeMut`], errors |
//! | `dump.rs` | [`SceneRef::dump`], [`SceneStats`] |
//! | `ext.rs` | [`ExtKey`] and one typed column per key (sparse or dense), [`ExtEntry`], [`ExtTicks`] (SG2) |
//! | `journal.rs` | [`Change`], [`Observer`], [`ChangeFilter`], [`ChangeSet`], [`Damage`] (SG2) |
//! | `query.rs` | kind sets, anchor maps, [`Query`], [`Cursor`], [`Visit`] (SG3) |
//! | `hit.rs` | per-viewport hit lists, [`SceneRef::hit_test`], [`Hit`], [`HitCx`] (SG3) |
//! | `virt.rs` | seam for SG5 |
//!
//! Writer lane: SCENE.

pub(crate) mod access;
pub(crate) mod dirty;
pub(crate) mod dump;
pub(crate) mod ext;
pub(crate) mod hit;
pub(crate) mod id;
pub mod keys;
pub(crate) mod journal;
#[cfg(test)]
mod journal_tests;
pub(crate) mod node;
pub(crate) mod props;
pub(crate) mod query;
#[cfg(test)]
mod tests;
pub(crate) mod tree;
pub(crate) mod virt;

pub use access::{Children, Descendants, NodeMut, NodeRef, Origin, PropError, SceneBusy, SceneError, SceneHandle, SceneMut, SceneRef};
pub use hit::{Hit, HitCx};
pub use query::{Cursor, Query, Visit};
pub use journal::{Change, ChangeFilter, ChangeMask, ChangeSet, Damage, ExtChange, Observer, OriginMask, RootChange, Scope};
pub use dump::{DumpOpts, SceneStats};
pub use id::{Handle, KindId, NodeId, PropId, ProviderId, RootIx, Target, Tick, VNodeId};
pub use keys::{PropKey, PropValue};
pub use crate::provider::{ExtBlock, ExtEntry, ExtId, ExtKey, ExtTicks, IntoStateBits, StateBit, StateBits, Storage};
pub use node::{Common, Dirty, Node, NodeFlags, Owner, Visual, W};
pub use props::{default_value, get_prop, set_prop, tag_of};
pub use tree::TreeRoot;

use crate::kinds;
use dump::Counters;
use ext::{ExtColumns, ExtCx};
use id::SlotTable;
use journal::Journal;
use node::{DEFAULT_COLD, LayoutBox, NodeCold, NodeCore, Payload};
use std::sync::OnceLock;
use tree::RootTable;

/// The old name of the scene (pre-SG1 `Arena`), kept for `with_scene` callers.
pub type Arena = Scene;

/// Who may mutate the scene now (ARCH-SCENE §5.2 with R17).
#[non_exhaustive]
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Phase {
    /// Between frames, and in the app's own logic before the embed's.
    Idle,
    /// The embed's logic (closes, deferred writes, checkpoint, remeasure).
    Logic,
    /// From the end of logic to the end of the egui pass (R17): layout, the walk, the app's own ui
    /// between slots. `SceneHandle::write` fails with `SceneBusy`; `defer` works.
    Ui,
}

/// The scene: columns indexed by `NodeId::index()`, the root table and the counters.
pub struct Scene {
    pub(crate) slots: SlotTable,
    pub(crate) core: Vec<NodeCore>,
    /// Light children in order (layout and paint order).
    pub(crate) children: Vec<Vec<NodeId>>,
    pub(crate) layout: Vec<LayoutBox>,
    pub(crate) payload: Vec<Payload>,
    pub(crate) cold: Vec<Option<Box<NodeCold>>>,
    /// Provider data: one column per `ExtKey` (SG2).
    pub(crate) ext: ExtColumns,
    /// Change records for observers (SG2).
    pub(crate) journal: Journal,
    pub(crate) roots: RootTable,
    /// Kind sets (SG3; anchor maps are per root, in `RootEntry::anchors`).
    pub(crate) index: query::Indexes,
    /// Last pass's drawn nodes per viewport (SG3).
    pub(crate) hits: hit::HitLists,
    pub(crate) tick: u64,
    pub(crate) in_batch: bool,
    pub(crate) counters: Counters,
    /// Prop changes to announce to the host (`prop_changed`), from Rust writes (R18); flushed by the
    /// frame's logic.
    pub(crate) announce: Vec<(NodeId, u32)>,
    /// What a taken-out payload reads as (SG4, `Scene::w`).
    taken_w: W,
}

impl Default for Scene {
    fn default() -> Self {
        Scene {
            slots: SlotTable::default(),
            core: Vec::new(),
            children: Vec::new(),
            layout: Vec::new(),
            payload: Vec::new(),
            cold: Vec::new(),
            ext: ExtColumns::default(),
            journal: Journal::default(),
            roots: RootTable::default(),
            index: query::Indexes::default(),
            hits: hit::HitLists::default(),
            tick: 0,
            in_batch: false,
            counters: Counters::default(),
            announce: Vec::new(),
            taken_w: W::Box,
        }
    }
}

impl Scene {
    // ---- ids ----

    /// The slot of a live node.
    #[inline]
    pub(crate) fn idx(&self, id: NodeId) -> Option<usize> {
        self.slots.index(id)
    }

    /// The id of a live handle, or the host's "stale or foreign handle" error.
    pub(crate) fn live(&self, h: Handle) -> Result<NodeId, String> {
        NodeId::from_raw(h).filter(|&id| self.idx(id).is_some()).ok_or_else(|| format!("stale or foreign handle {h}"))
    }

    #[inline]
    pub(crate) fn ix(&self, id: NodeId) -> usize {
        self.idx(id).unwrap_or_else(|| panic!("svelters-egui: stale node {id}"))
    }

    /// A new tick for a mutation, or the batch's tick inside a batch.
    pub(crate) fn stamp(&mut self) -> id::Tick {
        if !self.in_batch {
            self.tick += 1;
        }
        id::Tick(self.tick)
    }

    // ---- journal ----

    /// Journals `c`, written by `origin`, if a live observer could want it (one mask test when none
    /// does). Every node `c` names must still be live.
    #[inline]
    pub(crate) fn record(&mut self, c: journal::Change, origin: Origin) {
        if self.journal.wants(c.mask()) {
            self.journal.record(&self.core, &self.roots, c, origin, self.tick);
        }
    }

    /// Journals a state change of node `i` (by the union of the observers' state-bit interests).
    pub(crate) fn record_state(&mut self, i: usize, before: StateBits, origin: Origin) {
        let after = self.core[i].state;
        if self.journal.wants_state(before, after)
            && let Some(node) = self.slots.id_at(i)
        {
            self.journal.record(&self.core, &self.roots, journal::Change::State { node, before, after }, origin, self.tick);
        }
    }

    /// The disjoint parts an extension write needs.
    pub(crate) fn ext_cx(&mut self) -> ExtCx<'_> {
        ExtCx { cols: &mut self.ext, journal: &mut self.journal, core: &mut self.core, roots: &self.roots, tick: &mut self.tick, in_batch: self.in_batch }
    }

    /// Enters an explicit batch (one tick, one `Change::Batch` marker): returns whether it is the
    /// outermost one, to pass to [`Scene::end_batch`].
    pub(crate) fn begin_batch(&mut self, label: &'static str) -> bool {
        if self.in_batch {
            return false;
        }
        self.tick += 1;
        self.in_batch = true;
        self.journal.batch = Some(label);
        true
    }

    /// Closes a batch a panic left open (SG4: provider code unwinding out of a batch).
    pub(crate) fn abort_batch(&mut self) {
        self.in_batch = false;
        self.journal.batch = None;
    }

    pub(crate) fn end_batch(&mut self, outer: bool) {
        if outer {
            self.in_batch = false;
            self.journal.batch = None;
        }
    }

    // ---- columns ----

    /// Node `i`'s payload. A provider's payload taken out while its own code runs (SG4: `ui`,
    /// `layout`, `prepare`) reads as an empty box: `NodeRef::widget` is `None` then, text is empty.
    pub(crate) fn w(&self, i: usize) -> &W {
        match &self.payload[i] {
            Payload::Builtin(w) => w,
            Payload::Vacant => &self.taken_w,
        }
    }


    pub(crate) fn w_mut(&mut self, i: usize) -> &mut W {
        match &mut self.payload[i] {
            Payload::Builtin(w) => w,
            Payload::Vacant => panic!("svelters-egui: vacant payload in slot {i}"),
        }
    }

    pub(crate) fn cold(&self, i: usize) -> &NodeCold {
        self.cold[i].as_deref().unwrap_or(&DEFAULT_COLD)
    }

    pub(crate) fn cold_mut(&mut self, i: usize) -> &mut NodeCold {
        self.cold[i].get_or_insert_with(|| Box::new(NodeCold::DEFAULT))
    }

    pub(crate) fn common(&self, i: usize) -> &Common {
        &self.cold(i).common
    }

    pub(crate) fn vis(&self, i: usize) -> Visual {
        if self.core[i].flags.contains(NodeFlags::HAS_VISUAL) { self.cold(i).vis } else { Visual::IDENTITY }
    }

    pub(crate) fn tag_at(&self, i: usize) -> &'static str {
        kinds::kind(self.core[i].kind as usize).tag.unwrap_or("?")
    }

    /// True if the node listens to `signal`.
    pub(crate) fn listens(&self, i: usize, signal: u32) -> bool {
        if signal == 0 {
            return false;
        }
        match member_bit(self.core[i].kind as usize, signal, Member::Signal) {
            Some(b) if b < 64 => self.core[i].listen & (1 << b) != 0,
            _ => self.cold(i).listen_overflow.contains(&signal),
        }
    }

    pub(crate) fn set_listen(&mut self, i: usize, signal: u32, on: bool) {
        match member_bit(self.core[i].kind as usize, signal, Member::Signal) {
            Some(b) if b < 64 => {
                if on {
                    self.core[i].listen |= 1 << b;
                } else {
                    self.core[i].listen &= !(1 << b);
                }
            }
            _ => {
                let v = &mut self.cold_mut(i).listen_overflow;
                v.retain(|&s| s != signal);
                if on {
                    v.push(signal);
                }
            }
        }
    }

    pub(crate) fn observes(&self, i: usize, prop: u32) -> bool {
        self.cold[i].as_deref().is_some_and(|c| c.observes(prop))
    }

    // ---- compat reads (pre-SG1 `Arena`; prefer `SceneRef`) ----

    /// A view of the live node `h`.
    pub fn get(&self, h: Handle) -> Option<Node<'_>> {
        let id = NodeId::from_raw(h)?;
        let i = self.idx(id)?;
        let c = &self.core[i];
        let lb = &self.layout[i];
        Some(Node {
            kind: c.kind as usize,
            handle: id,
            parent: c.parent,
            children: &self.children[i],
            id: c.egui_id,
            c: self.common(i),
            w: self.w(i),
            vis: self.vis(i),
            state: c.state,
            ext: ExtBlock::new(&self.ext, i as u32),
            rect: c.rect,
            drawn: c.drawn,
            presented: c.flags.contains(NodeFlags::PRESENTED),
            intrinsic: lb.intrinsic,
        })
    }

    /// [`Scene::get`] of a handle that must be live.
    pub fn n(&self, h: Handle) -> Node<'_> {
        self.get(h).unwrap_or_else(|| panic!("svelters-egui: stale handle {h}"))
    }

    pub fn contains(&self, h: Handle) -> bool {
        NodeId::from_raw(h).is_some_and(|id| self.idx(id).is_some())
    }

    /// Live nodes (O(1)).
    pub fn len(&self) -> usize {
        self.slots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The live handles, in slot order.
    pub fn handles(&self) -> Vec<Handle> {
        self.ids().map(NodeId::to_raw).collect()
    }

    /// The live ids, in slot order.
    pub(crate) fn ids(&self) -> impl Iterator<Item = NodeId> + '_ {
        (0..self.slots.capacity()).filter_map(|i| self.slots.id_at(i))
    }

    // ---- invariants ----

    /// Checks the scene's invariants (ARCH-SCENE §11): slot table and columns agree, parent/child
    /// symmetry, no cycles, the root cache, the root table, owners, and the dirty queues (every
    /// queued node is flagged and every flagged node queued; every node with `LAYOUT` reaches a
    /// queued boundary or a root due for a full layout). Returns every violation found.
    pub fn validate(&self) -> Result<(), String> {
        let mut errs: Vec<String> = Vec::new();
        let mut live = 0;
        for i in 0..self.slots.capacity() {
            let Some(id) = self.slots.id_at(i) else {
                if !matches!(self.payload[i], Payload::Vacant) || !self.children[i].is_empty() || self.cold[i].is_some() || self.ext.has_any(i as u32) {
                    errs.push(format!("free slot {i} keeps data"));
                }
                continue;
            };
            live += 1;
            let c = &self.core[i];
            if matches!(self.payload[i], Payload::Vacant) {
                errs.push(format!("{id}: vacant payload"));
            }
            // parent/child symmetry
            if let Some(p) = c.parent {
                match self.idx(p) {
                    None => errs.push(format!("{id}: stale parent {p}")),
                    Some(pi) => {
                        let n = self.children[pi].iter().filter(|&&x| x == id).count();
                        if n != 1 {
                            errs.push(format!("{id}: listed {n} times by its parent {p}"));
                        }
                        if self.core[pi].root != c.root {
                            errs.push(format!("{id}: root {:?} differs from its parent's {:?}", c.root, self.core[pi].root));
                        }
                    }
                }
            }
            for &k in &self.children[i] {
                match self.idx(k) {
                    None => errs.push(format!("{id}: stale child {k}")),
                    Some(ki) if self.core[ki].parent != Some(id) => errs.push(format!("{id}: child {k} has parent {:?}", self.core[ki].parent)),
                    _ => {}
                }
            }
            // no cycles: the parent chain ends within len steps
            let mut a = c.parent;
            let mut steps = 0;
            while let Some(p) = a {
                steps += 1;
                if p == id || steps > self.len() {
                    errs.push(format!("{id}: parent chain loops"));
                    break;
                }
                a = self.idx(p).and_then(|pi| self.core[pi].parent);
            }
            // root cache
            if c.parent.is_none() {
                let is_top = !c.root.is_detached();
                if is_top != c.flags.contains(NodeFlags::ROOT) {
                    errs.push(format!("{id}: parentless with root {:?} but ROOT flag {}", c.root, c.flags.contains(NodeFlags::ROOT)));
                }
                if is_top && self.roots.get(c.root).map(|e| e.top) != Some(id) {
                    errs.push(format!("{id}: root {:?} does not name it as its top", c.root));
                }
            } else if c.flags.contains(NodeFlags::ROOT) {
                errs.push(format!("{id}: ROOT flag on a child"));
            }
            if c.owner == Owner::Js && c.flags.contains(NodeFlags::ROOT) && !matches!(self.w(i), W::Window { .. }) {
                errs.push(format!("{id}: a runtime-owned root that is not a window"));
            }
            // queues
            if !c.root.is_detached() {
                match self.roots.get(c.root) {
                    None => errs.push(format!("{id}: root {:?} is not in the root table", c.root)),
                    Some(e) => {
                        if c.flags.contains(NodeFlags::IN_RELAYOUT) && !e.dirty.relayout.contains(&id) {
                            errs.push(format!("{id}: IN_RELAYOUT but not queued"));
                        }
                        if c.flags.contains(NodeFlags::IN_REMEASURE) && !e.dirty.remeasure.contains(&id) {
                            errs.push(format!("{id}: IN_REMEASURE but not queued"));
                        }
                        if c.dirty.intersects(Dirty::LAYOUT | Dirty::DESC_LAYOUT) && !e.dirty.full && e.laid_out() {
                            let mut a = Some(id);
                            let mut ok = false;
                            while let Some(x) = a {
                                let xi = self.ix(x);
                                if self.core[xi].flags.contains(NodeFlags::IN_RELAYOUT) {
                                    ok = true;
                                    break;
                                }
                                a = self.core[xi].parent;
                            }
                            if !ok {
                                errs.push(format!("{id}: dirty layout {:?} reaches no queued boundary", c.dirty));
                            }
                        }
                    }
                }
            } else if c.flags.intersects(NodeFlags::IN_RELAYOUT | NodeFlags::IN_REMEASURE) {
                errs.push(format!("{id}: detached but queued {:?}", c.flags));
            }
        }
        for (k, id) in self.ext.entries() {
            if self.idx(id).is_none() {
                errs.push(format!("ext {}: entry of a freed node {id}", k.name()));
            }
        }
        if live != self.len() {
            errs.push(format!("live count {} but {live} live slots", self.len()));
        }
        self.index.check(self, &mut errs);
        self.check_anchors(&mut errs);
        for (rix, e) in self.roots.iter() {
            match self.idx(e.top) {
                None => errs.push(format!("root {rix:?}: stale top {}", e.top)),
                Some(ti) => {
                    if self.core[ti].root != rix || self.core[ti].parent.is_some() {
                        errs.push(format!("root {rix:?}: top {} has root {:?}, parent {:?}", e.top, self.core[ti].root, self.core[ti].parent));
                    }
                }
            }
        }
        if errs.is_empty() { Ok(()) } else { Err(errs.join("\n")) }
    }
}

// ---- member ordinals (listen and explicit masks) ----

#[derive(Copy, Clone, PartialEq, Eq)]
pub(crate) enum Member {
    Prop,
    Signal,
}

/// The ordinal of a member id among the props (or signals) of `kind` and its bases, most derived
/// first; the bit it gets in `NodeCore::explicit` (or `listen`).
pub(crate) fn member_bit(kind: usize, id: u32, m: Member) -> Option<u32> {
    static TABLE: OnceLock<Vec<(Vec<u32>, Vec<u32>)>> = OnceLock::new();
    let t = TABLE.get_or_init(|| {
        kinds::KindRegistry::global().indexes()
            .map(|k| {
                let mut props = Vec::new();
                let mut sigs = Vec::new();
                for kk in kinds::chain(k) {
                    for p in kinds::kind(kk).props {
                        props.push(kinds::find_prop(k, p.name).map(|(id, _)| id).unwrap_or(0));
                    }
                    for s in kinds::kind(kk).signals {
                        sigs.push(kinds::find_signal(k, s.name).map(|(id, _)| id).unwrap_or(0));
                    }
                }
                (props, sigs)
            })
            .collect()
    });
    let (props, sigs) = t.get(kind)?;
    let v = if m == Member::Prop { props } else { sigs };
    v.iter().position(|&x| x == id).map(|p| p as u32)
}
