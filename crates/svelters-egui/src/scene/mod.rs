//! The retained scene: a generational arena of typed nodes (plans/research/EGUI.md §1.1).
//!
//! Host ops only edit this arena; they never touch egui. A handle is `generation << 24 | idx` (below 2^53,
//! never 0): a slot whose generation reaches 2^29 is retired instead of recycled, so handles are
//! never reused and a stale one fails with an error.
//!
//! The file cut is ARCH-SCENE §13 (SG0, done in EG0). This file holds the arena (`Scene`, `SceneCell`,
//! phases and `validate` from SG1 on); writer lane: SCENE.
//!
//! | File | Content |
//! |---|---|
//! | [`id`] | handles and their packing (SG1: `NodeId`, `SlotTable`, `VNodeId`, `Target`) |
//! | [`node`] | [`Node`], [`W`], [`Common`], [`Visual`], [`Dirty`] |
//! | [`tree`] | node creation and tree edits (create, insert, remove, destroy) |
//! | [`props`] | built-in prop get/set (writer: WIDGETS) |
//! | [`dirty`] | layout invalidation and the layout flags |
//! | [`access`] | read access for tests and devtools (`with_scene`, `prop`, `handles_of`) |
//! | [`ext`], [`journal`], [`query`], [`hit`], [`virt`], [`dump`] | empty seams (SG1–SG7) |

pub(crate) mod access;
pub(crate) mod dirty;
pub(crate) mod dump;
pub(crate) mod ext;
pub(crate) mod hit;
pub(crate) mod id;
pub(crate) mod journal;
pub(crate) mod node;
pub(crate) mod props;
pub(crate) mod query;
pub(crate) mod tree;
pub(crate) mod virt;

pub use id::Handle;
pub use node::{Common, Dirty, Node, Visual, W};
pub use props::{default_value, get_prop, set_prop, tag_of};
pub use tree::new_node;

struct Slot {
    generation: u32,
    node: Option<Node>,
}

#[derive(Default)]
pub struct Arena {
    slots: Vec<Slot>,
    free: Vec<u32>,
}

impl Arena {
    fn index(&self, h: Handle) -> Option<usize> {
        let (idx, generation) = id::split(h);
        let s = self.slots.get(idx)?;
        (s.generation == generation && s.node.is_some()).then_some(idx)
    }

    pub fn get(&self, h: Handle) -> Option<&Node> {
        self.index(h).and_then(|i| self.slots[i].node.as_ref())
    }

    pub fn get_mut(&mut self, h: Handle) -> Option<&mut Node> {
        let i = self.index(h)?;
        self.slots[i].node.as_mut()
    }

    pub fn n(&self, h: Handle) -> &Node {
        self.get(h).expect("live handle")
    }

    pub fn n_mut(&mut self, h: Handle) -> &mut Node {
        self.get_mut(h).expect("live handle")
    }

    pub fn contains(&self, h: Handle) -> bool {
        self.index(h).is_some()
    }

    /// Allocates a slot and builds the node for its handle.
    pub fn alloc(&mut self, make: impl FnOnce(Handle) -> Node) -> Handle {
        let idx = match self.free.pop() {
            Some(i) => i as usize,
            None => {
                self.slots.push(Slot { generation: 0, node: None });
                self.slots.len() - 1
            }
        };
        let s = &mut self.slots[idx];
        s.generation += 1;
        let h = id::join(s.generation, idx);
        s.node = Some(make(h));
        h
    }

    /// Frees a node's slot (its handle becomes stale forever).
    pub fn free(&mut self, h: Handle) -> Option<Node> {
        let i = self.index(h)?;
        let s = &mut self.slots[i];
        let node = s.node.take();
        if s.generation < id::MAX_GEN {
            self.free.push(i as u32);
        }
        node
    }

    pub fn len(&self) -> usize {
        self.slots.iter().filter(|s| s.node.is_some()).count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn handles(&self) -> Vec<Handle> {
        self.slots.iter().filter_map(|s| s.node.as_ref().map(|n| n.handle)).collect()
    }
}
