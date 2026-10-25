//! `virtual-list`: a list that realises only the rows near its viewport (the M5 model-view
//! protocol, spoken as GTK and Qt speak it, so `@svelters/lists`' `<ListView>` runs unchanged).
//! Ported from svelters-egui `kinds/virtuallist.rs`, `scene/virt.rs` and `views/selection.rs`.
//!
//! ```text
//! props  selection "none"|"single"|"multi"|"toggle"|"range", row-height (the estimate and minimum),
//!        uniform-rows, selected-ids (bindable), current (the cursor's id, bindable)
//! calls  splice(pos, removed, ids, labels?), count(), scrollTo(pos, hint?), setRow(row, widget), rowAt(pos)
//! sync   setup(row), bind(row, pos, id, gen), unbind(row, gen), teardown(row)
//! async  activate(pos), select(pos), selectionchange(ids), row-clicked(pos, id), row-context(pos, id),
//!        row-hovered(pos, id), copy(ids)
//! ```
//!
//! What this module computes:
//! - [`RowHeights`]: row offsets. Uniform rows need no storage; the first measured height that
//!   differs from the estimate builds a Fenwick tree (O(log n) offsets and `row_at`). Offsets are
//!   `f64`: a million 20 pt rows is past `f32`'s exact range.
//! - [`VirtualList::plan`]: the slot requests that make the bound rows match the window (the
//!   viewport plus [`OVERSCAN_ROWS`] each side): unbind the slots that left, bind the rows that
//!   entered into free slots (recycled: a slot is never destroyed while scrolling), set up new slots
//!   only when none is free, tear down free slots beyond the pool limit.
//! - [`KeySelect`]: selection keyed by item id (it survives splices, sorts and filters), the cursor,
//!   Shift ranges, Ctrl toggles, PageUp/PageDown/Home/End and type-ahead.
//!
//! **Scrolling is not the list's.** It sits in a scroll-area's content at its full height
//! ([`VirtualList::content_height`]); rows are placed at their content offsets and the scroll layer's
//! translation moves them (a compositor-only change). The scroll-area tells the list what part of it
//! is visible ([`VirtualList::set_viewport`]) and takes the offsets the list wants
//! ([`VirtualList::take_reveal`]: `scrollTo`, the keyboard cursor, scroll anchoring after a splice).

use super::draw;
use super::shim::{
    Event, EventCx, Handled, Kind, KindSpec, PropError, PropType, RecordCx, Value, prop, signal,
};
use presto_base::accesskit;
use presto_base::change::ChangeClass;
use presto_base::event::{KeyState, MouseButton, PointerPhase};
use presto_base::geom::{Point, Rect, Size};
use presto_base::ids::NodeId;
use presto_base::key::Key;
use presto_base::time::Time;
use std::collections::BTreeSet;
use std::ops::Range;

/// The row height a list assumes before it knows better.
pub const DEFAULT_ROW_HEIGHT: f32 = 32.0;
/// Rows bound beyond each edge of the viewport, in estimated rows.
pub const OVERSCAN_ROWS: usize = 2;
/// Seconds of no typing after which type-ahead starts a new prefix.
pub const TYPEAHEAD_RESET_MS: i64 = 1000;

// ---- the row-height model ----

/// Row geometry: offsets, heights and the rows in a span of the content.
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

    /// The height of rows never measured.
    pub fn estimate(&self) -> f32 {
        self.est
    }

    /// Every row has the estimated height.
    pub fn is_uniform(&self) -> bool {
        self.heights.is_none()
    }

    /// Changes the estimate: every row of a uniform model, only the unmeasured rows of a measured one.
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

    /// Sets the measured height of `row`. True if it changed.
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

    /// The rect of `row` in the list's local space, `width` wide.
    pub fn rect(&self, row: usize, width: f32) -> Rect {
        let y = self.offset(row) as f32;
        Rect::new(0.0, y, width, y + self.height(row))
    }
}

// ---- keyed selection ----

/// How a view selects.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SelectMode {
    None,
    #[default]
    Single,
    Multi,
    /// A click toggles its row, no modifier (Qt `MultiSelection`, checklists).
    Toggle,
    /// One contiguous run, Shift extends (Qt `ContiguousSelection`).
    Range,
}

pub const SELECTION: &[&str] = &["none", "single", "multi", "toggle", "range"];

impl SelectMode {
    pub fn parse(s: &str) -> Option<SelectMode> {
        Some(match s {
            "none" => SelectMode::None,
            "single" => SelectMode::Single,
            "multi" => SelectMode::Multi,
            "toggle" => SelectMode::Toggle,
            "range" => SelectMode::Range,
            _ => return None,
        })
    }

    /// More than one row can be selected (AccessKit `multiselectable`, Ctrl+A).
    pub fn is_multi(self) -> bool {
        matches!(self, SelectMode::Multi | SelectMode::Toggle | SelectMode::Range)
    }
}

/// A navigation key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Nav {
    Up,
    Down,
    PageUp,
    PageDown,
    Home,
    End,
}

impl Nav {
    pub fn of(key: Key) -> Option<Nav> {
        Some(match key {
            Key::ArrowUp => Nav::Up,
            Key::ArrowDown => Nav::Down,
            Key::PageUp => Nav::PageUp,
            Key::PageDown => Nav::PageDown,
            Key::Home => Nav::Home,
            Key::End => Nav::End,
            _ => return None,
        })
    }
}

/// The modifiers that matter to selection (`ctrl` is Cmd on macOS).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Mods {
    pub shift: bool,
    pub ctrl: bool,
}

/// What an input changed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Change {
    /// The cursor moved (reveal it).
    pub cursor: bool,
    /// The selected set changed (raise `selectionchange`).
    pub selection: bool,
}

/// The selection and cursor of one view, keyed by item id.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct KeySelect {
    pub mode: SelectMode,
    /// The focused row (keyboard cursor).
    pub cursor: Option<u64>,
    /// Where Shift ranges start.
    pub anchor: Option<u64>,
    pub selected: BTreeSet<u64>,
    typed: String,
    typed_at: Option<Time>,
}

fn pos_of(ids: &[u64], id: Option<u64>) -> Option<usize> {
    id.and_then(|id| ids.iter().position(|x| *x == id))
}

impl KeySelect {
    pub fn new(mode: SelectMode) -> KeySelect {
        KeySelect { mode, ..KeySelect::default() }
    }

    pub fn is_selected(&self, id: u64) -> bool {
        self.selected.contains(&id)
    }

    /// Replaces the selection (the host's `selected-ids`); single mode keeps the first id.
    pub fn set(&mut self, ids: impl IntoIterator<Item = u64>) -> bool {
        let mut new: BTreeSet<u64> = ids.into_iter().collect();
        match self.mode {
            SelectMode::None => new.clear(),
            SelectMode::Single => new = new.into_iter().take(1).collect(),
            SelectMode::Multi | SelectMode::Toggle | SelectMode::Range => {}
        }
        let changed = new != self.selected;
        self.selected = new;
        changed
    }

    fn only(&mut self, id: u64) -> bool {
        if self.mode == SelectMode::None {
            return false;
        }
        let changed = !(self.selected.len() == 1 && self.selected.contains(&id));
        self.selected.clear();
        self.selected.insert(id);
        changed
    }

    fn range(&mut self, ids: &[u64], to: usize) -> bool {
        let from = pos_of(ids, self.anchor).unwrap_or(to);
        let (a, b) = if from <= to { (from, to) } else { (to, from) };
        let new: BTreeSet<u64> = ids[a..=b].iter().copied().collect();
        let changed = new != self.selected;
        self.selected = new;
        changed
    }

    fn land(&mut self, ids: &[u64], to: usize, m: Mods) -> Change {
        let id = ids[to];
        let moved = self.cursor != Some(id);
        self.cursor = Some(id);
        let selection = match self.mode {
            SelectMode::None | SelectMode::Toggle => false,
            SelectMode::Multi | SelectMode::Range if m.shift => self.range(ids, to),
            SelectMode::Multi if m.ctrl => false,
            _ => {
                self.anchor = Some(id);
                self.only(id)
            }
        };
        Change { cursor: moved, selection }
    }

    /// A navigation key over the visible `ids`; `page` is the rows a page moves. No wrap.
    pub fn nav(&mut self, ids: &[u64], key: Nav, m: Mods, page: usize) -> Change {
        if ids.is_empty() {
            return Change::default();
        }
        let last = ids.len() - 1;
        let to = match (pos_of(ids, self.cursor), key) {
            (_, Nav::Home) => 0,
            (_, Nav::End) => last,
            // no cursor (or it was filtered away): any move starts at the top
            (None, _) => 0,
            (Some(p), Nav::Up) => p.saturating_sub(1),
            (Some(p), Nav::Down) => (p + 1).min(last),
            (Some(p), Nav::PageUp) => p.saturating_sub(page.max(1)),
            (Some(p), Nav::PageDown) => (p + page.max(1)).min(last),
        };
        self.land(ids, to, m)
    }

    /// Moves the cursor to `id` (a tree's parent or first child) as a plain move would.
    pub fn go_to(&mut self, ids: &[u64], id: u64, m: Mods) -> Change {
        match ids.iter().position(|x| *x == id) {
            Some(p) => self.land(ids, p, m),
            None => Change::default(),
        }
    }

    /// Space: toggles the cursor's row (multi), selects it (single).
    pub fn toggle_cursor(&mut self) -> Change {
        let Some(id) = self.cursor else { return Change::default() };
        match self.mode {
            SelectMode::None => Change::default(),
            SelectMode::Single => Change { cursor: false, selection: self.only(id) },
            SelectMode::Multi | SelectMode::Toggle => {
                if !self.selected.remove(&id) {
                    self.selected.insert(id);
                }
                self.anchor = Some(id);
                Change { cursor: false, selection: true }
            }
            SelectMode::Range => {
                self.anchor = Some(id);
                Change { cursor: false, selection: self.only(id) }
            }
        }
    }

    /// Ctrl+A in a multi mode.
    pub fn select_all(&mut self, ids: &[u64]) -> Change {
        if !self.mode.is_multi() {
            return Change::default();
        }
        let new: BTreeSet<u64> = ids.iter().copied().collect();
        let changed = new != self.selected;
        self.selected = new;
        Change { cursor: false, selection: changed }
    }

    /// A click on the row `id`.
    pub fn click(&mut self, ids: &[u64], id: u64, m: Mods) -> Change {
        let moved = self.cursor != Some(id);
        self.cursor = Some(id);
        let selection = match self.mode {
            SelectMode::None => false,
            SelectMode::Toggle => self.flip(id),
            SelectMode::Multi if m.ctrl => self.flip(id),
            SelectMode::Multi | SelectMode::Range if m.shift => match pos_of(ids, Some(id)) {
                Some(p) => self.range(ids, p),
                None => false,
            },
            _ => {
                self.anchor = Some(id);
                self.only(id)
            }
        };
        Change { cursor: moved, selection }
    }

    fn flip(&mut self, id: u64) -> bool {
        if !self.selected.remove(&id) {
            self.selected.insert(id);
        }
        self.anchor = Some(id);
        true
    }

    /// Typed text at `now`; `label(id)` is a row's type-ahead text. Moves the cursor (selecting as a
    /// plain move does) to the next row whose label starts with the typed prefix, ignoring case; one
    /// repeated character cycles through the rows starting with it.
    pub fn type_ahead(&mut self, ids: &[u64], text: &str, now: Time, label: impl Fn(u64) -> String) -> Change {
        if ids.is_empty() || text.is_empty() {
            return Change::default();
        }
        if self.typed_at.is_none_or(|t| (now - t).as_millis_f64() > TYPEAHEAD_RESET_MS as f64) {
            self.typed.clear();
        }
        self.typed_at = Some(now);
        self.typed.push_str(&text.to_lowercase());
        let mut chars = self.typed.chars();
        let first = chars.next();
        let repeat = first.is_some() && chars.all(|c| Some(c) == first) && self.typed.chars().count() > 1;
        let prefix: String = if repeat { first.into_iter().collect() } else { self.typed.clone() };
        let start = pos_of(ids, self.cursor).map_or(0, |p| if repeat || prefix.chars().count() == 1 { p + 1 } else { p });
        let n = ids.len();
        let hit = (0..n).map(|k| (start + k) % n).find(|&p| label(ids[p]).to_lowercase().starts_with(&prefix));
        match hit {
            Some(p) => self.land(ids, p, Mods::default()),
            None => Change::default(),
        }
    }
}

// ---- row slots and the protocol ----

/// One row slot: a recycled container JS fills once (`setup`, then `setRow`) and rebinds per row.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Slot {
    /// The widget `setRow` gave it.
    pub content: Option<NodeId>,
    /// `setup` was sent.
    pub setup: bool,
    /// The position it shows (`None`: unbound, or its item was spliced away).
    pub pos: Option<usize>,
    pub id: u64,
    /// Counts the slot's binds; `unbind` names the binding it ends.
    pub generation: i64,
    pub bound: bool,
}

/// What the list asks its host (JS, through the sync signals) to do with its slots, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowRequest {
    Setup { slot: usize },
    Bind { slot: usize, pos: usize, id: u64, generation: i64 },
    Unbind { slot: usize, generation: i64 },
    Teardown { slot: usize },
}

impl RowRequest {
    /// The M5 sync signal and its arguments.
    pub fn signal(&self) -> (&'static str, Value) {
        let i = |v: usize| Value::I64(v as i64);
        match *self {
            RowRequest::Setup { slot } => ("setup", Value::List(vec![i(slot)])),
            RowRequest::Bind { slot, pos, id, generation } => {
                ("bind", Value::List(vec![i(slot), i(pos), Value::I64(id as i64), Value::I64(generation)]))
            }
            RowRequest::Unbind { slot, generation } => ("unbind", Value::List(vec![i(slot), Value::I64(generation)])),
            RowRequest::Teardown { slot } => ("teardown", Value::List(vec![i(slot)])),
        }
    }
}

/// Counters of one list (tests, benchmarks).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ListStats {
    pub setups: u64,
    pub binds: u64,
    pub unbinds: u64,
    pub teardowns: u64,
}

/// `scrollTo`'s hint (Qt `QAbstractItemView::ScrollHint`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Reveal {
    /// The least scroll that shows the row.
    #[default]
    Nearest,
    Top,
    Bottom,
    Center,
}

impl Reveal {
    pub fn parse(s: &str) -> Option<Reveal> {
        Some(match s {
            "nearest" => Reveal::Nearest,
            "top" => Reveal::Top,
            "bottom" => Reveal::Bottom,
            "center" => Reveal::Center,
            _ => return None,
        })
    }
}

/// Children supplied on demand (stands in for `presto_core::VirtualChildren`, architecture §4.2):
/// what the tree, hit testing, queries and accessibility ask of a virtualiser.
pub trait VirtualChildren {
    /// Rows in the model.
    fn len(&self) -> usize;
    /// The stable key (item id) of `row`.
    fn key(&self, row: usize) -> u64;
    fn row_of(&self, key: u64) -> Option<usize>;
    /// The row's rect in the host's local space (every row has one, realised or not).
    fn row_rect(&self, row: usize) -> Option<Rect>;
    /// The realised rows: (content node, position), in position order.
    fn realised(&self) -> Vec<(NodeId, usize)>;
    /// The slot requests that bring the realised rows to the current window.
    fn plan(&mut self) -> Vec<RowRequest>;
}

/// The state of one `virtual-list` (and the row machinery of `tree-view`).
#[derive(Clone, Debug)]
pub struct VirtualList {
    /// The item ids, by position.
    ids: Vec<u64>,
    /// Type-ahead labels (empty, or one per id).
    labels: Vec<String>,
    pub sel: KeySelect,
    heights: RowHeights,
    /// Every row is `row-height` tall; rows are not measured.
    uniform: bool,
    slots: Vec<Slot>,
    /// Free slots kept beyond what the window needs; the rest are torn down.
    pool_limit: usize,
    /// The visible span in local coordinates (from the enclosing scroll-area).
    view_top: f64,
    view_height: f32,
    width: f32,
    /// An offset (list-local) the list wants scrolled to the viewport's top.
    reveal: Option<f64>,
    hovered: Option<u64>,
    focused: bool,
    /// The last primary click (id, time): a second one within the double-click time activates.
    last_click: Option<(u64, Time)>,
    stats: ListStats,
}

impl Default for VirtualList {
    fn default() -> Self {
        VirtualList::new(DEFAULT_ROW_HEIGHT)
    }
}

impl VirtualList {
    pub fn new(row_height: f32) -> VirtualList {
        VirtualList {
            ids: Vec::new(),
            labels: Vec::new(),
            sel: KeySelect::new(SelectMode::Single),
            heights: RowHeights::uniform(0, row_height),
            uniform: false,
            slots: Vec::new(),
            pool_limit: 8,
            view_top: 0.0,
            view_height: 0.0,
            width: 0.0,
            reveal: None,
            hovered: None,
            focused: false,
            last_click: None,
            stats: ListStats::default(),
        }
    }

    pub fn ids(&self) -> &[u64] {
        &self.ids
    }
    pub fn len(&self) -> usize {
        self.ids.len()
    }
    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }
    pub fn heights(&self) -> &RowHeights {
        &self.heights
    }
    pub fn slots(&self) -> &[Slot] {
        &self.slots
    }
    pub fn stats(&self) -> ListStats {
        self.stats
    }
    pub fn hovered(&self) -> Option<u64> {
        self.hovered
    }
    pub fn label(&self, id: u64) -> String {
        self.ids.iter().position(|&x| x == id).and_then(|p| self.labels.get(p).cloned()).unwrap_or_default()
    }
    /// The list's full height: what the scroll-area's content gets.
    pub fn content_height(&self) -> f64 {
        self.heights.total()
    }
    /// Free slots kept for reuse beyond the window's needs.
    pub fn set_pool_limit(&mut self, n: usize) {
        self.pool_limit = n;
    }

    /// The part of the list the viewport shows: `top` in list-local points (the scroll offset minus
    /// the list's offset in the scroll content), `height` the viewport's, `width` the list's.
    pub fn set_viewport(&mut self, top: f64, height: f32, width: f32) {
        self.view_top = top;
        self.view_height = height.max(0.0);
        self.width = width;
    }

    /// The offset (list-local, the row's top at the viewport's top) the list wants, taken once.
    pub fn take_reveal(&mut self) -> Option<f64> {
        self.reveal.take()
    }

    /// `scrollTo(pos, hint)`: the offset that shows `pos` (also kept for [`Self::take_reveal`]).
    pub fn scroll_to(&mut self, pos: usize, hint: Reveal) -> Option<f64> {
        if pos >= self.len() {
            return None;
        }
        let (y0, y1) = (self.heights.offset(pos), self.heights.offset(pos + 1));
        let view = self.view_height as f64;
        let top = self.view_top;
        let to = match hint {
            Reveal::Nearest if y0 < top || y1 - y0 > view => y0,
            Reveal::Nearest if y1 > top + view => y1 - view,
            Reveal::Nearest => return None,
            Reveal::Top => y0,
            Reveal::Bottom => y1 - view,
            Reveal::Center => (y0 + y1 - view) / 2.0,
        };
        let max = (self.content_height() - view).max(0.0);
        let to = to.clamp(0.0, max);
        self.reveal = Some(to);
        Some(to)
    }

    /// The rows bound right now: the viewport plus a margin, and never fewer rows than estimated rows
    /// need anywhere in the list (a margin cut at an end goes to the other side), so the slots set
    /// up at the top are enough while scrolling.
    pub fn window(&self) -> Range<usize> {
        if self.view_height <= 0.0 || self.is_empty() {
            return 0..0;
        }
        let est = self.heights.estimate() as f64;
        let m = OVERSCAN_ROWS as f64 * est;
        let r = self.heights.range(self.view_top - m, self.view_top + self.view_height as f64 + m);
        let len = self.len();
        let n = ((self.view_height as f64 / est).ceil() as usize + 1 + 2 * OVERSCAN_ROWS).min(len);
        let (mut start, mut end) = (r.start, r.end.max(r.start + n));
        if end > len {
            end = len;
            start = start.min(len - n);
        }
        start..end
    }

    /// Rows a page moves (the viewport in estimated rows).
    pub fn page_rows(&self) -> usize {
        ((self.view_height / self.heights.estimate()).floor() as usize).max(1)
    }

    /// `splice(pos, removed, ids, labels?)`: bound rows follow their items, removed ones unbind on the
    /// next plan; the first visible row keeps its place on screen (scroll anchoring, through
    /// [`Self::take_reveal`]) unless the list is at the top.
    pub fn splice(&mut self, pos: usize, removed: usize, add: &[u64], labels: Option<&[String]>) -> Result<(), String> {
        let len = self.ids.len();
        if pos > len || pos + removed > len {
            return Err(format!("splice {pos}+{removed} out of {len} items"));
        }
        if let Some(l) = labels
            && l.len() != add.len()
        {
            return Err(format!("splice: {} labels for {} ids", l.len(), add.len()));
        }
        let first = self.heights.row_at(self.view_top);
        let anchor = (self.view_top > 0.0 && first < len && pos <= first)
            .then(|| (self.ids[first], self.view_top - self.heights.offset(first)));
        self.ids.splice(pos..pos + removed, add.iter().copied());
        if labels.is_some() || !self.labels.is_empty() {
            self.labels.resize(len, String::new());
            let new: Vec<String> = match labels {
                Some(l) => l.to_vec(),
                None => vec![String::new(); add.len()],
            };
            self.labels.splice(pos..pos + removed, new);
        }
        self.heights.splice(pos, removed, add.len());
        let shift = |p: usize| -> Option<usize> {
            if p < pos {
                Some(p)
            } else if p < pos + removed {
                None
            } else {
                Some(p - removed + add.len())
            }
        };
        for sl in &mut self.slots {
            sl.pos = sl.pos.and_then(shift);
        }
        if let Some((id, delta)) = anchor
            && let Some(p) = self.ids.iter().position(|&x| x == id)
        {
            let to = (self.heights.offset(p) + delta).max(0.0);
            if to != self.view_top {
                self.view_top = to;
                self.reveal = Some(to);
            }
        }
        Ok(())
    }

    /// Replaces every row's type-ahead label (one per id; anything else is refused).
    pub fn set_labels(&mut self, labels: Vec<String>) -> bool {
        if labels.len() != self.ids.len() {
            return false;
        }
        self.labels = labels;
        true
    }

    /// `setRow(row, widget)`: the content of slot `row` (null clears it).
    pub fn set_row(&mut self, slot: usize, content: Option<NodeId>) {
        if slot >= self.slots.len() {
            self.slots.resize_with(slot + 1, || Slot { setup: true, ..Slot::default() });
        }
        self.slots[slot].content = content;
    }

    /// `rowAt(pos)`: the slot bound to `pos`.
    pub fn row_at_pos(&self, pos: usize) -> Option<usize> {
        self.slots.iter().position(|sl| sl.bound && sl.pos == Some(pos))
    }

    /// A row's measured height (unless `uniform-rows`); true if the layout changed.
    pub fn measured(&mut self, pos: usize, h: f32) -> bool {
        !self.uniform && self.heights.set(pos, h.max(self.heights.estimate()))
    }

    /// The position under `p` (list-local).
    pub fn pos_at(&self, p: Point) -> Option<usize> {
        if self.is_empty() || p.y < 0.0 || p.y as f64 >= self.content_height() {
            return None;
        }
        Some(self.heights.row_at(p.y as f64))
    }

    fn reveal_cursor(&mut self) {
        if let Some(p) = pos_of(&self.ids, self.sel.cursor) {
            self.scroll_to(p, Reveal::Nearest);
        }
    }

    /// Applies a selection [`Change`]: reveal, `select`/`selectionchange`, the bindable props.
    fn changed(&mut self, cx: &mut EventCx<'_>, c: Change) {
        if c.cursor {
            self.reveal_cursor();
            cx.emit("current", Value::I64(self.sel.cursor.map_or(-1, |c| c as i64)));
            cx.mark(ChangeClass::Paint);
        }
        if c.selection {
            if let Some(p) = pos_of(&self.ids, self.sel.cursor).filter(|_| self.sel.cursor.is_some_and(|c| self.sel.is_selected(c))) {
                cx.emit("select", Value::I64(p as i64));
            }
            cx.emit("selectionchange", Value::List(self.sel.selected.iter().map(|&i| Value::I64(i as i64)).collect()));
            cx.mark(ChangeClass::Paint);
        }
    }

    /// The selection, keyboard and pointer defaults shared with `tree-view` (which handles its own
    /// Left/Right first).
    pub fn handle(&mut self, cx: &mut EventCx<'_>, ev: &Event) -> Handled {
        let macos = cx.policy.os == super::shim::Os::MacOs;
        match ev {
            Event::Focus(f) => {
                self.focused = *f;
                cx.mark(ChangeClass::Paint);
                Handled::Yes
            }
            Event::Key(k) if k.state == KeyState::Pressed => {
                let m = Mods { shift: k.modifiers.contains(presto_base::event::Modifiers::SHIFT), ctrl: k.modifiers.primary(macos) };
                let ids = self.ids.clone();
                if let Some(nav) = k.key.and_then(Nav::of) {
                    let page = self.page_rows();
                    let c = self.sel.nav(&ids, nav, m, page);
                    self.changed(cx, c);
                    return Handled::Yes;
                }
                match k.key {
                    Some(Key::Space) if !m.shift => {
                        let c = self.sel.toggle_cursor();
                        self.changed(cx, c);
                        Handled::Yes
                    }
                    Some(Key::Enter) => match pos_of(&ids, self.sel.cursor) {
                        Some(p) => {
                            cx.emit("activate", Value::I64(p as i64));
                            Handled::Yes
                        }
                        None => Handled::No,
                    },
                    Some(Key::A) if m.ctrl && self.sel.mode.is_multi() => {
                        let c = self.sel.select_all(&ids);
                        self.changed(cx, c);
                        Handled::Yes
                    }
                    Some(Key::C) if m.ctrl => {
                        cx.emit("copy", Value::List(self.sel.selected.iter().map(|&i| Value::I64(i as i64)).collect()));
                        Handled::Yes
                    }
                    _ => match &k.text {
                        Some(t) if !m.ctrl && !t.is_empty() && t.chars().all(|c| !c.is_control()) => {
                            let labels = self.labels.clone();
                            let c = self.sel.type_ahead(&ids, t, cx.now, |id| {
                                ids.iter().position(|&x| x == id).and_then(|p| labels.get(p).cloned()).unwrap_or_default()
                            });
                            self.changed(cx, c);
                            if c == Change::default() { Handled::No } else { Handled::Yes }
                        }
                        _ => Handled::No,
                    },
                }
            }
            Event::Pointer(p) => {
                let pos = self.pos_at(p.pos);
                match p.phase {
                    PointerPhase::Move | PointerPhase::Enter => {
                        let id = pos.map(|x| self.ids[x]);
                        if id != self.hovered {
                            self.hovered = id;
                            cx.mark(ChangeClass::Paint);
                            if let (Some(x), Some(id)) = (pos, id) {
                                cx.emit("row-hovered", Value::List(vec![Value::I64(x as i64), Value::I64(id as i64)]));
                            }
                        }
                        Handled::No
                    }
                    PointerPhase::Leave | PointerPhase::Cancel => {
                        if self.hovered.take().is_some() {
                            cx.mark(ChangeClass::Paint);
                        }
                        Handled::No
                    }
                    PointerPhase::Down => {
                        let Some(x) = pos else { return Handled::No };
                        let id = self.ids[x];
                        let ids = self.ids.clone();
                        let m = Mods { shift: p.modifiers.contains(presto_base::event::Modifiers::SHIFT), ctrl: p.modifiers.primary(macos) };
                        cx.focus = true;
                        let args = Value::List(vec![Value::I64(x as i64), Value::I64(id as i64)]);
                        match p.button {
                            Some(MouseButton::Primary) => {
                                let c = self.sel.click(&ids, id, m);
                                self.changed(cx, c);
                                cx.emit("row-clicked", args);
                                let double = self.last_click.is_some_and(|(i, t)| i == id && cx.now - t <= cx.policy.double_click);
                                if double {
                                    cx.emit("activate", Value::I64(x as i64));
                                    self.last_click = None;
                                } else {
                                    self.last_click = Some((id, cx.now));
                                }
                                Handled::Yes
                            }
                            Some(MouseButton::Secondary) => {
                                if cx.policy.context_click_selects && !self.sel.is_selected(id) && self.sel.mode != SelectMode::None {
                                    let c = self.sel.click(&ids, id, Mods::default());
                                    self.changed(cx, c);
                                }
                                cx.emit("row-context", args);
                                Handled::Yes
                            }
                            _ => Handled::No,
                        }
                    }
                    PointerPhase::Up => Handled::No,
                }
            }
            Event::A11y(r) if r.action == accesskit::Action::Focus => {
                cx.focus = true;
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    /// Records the row backgrounds of the window (selection, hover, the focused cursor); the rows'
    /// content is their own nodes.
    pub fn record_rows(&self, cx: &mut RecordCx<'_>) {
        let pal = *cx.palette;
        let width = if self.width > 0.0 { self.width } else { cx.size.w };
        for pos in self.window() {
            let id = self.ids[pos];
            let r = self.heights.rect(pos, width);
            if self.sel.is_selected(id) {
                draw::fill(cx.list, r, 0.0, pal.selection);
            } else if self.hovered == Some(id) {
                draw::fill(cx.list, r, 0.0, pal.hover);
            }
            if self.focused && self.sel.cursor == Some(id) {
                draw::boxed(cx.list, r, 0.0, presto_base::color::Color::TRANSPARENT, 1.0, pal.accent);
            }
        }
    }

    /// The AccessKit node of the row at `pos` (realised rows only: AccessKit has no virtualised
    /// list, so the set size and position carry the rest).
    pub fn row_a11y(&self, pos: usize, role: accesskit::Role) -> Option<accesskit::Node> {
        let id = *self.ids.get(pos)?;
        let mut n = accesskit::Node::new(role);
        n.set_position_in_set(pos + 1);
        n.set_size_of_set(self.len());
        if self.sel.mode != SelectMode::None {
            n.set_selected(self.sel.is_selected(id));
        }
        if let Some(l) = self.labels.get(pos).filter(|l| !l.is_empty()) {
            n.set_label(l.as_str());
        }
        let r = self.heights.rect(pos, self.width);
        n.set_bounds(accesskit::Rect { x0: r.min.x as f64, y0: r.min.y as f64, x1: r.max.x as f64, y1: r.max.y as f64 });
        n.add_action(accesskit::Action::Click);
        n.add_action(accesskit::Action::ScrollIntoView);
        Some(n)
    }

    fn set_mode(&mut self, mode: SelectMode) {
        self.sel.mode = mode;
        let keep: Vec<u64> = self.sel.selected.iter().copied().collect();
        self.sel.set(keep);
    }
}

impl VirtualChildren for VirtualList {
    fn len(&self) -> usize {
        self.ids.len()
    }
    fn key(&self, row: usize) -> u64 {
        self.ids[row]
    }
    fn row_of(&self, key: u64) -> Option<usize> {
        self.ids.iter().position(|&x| x == key)
    }
    fn row_rect(&self, row: usize) -> Option<Rect> {
        (row < self.len()).then(|| self.heights.rect(row, self.width))
    }
    fn realised(&self) -> Vec<(NodeId, usize)> {
        let mut v: Vec<(NodeId, usize)> = self.slots.iter().filter(|sl| sl.bound).filter_map(|sl| Some((sl.content?, sl.pos?))).collect();
        v.sort_by_key(|&(_, p)| p);
        v
    }

    /// Unbinds the slots that left the window, binds the rows that entered into free slots with
    /// content, sets up new slots only when none is free (one per missing row, counting setups still
    /// waiting for `setRow`), and tears down trailing free slots beyond the pool limit. The slots'
    /// state is updated as if every request succeeded.
    fn plan(&mut self) -> Vec<RowRequest> {
        let mut out = Vec::new();
        let want = self.window();
        for (k, sl) in self.slots.iter_mut().enumerate() {
            if sl.bound && sl.pos.is_none_or(|x| !want.contains(&x)) {
                out.push(RowRequest::Unbind { slot: k, generation: sl.generation });
                sl.bound = false;
                sl.pos = None;
            }
        }
        let mut have = vec![false; want.len()];
        for sl in &self.slots {
            if let (true, Some(x)) = (sl.bound, sl.pos) {
                have[x - want.start] = true;
            }
        }
        let mut free: Vec<usize> =
            self.slots.iter().enumerate().filter(|(_, sl)| !sl.bound && sl.content.is_some()).map(|(k, _)| k).collect();
        free.reverse();
        let mut pending_setup = self.slots.iter().filter(|sl| sl.setup && sl.content.is_none()).count();
        for x in want.clone() {
            if have[x - want.start] {
                continue;
            }
            match free.pop() {
                Some(k) => {
                    let sl = &mut self.slots[k];
                    sl.bound = true;
                    sl.pos = Some(x);
                    sl.id = self.ids[x];
                    sl.generation += 1;
                    out.push(RowRequest::Bind { slot: k, pos: x, id: sl.id, generation: sl.generation });
                }
                // set up earlier, waiting for setRow
                None if pending_setup > 0 => pending_setup -= 1,
                None => {
                    self.slots.push(Slot { setup: true, ..Slot::default() });
                    out.push(RowRequest::Setup { slot: self.slots.len() - 1 });
                }
            }
        }
        // trailing free slots beyond the limit go (slot numbers of the others stay stable)
        let mut spare = free.len();
        while spare > self.pool_limit
            && let Some(last) = self.slots.last()
            && !last.bound
            && last.content.is_some()
        {
            self.slots.pop();
            out.push(RowRequest::Teardown { slot: self.slots.len() });
            spare -= 1;
        }
        for r in &out {
            match r {
                RowRequest::Setup { .. } => self.stats.setups += 1,
                RowRequest::Bind { .. } => self.stats.binds += 1,
                RowRequest::Unbind { .. } => self.stats.unbinds += 1,
                RowRequest::Teardown { .. } => self.stats.teardowns += 1,
            }
        }
        out
    }
}

static LIST_SPEC: KindSpec = KindSpec {
    name: "virtual-list",
    props: &[
        prop("selection", PropType::Enum(SELECTION), ChangeClass::Paint),
        prop("row-height", PropType::F64, ChangeClass::Layout),
        prop("uniform-rows", PropType::Bool, ChangeClass::Layout),
        prop("selected-ids", PropType::List, ChangeClass::Paint),
        prop("current", PropType::I64, ChangeClass::Paint),
        prop("pool-limit", PropType::I64, ChangeClass::Property),
    ],
    signals: &[
        signal("setup", false, false),
        signal("bind", false, false),
        signal("unbind", false, false),
        signal("teardown", false, false),
        signal("activate", true, false),
        signal("select", true, false),
        signal("selectionchange", true, false),
        signal("current", true, false),
        signal("row-clicked", true, false),
        signal("row-context", true, false),
        signal("row-hovered", true, false),
        signal("copy", true, false),
    ],
    slots: &[""],
    role: accesskit::Role::ListBox,
};

/// Reads an id list prop (`selected-ids`).
pub(crate) fn id_list(v: &Value) -> Result<Vec<u64>, PropError> {
    match v {
        Value::Null => Ok(Vec::new()),
        Value::List(l) => l.iter().map(|x| x.as_i64().map(|i| i as u64).ok_or(PropError::Invalid("ids are integers"))).collect(),
        _ => Err(PropError::Invalid("a list of ids")),
    }
}

/// Shared prop writes of lists and trees.
pub(crate) fn set_list_prop(l: &mut VirtualList, name: &str, value: &Value) -> Result<ChangeClass, PropError> {
    match name {
        "selection" => {
            let m = value.as_str().and_then(SelectMode::parse).ok_or(PropError::Invalid("selection"))?;
            l.set_mode(m);
            Ok(ChangeClass::Paint)
        }
        "row-height" => {
            let h = value.as_f32().filter(|h| *h > 0.0).ok_or(PropError::Invalid("row-height > 0"))?;
            l.heights.set_estimate(h);
            Ok(ChangeClass::Layout)
        }
        "uniform-rows" => {
            l.uniform = value.as_bool().ok_or(PropError::Invalid("bool"))?;
            if l.uniform && !l.heights.is_uniform() {
                l.heights = RowHeights::uniform(l.heights.len(), l.heights.estimate());
            }
            Ok(ChangeClass::Layout)
        }
        "selected-ids" => {
            l.sel.set(id_list(value)?);
            Ok(ChangeClass::Paint)
        }
        "current" => {
            let c = value.as_i64().ok_or(PropError::Invalid("an id or -1"))?;
            l.sel.cursor = (c >= 0).then_some(c as u64);
            Ok(ChangeClass::Paint)
        }
        "pool-limit" => {
            let n = value.as_i64().filter(|n| *n >= 0).ok_or(PropError::Invalid("pool-limit >= 0"))?;
            l.pool_limit = n as usize;
            Ok(ChangeClass::Property)
        }
        _ => Err(PropError::Unknown),
    }
}

impl Kind for VirtualList {
    fn spec() -> &'static KindSpec {
        &LIST_SPEC
    }
    fn set_prop(&mut self, name: &str, value: &Value) -> Result<ChangeClass, PropError> {
        set_list_prop(self, name, value)
    }
    fn record(&self, cx: &mut RecordCx<'_>) {
        self.record_rows(cx);
    }
    fn event(&mut self, cx: &mut EventCx<'_>, ev: &Event) -> Handled {
        self.handle(cx, ev)
    }
    fn a11y(&self, node: &mut accesskit::Node) {
        if self.sel.mode == SelectMode::None {
            node.set_role(accesskit::Role::List);
        }
        if self.sel.mode.is_multi() {
            node.set_multiselectable();
        }
        node.set_size_of_set(self.len());
    }
}

/// The list's measured size for layout: its width as given, its full content height.
pub fn intrinsic_size(l: &VirtualList, width: f32) -> Size {
    Size::new(width, l.content_height() as f32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::views::shim::{FixedLabels, Os, Palette, Policy};
    use presto_base::display::DisplayList;
    use presto_base::event::{KeyEvent, KeyLocation, Modifiers, PointerEvent, PointerType};
    use presto_base::time::Span;

    fn list(n: u64, view: f32) -> VirtualList {
        let mut l = VirtualList::new(20.0);
        l.splice(0, 0, &(0..n).map(|i| 100 + i).collect::<Vec<_>>(), None).unwrap();
        l.set_viewport(0.0, view, 200.0);
        l
    }

    /// Runs plans, answering every `setup` with a `setRow`, until nothing is left to do.
    fn settle(l: &mut VirtualList) -> Vec<RowRequest> {
        let mut all = Vec::new();
        for _ in 0..4 {
            let p = l.plan();
            if p.is_empty() {
                break;
            }
            for r in &p {
                if let RowRequest::Setup { slot } = r {
                    l.set_row(*slot, Some(NodeId::new(*slot as u32 + 1, 1)));
                }
            }
            all.extend(p);
        }
        all
    }

    fn binds(rs: &[RowRequest]) -> Vec<usize> {
        rs.iter().filter_map(|r| if let RowRequest::Bind { pos, .. } = r { Some(*pos) } else { None }).collect()
    }
    fn unbinds(rs: &[RowRequest]) -> usize {
        rs.iter().filter(|r| matches!(r, RowRequest::Unbind { .. })).count()
    }

    #[test]
    fn row_heights_uniform_and_measured() {
        let mut h = RowHeights::uniform(10, 20.0);
        assert_eq!(h.total(), 200.0);
        assert_eq!(h.row_at(45.0), 2);
        assert_eq!(h.range(30.0, 70.0), 1..4);
        assert!(h.set(1, 50.0));
        assert!(!h.is_uniform());
        assert_eq!(h.offset(2), 70.0);
        assert_eq!(h.total(), 230.0);
        assert_eq!(h.row_at(69.0), 1);
        assert_eq!(h.row_at(70.0), 2);
        assert_eq!(h.range(25.0, 75.0), 1..3);
        h.splice(0, 1, 3);
        assert_eq!(h.len(), 12);
        assert_eq!(h.offset(3), 60.0);
        assert_eq!(h.height(3), 50.0, "the measured row moved down with its splice");
    }

    #[test]
    fn the_first_plan_sets_up_then_binds_the_window() {
        let mut l = list(1000, 100.0);
        // 100 pt of 20 pt rows: 5 visible + 1 + 2 overscan each side
        assert_eq!(l.window(), 0..10);
        let p = l.plan();
        assert_eq!(p.len(), 10);
        assert!(p.iter().all(|r| matches!(r, RowRequest::Setup { .. })));
        for r in &p {
            if let RowRequest::Setup { slot } = r {
                l.set_row(*slot, Some(NodeId::new(*slot as u32 + 1, 1)));
            }
        }
        let p = l.plan();
        assert_eq!(binds(&p), (0..10).collect::<Vec<_>>());
        assert_eq!(l.realised().len(), 10);
        assert!(l.plan().is_empty(), "nothing moved");
    }

    #[test]
    fn scrolling_recycles_slots_and_binds_only_the_new_rows() {
        let mut l = list(1000, 100.0);
        settle(&mut l);
        let created = l.slots().len();
        // two rows down: rows 0..2 leave the margin, 10..12 enter
        l.set_viewport(80.0, 100.0, 200.0);
        assert_eq!(l.window(), 2..12);
        let p = l.plan();
        assert_eq!(unbinds(&p), 2);
        assert_eq!(binds(&p), vec![10, 11]);
        assert!(!p.iter().any(|r| matches!(r, RowRequest::Setup { .. })), "no new slot");
        assert_eq!(l.slots().len(), created);
        // the slots of rows 2..10 kept their rows (no rebind)
        assert_eq!(l.stats().binds, 12);
        // a long jump rebinds every slot, still without new ones
        l.set_viewport(10_000.0, 100.0, 200.0);
        let p = l.plan();
        assert_eq!(unbinds(&p), 10);
        assert_eq!(binds(&p).len(), 10);
        assert_eq!(l.slots().len(), created);
        let gens: Vec<i64> = l.slots().iter().map(|s| s.generation).collect();
        assert!(gens.iter().all(|&g| g >= 2), "each slot was bound again: {gens:?}");
    }

    #[test]
    fn a_splice_shifts_bound_rows_and_unbinds_removed_ones() {
        let mut l = list(100, 100.0);
        settle(&mut l);
        let slot_of_5 = l.row_at_pos(5).unwrap();
        // two items inserted at 0, then items 3 and 4 (old 1, 2) removed
        l.splice(0, 0, &[1, 2], None).unwrap();
        assert_eq!(l.slots()[slot_of_5].pos, Some(7));
        assert_eq!(l.slots()[slot_of_5].id, 105);
        l.splice(3, 2, &[], None).unwrap();
        assert_eq!(l.slots()[slot_of_5].pos, Some(5));
        let p = l.plan();
        // old rows 1 and 2 were spliced away: their slots unbind and take the rows now in the window
        assert_eq!(unbinds(&p), 2 + 0);
        assert_eq!(binds(&p).len(), 2);
        assert_eq!(l.ids()[0..3], [1, 2, 100]);
        assert!(l.splice(200, 0, &[], None).is_err());
    }

    #[test]
    fn a_splice_above_the_viewport_keeps_the_first_row_in_place() {
        let mut l = list(100, 100.0);
        l.set_viewport(400.0, 100.0, 200.0);
        l.splice(0, 0, &[1, 2, 3], None).unwrap();
        assert_eq!(l.take_reveal(), Some(460.0));
        assert_eq!(l.take_reveal(), None);
    }

    #[test]
    fn the_pool_limit_tears_down_trailing_free_slots() {
        let mut l = list(100, 200.0);
        l.set_pool_limit(2);
        settle(&mut l);
        let n = l.slots().len();
        // the viewport shrinks: most slots fall free, all but two of the trailing ones go
        l.set_viewport(0.0, 20.0, 200.0);
        let p = l.plan();
        let downs = p.iter().filter(|r| matches!(r, RowRequest::Teardown { .. })).count();
        assert!(downs > 0);
        assert_eq!(l.slots().len(), n - downs);
        assert_eq!(l.slots().iter().filter(|s| !s.bound).count(), 2);
    }

    #[test]
    fn variable_heights_change_the_window() {
        let mut l = list(100, 100.0);
        assert!(l.measured(0, 100.0));
        assert!(!l.measured(1, 10.0), "the estimate is the minimum");
        // row 0 fills the viewport; the window still holds the estimated row count
        assert_eq!(l.window(), 0..10);
        l.set_viewport(100.0, 100.0, 200.0);
        // rows 1..=5 are visible, with 2 rows of margin above (row 0 and the start) and below
        assert_eq!(l.window(), 0..10);
        assert_eq!(l.content_height(), 100.0 + 99.0 * 20.0);
    }

    #[test]
    fn scroll_to_hints() {
        let mut l = list(100, 100.0);
        assert_eq!(l.scroll_to(2, Reveal::Nearest), None, "already visible");
        assert_eq!(l.scroll_to(10, Reveal::Nearest), Some(120.0));
        assert_eq!(l.scroll_to(10, Reveal::Top), Some(200.0));
        assert_eq!(l.scroll_to(10, Reveal::Center), Some(160.0));
        assert_eq!(l.scroll_to(99, Reveal::Top), Some(1900.0), "clamped to the end");
    }

    const IDS: [u64; 5] = [10, 20, 30, 40, 50];
    const SHIFT: Mods = Mods { shift: true, ctrl: false };
    const CTRL: Mods = Mods { shift: false, ctrl: true };

    fn sel(k: &KeySelect) -> Vec<u64> {
        k.selected.iter().copied().collect()
    }

    #[test]
    fn arrows_move_and_select_single() {
        let mut k = KeySelect::new(SelectMode::Single);
        assert_eq!(k.nav(&IDS, Nav::Down, Mods::default(), 3), Change { cursor: true, selection: true });
        assert_eq!(k.cursor, Some(10));
        k.nav(&IDS, Nav::Down, Mods::default(), 3);
        k.nav(&IDS, Nav::PageDown, Mods::default(), 3);
        assert_eq!((k.cursor, sel(&k)), (Some(50), vec![50]));
        k.nav(&IDS, Nav::Home, SHIFT, 3);
        assert_eq!(sel(&k), vec![10], "shift does not extend in single mode");
        assert_eq!(k.nav(&[], Nav::Down, Mods::default(), 3), Change::default());
    }

    #[test]
    fn multi_ranges_toggles_and_ctrl_moves() {
        let mut k = KeySelect::new(SelectMode::Multi);
        k.click(&IDS, 20, Mods::default());
        k.nav(&IDS, Nav::Down, SHIFT, 3);
        k.nav(&IDS, Nav::Down, SHIFT, 3);
        assert_eq!(sel(&k), vec![20, 30, 40]);
        k.nav(&IDS, Nav::Down, CTRL, 3);
        assert_eq!((k.cursor, sel(&k)), (Some(50), vec![20, 30, 40]), "ctrl moves without selecting");
        k.toggle_cursor();
        assert_eq!(sel(&k), vec![20, 30, 40, 50]);
        k.click(&IDS, 30, CTRL);
        assert_eq!(sel(&k), vec![20, 40, 50]);
        k.click(&IDS, 10, SHIFT);
        assert_eq!(sel(&k), vec![10, 20, 30]);
        assert!(k.select_all(&IDS).selection);
        assert_eq!(sel(&k).len(), 5);
    }

    #[test]
    fn toggle_and_range_modes() {
        let mut k = KeySelect::new(SelectMode::Toggle);
        k.click(&IDS, 20, Mods::default());
        k.click(&IDS, 40, Mods::default());
        assert_eq!(sel(&k), vec![20, 40]);
        k.nav(&IDS, Nav::Down, Mods::default(), 3);
        assert_eq!(sel(&k), vec![20, 40], "moves only move the cursor");
        let mut k = KeySelect::new(SelectMode::Range);
        k.click(&IDS, 20, Mods::default());
        k.click(&IDS, 40, CTRL);
        assert_eq!(sel(&k), vec![40], "ctrl is a plain click");
        k.click(&IDS, 20, SHIFT);
        assert_eq!(sel(&k), vec![20, 30, 40]);
    }

    #[test]
    fn type_ahead_prefixes_and_cycles() {
        let labels = |id: u64| match id {
            10 => "apple",
            20 => "banana",
            30 => "blueberry",
            40 => "cherry",
            _ => "blackcurrant",
        }
        .to_string();
        let mut k = KeySelect::new(SelectMode::Single);
        let t = |ms: i64| Time::ZERO + Span::from_millis(ms);
        k.type_ahead(&IDS, "b", t(0), labels);
        assert_eq!(k.cursor, Some(20));
        k.type_ahead(&IDS, "l", t(100), labels);
        assert_eq!(k.cursor, Some(30), "prefix \"bl\"");
        // after the reset: one repeated character cycles
        k.type_ahead(&IDS, "b", t(3000), labels);
        assert_eq!(k.cursor, Some(50));
        k.type_ahead(&IDS, "b", t(3100), labels);
        assert_eq!(k.cursor, Some(20), "wraps");
    }

    fn key(k: Key, m: Modifiers) -> Event {
        Event::Key(KeyEvent { key: Some(k), physical: 0, text: None, state: KeyState::Pressed, repeat: false, location: KeyLocation::Standard, modifiers: m })
    }

    fn press(x: f32, y: f32, b: MouseButton) -> Event {
        Event::Pointer(PointerEvent {
            phase: PointerPhase::Down,
            pointer: 0,
            kind: PointerType::Mouse,
            button: Some(b),
            buttons: 1,
            pos: Point::new(x, y),
            modifiers: Modifiers::default(),
            pressure: 0.5,
            tilt: Default::default(),
            history: Default::default(),
        })
    }

    #[test]
    fn keys_select_reveal_and_emit() {
        let pol = Policy::for_os(Os::Linux);
        let mut l = list(100, 100.0);
        let mut cx = EventCx::new(Time::ZERO, Size::new(200.0, 2000.0), &pol);
        assert_eq!(l.event(&mut cx, &key(Key::End, Modifiers::default())), Handled::Yes);
        assert_eq!(l.sel.cursor, Some(199));
        assert_eq!(cx.emitted("select").next().unwrap().detail, Value::I64(99));
        assert_eq!(l.take_reveal(), Some(1900.0));
        let mut cx = EventCx::new(Time::ZERO, Size::new(200.0, 2000.0), &pol);
        l.event(&mut cx, &key(Key::Enter, Modifiers::default()));
        assert_eq!(cx.emitted("activate").next().unwrap().detail, Value::I64(99));
    }

    #[test]
    fn clicks_select_and_double_clicks_activate() {
        let pol = Policy::for_os(Os::Windows);
        let mut l = list(100, 100.0);
        let mut cx = EventCx::new(Time::ZERO, Size::new(200.0, 2000.0), &pol);
        l.event(&mut cx, &press(10.0, 45.0, MouseButton::Primary));
        assert_eq!(sel(&l.sel), vec![102]);
        assert!(cx.focus);
        assert!(cx.emitted("activate").next().is_none());
        let mut cx = EventCx::new(Time::ZERO + Span::from_millis(200), Size::new(200.0, 2000.0), &pol);
        l.event(&mut cx, &press(10.0, 45.0, MouseButton::Primary));
        assert_eq!(cx.emitted("activate").next().unwrap().detail, Value::I64(2));
        // a secondary click on an unselected row selects it first (Windows)
        let mut cx = EventCx::new(Time::ZERO + Span::from_millis(900), Size::new(200.0, 2000.0), &pol);
        l.event(&mut cx, &press(10.0, 65.0, MouseButton::Secondary));
        assert_eq!(sel(&l.sel), vec![103]);
        assert!(cx.emitted("row-context").next().is_some());
    }

    #[test]
    fn records_selected_and_hovered_rows() {
        let mut l = list(100, 100.0);
        l.sel.set([101]);
        l.hovered = Some(103);
        let mut list = DisplayList::new();
        let pal = Palette::default();
        let mut labels = FixedLabels;
        l.record(&mut RecordCx { size: Size::new(200.0, 2000.0), list: &mut list, palette: &pal, labels: &mut labels, now: Time::ZERO });
        assert_eq!(list.items.len(), 2);
        assert_eq!(list.bounds, Rect::new(0.0, 20.0, 200.0, 80.0));
    }

    #[test]
    fn rows_expose_set_size_and_position() {
        let mut l = list(100, 100.0);
        l.splice(0, 1, &[7], Some(&["seven".to_string()])).unwrap();
        let n = l.row_a11y(0, accesskit::Role::ListBoxOption).unwrap();
        assert_eq!(n.position_in_set(), Some(1));
        assert_eq!(n.size_of_set(), Some(100));
        assert_eq!(n.label(), Some("seven"));
        assert_eq!(n.is_selected(), Some(false));
        assert!(l.set_prop("selection", &Value::Str("multi".into())).is_ok());
        let mut root = accesskit::Node::new(accesskit::Role::ListBox);
        l.a11y(&mut root);
        assert!(root.is_multiselectable());
    }
}
