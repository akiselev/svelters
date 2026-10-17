//! The handle table (pure, generic over the native reference type so it is tested on Linux).
//!
//! ABI v2 rules it enforces: handles are minted monotonically from 1 and never reused, they stay
//! below 2^53, and each records its kind, lifetime class and the tree edge the host made with
//! `insert` (parent, slot, ordered children), which drives the native-death cascade.

use std::collections::HashMap;

/// One live handle.
#[derive(Debug)]
pub struct Entry<O> {
    pub obj: O,
    pub kind: u32,
    /// `SVL_LIFE_*`.
    pub life: u32,
    /// The handle this one was inserted under, and the slot id.
    pub parent: Option<(u64, u32)>,
    /// Handles inserted under this one, in insertion order (not native order).
    pub children: Vec<u64>,
    /// For a borrowed handle: the handle whose getter minted it (it dies with that one).
    pub owner: Option<u64>,
}

impl<O> std::ops::Deref for Entry<O> {
    type Target = O;
    fn deref(&self) -> &O {
        &self.obj
    }
}

impl<O> std::ops::DerefMut for Entry<O> {
    fn deref_mut(&mut self) -> &mut O {
        &mut self.obj
    }
}

#[derive(Debug)]
pub struct HandleTable<O> {
    next: u64,
    map: HashMap<u64, Entry<O>>,
}

impl<O> Default for HandleTable<O> {
    fn default() -> Self {
        HandleTable { next: 1, map: HashMap::new() }
    }
}

/// Handles stay exact as JS numbers.
pub const MAX_HANDLE: u64 = 1 << 53;

impl<O> HandleTable<O> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds an object and returns its new handle.
    pub fn mint(&mut self, obj: O, kind: u32, life: u32) -> u64 {
        let h = self.next;
        assert!(h < MAX_HANDLE, "handle space exhausted");
        self.next += 1;
        self.map.insert(h, Entry { obj, kind, life, parent: None, children: Vec::new(), owner: None });
        h
    }

    pub fn get(&self, h: u64) -> Option<&Entry<O>> {
        self.map.get(&h)
    }

    pub fn get_mut(&mut self, h: u64) -> Option<&mut Entry<O>> {
        self.map.get_mut(&h)
    }

    pub fn contains(&self, h: u64) -> bool {
        self.map.contains_key(&h)
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Every live handle, ascending.
    pub fn handles(&self) -> Vec<u64> {
        let mut v: Vec<u64> = self.map.keys().copied().collect();
        v.sort_unstable();
        v
    }

    pub fn iter(&self) -> impl Iterator<Item = (u64, &Entry<O>)> {
        self.map.iter().map(|(h, e)| (*h, e))
    }

    /// Records `child` under `parent` in `slot`, before `before` (end when `None` or not a child).
    /// Fails if the child is already inserted somewhere.
    pub fn link(&mut self, parent: u64, child: u64, slot: u32, before: Option<u64>) -> Result<(), String> {
        if parent == child {
            return Err("cannot insert a handle into itself".into());
        }
        if !self.contains(parent) || !self.contains(child) {
            return Err("stale handle".into());
        }
        if let Some((p, _)) = self.map[&child].parent {
            return Err(format!("handle {child} is already inserted under {p}"));
        }
        if self.is_ancestor(child, parent) {
            return Err("cannot insert a handle into its own descendant".into());
        }
        let kids = &mut self.map.get_mut(&parent).unwrap().children;
        match before.and_then(|b| kids.iter().position(|&c| c == b)) {
            Some(i) => kids.insert(i, child),
            None => kids.push(child),
        }
        self.map.get_mut(&child).unwrap().parent = Some((parent, slot));
        Ok(())
    }

    /// Undoes [`link`](Self::link). Returns the slot the child was in.
    pub fn unlink(&mut self, parent: u64, child: u64) -> Result<u32, String> {
        let slot = match self.map.get(&child).and_then(|e| e.parent) {
            Some((p, s)) if p == parent => s,
            _ => return Err(format!("handle {child} is not a child of {parent}")),
        };
        if let Some(p) = self.map.get_mut(&parent) {
            p.children.retain(|&c| c != child);
        }
        self.map.get_mut(&child).unwrap().parent = None;
        Ok(slot)
    }

    fn is_ancestor(&self, a: u64, mut h: u64) -> bool {
        while let Some((p, _)) = self.map.get(&h).and_then(|e| e.parent) {
            if p == a {
                return true;
            }
            h = p;
        }
        false
    }

    /// Siblings of `child` in its slot that follow it (in insertion order).
    pub fn children_in_slot(&self, parent: u64, slot: u32) -> Vec<u64> {
        let Some(p) = self.map.get(&parent) else { return Vec::new() };
        p.children.iter().copied().filter(|c| self.map.get(c).and_then(|e| e.parent).map(|(_, s)| s) == Some(slot)).collect()
    }

    /// `h` and everything below it (inserted children and borrowed handles it minted), parents first.
    pub fn subtree(&self, h: u64) -> Vec<u64> {
        let mut out = Vec::new();
        let mut stack = vec![h];
        while let Some(x) = stack.pop() {
            if !self.contains(x) || out.contains(&x) {
                continue;
            }
            out.push(x);
            let e = &self.map[&x];
            let mut owned: Vec<u64> = self.map.iter().filter(|(_, c)| c.owner == Some(x)).map(|(k, _)| *k).collect();
            owned.sort_unstable();
            for c in owned.into_iter().rev() {
                stack.push(c);
            }
            for &c in e.children.iter().rev() {
                stack.push(c);
            }
        }
        out
    }

    /// Removes a handle: detaches it from its parent, orphans its children (they keep their own
    /// entries) and returns the entry.
    pub fn take(&mut self, h: u64) -> Option<Entry<O>> {
        let e = self.map.remove(&h)?;
        if let Some((p, _)) = e.parent {
            if let Some(pe) = self.map.get_mut(&p) {
                pe.children.retain(|&c| c != h);
            }
        }
        for c in &e.children {
            if let Some(ce) = self.map.get_mut(c) {
                ce.parent = None;
            }
        }
        for ce in self.map.values_mut() {
            if ce.owner == Some(h) {
                ce.owner = None;
            }
        }
        Some(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mint_never_reuses() {
        let mut t: HandleTable<&str> = HandleTable::new();
        let a = t.mint("a", 1, 1);
        let b = t.mint("b", 1, 1);
        assert_eq!((a, b), (1, 2));
        t.take(a);
        assert_eq!(t.mint("c", 1, 1), 3);
        assert!(!t.contains(a));
    }

    #[test]
    fn link_order_and_errors() {
        let mut t: HandleTable<()> = HandleTable::new();
        let p = t.mint((), 1, 1);
        let a = t.mint((), 1, 1);
        let b = t.mint((), 1, 1);
        let c = t.mint((), 1, 1);
        t.link(p, a, 0, None).unwrap();
        t.link(p, c, 0, None).unwrap();
        t.link(p, b, 0, Some(c)).unwrap();
        assert_eq!(t.get(p).unwrap().children, vec![a, b, c]);
        assert!(t.link(p, a, 0, None).is_err(), "already inserted");
        assert!(t.link(a, p, 0, None).is_err(), "cycle");
        assert!(t.link(p, p, 0, None).is_err(), "self");
        assert_eq!(t.unlink(p, b), Ok(0));
        assert!(t.unlink(p, b).is_err());
        assert_eq!(t.get(p).unwrap().children, vec![a, c]);
    }

    #[test]
    fn slots_and_subtree() {
        let mut t: HandleTable<()> = HandleTable::new();
        let w = t.mint((), 1, 2);
        let s = t.mint((), 2, 1);
        let x = t.mint((), 3, 1);
        let y = t.mint((), 3, 1);
        let borrowed = t.mint((), 3, 3);
        t.get_mut(borrowed).unwrap().owner = Some(s);
        t.link(w, s, 7, None).unwrap();
        t.link(s, x, 0, None).unwrap();
        t.link(s, y, 1, None).unwrap();
        assert_eq!(t.children_in_slot(s, 0), vec![x]);
        assert_eq!(t.subtree(w), vec![w, s, x, y, borrowed]);
        let e = t.take(s).unwrap();
        assert_eq!(e.children, vec![x, y]);
        assert!(t.get(x).unwrap().parent.is_none());
        assert!(t.get(w).unwrap().children.is_empty());
        assert!(t.get(borrowed).unwrap().owner.is_none());
    }
}
