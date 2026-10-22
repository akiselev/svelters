//! Extension storage: [`ExtKey`] and one typed column per key (ARCH-EMBED §8.4, ARCH-SCENE §4.6,
//! S2(a)).
//!
//! A provider declares a key once, as a static, and reads and writes typed data on any node through
//! it:
//!
//! ```ignore
//! pub static OP: ExtKey<OpView> = ExtKey::new("cad.op");
//! n.ext_mut(&OP).set(op_view);
//! let op: Option<&OpView> = n.ext(&OP);
//! for (node, op) in scene.with_ext(&OP) { … }        // O(entries), not O(nodes)
//! ```
//!
//! The API names a key and a node, never the storage (RA1's API, unchanged by SG2). Behind it each
//! key has its own column in the scene, created on first insert:
//!
//! - [`Storage::Sparse`] ([`ExtKey::new`]): a sparse set (Bevy's `SparseSet`): slot → dense position,
//!   then the node ids, values and change ticks packed densely. Access O(1), iteration over the entries
//!   only, removal by swap-remove.
//! - [`Storage::Dense`] ([`ExtKey::dense`]): `Vec<Option<T>>` by slot, for keys most nodes carry.
//!   Iteration is over the slots.
//!
//! Every entry has [`ExtTicks`] (added, changed). Writes go through [`ExtEntry`], which stamps the
//! node's tick and journals `Change::Ext { Added | Changed | Removed }`; a node freed with entries
//! journals `Removed` for each of them before `Destroyed` (R19: observers can track a marked set).
//! The columns hold `Box<dyn ErasedColumn>`, downcast with `Any` once per access (no `unsafe`).
//!
//! Writer lane: SCENE (SG2; RA1 wrote the key and entry API).

use super::Node;
use super::access::Origin;
use super::id::{NodeId, Tick};
use super::journal::{Change, ExtChange, Journal};
use super::node::NodeCore;
use super::tree::RootTable;
use std::any::Any;
use std::marker::PhantomData;
use std::sync::{Mutex, OnceLock};

/// The column shape a key asks for (ARCH-SCENE §4.6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Storage {
    /// Few nodes carry it: a sparse set (iteration over the entries only).
    Sparse,
    /// Most nodes carry it: `Vec<Option<T>>` by slot.
    Dense,
}

/// A process-wide extension id: the key's column index in every scene.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ExtId(pub u32);

fn ext_names() -> &'static Mutex<Vec<&'static str>> {
    static NAMES: OnceLock<Mutex<Vec<&'static str>>> = OnceLock::new();
    NAMES.get_or_init(|| Mutex::new(Vec::new()))
}

impl ExtId {
    /// The name of the key with this id (`"cad.pick"`), `"?"` if no key has it.
    pub fn name(self) -> &'static str {
        ext_names()
            .lock()
            .unwrap()
            .get(self.0 as usize)
            .copied()
            .unwrap_or("?")
    }
}

/// A typed key for per-node provider data. Declare it as a `static`.
pub struct ExtKey<T: 'static> {
    name: &'static str,
    storage: Storage,
    id: OnceLock<ExtId>,
    _t: PhantomData<fn() -> T>,
}

impl<T: 'static> ExtKey<T> {
    /// A key whose data few nodes carry (`Storage::Sparse`). `name` is `<namespace>.<name>`, for dumps,
    /// MCP and messages.
    pub const fn new(name: &'static str) -> Self {
        ExtKey {
            name,
            storage: Storage::Sparse,
            id: OnceLock::new(),
            _t: PhantomData,
        }
    }

    /// A key most nodes carry (`Storage::Dense`).
    pub const fn dense(name: &'static str) -> Self {
        ExtKey {
            name,
            storage: Storage::Dense,
            id: OnceLock::new(),
            _t: PhantomData,
        }
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    pub fn storage(&self) -> Storage {
        self.storage
    }

    /// The key's process-wide id, assigned on first use.
    #[inline]
    pub fn id(&self) -> ExtId {
        *self.id.get_or_init(|| {
            let mut names = ext_names().lock().unwrap();
            names.push(self.name);
            ExtId(names.len() as u32 - 1)
        })
    }
}

/// When an entry was added and last changed (Bevy's `ComponentTicks`): compare with a [`Tick`] read
/// earlier (`SceneRef::tick`) to see what changed since.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct ExtTicks {
    pub added: Tick,
    pub changed: Tick,
}

impl ExtTicks {
    pub fn is_added_since(self, t: Tick) -> bool {
        self.added > t
    }
    pub fn is_changed_since(self, t: Tick) -> bool {
        self.changed > t
    }
}

// ---- columns ----

/// A column without its type: what freeing a slot, dumps and stats need.
pub(crate) trait ErasedColumn: Any {
    fn contains(&self, i: u32) -> bool;
    /// Drops slot `i`'s entry (the node is freed); true if it had one.
    fn free(&mut self, i: u32) -> bool;
    fn len(&self) -> usize;
    /// Heap bytes (capacity × element sizes).
    fn bytes(&self) -> usize;
    /// The nodes with an entry.
    fn nodes(&self) -> Vec<NodeId>;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

const ABSENT: u32 = u32::MAX;

/// One key's column. Both shapes keep each entry's node id, so iteration builds `NodeRef`s without
/// the slot table.
pub(crate) enum Column<T> {
    Sparse {
        /// Slot → position in the packed vectors (`ABSENT`: no entry).
        sparse: Vec<u32>,
        ids: Vec<NodeId>,
        data: Vec<T>,
        ticks: Vec<ExtTicks>,
    },
    Dense {
        /// By slot; `ids[i]` is meaningful where `data[i]` is `Some`.
        ids: Vec<NodeId>,
        data: Vec<Option<T>>,
        ticks: Vec<ExtTicks>,
        len: usize,
    },
}

impl<T: 'static> Column<T> {
    fn new(storage: Storage) -> Self {
        match storage {
            Storage::Sparse => Column::Sparse {
                sparse: Vec::new(),
                ids: Vec::new(),
                data: Vec::new(),
                ticks: Vec::new(),
            },
            Storage::Dense => Column::Dense {
                ids: Vec::new(),
                data: Vec::new(),
                ticks: Vec::new(),
                len: 0,
            },
        }
    }

    #[inline]
    fn pos(sparse: &[u32], i: u32) -> Option<usize> {
        match sparse.get(i as usize) {
            Some(&p) if p != ABSENT => Some(p as usize),
            _ => None,
        }
    }

    #[inline]
    pub fn get(&self, i: u32) -> Option<&T> {
        match self {
            Column::Sparse { sparse, data, .. } => Self::pos(sparse, i).map(|p| &data[p]),
            Column::Dense { data, .. } => data.get(i as usize).and_then(Option::as_ref),
        }
    }

    pub fn get_mut(&mut self, i: u32) -> Option<&mut T> {
        match self {
            Column::Sparse { sparse, data, .. } => Self::pos(sparse, i).map(|p| &mut data[p]),
            Column::Dense { data, .. } => data.get_mut(i as usize).and_then(Option::as_mut),
        }
    }

    pub fn ticks(&self, i: u32) -> Option<ExtTicks> {
        match self {
            Column::Sparse { sparse, ticks, .. } => Self::pos(sparse, i).map(|p| ticks[p]),
            Column::Dense { data, ticks, .. } => data
                .get(i as usize)
                .and_then(Option::as_ref)
                .map(|_| ticks[i as usize]),
        }
    }

    /// Stamps slot `i`'s entry changed at `t`.
    fn touch(&mut self, i: u32, t: Tick) {
        match self {
            Column::Sparse { sparse, ticks, .. } => {
                if let Some(p) = Self::pos(sparse, i) {
                    ticks[p].changed = t;
                }
            }
            Column::Dense { ticks, .. } => {
                if let Some(x) = ticks.get_mut(i as usize) {
                    x.changed = t;
                }
            }
        }
    }

    /// Sets `node`'s value at tick `t`; returns the previous one.
    fn insert(&mut self, node: NodeId, v: T, t: Tick) -> Option<T> {
        let i = node.index();
        match self {
            Column::Sparse {
                sparse,
                ids,
                data,
                ticks,
            } => match Self::pos(sparse, i) {
                Some(p) => {
                    ticks[p].changed = t;
                    Some(std::mem::replace(&mut data[p], v))
                }
                None => {
                    if sparse.len() <= i as usize {
                        sparse.resize(i as usize + 1, ABSENT);
                    }
                    sparse[i as usize] = ids.len() as u32;
                    ids.push(node);
                    data.push(v);
                    ticks.push(ExtTicks {
                        added: t,
                        changed: t,
                    });
                    None
                }
            },
            Column::Dense {
                ids,
                data,
                ticks,
                len,
            } => {
                if data.len() <= i as usize {
                    data.resize_with(i as usize + 1, || None);
                    ids.resize(i as usize + 1, node);
                    ticks.resize(i as usize + 1, ExtTicks::default());
                }
                let old = data[i as usize].replace(v);
                if old.is_none() {
                    *len += 1;
                    ticks[i as usize].added = t;
                }
                ids[i as usize] = node;
                ticks[i as usize].changed = t;
                old
            }
        }
    }

    fn remove(&mut self, i: u32) -> Option<T> {
        match self {
            Column::Sparse {
                sparse,
                ids,
                data,
                ticks,
            } => {
                let p = Self::pos(sparse, i)?;
                sparse[i as usize] = ABSENT;
                ids.swap_remove(p);
                ticks.swap_remove(p);
                let v = data.swap_remove(p);
                if let Some(&moved) = ids.get(p) {
                    sparse[moved.index() as usize] = p as u32;
                }
                Some(v)
            }
            Column::Dense { data, len, .. } => {
                let v = data.get_mut(i as usize)?.take()?;
                *len -= 1;
                Some(v)
            }
        }
    }

    /// The entries: `(node, value)`. Sparse: in packed order; dense: in slot order.
    pub fn values(&self) -> ColumnValues<'_, T> {
        match self {
            Column::Sparse { ids, data, .. } => ColumnValues::Sparse { ids, data, at: 0 },
            Column::Dense { ids, data, .. } => ColumnValues::Dense { ids, data, at: 0 },
        }
    }

    /// The entries with their ticks: `(node, value, ticks)`.
    pub fn iter(&self) -> impl Iterator<Item = (NodeId, &T, ExtTicks)> + '_ {
        let (ticks, sparse): (&[ExtTicks], bool) = match self {
            Column::Sparse { ticks, .. } => (ticks, true),
            Column::Dense { ticks, .. } => (ticks, false),
        };
        let mut pos = 0usize;
        self.values().map(move |(id, v)| {
            let t = if sparse {
                ticks[pos]
            } else {
                ticks[id.index() as usize]
            };
            pos += 1;
            (id, v, t)
        })
    }
}

/// The entries of a column, `(node, value)`: a plain cursor over the packed (or by-slot) vectors.
pub(crate) enum ColumnValues<'a, T> {
    Sparse {
        ids: &'a [NodeId],
        data: &'a [T],
        at: usize,
    },
    Dense {
        ids: &'a [NodeId],
        data: &'a [Option<T>],
        at: usize,
    },
}

impl<'a, T> Iterator for ColumnValues<'a, T> {
    type Item = (NodeId, &'a T);
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            ColumnValues::Sparse { ids, data, at } => {
                let k = *at;
                if k >= data.len() {
                    return None;
                }
                *at = k + 1;
                Some((ids[k], &data[k]))
            }
            ColumnValues::Dense { ids, data, at } => {
                while *at < data.len() {
                    let k = *at;
                    *at = k + 1;
                    if let Some(v) = &data[k] {
                        return Some((ids[k], v));
                    }
                }
                None
            }
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            ColumnValues::Sparse { data, at, .. } => (data.len() - *at, Some(data.len() - *at)),
            ColumnValues::Dense { data, at, .. } => (0, Some(data.len() - *at)),
        }
    }
}

impl<T: 'static> ErasedColumn for Column<T> {
    fn contains(&self, i: u32) -> bool {
        self.get(i).is_some()
    }
    fn free(&mut self, i: u32) -> bool {
        self.remove(i).is_some()
    }
    fn len(&self) -> usize {
        match self {
            Column::Sparse { ids, .. } => ids.len(),
            Column::Dense { len, .. } => *len,
        }
    }
    fn bytes(&self) -> usize {
        use std::mem::size_of;
        match self {
            Column::Sparse {
                sparse,
                ids,
                data,
                ticks,
            } => {
                sparse.capacity() * 4
                    + ids.capacity() * 8
                    + data.capacity() * size_of::<T>()
                    + ticks.capacity() * size_of::<ExtTicks>()
            }
            Column::Dense {
                ids, data, ticks, ..
            } => {
                ids.capacity() * 8
                    + data.capacity() * size_of::<Option<T>>()
                    + ticks.capacity() * size_of::<ExtTicks>()
            }
        }
    }
    fn nodes(&self) -> Vec<NodeId> {
        self.values().map(|(id, _)| id).collect()
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// The scene's extension columns, indexed by [`ExtId`] (a key without entries in this scene has no
/// column).
#[derive(Default)]
pub(crate) struct ExtColumns {
    cols: Vec<Option<Box<dyn ErasedColumn>>>,
}

impl ExtColumns {
    #[inline]
    pub fn col<T: 'static>(&self, key: &ExtKey<T>) -> Option<&Column<T>> {
        self.cols
            .get(key.id().0 as usize)?
            .as_deref()?
            .as_any()
            .downcast_ref::<Column<T>>()
    }

    fn col_mut<T: 'static>(&mut self, key: &ExtKey<T>) -> Option<&mut Column<T>> {
        self.cols
            .get_mut(key.id().0 as usize)?
            .as_deref_mut()?
            .as_any_mut()
            .downcast_mut::<Column<T>>()
    }

    fn col_or_insert<T: 'static>(&mut self, key: &ExtKey<T>) -> &mut Column<T> {
        let k = key.id().0 as usize;
        if self.cols.len() <= k {
            self.cols.resize_with(k + 1, || None);
        }
        self.cols[k]
            .get_or_insert_with(|| Box::new(Column::<T>::new(key.storage())))
            .as_any_mut()
            .downcast_mut::<Column<T>>()
            .expect("an ExtId has one value type")
    }

    #[inline]
    pub fn get<T: 'static>(&self, key: &ExtKey<T>, i: u32) -> Option<&T> {
        self.col(key)?.get(i)
    }

    /// The keys with an entry in slot `i`, by id.
    pub fn ids_at(&self, i: u32) -> impl Iterator<Item = ExtId> + '_ {
        self.cols
            .iter()
            .enumerate()
            .filter(move |(_, c)| c.as_ref().is_some_and(|c| c.contains(i)))
            .map(|(k, _)| ExtId(k as u32))
    }

    pub fn has_any(&self, i: u32) -> bool {
        self.ids_at(i).next().is_some()
    }

    /// Drops slot `i`'s entries; calls `removed` for each key that had one.
    pub fn free(&mut self, i: u32, mut removed: impl FnMut(ExtId)) {
        for (k, c) in self.cols.iter_mut().enumerate() {
            if let Some(c) = c
                && c.free(i)
            {
                removed(ExtId(k as u32));
            }
        }
    }

    pub fn bytes(&self) -> usize {
        self.cols.capacity() * std::mem::size_of::<Option<Box<dyn ErasedColumn>>>()
            + self.cols.iter().flatten().map(|c| c.bytes()).sum::<usize>()
    }

    /// Every `(key, node)` entry (`validate`).
    pub fn entries(&self) -> Vec<(ExtId, NodeId)> {
        let mut v = Vec::new();
        for (k, c) in self.cols.iter().enumerate() {
            if let Some(c) = c {
                v.extend(c.nodes().into_iter().map(|id| (ExtId(k as u32), id)));
            }
        }
        v
    }
}

// ---- the write path ----

/// The parts of the scene an extension write touches, borrowed apart from the payload column so a
/// provider widget can write its own node's data while it draws (`provider::paint`).
pub(crate) struct ExtCx<'a> {
    pub cols: &'a mut ExtColumns,
    pub journal: &'a mut Journal,
    pub core: &'a mut [NodeCore],
    pub roots: &'a RootTable,
    pub tick: &'a mut u64,
    pub in_batch: bool,
}

impl ExtCx<'_> {
    pub fn reborrow(&mut self) -> ExtCx<'_> {
        ExtCx {
            cols: self.cols,
            journal: self.journal,
            core: self.core,
            roots: self.roots,
            tick: self.tick,
            in_batch: self.in_batch,
        }
    }

    /// A tick for this write (the batch's inside a batch), stamped on the node; journals `what`.
    fn changed(
        &mut self,
        i: u32,
        node: NodeId,
        ext: ExtId,
        what: ExtChange,
        origin: Origin,
    ) -> Tick {
        if !self.in_batch {
            *self.tick += 1;
        }
        let t = Tick(*self.tick);
        self.core[i as usize].changed = t;
        if self.journal.wants_ext(ext) {
            self.journal.record(
                self.core,
                self.roots,
                Change::Ext { node, ext, what },
                origin,
                *self.tick,
            );
        }
        t
    }
}

/// One key's entry on one node (`n.ext_mut(&KEY)`): get, set or remove. Every write stamps the
/// node's tick and the entry's [`ExtTicks`], and is journaled (`Change::Ext`).
pub struct ExtEntry<'a, T: 'static> {
    cx: ExtCx<'a>,
    key: &'a ExtKey<T>,
    i: u32,
    node: NodeId,
    origin: Origin,
}

impl<'a, T: 'static> ExtEntry<'a, T> {
    pub(crate) fn new(cx: ExtCx<'a>, key: &'a ExtKey<T>, node: NodeId, origin: Origin) -> Self {
        ExtEntry {
            cx,
            key,
            i: node.index(),
            node,
            origin,
        }
    }
}

impl<T: 'static> ExtEntry<'_, T> {
    pub fn get(&self) -> Option<&T> {
        self.cx.cols.get(self.key, self.i)
    }

    /// The value, for changing in place: marks the entry changed (journaled) if there is one.
    pub fn get_mut(&mut self) -> Option<&mut T> {
        self.get()?;
        let t = self.cx.changed(
            self.i,
            self.node,
            self.key.id(),
            ExtChange::Changed,
            self.origin,
        );
        let col = self.cx.cols.col_mut(self.key)?;
        col.touch(self.i, t);
        col.get_mut(self.i)
    }

    /// Sets the value; returns the previous one.
    pub fn set(&mut self, v: T) -> Option<T> {
        let what = if self.get().is_some() {
            ExtChange::Changed
        } else {
            ExtChange::Added
        };
        let t = self
            .cx
            .changed(self.i, self.node, self.key.id(), what, self.origin);
        self.cx.cols.col_or_insert(self.key).insert(self.node, v, t)
    }

    /// Removes the value (journaled as `Removed` if there was one).
    pub fn remove(&mut self) -> Option<T> {
        let v = self.cx.cols.col_mut(self.key)?.remove(self.i)?;
        self.cx.changed(
            self.i,
            self.node,
            self.key.id(),
            ExtChange::Removed,
            self.origin,
        );
        Some(v)
    }

    /// The value, inserted with `f` first if absent (`Added`), else marked changed.
    pub fn or_insert_with(&mut self, f: impl FnOnce() -> T) -> &mut T {
        if self.get().is_none() {
            self.set(f());
            return self
                .cx
                .cols
                .col_mut(self.key)
                .and_then(|c| c.get_mut(self.i))
                .expect("just inserted");
        }
        self.get_mut().expect("present")
    }

    /// When the entry was added and last changed.
    pub fn ticks(&self) -> Option<ExtTicks> {
        self.cx.cols.col(self.key)?.ticks(self.i)
    }
}

// ---- the read view of one node ----

/// One node's provider data, every key (dumps, inspectors). A view of the scene's columns.
#[derive(Clone, Copy)]
pub struct ExtBlock<'a> {
    cols: &'a ExtColumns,
    i: u32,
}

impl<'a> ExtBlock<'a> {
    pub(crate) fn new(cols: &'a ExtColumns, i: u32) -> Self {
        ExtBlock { cols, i }
    }

    pub fn get<T: 'static>(&self, key: &ExtKey<T>) -> Option<&'a T> {
        self.cols.get(key, self.i)
    }

    /// The ids of the keys present, in id order.
    pub fn ids(&self) -> impl Iterator<Item = ExtId> + use<'a> {
        self.cols.ids_at(self.i)
    }

    /// The names of the keys present, in id order.
    pub fn names(&self) -> impl Iterator<Item = &'static str> + use<'a> {
        self.ids().map(ExtId::name)
    }

    pub fn len(&self) -> usize {
        self.ids().count()
    }

    pub fn is_empty(&self) -> bool {
        !self.cols.has_any(self.i)
    }
}

/// The read accessors of the pre-SG1 node view (`NodeRef::ext`, `NodeMut::ext_mut` are the scene
/// API).
impl<'n> Node<'n> {
    /// This node's value for `key`, if it has one.
    pub fn ext<T: 'static>(&self, key: &ExtKey<T>) -> Option<&'n T> {
        self.ext.get(key)
    }

    /// Every key present on this node.
    pub fn ext_block(&self) -> ExtBlock<'n> {
        self.ext
    }
}
