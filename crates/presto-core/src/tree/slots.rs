//! The slot table behind [`NodeId`]: a generation per slot, liveness, a free list and the live
//! count. A node id is `generation << 24 | index`; generations start at 1, so an id is never 0, and a
//! slot is retired instead of recycled once its next generation would reach 2^28: ids stay below
//! 2^52 (the svelters host's app-export handle bit) and are never reused.

use presto_base::ids::NodeId;

/// The first generation that is never handed out: `MAX_GEN << 24 == 1 << 52`.
pub(crate) const MAX_GEN: u32 = 1 << 28;
/// Live slots at most (index bits).
pub(crate) const MAX_SLOTS: usize = 1 << NodeId::IDX_BITS;

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
        (!id.is_none() && i < self.gens.len() && self.live[i] && self.gens[i] == id.generation()).then_some(i)
    }

    /// The id of the live node in slot `i`.
    #[inline]
    pub fn id_at(&self, i: usize) -> Option<NodeId> {
        (i < self.live.len() && self.live[i]).then(|| NodeId::new(i as u32, self.gens[i]))
    }

    /// Takes a slot: a fresh one, or a freed one with its next generation. Returns its id and
    /// whether it is new (the columns must grow).
    pub fn alloc(&mut self) -> (NodeId, bool) {
        let (i, fresh) = match self.free.pop() {
            Some(i) => (i as usize, false),
            None => {
                assert!(self.gens.len() < MAX_SLOTS, "presto-core: more than 2^24 live nodes");
                self.gens.push(0);
                self.live.push(false);
                (self.gens.len() - 1, true)
            }
        };
        self.gens[i] += 1;
        debug_assert!(self.gens[i] < MAX_GEN);
        self.live[i] = true;
        self.len += 1;
        (NodeId::new(i as u32, self.gens[i]), fresh)
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
    fn ids_pack() {
        let id = NodeId::new(5, 3);
        assert_eq!(id.index(), 5);
        assert_eq!(id.generation(), 3);
        assert!(NodeId::new(MAX_SLOTS as u32 - 1, MAX_GEN - 1).0 < 1 << 52);
    }

    #[test]
    fn stale_ids_fail_lookups() {
        let mut t = SlotTable::default();
        let (a, fresh) = t.alloc();
        assert!(fresh);
        assert_eq!(t.index(a), Some(0));
        t.free(0);
        assert_eq!(t.index(a), None);
        let (b, fresh) = t.alloc();
        assert!(!fresh);
        assert_eq!(b.index(), a.index());
        assert_ne!(a, b);
        assert_eq!(t.index(a), None);
        assert_eq!(t.index(NodeId(0)), None);
    }

    /// The last generation a slot hands out is 2^28 - 1, so no node id ever has bit 52.
    #[test]
    fn generations_retire_below_the_app_handle_bit() {
        let mut t = SlotTable::default();
        let (a, _) = t.alloc();
        t.free(a.index() as usize);
        t.set_generation(0, MAX_GEN - 2);
        let (b, fresh) = t.alloc();
        assert!(!fresh);
        assert_eq!(b.generation(), MAX_GEN - 1);
        assert_eq!(b.0 & (1 << 52), 0);
        t.free(0);
        assert_eq!(t.free_len(), 0, "the slot is retired, not recycled");
        let (c, fresh) = t.alloc();
        assert!(fresh);
        assert_eq!(c.index(), 1);
    }
}
