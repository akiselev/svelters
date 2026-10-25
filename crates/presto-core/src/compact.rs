//! Small per-node data for 10k–100k-node screens (ported from svelters-egui's `scene/compact.rs`,
//! ARCH-SCENE §8.5): [`Interner`] keeps one shared `Arc` per distinct value. Most nodes share one of
//! a few dozen taffy styles (every row box of a property grid has the same one), so the layout
//! column holds an `Arc<taffy::Style>` (8 bytes, not a whole style). Values are bucketed by a cheap
//! structural key and compared with `==`; entries nobody else holds are pruned as the table grows.
//! The computed box (`ComputedBox`) is already shared per node through the cascade's `Arc`.

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;

/// Buckets longer than this are not searched: the value gets its own `Arc` (a style family that
/// differs only in fields the key skips, which no real screen has).
const MAX_BUCKET: usize = 64;

/// One shared `Arc` per distinct value (see the module docs).
pub(crate) struct Interner<T> {
    buckets: HashMap<u64, Vec<Arc<T>>>,
    len: usize,
    prune_at: usize,
    /// Interned values that were already in the table.
    pub hits: u64,
}

impl<T> Default for Interner<T> {
    fn default() -> Self {
        Interner { buckets: HashMap::new(), len: 0, prune_at: 1024, hits: 0 }
    }
}

impl<T: PartialEq> Interner<T> {
    /// The shared `Arc` equal to `v` (`key` is the value's structural key).
    pub fn intern(&mut self, v: T, key: u64) -> Arc<T> {
        if let Some(found) = self.buckets.get(&key).and_then(|b| b.iter().find(|a| ***a == v)) {
            self.hits += 1;
            return found.clone();
        }
        let a = Arc::new(v);
        let b = self.buckets.entry(key).or_default();
        if b.len() >= MAX_BUCKET {
            return a;
        }
        b.push(a.clone());
        self.len += 1;
        if self.len >= self.prune_at {
            self.prune();
        }
        a
    }

    /// Drops the values only the table holds.
    pub fn prune(&mut self) {
        self.buckets.retain(|_, b| {
            b.retain(|a| Arc::strong_count(a) > 1);
            !b.is_empty()
        });
        self.len = self.buckets.values().map(Vec::len).sum();
        self.prune_at = (self.len * 2).max(1024);
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.len
    }
}

fn len_key(h: &mut DefaultHasher, l: taffy::CompactLength) {
    l.tag().hash(h);
    l.value().to_bits().hash(h);
}

/// The interning key of a taffy style: its kind of box and its own size (what tells the few dozen
/// styles of a screen apart); `==` settles the rest.
pub(crate) fn style_key(s: &taffy::Style) -> u64 {
    let mut h = DefaultHasher::new();
    std::mem::discriminant(&s.display).hash(&mut h);
    std::mem::discriminant(&s.position).hash(&mut h);
    std::mem::discriminant(&s.flex_direction).hash(&mut h);
    std::mem::discriminant(&s.flex_wrap).hash(&mut h);
    let (sz, min, max, pad, gap) = (s.size, s.min_size, s.max_size, s.padding, s.gap);
    for l in [sz.width.into_raw(), sz.height.into_raw(), min.width.into_raw(), min.height.into_raw(), max.width.into_raw(), max.height.into_raw()] {
        len_key(&mut h, l);
    }
    for l in [pad.left.into_raw(), pad.top.into_raw(), gap.width.into_raw(), gap.height.into_raw()] {
        len_key(&mut h, l);
    }
    s.flex_grow.to_bits().hash(&mut h);
    s.flex_shrink.to_bits().hash(&mut h);
    h.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_styles_share_one_arc() {
        let mut i = Interner::default();
        let a = i.intern(taffy::Style::default(), style_key(&taffy::Style::default()));
        let b = i.intern(taffy::Style::default(), style_key(&taffy::Style::default()));
        assert!(Arc::ptr_eq(&a, &b));
        assert_eq!(i.hits, 1);
        let other = taffy::Style { flex_grow: 1.0, ..Default::default() };
        let c = i.intern(other.clone(), style_key(&other));
        assert!(!Arc::ptr_eq(&a, &c));
        assert_eq!(i.len(), 2);
        drop((a, b, c));
        i.prune();
        assert_eq!(i.len(), 0, "values only the table holds are pruned");
    }
}
