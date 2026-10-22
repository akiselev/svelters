//! Virtualisation (ARCH-SCENE §8.2, with R19 and R22; SG5 with LISTS LS1): the pieces both tiers
//! share.
//!
//! - [`RowHeights`]: the row-height model. Uniform (no storage, so 1M rows cost nothing) until a
//!   row's height differs from the estimate; then a Fenwick tree over the heights (O(log n) offsets
//!   and `row_at`, O(n) rebuild on a splice).
//! - [`VScroll`]: a virtualiser's own scroll offset. R22: the offset a frame draws with is decided
//!   *before* its rows are bound, from the stored offset, last pass's scroll-bar drags and **this
//!   pass's** wheel input ([`VScroll::advance`]), so realised rows never lag a frame. egui's
//!   `ScrollArea` cannot give that: it applies wheel input at the end of its pass
//!   (`egui-0.37.0/src/containers/scroll_area.rs:1326`) and kinetic and animated steps at its
//!   begin, from private state. Virtualisers that bind rows therefore scroll with `VScroll`;
//!   painted-row widgets, which bind nothing, may keep `ScrollArea::show_viewport`.
//! - [`RowPool`]: realised rows recycled by key and template (no create/destroy while scrolling),
//!   with R19's [`RowPool::clears`].
//! - [`VirtualChildren`]: painted rows (no nodes) exposed to a11y, anchors, queries and hit-tests,
//!   as `Target::Virtual(VNodeId { host, key })` (`query.rs` `include_virtual`, `target_by_path`).
//! - [`RowSource`]: realised rows bound from a Rust model by the built-in `virtual-list`
//!   (`kinds/virtuallist.rs`) in its prepare step, before layout (the property-grid shape of
//!   ARCH-SCENE §10.2).
//!
//! Writer lane: LISTS.

use super::access::{SceneMut, SceneRef};
use super::ext::ExtKey;
use super::id::{NodeId, Target, VNodeId};
use super::node::W;
use super::Scene;
use std::borrow::Cow;
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
    pub fn rect(&self, row: usize, width: f32, scroll: f64) -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(0.0, (self.offset(row) - scroll) as f32), egui::vec2(width, self.height(row)))
    }
}

// ---- the scroll offset (R22) ----

/// A virtualiser's own vertical scroll offset (content points from the top), with the input it
/// has not applied yet.
#[derive(Clone, Debug, PartialEq)]
pub struct VScroll {
    /// The offset of the frame being drawn (after [`VScroll::advance`]).
    pub offset: f64,
    /// Scroll-bar drags and wheel input seen during a pass that had already bound its rows:
    /// applied by the next `advance`.
    pending: f64,
    /// The cumulative pass whose offset is decided.
    pass: Option<u64>,
    /// Wheel input `advance` took from that pass (the walk consumes it, so outer scroll areas do
    /// not scroll too).
    taken: f32,
    /// Where the virtualiser was drawn last (screen points) and its layer: wheel input counts only
    /// when the pointer is over it and no other layer is on top.
    pub(crate) last_rect: egui::Rect,
    pub(crate) layer: Option<egui::LayerId>,
}

impl Default for VScroll {
    fn default() -> Self {
        VScroll { offset: 0.0, pending: 0.0, pass: None, taken: 0.0, last_rect: egui::Rect::NOTHING, layer: None }
    }
}

impl VScroll {
    /// Decides this pass's offset once (later calls in the same pass return `false`): the stored
    /// offset, plus pending drags, minus this pass's wheel input when the pointer is over the
    /// virtualiser's last drawn rect, clamped to `0..=max`. True if the offset changed.
    pub fn advance(&mut self, ctx: &egui::Context, max: f64) -> bool {
        let pass = ctx.cumulative_pass_nr();
        if self.pass == Some(pass) {
            return false;
        }
        self.pass = Some(pass);
        let mut off = self.offset + std::mem::take(&mut self.pending);
        self.taken = 0.0;
        if self.hovered(ctx) {
            let d = ctx.input(|i| i.smooth_scroll_delta.y);
            if d != 0.0 && (d > 0.0 && off > 0.0 || d < 0.0 && off < max) {
                off -= d as f64;
                self.taken = d;
            }
        }
        let off = off.clamp(0.0, max.max(0.0));
        let changed = off != self.offset;
        self.offset = off;
        changed
    }

    fn hovered(&self, ctx: &egui::Context) -> bool {
        let (Some(p), Some(layer)) = (ctx.pointer_hover_pos(), self.layer) else { return false };
        self.last_rect.contains(p) && ctx.layer_id_at(p).is_none_or(|l| l == layer) && ctx.dragged_id().is_none()
    }

    /// Whether this pass's offset is decided (`advance` ran in it).
    pub fn decided_in(&self, ctx: &egui::Context) -> bool {
        self.pass == Some(ctx.cumulative_pass_nr())
    }

    /// The wheel input this pass's `advance` took.
    pub fn taken(&self) -> f32 {
        self.taken
    }

    /// Input seen after the rows were bound: applied by the next `advance`.
    pub fn add_pending(&mut self, d: f64) {
        self.pending += d;
    }

    /// Scrolls so that the content span `y0..y1` is in a viewport `view` high (the least move).
    pub fn reveal(&mut self, y0: f64, y1: f64, view: f64) {
        if y0 < self.offset {
            self.pending += y0 - self.offset;
        } else if y1 > self.offset + view {
            self.pending += (y1 - view).min(y0) - self.offset;
        }
    }
}

// ---- realised rows ----

type Clear = Box<dyn Fn(&mut SceneMut<'_>, NodeId)>;

/// Realised rows recycled by key (ARCH-SCENE §8.2): owned by the virtualiser; its rows are
/// children of the host node. A key that stays requested keeps its node (focus, egui memory and
/// a11y identity survive), a released row goes to the free list of its template (kept, not
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

    /// R19: `release_unused` removes the node's entry for `key` from every released row, so a row
    /// in the free list never presents stale provider data (`PRESENTS`, pick states).
    pub fn clears<T: 'static>(mut self, key: &'static ExtKey<T>) -> RowPool {
        self.clears.push(Box::new(move |m, n| {
            if let Some(mut nm) = m.node(n) {
                nm.ext_mut(key).remove();
            }
        }));
        self
    }

    /// The row bound to `key`: the same node while the key stays requested, else a free row of
    /// `template`, else `build` (once per new row).
    pub fn row(&mut self, s: &mut SceneMut<'_>, key: u64, template: u32, build: impl FnOnce(&mut SceneMut<'_>) -> NodeId) -> NodeId {
        if std::mem::take(&mut self.fresh) {
            self.order.clear();
        }
        if let Some(&(n, _)) = self.requested.get(&key) {
            return n;
        }
        let live = |s: &SceneMut<'_>, n: NodeId| s.read().get(n).is_some();
        let n = match self.bound.remove(&key) {
            Some((n, t)) if t == template && live(s, n) => n,
            other => {
                if let Some((n, t)) = other
                    && live(s, n)
                {
                    self.free.push((t, n));
                }
                self.rebinds += 1;
                let mut found = None;
                while let Some(p) = self.free.iter().rposition(|&(t, _)| t == template) {
                    let (_, n) = self.free.swap_remove(p);
                    if live(s, n) {
                        found = Some(n);
                        break;
                    }
                }
                match found {
                    Some(n) => n,
                    None => {
                        self.created += 1;
                        build(s)
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
    pub fn release_except(&mut self, s: &mut SceneMut<'_>, keep: impl Fn(u64) -> bool) {
        let gone: Vec<u64> = self.bound.keys().copied().filter(|&k| !keep(k)).collect();
        for k in gone {
            let Some((n, t)) = self.bound.remove(&k) else { continue };
            for c in &self.clears {
                c(s, n);
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
    pub fn release_unused(&mut self, s: &mut SceneMut<'_>) {
        let gone = std::mem::replace(&mut self.bound, std::mem::take(&mut self.requested));
        for (_, (n, t)) in gone {
            for c in &self.clears {
                c(s, n);
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

/// Realised rows from a Rust model, bound by a `virtual-list` before layout (ARCH-SCENE §10.2's
/// property grid). `build` makes a row's subtree once per new row; `bind` writes a row's values
/// (`NodeMut::set` compares, so an unchanged value is no write).
pub trait RowSource: std::any::Any {
    fn len(&self) -> usize;
    /// A stable key per row (the same row keeps its node while it stays in view).
    fn key(&self, row: usize) -> u64;
    /// Rows of different templates are never recycled into each other.
    fn template(&self, _row: usize) -> u32 {
        0
    }
    /// Every row this high (no measuring): `None` measures rows at the list's width.
    fn uniform_height(&self) -> Option<f32> {
        None
    }
    fn build(&mut self, s: &mut SceneMut<'_>, template: u32) -> NodeId;
    fn bind(&mut self, s: &mut SceneMut<'_>, node: NodeId, row: usize);
    /// The row's anchor (`"row[param:extents]"`), for `by_path` and tutorials.
    fn anchor(&self, _key: u64) -> Option<String> {
        None
    }
    fn find_anchor(&self, _anchor: &str) -> Option<u64> {
        None
    }
    /// The model changed: the virtualiser binds its rows again before the next layout.
    fn version(&self) -> u64 {
        0
    }
}

// ---- painted rows ----

/// Painted rows, which are not nodes, as the scene's queries, anchors, hit-tests and a11y see them
/// (ARCH-SCENE §8.2). Rows are `Target::Virtual(VNodeId { host, key })`.
pub trait VirtualChildren {
    fn len(&self) -> usize;
    fn key(&self, row: usize) -> u64;
    fn row_of(&self, key: u64) -> Option<usize>;
    /// Node-local rect (relative to the host's drawn rect), `None` when not laid out.
    fn rect(&self, row: usize) -> Option<egui::Rect>;
    /// The row's anchor, `"row[feature:1234]"`.
    fn anchor(&self, key: u64) -> Option<Cow<'_, str>>;
    fn find_anchor(&self, anchor: &str) -> Option<u64>;
    /// Role, name, level, expanded, selected (set size and position are filled by [`a11y_row`]).
    fn row_a11y(&self, _key: u64, _node: &mut egui::accesskit::Node) {}
}

/// An AccessKit node for painted or realised row `index` of `len` under id `id` (built during the
/// walk; egui assembles the tree): `f` fills the rest, then set size and position (1-based).
pub fn a11y_row(ctx: &egui::Context, id: egui::Id, index: usize, len: usize, f: impl FnOnce(&mut egui::accesskit::Node)) {
    ctx.accesskit_node_builder(id, |n| {
        f(n);
        n.set_position_in_set(index + 1);
        n.set_size_of_set(len);
    });
}

/// How many widgets this pass has registered on `layer` so far: the mark to pass to
/// [`label_row_by_contents`] before painting a row's content.
pub fn widgets_in_layer(ctx: &egui::Context, layer: egui::LayerId) -> usize {
    ctx.viewport(|vp| vp.this_pass.widgets.get_layer(layer).count())
}

/// Names row `id` by its content (a list box option needs a name): the row is `labelled_by` the
/// static texts registered on `layer` since `first` (else the named widgets), as testing-library's
/// `getByLabelText` and a screen reader's name-from-contents expect. A row with a label of its own
/// keeps it.
pub fn label_row_by_contents(ctx: &egui::Context, id: egui::Id, layer: egui::LayerId, first: usize) {
    use egui::accesskit::Role;
    let named = |n: &egui::accesskit::Node| {
        let t = if n.role() == Role::Label { n.value().or(n.label()) } else { n.label() };
        t.is_some_and(|t| !t.trim().is_empty())
    };
    let (texts, others) = ctx.viewport(|vp| {
        let Some(state) = &vp.this_pass.accesskit_state else { return (Vec::new(), Vec::new()) };
        let (mut texts, mut others) = (Vec::new(), Vec::new());
        for w in vp.this_pass.widgets.get_layer(layer).skip(first) {
            if w.id == id {
                continue;
            }
            let Some(n) = state.nodes.get(&w.id) else { continue };
            if !named(n) {
                continue;
            }
            let to = if n.role() == Role::Label { &mut texts } else { &mut others };
            to.push(w.id.accesskit_id());
        }
        (texts, others)
    });
    let by = if texts.is_empty() { others } else { texts };
    if by.is_empty() {
        return;
    }
    ctx.accesskit_node_builder(id, |n| {
        if n.label().is_none_or(|l| l.trim().is_empty()) {
            n.clear_label();
            n.set_labelled_by(by);
        }
    });
}

/// `"row[<key>]"`: the default anchor of a row with no anchor of its own.
pub fn default_anchor(key: u64) -> String {
    format!("row[{key}]")
}

/// The key in a default anchor.
pub fn parse_default_anchor(a: &str) -> Option<u64> {
    a.strip_prefix("row[")?.strip_suffix(']')?.parse().ok()
}

/// Runs `f` with the painted rows of node `i`: a built-in `virtual-list`, or a provider kind whose
/// `Widget::virtual_children` answers (skipped while the widget draws itself).
pub(crate) fn with_virtual<R>(s: &Scene, i: usize, f: impl FnOnce(&dyn VirtualChildren) -> R) -> Option<R> {
    match &s.payload[i] {
        super::node::Payload::Builtin(W::VirtualList(v)) => Some(f(&**v)),
        super::node::Payload::Builtin(W::Ext(x)) => {
            let b = x.0.try_borrow().ok()?;
            let vc = b.virtual_children()?;
            Some(f(vc))
        }
        _ => None,
    }
}

// ---- reads ----

impl<'a> SceneRef<'a> {
    /// The painted (or realised) rows of node `id` as targets, in row order; empty for a node that
    /// is not a virtualiser.
    pub fn virtual_rows(self, id: NodeId) -> Vec<Target> {
        let Some(i) = self.s.idx(id) else { return Vec::new() };
        with_virtual(self.s, i, |vc| (0..vc.len()).map(|r| Target::Virtual(VNodeId { host: id, key: vc.key(r) })).collect()).unwrap_or_default()
    }

    /// Runs `f` with node `id`'s [`VirtualChildren`].
    pub fn with_virtual<R>(self, id: NodeId, f: impl FnOnce(&dyn VirtualChildren) -> R) -> Option<R> {
        with_virtual(self.s, self.s.idx(id)?, f)
    }

    /// A target by anchor in root `r`: a node (`by_anchor`), or `"<host anchor>/<row anchor>"`
    /// for a painted row, resolved through `VirtualChildren::find_anchor` whether or not the row
    /// is on screen.
    pub fn target_by_anchor(self, r: super::id::RootIx, anchor: &str) -> Option<Target> {
        if let Some(n) = self.by_anchor(r, anchor) {
            return Some(Target::Node(n.id()));
        }
        let (host, row) = anchor.rsplit_once('/')?;
        let h = self.by_anchor(r, host)?;
        let key = with_virtual(self.s, h.i, |vc| vc.find_anchor(row))??;
        Some(Target::Virtual(VNodeId { host: h.id(), key }))
    }

    /// [`SceneRef::by_path`] for targets: `"<root key>/<anchor>"` or
    /// `"<root key>/<host anchor>/<row anchor>"` (ARCH-SCENE §8.2, R24).
    pub fn target_by_path(self, path: &str) -> Option<Target> {
        let (key, anchor) = path.split_once('/')?;
        self.target_by_anchor(self.root_by_key(key)?, anchor)
    }

    /// The screen rect of a target drawn last frame: a node's drawn rect, or a row's rect inside
    /// its host's (`None` when the row is not laid out).
    pub fn target_rect(self, t: Target) -> Option<(egui::ViewportId, egui::Rect)> {
        match t {
            Target::Node(n) => self.get(n)?.screen_rect(),
            Target::Virtual(v) => {
                let host = self.get(v.host)?;
                let (vp, hr) = host.screen_rect()?;
                let r = with_virtual(self.s, host.i, |vc| vc.row_of(v.key).and_then(|row| vc.rect(row)))??;
                Some((vp, r.translate(hr.min.to_vec2())))
            }
        }
    }

    /// The anchor of a target: a node's `anchor` prop, or its host's anchor joined with the row's.
    pub fn target_anchor(self, t: Target) -> Option<String> {
        match t {
            Target::Node(n) => self.get(n)?.anchor().map(str::to_string),
            Target::Virtual(v) => {
                let host = self.get(v.host)?;
                let row = with_virtual(self.s, host.i, |vc| vc.anchor(v.key).map(|a| a.into_owned()))??;
                Some(match host.anchor() {
                    Some(h) => format!("{h}/{row}"),
                    None => row,
                })
            }
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
    }

    #[test]
    fn reveal_moves_the_least() {
        let mut s = VScroll { offset: 100.0, ..VScroll::default() };
        s.reveal(150.0, 170.0, 100.0);
        assert_eq!(s.pending, 0.0, "already in view");
        s.reveal(250.0, 270.0, 100.0);
        assert_eq!(s.offset + s.pending, 170.0, "bottom-aligned");
    }
}
