//! SG6 (ARCH-SCENE §8.5, §15 S11): the per-node data made small for 10k–100k-node screens.
//!
//! - [`Interner`]: one shared `Arc` per distinct value. Most nodes share one of a few dozen taffy
//!   styles and common-prop sets (every row box of a PropertyGrid has the same ones), so the
//!   `layout` column holds an `Arc<Style>` (8 B, not 552) and the `common` column an
//!   `Option<Arc<Common>>` (8 B, not a 600 B cold box). Values are bucketed by a cheap structural
//!   key and compared with `==`; entries nobody else holds are pruned as the table grows.
//! - [`LayoutCache`]: taffy's per-node layout cache, boxed, for containers and provider widgets
//!   only (8 B for a leaf, not 408). ARCH-SCENE §8.5 planned a compact cache with the final entry
//!   and 2 measure entries; measured on a PropertyGrid, taffy 0.15's flexbox stores 7 measure
//!   entries per node on average (`SceneStats::cache_entries` with 9 slots), and a 2- or 4-entry
//!   cache spilled on every node and laid out 23–47% more leaves (`plans/milestones/SG6-…`). What a
//!   leaf's cache saves is cheap to recompute (its intrinsic size is cached in `LayoutBox`), so the
//!   leaves go without one instead.
//! - [`BoxRect`]: the location and size of a taffy `Layout` (16 B, not 92): the walk and `place`
//!   read nothing else, and `round_layout` rounds the rest only for us to drop it.

use crate::scene::Common;
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;
use taffy::tree::{LayoutInput, LayoutOutput};
use taffy::{Layout, Point, Size};

/// Buckets longer than this are not searched: the value gets its own `Arc` (a style family that
/// differs only in fields the key skips, which no real screen has).
const MAX_BUCKET: usize = 64;

/// One shared `Arc` per distinct value (see the module docs).
pub(crate) struct Interner<T> {
    buckets: HashMap<u64, Vec<Arc<T>>>,
    len: usize,
    prune_at: usize,
    /// Interned values that were already in the table (`SceneStats`-style diagnostics).
    pub hits: u64,
}

impl<T> Default for Interner<T> {
    fn default() -> Self {
        Interner { buckets: HashMap::new(), len: 0, prune_at: 1024, hits: 0 }
    }
}

impl<T: PartialEq + Clone> Interner<T> {
    /// The shared `Arc` equal to `v` (`key` is the value's structural key).
    pub fn intern(&mut self, v: T, key: u64) -> Arc<T> {
        self.find(&v, key).unwrap_or_else(|| self.insert(Arc::new(v), key))
    }

    /// As [`Interner::intern`] for a value already in an `Arc` (after `Arc::make_mut`).
    pub fn intern_arc(&mut self, a: Arc<T>, key: u64) -> Arc<T> {
        self.find(&a, key).unwrap_or_else(|| self.insert(a, key))
    }

    fn find(&mut self, v: &T, key: u64) -> Option<Arc<T>> {
        let b = self.buckets.get(&key)?;
        let found = b.iter().find(|a| ***a == *v)?.clone();
        self.hits += 1;
        Some(found)
    }

    fn insert(&mut self, a: Arc<T>, key: u64) -> Arc<T> {
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

    /// The interned values.
    pub fn values(&self) -> impl Iterator<Item = &Arc<T>> {
        self.buckets.values().flatten()
    }

    pub fn len(&self) -> usize {
        self.len
    }

    /// Heap bytes: the shared values (with `Arc`'s two counts) and the table.
    pub fn bytes(&self) -> usize {
        use std::mem::size_of;
        let table = self.buckets.capacity() * (size_of::<u64>() + size_of::<Vec<Arc<T>>>() + 1);
        let entries: usize = self.buckets.values().map(|b| b.capacity() * size_of::<Arc<T>>()).sum();
        table + entries + self.len * (size_of::<T>() + 16)
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

/// The interning key of a node's common props: the strings and the main layout props.
pub(crate) fn common_key(c: &Common) -> u64 {
    let mut h = DefaultHasher::new();
    (&c.accessible_name, &c.tooltip, &c.anchor, c.enabled, c.visible, c.flex_direction, c.align_items).hash(&mut h);
    for v in [c.width, c.height, c.min_width, c.max_width, c.flex_basis] {
        v.map(f64::to_bits).hash(&mut h);
    }
    for v in [c.gap, c.padding, c.margin, c.flex_grow, c.flex_shrink] {
        v.to_bits().hash(&mut h);
    }
    h.finish()
}

/// The location and size of a taffy [`Layout`] (see the module docs).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct BoxRect {
    pub location: Point<f32>,
    pub size: Size<f32>,
}

impl BoxRect {
    pub fn of(l: &Layout) -> BoxRect {
        BoxRect { location: l.location, size: l.size }
    }
    pub fn to_layout(self) -> Layout {
        Layout { location: self.location, size: self.size, ..Layout::with_order(0) }
    }
}

// ---- the layout cache ----

/// Taffy's per-node layout cache (`taffy::Cache`, 408 B), boxed and only for the nodes that need
/// it (see the module docs): `layout.rs` stores entries only for containers and provider widgets.
/// A built-in leaf runs taffy's leaf layout again from its cached intrinsic size, which costs less
/// than the cache lookup's bookkeeping saves, so it never gets one.
#[derive(Clone, Debug, Default)]
pub(crate) struct LayoutCache(Option<Box<taffy::Cache>>);

impl LayoutCache {
    pub const fn new() -> LayoutCache {
        LayoutCache(None)
    }
    pub fn get(&mut self, input: &LayoutInput) -> Option<LayoutOutput> {
        self.0.as_mut()?.get(input)
    }
    pub fn store(&mut self, input: &LayoutInput, out: LayoutOutput) {
        self.0.get_or_insert_with(|| Box::new(taffy::Cache::new())).store(input, out);
    }
    pub fn clear(&mut self) {
        if let Some(c) = &mut self.0 {
            c.clear();
        }
    }
    pub fn is_empty(&self) -> bool {
        self.0.as_ref().is_none_or(|c| c.is_empty())
    }
    /// True once the node stored an entry (it has a boxed cache).
    pub fn is_boxed(&self) -> bool {
        self.0.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use taffy::Line;
    use taffy::AvailableSpace;
    use taffy::tree::{RequestedAxis, RunMode, SizingMode};

    fn input(run_mode: RunMode, width: f32) -> LayoutInput {
        LayoutInput {
            run_mode,
            sizing_mode: SizingMode::InherentSize,
            axis: RequestedAxis::Both,
            known_dimensions: Size { width: Some(width), height: None },
            known_dimensions_are_definite: Size { width: true, height: true },
            parent_size: Size::NONE,
            available_space: Size { width: AvailableSpace::MaxContent, height: AvailableSpace::MaxContent },
            vertical_margins_are_collapsible: Line::FALSE,
        }
    }

    fn out(w: f32) -> LayoutOutput {
        LayoutOutput::from_outer_size(Size { width: w, height: w })
    }

    /// A cache is boxed by its first store and answers as taffy's.
    #[test]
    fn layout_cache_boxes_on_first_store() {
        let mut c = LayoutCache::new();
        assert!(c.is_empty() && !c.is_boxed());
        assert_eq!(c.get(&input(RunMode::ComputeSize, 1.0)), None);
        c.store(&input(RunMode::ComputeSize, 1.0), out(1.0));
        assert!(c.is_boxed() && !c.is_empty());
        assert_eq!(c.get(&input(RunMode::ComputeSize, 1.0)), Some(out(1.0)));
        c.clear();
        assert!(c.is_empty() && c.is_boxed());
    }

    #[test]
    fn interner_shares_equal_values_and_prunes() {
        let mut i: Interner<taffy::Style> = Interner::default();
        let s = taffy::Style { flex_grow: 1.0, ..Default::default() };
        let a = i.intern(s.clone(), style_key(&s));
        let b = i.intern(s.clone(), style_key(&s));
        assert!(Arc::ptr_eq(&a, &b));
        let other = taffy::Style::default();
        let c = i.intern(other.clone(), style_key(&other));
        assert!(!Arc::ptr_eq(&a, &c));
        drop((a, b));
        i.prune();
        assert_eq!(i.len(), 1, "only the style still held");
    }
}
