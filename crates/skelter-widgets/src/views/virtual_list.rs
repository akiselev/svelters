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

use super::common::{emit, ms, platform};
use crate::interact::{Platform, StateBits};
use crate::kind_props;
use crate::paint::{self, PaintCx, StyleView, Ua, UaRule, UaValue::*};
use crate::shim::core::{A11yCx, Event, EventCx, Handled, Kind, KindSpec, MeasureCx, NameFrom, PropError, PropId, RecordCx, SignalSpec, StyleCx, Value};
use skelter_base::accesskit;
use skelter_base::event::{KeyState, MouseButton, PointerPhase};
use skelter_base::geom::{Point, Rect, Size, Vec2};
use skelter_base::ids::NodeId;
use skelter_base::key::Key;
use skelter_base::time::Time;
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
    pub fn signal(&self) -> (&'static str, Vec<Value>) {
        let i = |v: usize| Value::I64(v as i64);
        match *self {
            RowRequest::Setup { slot } => ("setup", vec![i(slot)]),
            RowRequest::Bind { slot, pos, id, generation } => ("bind", vec![i(slot), i(pos), Value::I64(id as i64), Value::I64(generation)]),
            RowRequest::Unbind { slot, generation } => ("unbind", vec![i(slot), Value::I64(generation)]),
            RowRequest::Teardown { slot } => ("teardown", vec![i(slot)]),
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

/// The state of one `virtual-list` (and the row machinery of `tree-view`).
#[derive(Clone, Debug)]
pub struct VirtualList {
    /// The props as last written (`selected-ids` and `current` are read live from [`Self::sel`]).
    pub props: ListProps,
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
    /// The viewport (content space) the document last asked rows for, while the list is a
    /// document node that scrolls itself ([`VirtualChildren::visible`](skelter_core::VirtualChildren::visible)).
    seen: std::cell::Cell<Option<Rect>>,
    /// Its scroll bars over core's offset, in its foreground (POL2-LAYOUT 1).
    pub(crate) bars: super::scroll::CoreBars,
}

impl Default for VirtualList {
    fn default() -> Self {
        VirtualList::new(DEFAULT_ROW_HEIGHT)
    }
}

impl VirtualList {
    pub fn new(row_height: f32) -> VirtualList {
        let mut props = ListProps::default();
        props.row_height = row_height as f64;
        VirtualList {
            props,
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
            seen: std::cell::Cell::new(None),
            bars: Default::default(),
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
        self.window_in(self.view_top, self.view_height)
    }

    /// [`Self::window`] for a viewport `height` tall at `top` (list-local).
    pub fn window_in(&self, top: f64, height: f32) -> Range<usize> {
        if height <= 0.0 || self.is_empty() {
            return 0..0;
        }
        let est = self.heights.estimate() as f64;
        let m = OVERSCAN_ROWS as f64 * est;
        let r = self.heights.range(top - m, top + height as f64 + m);
        let len = self.len();
        let n = ((height as f64 / est).ceil() as usize + 1 + 2 * OVERSCAN_ROWS).min(len);
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

    /// On a document the list scrolls itself ([`skelter_core::Kind::self_scroll`]): vertically,
    /// over its rows' full height; it records in content space.
    pub fn self_scroll_extent(&self) -> skelter_core::SelfScroll {
        skelter_core::SelfScroll { content: Size::new(self.width, self.content_height() as f32), x: false, y: true }
    }

    /// An offset waits to be scrolled to ([`Self::scroll_to`], the cursor, scroll anchoring).
    pub fn reveal_pending(&self) -> bool {
        self.reveal.is_some()
    }

    /// How far the list has scrolled itself (0 while an enclosing scroll-area scrolls it): what
    /// separates its node-local points from its content's.
    pub fn scroll_top(&self) -> f32 {
        self.seen.get().map_or(0.0, |v| v.min.y)
    }

    /// Node-local `p` in content points.
    pub fn to_content(&self, p: Point) -> Point {
        Point::new(p.x, p.y + self.scroll_top())
    }

    /// The position under `p` (node-local).
    pub fn pos_at(&self, p: Point) -> Option<usize> {
        let p = self.to_content(p);
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

    /// The selected ids as a list value (`selected-ids`, `selectionchange`, `copy`).
    pub fn selected_value(&self) -> Value {
        Value::List(self.sel.selected.iter().map(|&i| Value::I64(i as i64)).collect())
    }

    /// Applies a selection [`Change`] under `spec` (the list's or the tree's): reveal,
    /// `current`/`select`/`selectionchange`, and the bindable props changed natively.
    pub(crate) fn changed(&mut self, cx: &mut EventCx<'_>, spec: &KindSpec, c: Change) {
        if c.cursor {
            self.reveal_cursor();
            emit(cx, spec, "current", vec![Value::I64(self.sel.cursor.map_or(-1, |c| c as i64))]);
            if let Some(id) = spec.prop("current") {
                cx.prop_changed(id);
            }
            cx.repaint = true;
        }
        if c.selection {
            if let Some(p) = pos_of(&self.ids, self.sel.cursor).filter(|_| self.sel.cursor.is_some_and(|c| self.sel.is_selected(c))) {
                emit(cx, spec, "select", vec![Value::I64(p as i64)]);
            }
            emit(cx, spec, "selectionchange", vec![self.selected_value()]);
            for name in ["selected-ids", "selected"] {
                if let Some(id) = spec.prop(name) {
                    cx.prop_changed(id);
                }
            }
            cx.repaint = true;
        }
    }

    /// The selection, keyboard and pointer defaults shared with `tree-view` (which handles its own
    /// Left/Right first); signals and native prop changes go out under `spec`.
    pub fn handle(&mut self, cx: &mut EventCx<'_>, spec: &KindSpec, ev: &Event) -> Handled {
        let macos = platform(cx.policy) == Platform::Macos;
        match ev {
            Event::Focus { gained, .. } => {
                self.focused = *gained;
                cx.repaint = true;
                Handled::Yes
            }
            Event::Hover(false) => {
                if self.hovered.take().is_some() {
                    cx.repaint = true;
                }
                Handled::No
            }
            Event::Key(k) if k.state == KeyState::Pressed => {
                let m = Mods { shift: k.modifiers.contains(skelter_base::event::Modifiers::SHIFT), ctrl: k.modifiers.primary(macos) };
                let ids = self.ids.clone();
                if let Some(nav) = k.key.and_then(Nav::of) {
                    let page = self.page_rows();
                    let c = self.sel.nav(&ids, nav, m, page);
                    self.changed(cx, spec, c);
                    return Handled::Yes;
                }
                match k.key {
                    Some(Key::Space) if !m.shift => {
                        let c = self.sel.toggle_cursor();
                        self.changed(cx, spec, c);
                        Handled::Yes
                    }
                    Some(Key::Enter) => match pos_of(&ids, self.sel.cursor) {
                        Some(p) => {
                            emit(cx, spec, "activate", vec![Value::I64(p as i64)]);
                            Handled::Yes
                        }
                        None => Handled::No,
                    },
                    Some(Key::A) if m.ctrl && self.sel.mode.is_multi() => {
                        let c = self.sel.select_all(&ids);
                        self.changed(cx, spec, c);
                        Handled::Yes
                    }
                    Some(Key::C) if m.ctrl => {
                        let v = self.selected_value();
                        emit(cx, spec, "copy", vec![v]);
                        Handled::Yes
                    }
                    _ => match &k.text {
                        Some(t) if !m.ctrl && !t.is_empty() && t.chars().all(|c| !c.is_control()) => {
                            let labels = self.labels.clone();
                            let c = self.sel.type_ahead(&ids, t, cx.now, |id| {
                                ids.iter().position(|&x| x == id).and_then(|p| labels.get(p).cloned()).unwrap_or_default()
                            });
                            self.changed(cx, spec, c);
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
                            cx.repaint = true;
                            if let (Some(x), Some(id)) = (pos, id) {
                                emit(cx, spec, "row-hovered", vec![Value::I64(x as i64), Value::I64(id as i64)]);
                            }
                        }
                        Handled::No
                    }
                    PointerPhase::Leave | PointerPhase::Cancel => {
                        if self.hovered.take().is_some() {
                            cx.repaint = true;
                        }
                        Handled::No
                    }
                    PointerPhase::Down => {
                        let Some(x) = pos else { return Handled::No };
                        let id = self.ids[x];
                        let ids = self.ids.clone();
                        let m = Mods { shift: p.modifiers.contains(skelter_base::event::Modifiers::SHIFT), ctrl: p.modifiers.primary(macos) };
                        cx.focus_request = Some(true);
                        let args = vec![Value::I64(x as i64), Value::I64(id as i64)];
                        match p.button {
                            Some(MouseButton::Primary) => {
                                let c = self.sel.click(&ids, id, m);
                                self.changed(cx, spec, c);
                                emit(cx, spec, "row-clicked", args);
                                let double = self.last_click.is_some_and(|(i, t)| i == id && cx.now - t <= ms(cx.policy.double_click_ms));
                                if double {
                                    emit(cx, spec, "activate", vec![Value::I64(x as i64)]);
                                    self.last_click = None;
                                } else {
                                    self.last_click = Some((id, cx.now));
                                }
                                Handled::Yes
                            }
                            Some(MouseButton::Secondary) => {
                                if cx.policy.context_click_selects && !self.sel.is_selected(id) && self.sel.mode != SelectMode::None {
                                    let c = self.sel.click(&ids, id, Mods::default());
                                    self.changed(cx, spec, c);
                                }
                                emit(cx, spec, "row-context", args);
                                Handled::Yes
                            }
                            _ => Handled::No,
                        }
                    }
                    PointerPhase::Up => Handled::No,
                }
            }
            Event::A11y(r) if r.action == accesskit::Action::Focus => {
                cx.focus_request = Some(true);
                Handled::Yes
            }
            _ => Handled::No,
        }
    }

    /// The list's own style view (its computed style over [`UA`]).
    pub(crate) fn host_view<'a>(cx: &RecordCx<'a>) -> StyleView<'a> {
        StyleView::new(cx.style.computed, UA, cx.style.state, cx.style.dark).with_theme(cx.style.theme).with_inherited(cx.style.inherited)
    }

    /// The state of the row at `pos`: `:hover`, `:selected`, `:focus` (the cursor of a focused list)
    /// and `:focus-within` (the list has focus: the selection draws active).
    fn row_state(&self, pos: usize, active: bool) -> StateBits {
        let id = self.ids[pos];
        let mut st = StateBits::NONE;
        st.set(StateBits::HOVERED, self.hovered == Some(id));
        st.set(StateBits::SELECTED, self.sel.is_selected(id));
        st.set(StateBits::FOCUSED, active && self.sel.cursor == Some(id));
        st.set(StateBits::FOCUS_WITHIN, active);
        st
    }

    /// Records the list's box (when CSS gives it one) and the window's rows as `::part(row)` boxes
    /// over [`ROW_UA`] (selection, hover, the focused cursor's outline); the rows' content is their
    /// own nodes.
    pub fn record_rows(&self, cx: &mut RecordCx<'_>) {
        let host = Self::host_view(cx);
        let row_css = cx.style.part_css("row");
        let active = self.focused || cx.style.state.contains(StateBits::FOCUSED);
        let width = if self.width > 0.0 { self.width } else { cx.size.w };
        let bounds = cx.rect();
        let mut pc = PaintCx::of(cx);
        // its inset shadows (R15's edge shadows) go over the rows, in the foreground
        paint::paint_box_layers(&mut pc, &host, bounds, false);
        for pos in self.window() {
            let r = self.heights.rect(pos, width);
            let row = host.part(row_css, ROW_UA).in_state(self.row_state(pos, active));
            paint::paint_box(&mut pc, &row, r);
            paint::paint_outline(&mut pc, &row, r);
        }
    }

    /// The AccessKit node of the row at `pos` (realised rows only: AccessKit has no virtualised
    /// list, so the set size and position carry the rest).
    pub fn row_a11y(&self, pos: usize, role: accesskit::Role) -> Option<accesskit::Node> {
        let mut n = accesskit::Node::new(role);
        if !self.member_a11y(pos, &mut n) {
            return None;
        }
        let r = self.heights.rect(pos, self.width);
        n.set_bounds(accesskit::Rect { x0: r.min.x as f64, y0: r.min.y as f64, x1: r.max.x as f64, y1: r.max.y as f64 });
        n.add_action(accesskit::Action::Click);
        n.add_action(accesskit::Action::ScrollIntoView);
        Some(n)
    }

    /// The row role of this list's members: an option of a list box, an item of a plain list.
    pub fn member_role(&self) -> accesskit::Role {
        if self.sel.mode == SelectMode::None { accesskit::Role::ListItem } else { accesskit::Role::ListBoxOption }
    }

    /// What the row at `pos` is as a member of the list, on `n`: its position and the set size, and,
    /// in a selecting list, `selected`; its label when [`VirtualList::splice`] gave one. False when
    /// there is no such row.
    pub fn member_a11y(&self, pos: usize, n: &mut accesskit::Node) -> bool {
        let Some(&id) = self.ids.get(pos) else { return false };
        n.set_position_in_set(pos + 1);
        n.set_size_of_set(self.len());
        if self.sel.mode != SelectMode::None {
            n.set_selected(self.sel.is_selected(id));
        }
        if let Some(l) = self.labels.get(pos).filter(|l| !l.is_empty()) {
            n.set_label(l.as_str());
        }
        true
    }

    fn set_mode(&mut self, mode: SelectMode) {
        self.sel.mode = mode;
        let keep: Vec<u64> = self.sel.selected.iter().copied().collect();
        self.sel.set(keep);
    }
}

/// The protocol side of the list: what the document and JS's `<ListView>` ask of the virtualiser
/// beyond [`skelter_core::VirtualChildren`] (keys, the realised rows, the slot plan).
impl VirtualList {
    /// The stable key (item id) of `row`.
    pub fn key(&self, row: usize) -> u64 {
        self.ids[row]
    }
    pub fn row_of(&self, key: u64) -> Option<usize> {
        self.ids.iter().position(|&x| x == key)
    }
    /// The row's rect in the list's local space (every row has one, realised or not).
    pub fn row_rect(&self, row: usize) -> Option<Rect> {
        (row < self.len()).then(|| self.heights.rect(row, self.width))
    }
    /// The realised rows: (content node, position), in position order.
    pub fn realised(&self) -> Vec<(NodeId, usize)> {
        let mut v: Vec<(NodeId, usize)> = self.slots.iter().filter(|sl| sl.bound).filter_map(|sl| Some((sl.content?, sl.pos?))).collect();
        v.sort_by_key(|&(_, p)| p);
        v
    }

    /// Unbinds the slots that left the window, binds the rows that entered into free slots with
    /// content, sets up new slots only when none is free (one per missing row, counting setups still
    /// waiting for `setRow`), and tears down trailing free slots beyond the pool limit. The slots'
    /// state is updated as if every request succeeded.
    pub fn plan(&mut self) -> Vec<RowRequest> {
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

impl skelter_core::VirtualChildren for VirtualList {
    fn row_count(&self) -> usize {
        self.len()
    }
    /// The rows to realise for `viewport` (list-local): the rows it overlaps plus the overscan.
    fn visible(&self, viewport: Rect) -> Range<usize> {
        self.seen.set(Some(viewport));
        self.window_in(viewport.min.y as f64, viewport.height())
    }
    fn row_rect(&self, i: usize) -> Rect {
        self.heights.rect(i, self.width)
    }
    fn content_extent(&self) -> Vec2 {
        Vec2::new(self.width, self.content_height() as f32)
    }
    fn realize(&mut self, cx: &mut skelter_core::RealizeCx, _rows: Range<usize>) -> Vec<(usize, NodeId)> {
        self.realize_rows(cx, &SPEC)
    }
    /// The node bound to a row is an option of the list box (`selected`, its position and the set
    /// size), a list item when the list doesn't select, whatever kind renders it.
    fn row_a11y(&self, row: usize, node: &mut accesskit::Node) {
        if self.member_a11y(row, node) {
            node.set_role(self.member_role());
        }
    }
}

/// The JS row protocol on a document (`@svelters/lists`, svelters-egui's `virtual-list`), shared
/// with `tree-view`: the list is a scroll container of its own (`overflow-y: scroll`, sized by its
/// layout), the document asks it for the rows of its viewport before layout, and it answers with
/// the slots bound there. The slot requests go to JS as `setup(row)`, `bind(row, pos, id, gen)`,
/// `unbind(row, gen)` and `teardown(row)`, queued with the frame's events: JS answers after the
/// frame (`setRow`, the row's content), and the list realises again.
impl VirtualList {
    /// [`skelter_core::VirtualChildren::realize`] under `spec` (the list's or the tree's signals):
    /// takes the viewport, applies a pending reveal (`scrollTo`, the cursor, scroll anchoring) to
    /// the node's scroll offset, plans the slots, emits the requests and returns the bound rows.
    pub fn realize_rows(&mut self, cx: &mut skelter_core::RealizeCx, spec: &KindSpec) -> Vec<(usize, NodeId)> {
        let node = cx.node();
        if let Some(v) = self.seen.get() {
            self.set_viewport(v.min.y as f64, v.height(), v.width());
        }
        if let Some(to) = self.reveal.take() {
            cx.doc().set_scroll_offset(node, Vec2::new(0.0, to as f32));
            self.view_top = cx.doc().scroll_offset(node).y as f64;
            let top = self.view_top as f32;
            self.seen.set(self.seen.get().map(|v| Rect::new(v.min.x, top, v.max.x, top + v.height())));
        }
        for r in self.plan() {
            let (name, args) = r.signal();
            if let Some((sig, arg)) = crate::host::core_signal(spec, name, &args) {
                cx.emit(sig, arg);
            }
        }
        self.realised().into_iter().map(|(n, p)| (p, n)).collect()
    }

    /// svelters-egui's list methods ([`LIST_METHODS`], by name): `splice(pos, removed, ids,
    /// labels?)`, `setLabels(labels)` (SV59), `count()`, `scrollTo(pos, hint?)`, `setRow(row, widget)` (the widget is the
    /// document node the host made a child of the list, by its raw id) and `rowAt(pos)`. `None`:
    /// not one of them.
    pub fn call_list(&mut self, cx: &mut EventCx<'_>, name: &str, args: &[skelter_core::Value]) -> Option<Handled> {
        use skelter_core::Value as V;
        let int = |k: usize| args.get(k).and_then(V::as_i64);
        let done = match name {
            "splice" => {
                let (Some(pos), Some(removed)) = (int(0), int(1)) else { return Some(Handled::No) };
                let ids: Vec<u64> = match args.get(2) {
                    Some(V::List(l)) => l.iter().filter_map(V::as_i64).map(|i| i as u64).collect(),
                    _ => Vec::new(),
                };
                let labels: Option<Vec<String>> = match args.get(3) {
                    Some(V::List(l)) => Some(l.iter().map(|x| x.as_str().unwrap_or("").to_string()).collect()),
                    _ => None,
                };
                self.splice(pos.max(0) as usize, removed.max(0) as usize, &ids, labels.as_deref()).is_ok()
            }
            "setLabels" => {
                let Some(V::List(l)) = args.first() else { return Some(Handled::No) };
                // the realised rows go out again with their names (`row_a11y`)
                self.set_labels(l.iter().map(|x| x.as_str().unwrap_or("").to_string()).collect())
            }
            "count" => {
                cx.ret = Some(V::I64(self.len() as i64));
                return Some(Handled::Yes);
            }
            "scrollTo" => {
                let Some(pos) = int(0) else { return Some(Handled::No) };
                let hint = match args.get(1).and_then(V::as_str) {
                    Some(h) => match Reveal::parse(h) {
                        Some(h) => h,
                        None => return Some(Handled::No),
                    },
                    None => Reveal::Nearest,
                };
                if pos >= 0 {
                    self.scroll_to(pos as usize, hint);
                }
                true
            }
            "setRow" => {
                let Some(row) = int(0).filter(|r| *r >= 0) else { return Some(Handled::No) };
                self.set_row(row as usize, int(1).filter(|w| *w > 0).map(|w| NodeId(w as u64)));
                true
            }
            "rowAt" => {
                let pos = int(0).unwrap_or(-1);
                cx.ret = Some(V::I64(if pos < 0 { -1 } else { self.row_at_pos(pos as usize).map_or(-1, |r| r as i64) }));
                return Some(Handled::Yes);
            }
            _ => return None,
        };
        if done {
            cx.refresh_virtual = true;
            cx.repaint = true;
        }
        Some(if done { Handled::Yes } else { Handled::No })
    }
}

/// svelters-egui's `virtual-list` methods (`VirtualList::call_list`), in order.
pub const LIST_METHODS: &[skelter_core::MethodSpec] = &[
    skelter_core::MethodSpec::new(
        "splice",
        &[
            skelter_core::ArgSpec::new("pos", skelter_core::ValueType::I64),
            skelter_core::ArgSpec::new("removed", skelter_core::ValueType::I64),
            skelter_core::ArgSpec::new("ids", skelter_core::ValueType::List).nullable(),
            skelter_core::ArgSpec::new("labels", skelter_core::ValueType::List).nullable(),
        ],
        None,
    ),
    skelter_core::MethodSpec::new("count", &[], Some(skelter_core::ArgSpec::new("return", skelter_core::ValueType::I64))),
    skelter_core::MethodSpec::new(
        "scrollTo",
        &[skelter_core::ArgSpec::new("pos", skelter_core::ValueType::I64), skelter_core::ArgSpec::new("hint", skelter_core::ValueType::Str).nullable()],
        None,
    ),
    skelter_core::MethodSpec::new(
        "setRow",
        &[skelter_core::ArgSpec::new("row", skelter_core::ValueType::I64), skelter_core::ArgSpec::new("widget", skelter_core::ValueType::I64).nullable()],
        None,
    ),
    skelter_core::MethodSpec::new("rowAt", &[skelter_core::ArgSpec::new("pos", skelter_core::ValueType::I64)], Some(skelter_core::ArgSpec::new("return", skelter_core::ValueType::I64))),    // SV59 (last: `tree-view` takes the first five by index)
    skelter_core::MethodSpec::new("setLabels", &[skelter_core::ArgSpec::new("labels", skelter_core::ValueType::List)], None),
];

/// The list's own look: none (CSS may give it a box); its rows are [`ROW_UA`].
pub const UA: Ua = Ua(&[]);

/// `::part(row)`: VS Code's list colours. A row's state is `:hover`, `:selected`, `:focus` (the
/// cursor of a focused list) and `:focus-within` (the list has focus): a selection in an unfocused
/// list draws in `list-sel-inactive`.
pub const ROW_UA: Ua = Ua(&[
    UaRule::base(&[
        ("outline-style", Keyword("none")),
        ("outline-color", Token("list-focus-outline")),
        ("outline-width", Px(1.0)),
        ("outline-offset", Px(-1.0)),
    ]),
    UaRule::when(StateBits::HOVERED, &[("background-color", Token("list-hover"))]),
    // the high-contrast themes have no selection fill: a selected row is outlined dotted in
    // `contrastActiveBorder` (VS Code's `listSelectionOutline`); the other themes have no such colour
    UaRule::when(
        StateBits::SELECTED,
        &[("background-color", Token("list-sel")), ("color", Token("list-sel-fg")), ("outline-style", Keyword("dotted")), ("outline-color", Token("--vscode-contrastActiveBorder"))],
    ),
    UaRule::when_not(StateBits::SELECTED, StateBits::FOCUS_WITHIN, &[("background-color", Token("list-sel-inactive"))]),
    UaRule::when(StateBits::FOCUSED, &[("outline-style", Keyword("solid")), ("outline-color", Token("list-focus-outline"))]),
]);

kind_props! {
    /// The list's props (also the first six of `tree-view`'s, in this order).
    pub struct ListProps;
    /// How clicks and keys select.
    selection "selection": String = Enum("single") ; Enum(SELECTION), Paint;
    /// The estimated (and minimum) row height.
    row_height "row-height": f64 = F64(32.0) ; F64, Layout;
    /// Every row is `row-height` tall; rows are not measured.
    uniform_rows "uniform-rows": bool = Bool(false) ; Bool, Layout;
    /// The selected item ids (native-owned: clicks and keys change it).
    selected_ids "selected-ids": Vec<Value> = None ; List, Paint, bindable;
    /// The cursor's item id (-1: none; native-owned).
    current "current": i64 = I64(-1) ; I64, Paint, bindable;
    /// Free row slots kept for reuse beyond the window's needs.
    pool_limit "pool-limit": i64 = I64(8) ; I64, Property;
    /// svelters-egui's `selected`: the position of the (first) selected item, -1 for none
    /// (native-owned: clicks and keys change it; a write selects that item and raises nothing).
    /// The list's own: `tree-view` forwards only the six above.
    selected "selected": i64 = I64(-1) ; I64, Paint, bindable;
}

/// A sync M5 signal: the slot request waits for JS.
pub(crate) const fn sync(s: SignalSpec) -> SignalSpec {
    SignalSpec { sync: true, ..s }
}

/// The list's signals (`tree-view` adds its own after these): the M5 slot requests, then the
/// selection and row events.
macro_rules! list_signals {
    ($($extra:expr),* $(,)?) => {
        &[
            $crate::views::virtual_list::sync($crate::shim::core::SignalSpec::new("setup", &[("row", $crate::shim::core::ValueType::I64)], "M5: slot `row` is new: JS builds its row widget and calls `setRow`.")),
            $crate::views::virtual_list::sync($crate::shim::core::SignalSpec::new("bind", &[("row", $crate::shim::core::ValueType::I64), ("pos", $crate::shim::core::ValueType::I64), ("id", $crate::shim::core::ValueType::I64), ("gen", $crate::shim::core::ValueType::I64)], "M5: slot `row` now shows item `id` at `pos`.")),
            $crate::views::virtual_list::sync($crate::shim::core::SignalSpec::new("unbind", &[("row", $crate::shim::core::ValueType::I64), ("gen", $crate::shim::core::ValueType::I64)], "M5: the binding `gen` of slot `row` ended.")),
            $crate::views::virtual_list::sync($crate::shim::core::SignalSpec::new("teardown", &[("row", $crate::shim::core::ValueType::I64)], "M5: slot `row` is gone.")),
            $crate::shim::core::SignalSpec::new("activate", &[("pos", $crate::shim::core::ValueType::I64)], "Enter or a double click on a row."),
            $crate::shim::core::SignalSpec::new("select", &[("pos", $crate::shim::core::ValueType::I64)], "The cursor's row was selected."),
            $crate::shim::core::SignalSpec::new("selectionchange", &[("ids", $crate::shim::core::ValueType::List)], ""),
            $crate::shim::core::SignalSpec::new("current", &[("id", $crate::shim::core::ValueType::I64)], "The cursor moved (-1: none)."),
            $crate::shim::core::SignalSpec::new("row-clicked", &[("pos", $crate::shim::core::ValueType::I64), ("id", $crate::shim::core::ValueType::I64)], ""),
            $crate::shim::core::SignalSpec::new("row-context", &[("pos", $crate::shim::core::ValueType::I64), ("id", $crate::shim::core::ValueType::I64)], "A secondary press on a row (after selecting it where the platform does)."),
            $crate::shim::core::SignalSpec::new("row-hovered", &[("pos", $crate::shim::core::ValueType::I64), ("id", $crate::shim::core::ValueType::I64)], ""),
            $crate::shim::core::SignalSpec::new("copy", &[("ids", $crate::shim::core::ValueType::List)], "Ctrl/Cmd+C with the selection."),
            $($extra,)*
        ]
    };
}
pub(crate) use list_signals;

pub const SIGNALS: &[SignalSpec] = list_signals!();

pub static SPEC: KindSpec = KindSpec {
    tag: "virtual-list",
    props: ListProps::SPECS,
    signals: SIGNALS,
    slots: &[],
    children: true,
    role: accesskit::Role::ListBox,
    name_from: NameFrom::None,
    text_prop: None,
    focusable: true,
};

/// Reads an id list (`selected-ids`, `expanded-ids`); unset is empty.
pub(crate) fn id_list(v: &Value) -> Option<Vec<u64>> {
    match v {
        Value::List(l) => l.iter().map(|x| x.as_i64().map(|i| i as u64)).collect(),
        _ => None,
    }
}

const PROPS: [P; 7] = [P::selection, P::row_height, P::uniform_rows, P::selected_ids, P::current, P::pool_limit, P::selected];

impl VirtualList {
    /// Writes one of the list props (by [`ListProps`] index; `tree-view` forwards its first six).
    pub fn set_list_prop(&mut self, prop: PropId, v: &Value) -> Result<(), PropError> {
        let spec = ListProps::SPECS.get(prop.0 as usize).ok_or(PropError::UnknownProp)?;
        let bad = || PropError::Type { prop: spec.name, expected: spec.ty };
        let which = PROPS[prop.0 as usize];
        let ok = match which {
            P::row_height => v.as_f64().is_some_and(|h| h > 0.0),
            P::pool_limit => v.as_i64().is_some_and(|n| n >= 0),
            P::selected_ids => id_list(v).is_some(),
            P::selected => v.as_i64().is_some(),
            _ => true,
        };
        if !ok {
            return Err(bad());
        }
        self.props.set(prop, v)?;
        match which {
            P::selection => self.set_mode(SelectMode::parse(&self.props.selection).ok_or_else(bad)?),
            P::row_height => self.heights.set_estimate(self.props.row_height as f32),
            P::uniform_rows => {
                self.uniform = self.props.uniform_rows;
                if self.uniform && !self.heights.is_uniform() {
                    self.heights = RowHeights::uniform(self.heights.len(), self.heights.estimate());
                }
            }
            P::selected_ids => {
                self.sel.set(id_list(v).unwrap_or_default());
            }
            P::current => self.sel.cursor = (self.props.current >= 0).then_some(self.props.current as u64),
            P::pool_limit => self.pool_limit = self.props.pool_limit as usize,
            P::selected => {
                let pos = self.props.selected;
                let id = (pos >= 0).then(|| self.ids.get(pos as usize).copied()).flatten();
                self.sel.set(id);
            }
        }
        Ok(())
    }

    /// Reads one of the list props; `selected-ids` and `current` are the live native values.
    pub fn list_prop(&self, prop: PropId) -> Option<Value> {
        match PROPS.get(prop.0 as usize)? {
            P::selected_ids => Some(self.selected_value()),
            P::current => Some(Value::I64(self.sel.cursor.map_or(-1, |c| c as i64))),
            P::selected => Some(Value::I64(self.ids.iter().position(|id| self.sel.is_selected(*id)).map_or(-1, |p| p as i64))),
            _ => self.props.get(prop),
        }
    }

    /// The list's own AccessKit properties (role, multi-selection, set size).
    pub fn list_a11y(&self, node: &mut accesskit::Node) {
        node.set_role(if self.sel.mode == SelectMode::None { accesskit::Role::List } else { accesskit::Role::ListBox });
        if self.sel.mode.is_multi() {
            node.set_multiselectable();
        }
        node.set_size_of_set(self.len());
    }
}

impl Kind for VirtualList {
    fn parts() -> &'static [&'static str] {
        &["row"]
    }
    fn spec() -> &'static KindSpec {
        &SPEC
    }
    fn kind_spec(&self) -> &'static KindSpec {
        &SPEC
    }
    fn set_prop(&mut self, prop: PropId, v: &Value) -> Result<(), PropError> {
        self.set_list_prop(prop, v)
    }
    fn get_prop(&self, prop: PropId) -> Option<Value> {
        self.list_prop(prop)
    }
    /// Its given width and its full content height (it sits in a scroll-area's content).
    fn measure(&self, _cx: &mut MeasureCx, known: taffy::Size<Option<f32>>, _avail: taffy::Size<taffy::AvailableSpace>) -> taffy::Size<f32> {
        let s = intrinsic_size(self, known.width.unwrap_or(self.width));
        taffy::Size { width: s.w, height: s.h }
    }
    fn layout_style(&self, cx: &StyleCx) -> taffy::Style {
        scrolling_layout(cx)
    }
    fn record(&self, cx: &mut RecordCx) {
        self.record_rows(cx);
    }
    fn record_foreground(&self, cx: &mut RecordCx, g: Option<&skelter_core::ScrollGeometry>) {
        self.bars.record(cx, g);
    }
    fn scroll_geometry(&mut self, g: &skelter_core::ScrollGeometry) {
        self.bars.sync(g);
    }
    fn event(&mut self, cx: &mut EventCx, ev: &Event) -> Handled {
        if let Some(h) = self.bars.event(cx, ev) {
            return h;
        }
        let h = self.handle(cx, &SPEC, ev);
        if self.reveal.is_some() {
            cx.refresh_virtual = true;
        }
        h
    }
    fn methods() -> &'static [skelter_core::MethodSpec] {
        LIST_METHODS
    }
    fn call(&mut self, cx: &mut EventCx, method: usize, args: &[skelter_core::Value]) -> Handled {
        LIST_METHODS.get(method).and_then(|m| self.call_list(cx, m.name, args)).unwrap_or(Handled::No)
    }
    fn virtual_children(&mut self) -> Option<&mut dyn skelter_core::VirtualChildren> {
        Some(self)
    }
    fn self_scroll(&self) -> Option<skelter_core::SelfScroll> {
        Some(self.self_scroll_extent())
    }
    fn a11y(&self, cx: &mut A11yCx) {
        self.list_a11y(cx.node);
    }
}

/// The layout of a list (or tree) on a document: the cascade's, scrolling vertically itself (a
/// scroll container's automatic minimum size is zero, so its size is its layout's: `flex-grow`,
/// a CSS `height`), unless CSS says otherwise.
pub fn scrolling_layout(cx: &StyleCx) -> taffy::Style {
    let mut s = cx.default_layout();
    let set = |name: &str| cx.computed.is_some_and(|c| c.get(name).is_some());
    if !set("overflow-y") && !set("overflow") {
        s.overflow = taffy::Point { x: taffy::Overflow::Hidden, y: taffy::Overflow::Scroll };
        s.scrollbar_width = 0.0;
    }
    // it takes the room its siblings leave (egui's lists fill what is available), not its rows'
    // full height (that would squeeze its siblings away); CSS `flex-grow: 0` or a `flex-basis`
    // sizes it by its content instead (A25c)
    if !set("flex-grow") && !set("flex") {
        s.flex_grow = 1.0;
    }
    if s.flex_grow > 0.0 && !set("flex-basis") && !set("flex") {
        s.flex_basis = taffy::Dimension::length(0.0);
    }
    // its rows are absolute boxes placed in its own box: it is their containing block
    if s.position == taffy::Position::Relative || !set("position") {
        s.position = taffy::Position::Relative;
    }
    s
}

/// The list's measured size for layout: its width as given, its full content height.
pub fn intrinsic_size(l: &VirtualList, width: f32) -> Size {
    Size::new(width, l.content_height() as f32)
}

/// Driving the list's and tree's event code without a document (tests).
#[cfg(test)]
pub(crate) mod test_support {
    use crate::interact::{Policy, StateBits};
    use crate::shim::core::{Event, EventCx, Handled, Kind, Value};
    use skelter_base::geom::Size;
    use skelter_base::time::Time;

    /// What one call did: its result, the signals by name with their arguments, the props changed
    /// natively, the focus request and the wake-up asked for.
    pub struct Sent {
        pub handled: Handled,
        pub signals: Vec<(&'static str, Vec<Value>)>,
        pub changed: Vec<&'static str>,
        pub focus: Option<bool>,
        pub wake: Option<Time>,
    }

    impl Sent {
        /// The first argument of the first signal named `name`.
        pub fn arg(&self, name: &str) -> Option<Value> {
            self.signals.iter().find(|(n, _)| *n == name).and_then(|(_, a)| a.first().cloned())
        }
        pub fn has(&self, name: &str) -> bool {
            self.signals.iter().any(|(n, _)| *n == name)
        }
    }

    /// Runs `f` on `k` with a fresh event context at `now` and collects what it did.
    pub fn with_cx<K: Kind, R>(k: &mut K, policy: &Policy, now: Time, f: impl FnOnce(&mut K, &mut EventCx) -> R) -> (R, Sent) {
        let spec = k.kind_spec();
        let mut state = StateBits::NONE;
        let mut cx = EventCx::new(now, Size::new(200.0, 2000.0), &mut state, policy);
        let r = f(k, &mut cx);
        let sent = Sent {
            handled: Handled::No,
            signals: cx.emitted.iter().map(|e| (spec.signals[e.signal.0 as usize].name, e.args.clone())).collect(),
            changed: cx.changed.iter().map(|p| spec.props[p.0 as usize].name).collect(),
            focus: cx.focus_request,
            wake: cx.wake,
        };
        (r, sent)
    }

    /// Sends `ev` to `k` at `now`.
    pub fn send<K: Kind>(k: &mut K, policy: &Policy, now: Time, ev: &Event) -> Sent {
        let (handled, mut sent) = with_cx(k, policy, now, |k, cx| k.event(cx, ev));
        sent.handled = handled;
        sent
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::send;
    use super::*;
    use crate::controls::testkit;
    use crate::interact::Native;
    use skelter_base::event::{KeyEvent, KeyLocation, Modifiers, PointerEvent, PointerType};
    use skelter_base::time::Span;

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
        let pol = *Native::Gtk.policy();
        let mut l = list(100, 100.0);
        let sent = send(&mut l, &pol, Time::ZERO, &key(Key::End, Modifiers::default()));
        assert_eq!(sent.handled, Handled::Yes);
        assert_eq!(l.sel.cursor, Some(199));
        assert_eq!(sent.arg("select"), Some(Value::I64(99)));
        assert!(sent.changed.contains(&"current") && sent.changed.contains(&"selected-ids"), "{:?}", sent.changed);
        assert_eq!(l.take_reveal(), Some(1900.0));
        let sent = send(&mut l, &pol, Time::ZERO, &key(Key::Enter, Modifiers::default()));
        assert_eq!(sent.arg("activate"), Some(Value::I64(99)));
        // the bindable props read the live selection
        assert_eq!(l.get_prop(P::current.id()), Some(Value::I64(199)));
        assert_eq!(l.get_prop(P::selected_ids.id()), Some(Value::List(vec![Value::I64(199)])));
    }

    #[test]
    fn clicks_select_and_double_clicks_activate() {
        let pol = *Native::Windows.policy();
        let mut l = list(100, 100.0);
        let sent = send(&mut l, &pol, Time::ZERO, &press(10.0, 45.0, MouseButton::Primary));
        assert_eq!(sel(&l.sel), vec![102]);
        assert_eq!(sent.focus, Some(true));
        assert!(!sent.has("activate"));
        let sent = send(&mut l, &pol, Time::ZERO + Span::from_millis(200), &press(10.0, 45.0, MouseButton::Primary));
        assert_eq!(sent.arg("activate"), Some(Value::I64(2)));
        // a secondary click on an unselected row selects it first (Windows)
        let sent = send(&mut l, &pol, Time::ZERO + Span::from_millis(900), &press(10.0, 65.0, MouseButton::Secondary));
        assert_eq!(sel(&l.sel), vec![103]);
        assert!(sent.has("row-context"));
    }

    #[test]
    fn records_selected_and_hovered_rows() {
        let mut l = list(100, 100.0);
        l.sel.set([101]);
        l.hovered = Some(103);
        let (list, _) = testkit::record(&l, Size::new(200.0, 2000.0), StateBits::NONE);
        assert_eq!(testkit::kinds(&list), ["quad", "quad"]);
        assert_eq!(list.bounds, Rect::new(0.0, 20.0, 200.0, 80.0));
        // a focused list outlines its cursor
        l.focused = true;
        l.sel.cursor = Some(101);
        let (list, _) = testkit::record(&l, Size::new(200.0, 2000.0), StateBits::NONE);
        assert_eq!(testkit::kinds(&list), ["quad", "quad", "quad"]);
    }

    #[test]
    fn draws_its_bars_in_the_foreground_and_drags_the_thumb() {
        // POL2-LAYOUT 1: the list scrolls itself, so its bars are its foreground, at core's offset
        use crate::shim::core::Kind as _;
        let l = list(100, 100.0);
        let g = skelter_core::ScrollGeometry::new(Rect::new(0.0, 0.0, 200.0, 100.0), Size::new(200.0, 2000.0), Vec2::new(0.0, 950.0), (false, true));
        let (rows, _) = testkit::record(&l, Size::new(200.0, 100.0), StateBits::NONE);
        assert!(rows.items.is_empty(), "no bar in the scrolled rows");
        let fore = testkit::record_foreground(&l, Size::new(200.0, 100.0), StateBits::NONE, Some(&g));
        assert_eq!(testkit::kinds(&fore), ["quad"], "an overlay thumb");
        let skelter_base::display::Prim::Quad(q) = &fore.items[0] else { unreachable!() };
        assert_eq!((q.rect.min.x, q.rect.max.x), (190.0, 200.0));
        assert_eq!((q.rect.min.y, q.rect.max.y), (40.0, 60.0), "half way down, the shortest thumb");
        // a press on the thumb is the bars', not a row's; dragging it moves the offset
        let mut d = testkit::Driver::new(l, Size::new(200.0, 100.0));
        d.kind.scroll_geometry(&g);
        d.down(195.0, 50.0);
        d.move_to(195.0, 70.0);
        assert!(d.kind.bars.scroller().offset().y > 950.0, "{:?}", d.kind.bars.scroller().offset());
        assert!(d.kind.sel.cursor.is_none(), "no row was pressed");
    }

    #[test]
    fn spec_props_are_typed_and_selection_is_bindable() {
        let s = VirtualList::spec();
        assert_eq!(s.tag, "virtual-list");
        assert!(s.prop_spec(s.prop("selected-ids").unwrap()).unwrap().bindable);
        assert!(s.signals[s.signal("bind").unwrap().0 as usize].sync);
        let mut l = VirtualList::default();
        assert!(l.set_prop(P::row_height.id(), &Value::F64(0.0)).is_err());
        assert!(l.set_prop(P::selection.id(), &Value::str("bogus")).is_err());
        l.set_prop(P::row_height.id(), &Value::F64(24.0)).unwrap();
        assert_eq!(l.heights().estimate(), 24.0);
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
        assert!(l.set_prop(P::selection.id(), &Value::str("multi")).is_ok());
        let root = testkit::a11y(&l, StateBits::NONE, Size::new(200.0, 100.0));
        assert_eq!(root.role(), accesskit::Role::ListBox);
        assert!(root.is_multiselectable());
    }

    /// SV76: a press on a nested node inside row 1 reaches the list at the list's own local point
    /// (not the nested node's), so row 1 is selected.
    #[test]
    fn a_press_on_a_rows_content_selects_that_row() {
        use crate::testdoc::{child, doc, frame, pointer};
        use skelter_base::event::PointerPhase;
        use skelter_core::Value as V;
        let (mut d, root) = doc(Size::new(300.0, 200.0), |r| {
            crate::host::register::<VirtualList>(r).unwrap();
        });
        let spec = d.registry().get(skelter_core::KindId::of("virtual-list")).unwrap().spec;
        let (splice, set_row) = (spec.method_named("splice").unwrap(), spec.method_named("setRow").unwrap());
        let bind = spec.signal_named("bind").unwrap();
        let list = child(&mut d, root, skelter_core::KindId::of("virtual-list"), "width: 200px; height: 150px");
        d.call(list, splice, &[V::I64(0), V::I64(0), V::List(vec![V::I64(10), V::I64(11), V::I64(12)].into())]).unwrap();
        // JS listens to the row requests (svelters-skelter's ListView does)
        for name in ["setup", "bind", "unbind", "teardown"] {
            d.listen(list, spec.signal_named(name).unwrap(), skelter_core::ListenFlags::EMPTY);
        }
        let (setup, bind) = (spec.signal_named("setup").unwrap(), bind);
        let field = |e: &skelter_core::JsEvent, f: &str| e.arg.as_ref().and_then(|a| a.field(f).and_then(V::as_i64).or_else(|| a.as_i64()));
        frame(&mut d);
        // setup(slot): the slot's widget, with content 8 points into the row (another origin)
        let mut inner_of_slot = std::collections::HashMap::new();
        for e in d.take_events().iter().filter(|e| e.signal == setup) {
            let slot = field(e, "row").unwrap();
            let row = d.create(skelter_core::KindId::of("box"));
            d.insert(list, row, None, None).unwrap();
            let inner = child(&mut d, row, skelter_core::KindId::of("box"), "margin-top: 8px; margin-left: 8px; width: 40px; height: 12px; background-color: #ff0000");
            d.call(list, set_row, &[V::I64(slot), V::I64(row.0 as i64)]).unwrap();
            inner_of_slot.insert(slot, inner);
        }
        assert!(inner_of_slot.len() >= 3, "{inner_of_slot:?}");
        frame(&mut d);
        let inner_of: std::collections::HashMap<i64, NodeId> =
            d.take_events().iter().filter(|e| e.signal == bind).map(|e| (field(e, "pos").unwrap(), inner_of_slot[&field(e, "row").unwrap()])).collect();
        frame(&mut d);
        let r = d.world_rect(inner_of[&1]).unwrap();
        let (x, y) = ((r.min.x + r.max.x) / 2.0, (r.min.y + r.max.y) / 2.0);
        assert_eq!(d.hit_test(crate::testdoc::W, skelter_base::geom::Point::new(x, y)), Some(inner_of[&1]), "{r:?}");
        d.dispatch(&pointer(PointerPhase::Down, x, y, 1_000, false));
        d.dispatch(&pointer(PointerPhase::Up, x, y, 2_000, false));
        let ids = spec.prop_named("selected-ids").unwrap();
        assert_eq!(d.get_prop(list, ids), Some(V::List(vec![V::I64(11)].into())));
    }

    #[test]
    fn core_virtual_children_follow_the_window() {
        use skelter_core::VirtualChildren as _;
        let l = list(1000, 100.0);
        assert_eq!(skelter_core::VirtualChildren::row_count(&l), 1000);
        assert_eq!(l.visible(Rect::new(0.0, 0.0, 200.0, 100.0)), 0..10);
        assert_eq!(skelter_core::VirtualChildren::row_rect(&l, 3), Rect::new(0.0, 60.0, 200.0, 80.0));
        assert_eq!(l.content_extent(), Vec2::new(200.0, 20_000.0));
    }
}
