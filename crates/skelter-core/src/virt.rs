//! Virtualisation (ported from svelters-egui's `scene/virt.rs`): the row-height model and the row
//! pool virtualising kinds share.
//!
//! - [`RowHeights`]: uniform (no storage, so 1M rows cost nothing) until a row's height differs
//!   from the estimate; then a Fenwick tree over the heights (O(log n) offsets and `row_at`, O(n)
//!   rebuild on a splice).
//! - [`RowPool`]: realised rows recycled by key and template (no create/destroy while scrolling).
//!
//! A virtualising kind ([`Kind::virtual_children`](crate::Kind::virtual_children)) binds the rows
//! its viewport shows through a pool before layout and places them where its height model says.

use crate::document::Document;
use skelter_base::geom::{Point, Rect, Size};
use skelter_base::ids::NodeId;
use std::collections::HashMap;
use std::ops::Range;

/// The row height a virtualiser assumes before it knows better.
pub const DEFAULT_ROW_HEIGHT: f32 = 24.0;

// ---- the row-height model ----

/// Row geometry: offsets, heights and the rows in a range of the content (ARCH-SCENE §8.2).
///
/// Uniform rows need no storage; the first [`RowHeights::set`] of a height other than the
/// estimate builds a Fenwick tree over all heights. Offsets are `f64` (1M rows of 20 points is
/// 2·10⁷, past `f32`'s exact range).
#[derive(Clone, Debug, PartialEq)]
pub struct RowHeights {
    len: usize,
    est: f32,
    /// Per-row heights, once any differs from `est`.
    heights: Option<Vec<f32>>,
    /// Fenwick tree (1-based) over `heights`.
    tree: Vec<f64>,
}

impl Default for RowHeights {
    fn default() -> Self {
        RowHeights::uniform(0, DEFAULT_ROW_HEIGHT)
    }
}

impl RowHeights {
    /// `len` rows of height `h`.
    pub fn uniform(len: usize, h: f32) -> RowHeights {
        RowHeights { len, est: h.max(1.0), heights: None, tree: Vec::new() }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// The estimate (the height of rows never set).
    pub fn estimate(&self) -> f32 {
        self.est
    }

    /// True while every row has the estimated height.
    pub fn is_uniform(&self) -> bool {
        self.heights.is_none()
    }

    /// Changes the estimate. A uniform model changes every row; a measured one only the rows still
    /// at the old estimate keep it (they were never measured).
    pub fn set_estimate(&mut self, h: f32) {
        let h = h.max(1.0);
        if h == self.est {
            return;
        }
        let old = self.est;
        self.est = h;
        if let Some(v) = &mut self.heights {
            for x in v.iter_mut() {
                if *x == old {
                    *x = h;
                }
            }
            self.rebuild();
        }
    }

    pub fn height(&self, row: usize) -> f32 {
        match &self.heights {
            Some(v) => v.get(row).copied().unwrap_or(self.est),
            None => self.est,
        }
    }

    /// The content offset of the top of `row` (`row == len`: the total).
    pub fn offset(&self, row: usize) -> f64 {
        let row = row.min(self.len);
        match &self.heights {
            None => row as f64 * self.est as f64,
            Some(_) => {
                let mut i = row;
                let mut s = 0.0;
                while i > 0 {
                    s += self.tree[i];
                    i &= i - 1;
                }
                s
            }
        }
    }

    /// The content height of all rows.
    pub fn total(&self) -> f64 {
        self.offset(self.len)
    }

    /// Sets the height of `row`. True if it changed.
    pub fn set(&mut self, row: usize, h: f32) -> bool {
        let h = h.max(1.0);
        if row >= self.len || self.height(row) == h {
            return false;
        }
        if self.heights.is_none() {
            self.heights = Some(vec![self.est; self.len]);
            self.rebuild();
        }
        let v = self.heights.as_mut().expect("measured");
        let d = (h - v[row]) as f64;
        v[row] = h;
        let mut i = row + 1;
        while i <= self.len {
            self.tree[i] += d;
            i += i & i.wrapping_neg();
        }
        true
    }

    fn rebuild(&mut self) {
        let Some(v) = &self.heights else {
            self.tree.clear();
            return;
        };
        let n = v.len();
        self.tree = vec![0.0; n + 1];
        for (k, &h) in v.iter().enumerate() {
            self.tree[k + 1] += h as f64;
        }
        for i in 1..=n {
            let j = i + (i & i.wrapping_neg());
            if j <= n {
                let x = self.tree[i];
                self.tree[j] += x;
            }
        }
    }

    /// Replaces `removed` rows at `pos` with `added` rows of the estimated height.
    pub fn splice(&mut self, pos: usize, removed: usize, added: usize) {
        let pos = pos.min(self.len);
        let removed = removed.min(self.len - pos);
        self.len = self.len - removed + added;
        if let Some(v) = &mut self.heights {
            let est = self.est;
            v.splice(pos..pos + removed, std::iter::repeat_n(est, added));
            self.rebuild();
        }
    }

    /// The row whose span contains content offset `y` (clamped to the rows; 0 when empty).
    pub fn row_at(&self, y: f64) -> usize {
        if self.len == 0 || y <= 0.0 {
            return 0;
        }
        let r = match &self.heights {
            None => (y / self.est as f64).floor() as usize,
            Some(_) => {
                // the largest k with offset(k) <= y
                let mut k = 0;
                let mut rem = y;
                let mut step = (self.len + 1).next_power_of_two();
                while step > 0 {
                    let j = k + step;
                    if j <= self.len && self.tree[j] <= rem {
                        k = j;
                        rem -= self.tree[j];
                    }
                    step >>= 1;
                }
                k
            }
        };
        r.min(self.len - 1)
    }

    /// The rows intersecting the content span `y0..y1`.
    pub fn range(&self, y0: f64, y1: f64) -> Range<usize> {
        if self.len == 0 || y1 <= y0 || y1 <= 0.0 || y0 >= self.total() {
            return 0..0;
        }
        let a = self.row_at(y0.max(0.0));
        let b = self.row_at(y1);
        let b = if self.offset(b) < y1 { b + 1 } else { b };
        a..b.min(self.len)
    }

    /// The rect of `row` in content coordinates `width` wide, shifted up by `scroll`.
    pub fn rect(&self, row: usize, width: f32, scroll: f64) -> Rect {
        Rect::from_origin_size(Point::new(0.0, (self.offset(row) - scroll) as f32), Size::new(width, self.height(row)))
    }
}

type Clear = Box<dyn Fn(&mut Document, NodeId)>;

/// Realised rows recycled by key (ARCH-SCENE §8.2): owned by the virtualiser; its rows are
/// children of the host node. A key that stays requested keeps its node (focus and a11y identity
/// survive), a released row goes to the free list of its template (kept, not
/// destroyed), and a new key takes a free row of its template before a new one is built.
#[derive(Default)]
pub struct RowPool {
    /// Bound by the last completed prepare: key → (node, template).
    bound: HashMap<u64, (NodeId, u32)>,
    /// Requested in the prepare running now.
    requested: HashMap<u64, (NodeId, u32)>,
    order: Vec<NodeId>,
    /// The next `row` starts a new prepare (`release_unused` ran).
    fresh: bool,
    free: Vec<(u32, NodeId)>,
    clears: Vec<Clear>,
    created: u64,
    rebinds: u64,
}

impl std::fmt::Debug for RowPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RowPool").field("bound", &self.bound.len()).field("free", &self.free.len()).field("created", &self.created).finish()
    }
}

impl RowPool {
    pub fn new() -> RowPool {
        RowPool { fresh: true, ..RowPool::default() }
    }

    /// `release_unused` runs `clear` on every released row, so a row in the free list never
    /// presents stale data (selection, pick states).
    pub fn clears(mut self, clear: impl Fn(&mut Document, NodeId) + 'static) -> RowPool {
        self.clears.push(Box::new(clear));
        self
    }

    /// The row bound to `key`: the same node while the key stays requested, else a free row of
    /// `template`, else `build` (once per new row).
    pub fn row(&mut self, doc: &mut Document, key: u64, template: u32, build: impl FnOnce(&mut Document) -> NodeId) -> NodeId {
        if std::mem::take(&mut self.fresh) {
            self.order.clear();
        }
        if let Some(&(n, _)) = self.requested.get(&key) {
            return n;
        }
        let live = |doc: &Document, n: NodeId| doc.contains(n);
        let n = match self.bound.remove(&key) {
            Some((n, t)) if t == template && live(doc, n) => n,
            other => {
                if let Some((n, t)) = other
                    && live(doc, n)
                {
                    self.free.push((t, n));
                }
                self.rebinds += 1;
                let mut found = None;
                while let Some(p) = self.free.iter().rposition(|&(t, _)| t == template) {
                    let (_, n) = self.free.swap_remove(p);
                    if live(doc, n) {
                        found = Some(n);
                        break;
                    }
                }
                match found {
                    Some(n) => n,
                    None => {
                        self.created += 1;
                        build(doc)
                    }
                }
            }
        };
        self.requested.insert(key, (n, template));
        self.order.push(n);
        n
    }

    /// Before a prepare's requests: rows bound to keys `keep` rejects (the keys leaving the
    /// window) go to the free list now, with their [`RowPool::clears`] entries removed, so this
    /// prepare's new keys take them instead of building rows.
    pub fn release_except(&mut self, doc: &mut Document, keep: impl Fn(u64) -> bool) {
        let gone: Vec<u64> = self.bound.keys().copied().filter(|&k| !keep(k)).collect();
        for k in gone {
            let Some((n, t)) = self.bound.remove(&k) else { continue };
            for c in &self.clears {
                c(doc, n);
            }
            self.free.push((t, n));
        }
    }

    /// The rows requested by this prepare, in request order.
    pub fn bound_in_order(&self) -> impl Iterator<Item = NodeId> + '_ {
        self.order.iter().copied()
    }

    /// Rows bound before this prepare and not requested by it go to the free list (kept, not
    /// destroyed), with their [`RowPool::clears`] entries removed.
    pub fn release_unused(&mut self, doc: &mut Document) {
        let gone = std::mem::replace(&mut self.bound, std::mem::take(&mut self.requested));
        for (_, (n, t)) in gone {
            for c in &self.clears {
                c(doc, n);
            }
            self.free.push((t, n));
        }
        self.fresh = true;
    }

    /// The node bound to `key` by the last prepare.
    pub fn node_of(&self, key: u64) -> Option<NodeId> {
        self.bound.get(&key).or_else(|| self.requested.get(&key)).map(|&(n, _)| n)
    }

    /// Rows built so far (new nodes; recycled rows are not counted).
    pub fn created(&self) -> u64 {
        self.created
    }

    /// Keys bound to a row other than the one they had (rebinds).
    pub fn rebinds(&self) -> u64 {
        self.rebinds
    }

    /// Rows in the free list.
    pub fn free_len(&self) -> usize {
        self.free.len()
    }
}

// ---- the document's side ----

/// What the document realised for one virtualising node.
#[derive(Clone, Debug, Default)]
pub(crate) struct VirtState {
    pub range: Range<usize>,
    pub count: usize,
    pub extent: skelter_base::geom::Vec2,
    /// The bound rows, `(row, node)`.
    pub rows: Vec<(usize, NodeId)>,
    /// Realise at the next frame even if nothing above changed.
    pub dirty: bool,
    pub realised: bool,
}

impl Document {
    /// The virtualising `node`'s model changed (rows inserted, removed or rebound): realise its
    /// rows again at the next frame.
    pub fn refresh_virtual(&mut self, node: NodeId) {
        if let Some(st) = self.virt.get_mut(&node) {
            st.dirty = true;
            self.mark(node, crate::tree::Dirty::CHILDREN);
            if let Some(i) = self.idx(node) {
                self.invalidate_style(i);
            }
        }
    }

    /// The rows realised for `node` by the last frame, `(row, node)` in row order.
    pub fn virtual_rows(&self, node: NodeId) -> &[(usize, NodeId)] {
        self.virt.get(&node).map_or(&[], |s| &s.rows)
    }

    /// The scroll extent of a virtualising node (its model's, not its children's).
    pub(crate) fn virtual_extent(&self, node: NodeId) -> Option<skelter_base::geom::Vec2> {
        self.virt.get(&node).filter(|s| s.realised).map(|s| s.extent)
    }

    /// Node `i`'s visible rect in its content space: the scroll offset and the padding box.
    fn content_viewport(&self, i: usize, node: NodeId) -> Rect {
        let l = &self.layout[i].layout;
        let b = self.style.computed(i).border_widths;
        let size = self.scroll_viewport(i).unwrap_or(Size::new((l.size.width - b.left - b.right).max(0.0), (l.size.height - b.top - b.bottom).max(0.0)));
        let o = self.scroll_offset(node);
        Rect::from_origin_size(Point::new(o.x, o.y), size)
    }

    /// Realises the rows of `window`'s virtualising nodes whose viewport, row count or extent
    /// changed, and (re)applies every bound row's placement over its cascaded layout style. True
    /// when rows were realised (the caller restyles and lays out again).
    pub(crate) fn drive_virtual(&mut self, window: skelter_base::ids::WindowId) -> bool {
        if self.virt.is_empty() {
            return false;
        }
        let nodes: Vec<NodeId> = self.virt.keys().copied().filter(|&n| self.window_of(n) == Some(window)).collect();
        let mut realised = false;
        for node in nodes {
            let Some(i) = self.idx(node) else { continue };
            let Some(mut k) = self.kinds[i].take() else { continue };
            let viewport = self.content_viewport(i, node);
            let mut rects: Vec<(NodeId, Rect)> = Vec::new();
            if let Some(vc) = k.virtual_children() {
                let count = vc.row_count();
                let extent = vc.content_extent();
                let v = vc.visible(viewport);
                let range = v.start.min(count)..v.end.min(count);
                let st = self.virt.get(&node).cloned().unwrap_or_default();
                if !st.realised || st.dirty || st.range != range || st.count != count || st.extent != extent {
                    let rows = {
                        let mut cx = crate::cx::RealizeCx { doc: self, node };
                        vc.realize(&mut cx, range.clone())
                    };
                    let rows: Vec<(usize, NodeId)> = rows.into_iter().filter(|&(_, n)| self.parent(n) == Some(node)).collect();
                    for &(_, n) in &st.rows {
                        if !rows.iter().any(|&(_, m)| m == n) && self.contains(n) {
                            self.park_row(n);
                        }
                    }
                    for &(_, n) in &rows {
                        let ci = self.ix(n);
                        self.core[ci].flags.insert(crate::tree::NodeFlags::ROW);
                    }
                    self.virt.insert(node, VirtState { range, count, extent, rows, dirty: false, realised: true });
                    realised = true;
                }
                if let Some(st) = self.virt.get(&node) {
                    rects = st.rows.iter().filter(|&&(_, n)| self.contains(n)).map(|&(r, n)| (n, vc.row_rect(r))).collect();
                }
            }
            if let Some(i) = self.idx(node) {
                self.kinds[i] = Some(k);
            }
            for (n, r) in rects {
                self.place_row(n, r);
            }
        }
        realised
    }

    /// Lays row `n` out absolutely at `r` (content space of its parent).
    fn place_row(&mut self, n: NodeId, r: Rect) {
        use taffy::{Dimension, LengthPercentageAuto as Lpa, Position};
        let i = self.ix(n);
        let st = std::sync::Arc::make_mut(&mut self.layout[i].style);
        let inset = taffy::Rect { left: Lpa::length(r.min.x), top: Lpa::length(r.min.y), right: Lpa::auto(), bottom: Lpa::auto() };
        let size = taffy::Size { width: Dimension::length(r.width()), height: Dimension::length(r.height()) };
        let display = if st.display == taffy::Display::None { taffy::Display::Flex } else { st.display };
        if st.position == Position::Absolute && st.inset == inset && st.size == size && st.display == display {
            return;
        }
        st.position = Position::Absolute;
        st.inset = inset;
        st.size = size;
        st.display = display;
        self.mark_layout(n);
    }

    /// A pooled row out of view: not laid out, not drawn, not hit.
    fn park_row(&mut self, n: NodeId) {
        let i = self.ix(n);
        if self.layout[i].style.display != taffy::Display::None {
            std::sync::Arc::make_mut(&mut self.layout[i].style).display = taffy::Display::None;
            self.mark_layout(n);
        }
    }

    /// Keeps parked rows hidden over a restyle (their cascaded `display` comes back).
    pub(crate) fn keep_rows_parked(&mut self, window: skelter_base::ids::WindowId) {
        let parked: Vec<NodeId> = self
            .virt
            .iter()
            .filter(|(n, _)| self.window_of(**n) == Some(window))
            .flat_map(|(n, st)| self.children(*n).iter().copied().filter(move |c| !st.rows.iter().any(|&(_, r)| r == *c)))
            .filter(|&c| self.flags(c).is_some_and(|f| f.contains(crate::tree::NodeFlags::ROW)))
            .collect();
        for n in parked {
            self.park_row(n);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uniform_heights_need_no_storage() {
        let h = RowHeights::uniform(1_000_000, 20.0);
        assert!(h.is_uniform());
        assert_eq!(h.total(), 2.0e7);
        assert_eq!(h.offset(999_999), 19_999_980.0);
        assert_eq!(h.row_at(19_999_990.0), 999_999);
        assert_eq!(h.range(100.0, 140.0), 5..7);
        assert_eq!(h.range(100.0, 141.0), 5..8);
        assert_eq!(h.range(-50.0, 10.0), 0..1);
        assert_eq!(h.range(2.0e7, 2.1e7), 0..0);
    }

    #[test]
    fn measured_heights_keep_offsets_and_lookups_consistent() {
        let mut h = RowHeights::uniform(100, 10.0);
        assert!(h.set(3, 30.0));
        assert!(!h.set(3, 30.0));
        assert!(!h.is_uniform());
        assert_eq!(h.offset(3), 30.0);
        assert_eq!(h.offset(4), 60.0);
        assert_eq!(h.total(), 1020.0);
        for y in [0.0, 29.9, 30.0, 59.9, 60.0, 1019.0] {
            let r = h.row_at(y);
            assert!(h.offset(r) <= y && y < h.offset(r) + h.height(r) as f64, "{y} → {r}");
        }
        h.splice(0, 2, 5);
        assert_eq!(h.len(), 103);
        assert_eq!(h.height(6), 30.0);
        assert_eq!(h.offset(6), 60.0);
        h.splice(6, 1, 0);
        assert_eq!(h.total(), 1020.0);
        assert_eq!(h.rect(1, 50.0, 5.0), Rect::new(0.0, 5.0, 50.0, 15.0));
    }

    #[test]
    fn rows_are_recycled_by_key_and_template() {
        let (mut d, root) = crate::tests::doc();
        let mut pool = RowPool::new();
        let mut build = |d: &mut Document| {
            let n = d.create(crate::tests::BOX);
            d.insert(root, n, None, None).unwrap();
            n
        };
        let a = pool.row(&mut d, 1, 0, &mut build);
        let b = pool.row(&mut d, 2, 0, &mut build);
        assert_eq!(pool.row(&mut d, 1, 0, &mut build), a, "a key asked twice in one prepare");
        pool.release_unused(&mut d);
        // the next prepare keeps key 2 and asks for key 3: key 1's row is released first and reused
        pool.release_except(&mut d, |k| k == 2 || k == 3);
        assert_eq!(pool.row(&mut d, 2, 0, &mut build), b);
        let c = pool.row(&mut d, 3, 0, &mut build);
        assert_eq!(c, a, "recycled, not built");
        pool.release_unused(&mut d);
        assert_eq!(pool.created(), 2);
        assert_eq!(pool.node_of(3), Some(a));
        // another template is never recycled into
        pool.release_except(&mut d, |k| k == 2);
        let e = pool.row(&mut d, 4, 1, &mut build);
        assert_ne!(e, a);
        assert_eq!(pool.created(), 3);
    }
}
