//! Identities (ARCH-SCENE §4.2): [`NodeId`] (the generational node id, equal to the ABI handle of
//! nodes a runtime creates), the slot table behind it, [`VNodeId`]/[`Target`] for painted rows (SG5),
//! and the small ids of the access API ([`KindId`], [`PropId`], [`RootIx`], [`Tick`], [`ProviderId`]).
//!
//! A node id is `generation << 24 | index`. Generations start at 1 (so an id is never 0) and a slot
//! is retired instead of recycled once its next generation would reach 2^28: ids stay below 2^52, so
//! they never collide with the host's app-export handles in [2^52, 2^53)
//! (`svelters-host/src/exports/mod.rs`, `APP_HANDLE_BIT`), and an id is never reused.
//! Writer lane: SCENE.

use std::num::NonZeroU64;
use std::sync::{Mutex, OnceLock};
use svelters_abi::{svl_handle, svl_id};

/// The ABI handle type; equal to [`NodeId::to_raw`] for in-process nodes.
pub type Handle = svl_handle;

pub(crate) const IDX_BITS: u32 = 24;
pub(crate) const IDX_MASK: u64 = (1 << IDX_BITS) - 1;
/// The first generation that is never handed out: `MAX_GEN << 24 == 1 << 52`, the app-handle bit.
pub(crate) const MAX_GEN: u32 = 1 << 28;
/// Live slots at most (index bits).
pub(crate) const MAX_SLOTS: usize = 1 << IDX_BITS;

/// A live or stale node. Equal to the ABI `svl_handle` of nodes created by an in-process runtime.
/// A stale id (its node destroyed) fails every lookup: ids are never reused.
#[repr(transparent)]
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(NonZeroU64);

impl NodeId {
    pub(crate) const fn new(index: usize, generation: u32) -> NodeId {
        match NonZeroU64::new(((generation as u64) << IDX_BITS) | index as u64) {
            Some(v) => NodeId(v),
            None => panic!("generation 0 is never handed out"),
        }
    }
    /// The slot, `< 2^24`.
    pub const fn index(self) -> u32 {
        (self.0.get() & IDX_MASK) as u32
    }
    pub const fn generation(self) -> u32 {
        (self.0.get() >> IDX_BITS) as u32
    }
    /// The ABI handle.
    pub const fn to_raw(self) -> u64 {
        self.0.get()
    }
    /// The id of an ABI handle; `None` for 0 and for handles outside the node range (app handles).
    pub const fn from_raw(raw: u64) -> Option<NodeId> {
        if raw >> IDX_BITS >= MAX_GEN as u64 {
            return None;
        }
        match NonZeroU64::new(raw) {
            Some(v) => Some(NodeId(v)),
            None => None,
        }
    }
}

impl std::fmt::Debug for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "#{}", self.0)
    }
}

impl std::fmt::Display for NodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "#{}", self.0)
    }
}

/// Compares with a raw handle (tests and ops compare child lists with handles).
impl PartialEq<u64> for NodeId {
    fn eq(&self, other: &u64) -> bool {
        self.0.get() == *other
    }
}

impl From<NodeId> for u64 {
    fn from(id: NodeId) -> u64 {
        id.to_raw()
    }
}

/// A row painted by a virtualising widget, not a node (ARCH-SCENE §8.2, SG5). `key` is the model's
/// stable row key.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct VNodeId {
    pub host: NodeId,
    pub key: u64,
}

/// Either; what hit-tests, anchors and queries return.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Target {
    Node(NodeId),
    Virtual(VNodeId),
}

impl Target {
    /// The node: itself, or the host of a virtual part.
    pub fn node(self) -> NodeId {
        match self {
            Target::Node(n) => n,
            Target::Virtual(v) => v.host,
        }
    }
}

/// A kind's ABI id (ARCH-EMBED §8.1).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct KindId(pub svl_id);

/// A prop's ABI id.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct PropId(pub svl_id);

/// An index into the scene's root table. [`RootIx::DETACHED`]: not under any root.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct RootIx(pub(crate) u32);

impl RootIx {
    pub const DETACHED: RootIx = RootIx(u32::MAX);
    pub fn is_detached(self) -> bool {
        self == RootIx::DETACHED
    }
}

/// The scene's mutation counter: every mutation batch (one `write`, one `batch`, one host op) gets a
/// new tick, and nodes remember the tick of their last change.
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug, Default)]
pub struct Tick(pub u64);

/// Who wrote, for Rust code: a name per crate or module (debug, dumps, MCP), interned process-wide.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct ProviderId(u16);

fn provider_names() -> &'static Mutex<Vec<&'static str>> {
    static NAMES: OnceLock<Mutex<Vec<&'static str>>> = OnceLock::new();
    NAMES.get_or_init(|| Mutex::new(vec!["app"]))
}

impl ProviderId {
    /// The default writer (an application's own code).
    pub const APP: ProviderId = ProviderId(0);

    /// The id of `name`, the same for the same name everywhere in the process.
    pub fn named(name: &'static str) -> ProviderId {
        let mut names = provider_names().lock().unwrap();
        if let Some(i) = names.iter().position(|n| *n == name) {
            return ProviderId(i as u16);
        }
        assert!(names.len() < u16::MAX as usize, "too many provider names");
        names.push(name);
        ProviderId((names.len() - 1) as u16)
    }

    pub fn name(self) -> &'static str {
        provider_names().lock().unwrap().get(self.0 as usize).copied().unwrap_or("?")
    }
}

/// Generation per slot, liveness, free list and the live count (O(1) `len`).
#[derive(Default)]
pub(crate) struct SlotTable {
    gens: Vec<u32>,
    live: Vec<bool>,
    free: Vec<u32>,
    len: usize,
}

impl SlotTable {
    /// The slot of a live id.
    #[inline]
    pub fn index(&self, id: NodeId) -> Option<usize> {
        let i = id.index() as usize;
        (i < self.gens.len() && self.live[i] && self.gens[i] == id.generation()).then_some(i)
    }

    /// The id of the live node in slot `i`.
    #[inline]
    pub fn id_at(&self, i: usize) -> Option<NodeId> {
        (i < self.live.len() && self.live[i]).then(|| NodeId::new(i, self.gens[i]))
    }

    /// Takes a slot: a fresh one, or a freed one with its next generation. Returns its id and
    /// whether it is new (the columns must grow).
    pub fn alloc(&mut self) -> (NodeId, bool) {
        let (i, fresh) = match self.free.pop() {
            Some(i) => (i as usize, false),
            None => {
                assert!(self.gens.len() < MAX_SLOTS, "svelters-egui: more than 2^24 live nodes");
                self.gens.push(0);
                self.live.push(false);
                (self.gens.len() - 1, true)
            }
        };
        self.gens[i] += 1;
        debug_assert!(self.gens[i] < MAX_GEN);
        self.live[i] = true;
        self.len += 1;
        (NodeId::new(i, self.gens[i]), fresh)
    }

    /// Frees a live slot. It is recycled unless its next generation would reach [`MAX_GEN`].
    pub fn free(&mut self, i: usize) {
        debug_assert!(self.live[i]);
        self.live[i] = false;
        self.len -= 1;
        if self.gens[i] + 1 < MAX_GEN {
            self.free.push(i as u32);
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    /// Slots ever used (live or not).
    pub fn capacity(&self) -> usize {
        self.gens.len()
    }

    #[cfg(test)]
    pub fn free_len(&self) -> usize {
        self.free.len()
    }

    #[cfg(test)]
    pub fn set_generation(&mut self, i: usize, g: u32) {
        self.gens[i] = g;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_pack_and_reject_app_handles() {
        let id = NodeId::new(5, 3);
        assert_eq!(id.index(), 5);
        assert_eq!(id.generation(), 3);
        assert_eq!(NodeId::from_raw(id.to_raw()), Some(id));
        assert_eq!(NodeId::from_raw(0), None);
        assert_eq!(NodeId::from_raw(1 << 52), None, "app-export handles are not node ids");
        assert!(NodeId::new(MAX_SLOTS - 1, MAX_GEN - 1).to_raw() < 1 << 52);
    }

    /// The handle-collision fix (ARCH-SCENE §2, §4.2): the last generation a slot hands out is
    /// 2^28 - 1, so no node handle ever has the host's app-handle bit 52.
    #[test]
    fn generations_retire_below_the_app_handle_bit() {
        let mut t = SlotTable::default();
        let (a, _) = t.alloc();
        t.free(a.index() as usize);
        t.set_generation(0, MAX_GEN - 2);
        let (b, fresh) = t.alloc();
        assert!(!fresh);
        assert_eq!(b.generation(), MAX_GEN - 1);
        assert_eq!(b.to_raw() & (1 << 52), 0);
        t.free(0);
        assert_eq!(t.free_len(), 0, "the slot is retired, not recycled");
        let (c, fresh) = t.alloc();
        assert!(fresh);
        assert_eq!(c.index(), 1);
    }
}
