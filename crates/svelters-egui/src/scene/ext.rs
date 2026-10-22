//! Extension storage: [`ExtKey`] and per-node provider data (ARCH-EMBED §8.4, ARCH-SCENE §4.6).
//!
//! A provider declares a key once, as a static, and reads and writes typed data on any node through
//! it:
//!
//! ```ignore
//! pub static OP: ExtKey<OpView> = ExtKey::new("cad.op");
//! n.ext_mut(&OP).set(op_view);
//! let op: Option<&OpView> = n.ext(&OP);
//! ```
//!
//! RA1 stores the data in a small per-node [`ExtBlock`] (empty on most nodes). The API only names a
//! key and a node, never the storage, so SG2 can move each key into its own column (a sparse set or a
//! dense `Vec<Option<T>>`, ARCH-SCENE §4.6 / S2(a)) without changing a caller: [`ExtKey::id`] is the
//! process-wide column index, and [`Storage`] already records the column shape a key wants.
//!
//! Writer lane: RUSTAPI (RA1); SG2 adds the columns, change ticks and the journal.

use super::Node;
use std::any::Any;
use std::marker::PhantomData;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU32, Ordering};

/// The column shape a key asks for (ARCH-SCENE §4.6). RA1 stores both the same way.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Storage {
    /// Few nodes carry it: a sparse set (iteration over the entries only).
    Sparse,
    /// Most nodes carry it: `Vec<Option<T>>` by slot.
    Dense,
}

/// A process-wide extension id (the column index once SG2 lands columns).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ExtId(pub u32);

static NEXT_EXT: AtomicU32 = AtomicU32::new(0);

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
        ExtKey { name, storage: Storage::Sparse, id: OnceLock::new(), _t: PhantomData }
    }

    /// A key most nodes carry (`Storage::Dense`).
    pub const fn dense(name: &'static str) -> Self {
        ExtKey { name, storage: Storage::Dense, id: OnceLock::new(), _t: PhantomData }
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    pub fn storage(&self) -> Storage {
        self.storage
    }

    /// The key's process-wide id, assigned on first use.
    pub fn id(&self) -> ExtId {
        *self.id.get_or_init(|| ExtId(NEXT_EXT.fetch_add(1, Ordering::Relaxed)))
    }
}

/// The provider data of one node: `(key id, value)` pairs, sorted by id. Empty (no allocation) on
/// most nodes.
#[derive(Default)]
pub struct ExtBlock {
    entries: Vec<(ExtId, &'static str, Box<dyn Any>)>,
}

impl ExtBlock {
    fn pos(&self, id: ExtId) -> Result<usize, usize> {
        self.entries.binary_search_by_key(&id, |e| e.0)
    }

    pub fn get<T: 'static>(&self, key: &ExtKey<T>) -> Option<&T> {
        let i = self.pos(key.id()).ok()?;
        self.entries[i].2.downcast_ref::<T>()
    }

    pub fn get_mut<T: 'static>(&mut self, key: &ExtKey<T>) -> Option<&mut T> {
        let i = self.pos(key.id()).ok()?;
        self.entries[i].2.downcast_mut::<T>()
    }

    /// Sets the value; returns the previous one.
    pub fn insert<T: 'static>(&mut self, key: &ExtKey<T>, v: T) -> Option<T> {
        match self.pos(key.id()) {
            Ok(i) => std::mem::replace(&mut self.entries[i].2, Box::new(v)).downcast::<T>().ok().map(|b| *b),
            Err(i) => {
                self.entries.insert(i, (key.id(), key.name(), Box::new(v)));
                None
            }
        }
    }

    pub fn remove<T: 'static>(&mut self, key: &ExtKey<T>) -> Option<T> {
        let i = self.pos(key.id()).ok()?;
        self.entries.remove(i).2.downcast::<T>().ok().map(|b| *b)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The names of the keys present (dumps, inspectors).
    pub fn names(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.entries.iter().map(|e| e.1)
    }
}

/// One key's entry on one node (`n.ext_mut(&KEY)`): get, set or remove.
pub struct ExtEntry<'a, T: 'static> {
    block: &'a mut ExtBlock,
    key: &'a ExtKey<T>,
}

impl<'a, T: 'static> ExtEntry<'a, T> {
    pub(crate) fn new(block: &'a mut ExtBlock, key: &'a ExtKey<T>) -> Self {
        ExtEntry { block, key }
    }
}

impl<T: 'static> ExtEntry<'_, T> {
    pub fn get(&self) -> Option<&T> {
        self.block.get(self.key)
    }
    pub fn get_mut(&mut self) -> Option<&mut T> {
        self.block.get_mut(self.key)
    }
    pub fn set(&mut self, v: T) -> Option<T> {
        self.block.insert(self.key, v)
    }
    pub fn remove(&mut self) -> Option<T> {
        self.block.remove(self.key)
    }
    /// The value, inserted with `f` first if absent.
    pub fn or_insert_with(&mut self, f: impl FnOnce() -> T) -> &mut T {
        if self.block.get(self.key).is_none() {
            self.block.insert(self.key, f());
        }
        self.block.get_mut(self.key).expect("just inserted")
    }
}

/// The read accessors of the pre-SG1 node view (SG1: `NodeRef::ext`, `NodeMut::ext_mut` are the
/// scene API).
impl<'n> Node<'n> {
    /// This node's value for `key`, if it has one.
    pub fn ext<T: 'static>(&self, key: &ExtKey<T>) -> Option<&'n T> {
        self.ext.get(key)
    }

    /// Every key present on this node.
    pub fn ext_block(&self) -> &'n ExtBlock {
        self.ext
    }
}
