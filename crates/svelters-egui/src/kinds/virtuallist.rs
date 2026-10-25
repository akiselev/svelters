//! `virtual-list` on egui (LISTS LS1, SV40; ARCH-SCENE §8.2 tier T1 and realised Rust rows): the
//! M5 model-view protocol (plans/architecture/backend-abi.md "Model-view protocol (M5)"), spoken
//! exactly as GTK and Qt speak it, so `@svelters/lists`' `<ListView>` runs unchanged.
//!
//! ```text
//! props  selected i64 (-1: none), selection "none"|"single"|"multi"|"toggle"|"range", layout
//!        "list" ("grid" draws a list), row-height i64 (the estimate and minimum height of a row;
//!        rows are measured), selected-ids i64[] (WG5a: the selected item ids, bindable);
//!        OW5a1b: current i64 (the cursor's id, bindable), uniform-rows bool, row-drag-type str,
//!        row-drop-types str, internal-move bool
//! calls  splice(pos, removed, ids[], labels[]?), count(), scrollTo(pos, hint?), setRow(row,
//!        widget|null), rowAt(pos)
//! sync   setup(row), bind(row, pos, id, gen), unbind(row, gen), teardown(row)
//! async  activate(pos), select(pos), selectionchange(ids[]), row-clicked(pos, id),
//!        row-context(pos, id), row-hovered(pos, id), copy(ids[]), drop-row(id, where, type,
//!        data), move(from, to)
//! ```
//!
//! **Row drag and drop (OW5a1b)**, shared with `tree-view`: rows run one
//! [`DragSource`](crate::interact::DragSource) (`row-drag-type`, or `internal-move`) and the list
//! one [`DropTarget`](crate::interact::DropTarget) (`row-drop-types`, `internal-move`;
//! `:drop-target` on the list while a drag it takes is over it). The ghost is the list's
//! `::part(ghost)`. A list's payload is the dragged ids (the selection when the dragged row is
//! selected), a line each, plus `text/plain` labels; `internal-move` drags one row and reorders
//! (`move(from, to)`). A tree also auto-expands a collapsed row a drag holds `into`
//! (`Policy::drag_expand_ms`).
//!
//! **Keyboard and selection (WG5a).** A click focuses the list; while focused it takes the arrows,
//! PageUp/PageDown, Home/End, Space, Ctrl+A, Enter (activate) and typed text (type-ahead on the
//! `labels` given to `splice`, or [`RowSource::label`]), through WG5b's shared
//! [`KeySelect`](crate::views::selection::KeySelect). Selection is keyed by item id (`selected-ids`,
//! `selectionchange`), so it survives splices, sorts and filters; M5's `selected` is the cursor's
//! position when the cursor row is selected (`select(pos)` as before).
//!
//! **The frame.** JS rows are bound in `logic` (no JS runs in the pass), after the checkpoint
//! ([`EguiBackend::realize_lists`]): the list decides this frame's scroll offset first (R22,
//! [`VScroll::advance`]: stored offset, last pass's scroll-bar drags, this pass's wheel input), then
//! unbinds the slots that left the window, sets up slots it lacks and binds the rows that entered,
//! through `call_sync`, so the pass draws only bound rows. Rows from a Rust model ([`RowSource`],
//! the property-grid shape) are bound in the walk's prepare step before layout
//! ([`prepare_root`]), through a [`RowPool`]. Either way the walk draws the rows at the offset
//! that was bound; a row that is still unbound (a setup or bind that ran deferred, the list grown
//! since logic) is a placeholder for one frame and asks for another.
//!
//! **Rows** are the list's children: JS row outlets (`setRow`) or pool rows. Layout lays out only
//! the bound ones, at the list's width, measures their height (the estimate, `row-height`, is the
//! minimum) into the list's [`RowHeights`] and places them at `offset(pos) - scroll`; the list is a
//! layout boundary, so scrolling relays out the list alone. Scroll anchoring: a splice above the
//! first visible row, or a measured height above it, keeps that row (by its id) where it was on
//! screen.
//!
//! **A11y** (SV40 set size): the list is a `ListBox` (or `List` with `selection="none"`), every
//! drawn row an option with `position_in_set`/`size_of_set` over the whole list, the row's widgets
//! its children. Rows are `VirtualChildren` too (queries, anchors `row[<id>]`, `target_rect`).
//!
//! Not on egui (yet): N3 model mode (`model` prop: there is no native list model on egui; LS2's
//! Rust row-model trait replaces it), `layout="grid"`.
//!
//! Writer lane: LISTS.

use super::treeview::{DropAt, Tree, tree_keys, tree_row};
use super::{Def, KindDef, MethodDef, P, PropDef, SignalDef, Ty, input, prop};
use crate::backend::EguiBackend;
use crate::views::item_keys;
use crate::views::selection::{KeySelect, Mode};
use crate::scene::virt::{RowHeights, RowPool, RowSource, VScroll, VirtualChildren};
use crate::scene::{NodeFlags, NodeId, Origin, ProviderId, RootIx, Scene, SceneError, SceneMut, W};
use crate::walk::Out;
use crate::widgets::list_rows::{self, ListBar, RowState};
use std::borrow::Cow;
use svelters_abi::Value;

pub const SELECTION: &[&str] = &["none", "single", "multi", "toggle", "range"];
pub const LAYOUT: &[&str] = &["list", "grid"];

pub(super) static PROPS: &[PropDef] = &[
    input("selected", P::ListSelected, Ty::I64, Def::I64(-1), "The selected position, -1 for none. Setting it never raises `select`."),
    prop(
        "selection",
        P::ListSelection,
        Ty::Enum(SELECTION),
        Def::Str("single"),
        "`single`: a click or the keyboard selects a row (`select`); `multi`: Ctrl/Shift clicks and keys select several (`selectionchange`); `toggle`: a click toggles its row, no modifier (checklists); `range`: one contiguous run, Shift extends; `none`: rows are not selectable.",
    ),
    input("selected-ids", P::SelectedIds, Ty::IdList, Def::Null, "The selected item ids (`bind:selected-ids`); kept by id across splices, sorts and filters."),
    prop("layout", P::ListLayout, Ty::Enum(LAYOUT), Def::Str("list"), "`list` (egui draws `grid` as a list for now)."),
    prop("row-height", P::RowHeight, Ty::I64, Def::I64(32), "The height a row is assumed to have before it is measured, and its minimum, in points."),
    input("current", P::CurrentItem, Ty::I64, Def::I64(-1), "OW5a1b: the cursor's item id (`bind:current`), -1 for none. Setting it moves the cursor without selecting."),
    prop("uniform-rows", P::UniformRows, Ty::Bool, Def::Bool(false), "OW5a1b: every row is `row-height` tall; rows are not measured (Qt `uniformRowHeights`)."),
    prop("row-drag-type", P::RowDragType, Ty::Str, Def::Str(""), "OW5a1b: non-empty: rows drag, as a payload of this type (the dragged item ids, a line each) plus `text/plain` (their labels)."),
    prop("row-drop-types", P::RowDropTypes, Ty::Str, Def::Str(""), "OW5a1b: non-empty: rows take drops of these types (WG4b patterns) and raise `drop-row`."),
    prop("internal-move", P::InternalMove, Ty::Bool, Def::Bool(false), "OW5a1b: rows reorder by drag within the list (Qt `InternalMove`): a drop raises `move(from, to)`."),
];

const ROW: (&str, Ty) = ("row", Ty::I64);
const POS: (&str, Ty) = ("pos", Ty::I64);

pub(super) static SIGNALS: &[SignalDef] = &[
    SignalDef { name: "setup", args: &[ROW], sync: true, doc: "Row slot `row` was created: give it content with `setRow`." },
    SignalDef { name: "bind", args: &[ROW, POS, ("id", Ty::I64), ("gen", Ty::I64)], sync: true, doc: "Slot `row` now shows item `id` at `pos`; `gen` counts the binds of the slot." },
    SignalDef { name: "unbind", args: &[ROW, ("gen", Ty::I64)], sync: true, doc: "Slot `row` no longer shows binding `gen`." },
    SignalDef { name: "teardown", args: &[ROW], sync: true, doc: "Slot `row` is gone: release its widget." },
    SignalDef { name: "activate", args: &[POS], sync: false, doc: "The user activated the item at `pos` (double click)." },
    SignalDef { name: "select", args: &[POS], sync: false, doc: "The user selected the item at `pos`." },
    SignalDef { name: "selectionchange", args: &[("ids", Ty::IdList)], sync: false, doc: "The user changed the selection; carries the selected ids." },
    SignalDef { name: "row-clicked", args: &[POS, ("id", Ty::I64)], sync: false, doc: "OW5a: a click on the row at `pos` (item `id`), after the selection moved." },
    SignalDef { name: "row-context", args: &[POS, ("id", Ty::I64)], sync: false, doc: "OW5a: a secondary click on the row at `pos` (item `id`): open its context menu. The selection follows first (an unselected row is selected, per platform Policy)." },
    SignalDef { name: "row-hovered", args: &[POS, ("id", Ty::I64)], sync: false, doc: "OW5a1b: the pointer entered the row at `pos` (item `id`; Qt `entered`)." },
    SignalDef { name: "copy", args: &[("ids", Ty::IdList)], sync: false, doc: "OW5a1b: Ctrl+C on the focused list: copy the selected items (their ids)." },
    SignalDef {
        name: "drop-row",
        args: &[("id", Ty::I64), ("where", Ty::Str), ("type", Ty::Str), ("data", Ty::Str)],
        sync: false,
        doc: "OW5a1b: a drag was dropped `before`, `after` or `into` item `id` (-1: below the last row), as the entry of `type` it took.",
    },
    SignalDef { name: "move", args: &[("from", Ty::I64), ("to", Ty::I64)], sync: false, doc: "OW5a1b (`internal-move`): the row at `from` was dragged to the gap before position `to` (`count()`: the end); move it." },
];

pub(super) static METHODS: &[MethodDef] = &[
    MethodDef {
        name: "splice",
        doc: "`splice(pos, removed, ids, labels?)`: replaces `removed` ids at `pos` with `ids` (unique integers); `labels` (strings, one per id) are what type-ahead matches.",
    },
    MethodDef { name: "count", doc: "The number of items." },
    MethodDef { name: "scrollTo", doc: "`scrollTo(pos, hint?)`: scrolls the item into view; `hint` (Qt `ScrollHint`): `nearest` (the default, the least scroll), `top`, `bottom` or `center`." },
    MethodDef { name: "setRow", doc: "`setRow(row, widget)`: the content of row slot `row` (called from `setup`); null clears it." },
    MethodDef { name: "rowAt", doc: "`rowAt(pos)`: the slot bound to `pos`, or -1." },
];

pub(super) const DEF: KindDef = KindDef {
    id: "VirtualList",
    tag: Some("virtual-list"),
    native: "svelters_egui::kinds::virtuallist (recycled row slots over a VScroll)",
    base: Some("Widget"),
    toplevel: false,
    slot: None,
    text_content: None,
    present: None,
    props: PROPS,
    signals: SIGNALS,
    methods: METHODS,
    doc: "A virtualized list (the M5 model-view protocol): JS keeps the items, the list asks for the rows it shows.",
};

/// Rows bound around the viewport, beyond what it shows, in estimated rows.
const OVERSCAN_ROWS: f64 = 2.0;

// ---- state ----

/// One JS row slot.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Slot {
    /// The widget `setRow` gave it (a row outlet).
    pub outlet: Option<NodeId>,
    /// `setup` was sent.
    pub setup: bool,
    /// The position it is bound to (`None`: unbound, or its item was spliced away).
    pub pos: Option<usize>,
    pub id: u64,
    pub generation: i64,
    pub bound: bool,
    /// The bind did not run synchronously (JS was busy): bound again next logic.
    pub deferred: bool,
}

/// Counters of one list (tests, benchmarks; SV47).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ListStats {
    pub setups: u64,
    pub binds: u64,
    pub unbinds: u64,
    /// Row nodes a Rust row source built.
    pub rows_built: u64,
    /// Bound rows drawn, summed over frames.
    pub rows_drawn: u64,
    /// Visible positions drawn as placeholders because no bound row was there (R22's gate: 0
    /// while scrolling).
    pub unbound_drawn: u64,
    pub frames: u64,
}

/// Rows from a Rust model.
pub(crate) struct RustRows {
    pub source: Box<dyn RowSource>,
    pub pool: RowPool,
    pub version: Option<u64>,
    /// Bound by the last prepare: (row node, position).
    pub bound: Vec<(NodeId, usize)>,
    pub offset: f64,
}

/// A `virtual-list`'s state (`W::VirtualList`).
pub struct VList {
    pub(crate) selected: i64,
    pub(crate) selection: &'static str,
    pub(crate) layout: &'static str,
    pub(crate) row_height: i64,
    /// JS mode: the item ids, by position.
    pub(crate) ids: Vec<u64>,
    /// JS mode: type-ahead labels (empty, or one per id).
    pub(crate) labels: Vec<String>,
    /// WG5a: the keyed selection and cursor.
    pub(crate) sel: KeySelect,
    pub(crate) heights: RowHeights,
    pub(crate) scroll: VScroll,
    pub(crate) slots: Vec<Slot>,
    pub(crate) rust: Option<RustRows>,
    /// `scrollTo`, applied by the next offset decision (OW5a1b: with its hint).
    pub(crate) reveal: Option<(usize, Reveal)>,
    /// The size of the last layout (0: not laid out).
    pub(crate) view: egui::Vec2,
    pub(crate) stats: ListStats,
    /// WG5b: a `tree-view` (`treeview.rs`): rows are tree nodes.
    pub(crate) tree: Option<Box<Tree>>,
    /// OW5a: the scroll bar (OW2's Scrollable over [`VList::scroll`]).
    pub(crate) bar: ListBar,
    /// OW5a (R8): the list's focus came from the keyboard; it had focus last frame.
    pub(crate) kbd: bool,
    pub(crate) was_focused: bool,
    /// OW5a1b: the row props (lists and trees).
    pub(crate) dnd: RowDnd,
    /// OW5a1b `uniform-rows`: rows are `row-height` tall, not measured.
    pub(crate) uniform: bool,
    /// OW5a1b `bind:current`: the cursor last announced (or set by the host).
    pub(crate) current_seen: Option<u64>,
    /// OW5a1b `row-hovered`: the row under the pointer last frame.
    pub(crate) hovered: Option<u64>,
}

/// OW5a1b: `scrollTo`'s hint (Qt `QAbstractItemView::ScrollHint`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Reveal {
    /// The least scroll that shows the row (`EnsureVisible`).
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

/// OW5a1b: row drag and drop on a list or tree: the props, the units and the drop's hover.
#[derive(Clone, Debug, Default)]
pub(crate) struct RowDnd {
    pub drag_type: String,
    pub drop_types: String,
    /// `internal-move` (lists).
    pub internal_move: bool,
    /// The rows' [`DragSource`](crate::interact::DragSource) (one drag at a time).
    pub source: crate::interact::DragSource,
    /// The rows' drop zone (`:drop-target` on the list).
    pub target: crate::interact::DropTarget,
    /// DnD auto-expand: the collapsed tree row a drag hovers `into`, since when (seconds).
    pub expand_hover: Option<(u64, f64)>,
}

impl RowDnd {
    /// Rows drag (a type, or reordering).
    pub fn drags(&self) -> bool {
        !self.drag_type.is_empty() || self.internal_move
    }
    /// Rows take drops.
    pub fn drops(&self) -> bool {
        !self.drop_types.is_empty() || self.internal_move
    }
}

/// OW5a1b: the payload type of an `internal-move` drag (the list accepts only its own).
pub const ROW_MOVE: &str = "application/x-svelters-row-move";

impl Default for VList {
    fn default() -> Self {
        VList {
            selected: -1,
            selection: "single",
            layout: "list",
            row_height: 32,
            ids: Vec::new(),
            labels: Vec::new(),
            sel: KeySelect::new(Mode::Single),
            heights: RowHeights::uniform(0, 32.0),
            scroll: VScroll::default(),
            slots: Vec::new(),
            rust: None,
            reveal: None,
            view: egui::Vec2::ZERO,
            stats: ListStats::default(),
            tree: None,
            bar: ListBar::default(),
            kbd: false,
            was_focused: false,
            dnd: RowDnd::default(),
            uniform: false,
            current_seen: None,
            hovered: None,
        }
    }
}

impl Clone for VList {
    /// A fresh list with the same props (rows and bindings are not cloneable).
    fn clone(&self) -> VList {
        let mut v = VList { selected: self.selected, selection: self.selection, layout: self.layout, row_height: self.row_height, sel: KeySelect::new(self.sel.mode), tree: self.tree.as_ref().map(|t| Box::new(t.fresh())), uniform: self.uniform, ..VList::default() };
        v.dnd = RowDnd { drag_type: self.dnd.drag_type.clone(), drop_types: self.dnd.drop_types.clone(), internal_move: self.dnd.internal_move, ..RowDnd::default() };
        v.heights.set_estimate(v.row_height as f32);
        v
    }
}

impl PartialEq for VList {
    /// Identity, as `ExtWidget`.
    fn eq(&self, o: &VList) -> bool {
        std::ptr::eq(self, o)
    }
}

impl std::fmt::Debug for VList {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VList").field("len", &self.len()).field("offset", &self.scroll.offset).field("slots", &self.slots.len()).field("rust", &self.rust.is_some()).finish()
    }
}

impl VList {
    /// A `tree-view`'s state (WG5b): the list with a [`Tree`].
    pub fn tree() -> VList {
        VList { row_height: 24, heights: RowHeights::uniform(0, 24.0), tree: Some(Box::default()), ..VList::default() }
    }

    /// The tree of a `tree-view`.
    pub fn tree_state(&self) -> Option<&Tree> {
        self.tree.as_deref()
    }

    /// The item ids by position (JS mode).
    pub fn ids(&self) -> &[u64] {
        &self.ids
    }

    /// The points before row `pos`'s content (a tree's indent and disclosure gutter; 0 in a list).
    pub(crate) fn row_inset(&self, pos: usize) -> f32 {
        match &self.tree {
            Some(t) if pos < self.len() => t.inset(self.key_at(pos)),
            _ => 0.0,
        }
    }

    /// Items in the list.
    pub fn len(&self) -> usize {
        match &self.rust {
            Some(r) => r.source.len(),
            None => self.ids.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// This frame's scroll offset (content points).
    pub fn offset(&self) -> f64 {
        self.scroll.offset
    }

    pub fn stats(&self) -> ListStats {
        self.stats
    }

    pub fn heights(&self) -> &RowHeights {
        &self.heights
    }

    /// JS row slots that exist (set up), bound or not.
    pub fn slot_count(&self) -> usize {
        self.slots.len()
    }

    /// The pool of a Rust row source.
    pub fn pool(&self) -> Option<&RowPool> {
        self.rust.as_ref().map(|r| &r.pool)
    }

    pub(crate) fn row_height_points(&self) -> f32 {
        self.row_height as f32
    }

    pub(crate) fn key_at(&self, pos: usize) -> u64 {
        match &self.rust {
            Some(r) => r.source.key(pos),
            None => self.ids[pos],
        }
    }

    /// Every item id, in order (WG5a: what the keyboard selection walks).
    fn all_ids(&self) -> Cow<'_, [u64]> {
        match &self.rust {
            Some(r) => Cow::Owned((0..r.source.len()).map(|p| r.source.key(p)).collect()),
            None => Cow::Borrowed(&self.ids),
        }
    }

    /// The type-ahead label of position `pos`.
    fn label_at(&self, pos: usize) -> Option<String> {
        match &self.rust {
            Some(r) => r.source.label(pos),
            None => self.labels.get(pos).filter(|l| !l.is_empty()).cloned(),
        }
    }

    /// OW5a1b: the cursor's item id (a tree's own, or the list's).
    pub fn cursor(&self) -> Option<u64> {
        match &self.tree {
            Some(t) => t.sel.cursor,
            None => self.sel.cursor,
        }
    }

    /// The selection (WG5a): ids, cursor, mode.
    pub fn selection(&self) -> &KeySelect {
        &self.sel
    }

    /// Rows a page move skips: the rows that fit the view.
    fn page(&self) -> usize {
        ((self.view.y / self.heights.estimate().max(1.0)).floor() as usize).max(1)
    }

    fn max_offset(&self) -> f64 {
        (self.heights.total() - self.view.y as f64).max(0.0)
    }

    /// The positions to bind at this frame's offset: the viewport plus a margin, and never fewer
    /// rows than the most that estimated rows need anywhere in the list (a margin cut at the top or
    /// bottom goes to the other side), so the rows set up at the top are enough while scrolling.
    fn window(&self) -> std::ops::Range<usize> {
        if self.view.y <= 0.0 {
            return 0..0;
        }
        let est = self.heights.estimate() as f64;
        let m = OVERSCAN_ROWS * est;
        let off = self.scroll.offset;
        let r = self.heights.range(off - m, off + self.view.y as f64 + m);
        let len = self.heights.len();
        let n = ((self.view.y as f64 / est).ceil() as usize + 1 + 2 * OVERSCAN_ROWS as usize).min(len);
        let (mut start, mut end) = (r.start, r.end.max(r.start + n));
        if end > len {
            end = len;
            start = start.min(len - n);
        }
        start..end
    }

    /// Decides this pass's offset (R22) with a pending `scrollTo`. True if it changed.
    fn advance(&mut self, ctx: &egui::Context) -> bool {
        if let Some((pos, hint)) = self.reveal.take()
            && pos < self.len()
        {
            let (y0, y1) = (self.heights.offset(pos), self.heights.offset(pos + 1));
            let view = self.view.y as f64;
            match hint {
                Reveal::Nearest => self.scroll.reveal(y0, y1, view),
                Reveal::Top => self.scroll.reveal_at(y0),
                Reveal::Bottom => self.scroll.reveal_at(y1 - view),
                Reveal::Center => self.scroll.reveal_at((y0 + y1 - view) / 2.0),
            }
        }
        let max = self.max_offset();
        self.scroll.advance(ctx, max)
    }

    /// The rows bound now (node, position): what layout places and the walk draws.
    pub(crate) fn placed(&self, s: &Scene) -> Vec<(NodeId, usize)> {
        match &self.rust {
            Some(r) => r.bound.clone(),
            None => {
                let mut v: Vec<(NodeId, usize)> = self
                    .slots
                    .iter()
                    .filter(|sl| sl.bound && !sl.deferred)
                    .filter_map(|sl| Some((sl.outlet.filter(|&o| s.idx(o).is_some())?, sl.pos?)))
                    .collect();
                v.sort_by_key(|&(_, p)| p);
                v
            }
        }
    }

    /// Replaces `removed` ids at `pos` with `ids`; bound rows follow, the first visible row keeps
    /// its place on screen (scroll anchoring by key) unless the list is at the top.
    pub(crate) fn splice(&mut self, pos: usize, removed: usize, add: &[u64], labels: Option<&[String]>) -> Result<(), String> {
        let len = self.ids.len();
        if pos > len || pos + removed > len {
            return Err(format!("splice {pos}+{removed} out of {len} items"));
        }
        let first = self.heights.row_at(self.scroll.offset);
        let anchor = (self.scroll.offset > 0.0 && first < len && pos <= first).then(|| (self.ids[first], self.scroll.offset - self.heights.offset(first)));
        let mut keys = item_keys::RowKeys { ids: std::mem::take(&mut self.ids), labels: std::mem::take(&mut self.labels) };
        keys.splice(pos, removed, add, labels);
        (self.ids, self.labels) = (keys.ids, keys.labels);
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
        if self.selected >= 0 {
            self.selected = shift(self.selected as usize).map_or(-1, |p| p as i64);
        }
        if let Some((id, delta)) = anchor
            && let Some(p) = match shift(first) {
                Some(p) if self.ids.get(p) == Some(&id) => Some(p),
                _ => self.ids.iter().position(|&x| x == id),
            }
        {
            self.scroll.offset = (self.heights.offset(p) + delta).max(0.0);
        }
        Ok(())
    }

    fn row_at_pos(&self, pos: usize) -> i64 {
        self.slots.iter().position(|sl| sl.bound && sl.pos == Some(pos)).map_or(-1, |k| k as i64)
    }
}

impl VirtualChildren for VList {
    fn len(&self) -> usize {
        VList::len(self)
    }
    fn key(&self, row: usize) -> u64 {
        self.key_at(row)
    }
    fn row_of(&self, key: u64) -> Option<usize> {
        match &self.rust {
            Some(r) => (0..r.source.len()).find(|&p| r.source.key(p) == key),
            None => self.ids.iter().position(|&x| x == key),
        }
    }
    fn rect(&self, row: usize) -> Option<egui::Rect> {
        (self.view.x > 0.0 && row < self.len()).then(|| self.heights.rect(row, self.view.x, self.scroll.offset))
    }
    fn anchor(&self, key: u64) -> Option<Cow<'_, str>> {
        match &self.rust {
            Some(r) => r.source.anchor(key).map(Cow::Owned),
            None => self.ids.contains(&key).then(|| Cow::Owned(crate::scene::virt::default_anchor(key))),
        }
    }
    fn find_anchor(&self, anchor: &str) -> Option<u64> {
        let k = match &self.rust {
            Some(r) => r.source.find_anchor(anchor).or_else(|| crate::scene::virt::parse_default_anchor(anchor)),
            None => crate::scene::virt::parse_default_anchor(anchor),
        }?;
        self.row_of(k).map(|_| k)
    }
}

// ---- props ----

pub(crate) fn set(v: &mut VList, key: P, val: &Value) -> Result<(), String> {
    if let Some(t) = v.tree.as_deref_mut() {
        match (key, val) {
            (P::ListSelection, Value::Str(s)) => {
                let name = super::treeview::SELECTION.iter().copied().find(|x| x == s).ok_or_else(|| format!("`{s}` is not one of none, single, multi, toggle, range"))?;
                v.selection = name;
                t.sel.mode = Mode::parse(name).unwrap_or_default();
                let keep: Vec<u64> = t.sel.selected.iter().copied().collect();
                t.sel.set(if t.sel.mode == Mode::None { Vec::new() } else { keep });
                return Ok(());
            }
            (P::TreeIndent, Value::I64(x)) => t.indent = (*x).max(0) as f32,
            (P::TreeIndent, Value::F64(x)) => t.indent = x.max(0.0) as f32,
            (P::ExpandOn, Value::Str(x)) => t.expand_on = super::treeview::ExpandOn::parse(x).ok_or_else(|| format!("`{x}` is not one of twistie, click, double-click"))?,
            (P::RootDecorated, Value::Bool(b)) => t.root_decorated = *b,
            (P::TreeIndent | P::ExpandOn | P::RootDecorated, other) => return Err(format!("{key:?}: unexpected value {other:?}")),
            _ => return set_list(v, key, val),
        }
        return Ok(());
    }
    set_list(v, key, val)
}

fn set_list(v: &mut VList, key: P, val: &Value) -> Result<(), String> {
    match (key, val) {
        (P::ListSelected, Value::I64(_) | Value::F64(_)) => {
            let x = match val {
                Value::I64(x) => *x,
                Value::F64(x) => *x as i64,
                _ => unreachable!(),
            };
            v.selected = x.max(-1);
            // the host's position: the item there becomes the selection and the cursor
            if x >= 0 && (x as usize) < v.len() {
                let k = v.key_at(x as usize);
                v.sel.set([k]);
                v.sel.cursor = Some(k);
                v.sel.anchor = Some(k);
            } else if x < 0 {
                v.sel.set([]);
            }
        }
        (P::ListSelection, Value::Str(s)) => {
            v.selection = SELECTION.iter().copied().find(|x| x == s).ok_or_else(|| format!("`{s}` is not one of none, single, multi, toggle, range"))?;
            let keep: Vec<u64> = v.sel.selected.iter().copied().collect();
            v.sel.mode = Mode::parse(v.selection).unwrap_or_default();
            v.sel.set(keep);
        }
        (P::SelectedIds, ids) => {
            v.sel.set(item_keys::ids_of(ids, "selected-ids")?);
        }
        (P::ListLayout, Value::Str(s)) => v.layout = LAYOUT.iter().copied().find(|x| x == s).ok_or_else(|| format!("`{s}` is not one of list, grid"))?,
        (P::RowHeight, Value::I64(x)) => {
            v.row_height = (*x).max(1);
            v.heights.set_estimate(v.row_height as f32);
        }
        (P::RowHeight, Value::F64(x)) => {
            v.row_height = (*x as i64).max(1);
            v.heights.set_estimate(v.row_height as f32);
        }
        (P::RowDragType, Value::Str(x)) => v.dnd.drag_type = x.clone(),
        (P::RowDragType, Value::Null) => v.dnd.drag_type.clear(),
        (P::RowDropTypes, Value::Str(x)) => v.dnd.drop_types = x.clone(),
        (P::RowDropTypes, Value::Null) => v.dnd.drop_types.clear(),
        (P::InternalMove, Value::Bool(b)) => v.dnd.internal_move = *b,
        (P::UniformRows, Value::Bool(b)) => v.uniform = *b,
        (P::CurrentItem, Value::I64(_) | Value::F64(_)) => {
            let x = match val {
                Value::I64(x) => *x,
                Value::F64(x) => *x as i64,
                _ => unreachable!(),
            };
            // the host's cursor: moved, not selected, and not announced back
            let c = (x >= 0).then_some(x as u64);
            match v.tree.as_deref_mut() {
                Some(t) => t.sel.cursor = c,
                None => v.sel.cursor = c,
            }
            v.current_seen = c;
        }
        (k, other) => return Err(format!("{k:?}: unexpected value {other:?}")),
    }
    Ok(())
}

pub(crate) fn get(v: &VList, key: P) -> Option<Value> {
    Some(match key {
        P::ListSelected => Value::I64(v.selected),
        P::ListSelection => Value::Str(v.selection.to_string()),
        P::SelectedIds => item_keys::ids_value(v.sel.selected.iter().copied()),
        P::ListLayout => Value::Str(v.layout.to_string()),
        P::RowHeight => Value::I64(v.row_height),
        P::TreeIndent => Value::I64(v.tree.as_ref()?.indent as i64),
        P::RowDragType => Value::Str(v.dnd.drag_type.clone()),
        P::RowDropTypes => Value::Str(v.dnd.drop_types.clone()),
        P::InternalMove => Value::Bool(v.dnd.internal_move),
        P::UniformRows => Value::Bool(v.uniform),
        P::CurrentItem => Value::I64(v.cursor().map_or(-1, |k| k as i64)),
        P::ExpandOn => Value::Str(v.tree.as_ref()?.expand_on.name().into()),
        P::RootDecorated => Value::Bool(v.tree.as_ref()?.root_decorated),
        _ => return None,
    })
}

fn list_arg<'a>(v: Option<&'a Value>, what: &str) -> Result<&'a [Value], String> {
    match v {
        Some(Value::List(x)) => Ok(x),
        None | Some(Value::Null) => Ok(&[]),
        Some(other) => Err(format!("{what}: expected a list, got {other:?}")),
    }
}

fn int(v: Option<&Value>, what: &str) -> Result<i64, String> {
    match v {
        Some(Value::I64(x)) => Ok(*x),
        Some(Value::U64(x)) => Ok(*x as i64),
        Some(Value::F64(x)) if x.fract() == 0.0 => Ok(*x as i64),
        other => Err(format!("{what}: expected an integer, got {other:?}")),
    }
}

// ---- the host's calls ----

impl EguiBackend {
    /// A `virtual-list` method (`ops.rs` `call`).
    pub(crate) fn vlist_call(&self, h: svelters_abi::svl_handle, name: &str, args: &[Value]) -> Result<Value, String> {
        let mut scene = self.0.scene.borrow_mut();
        let list = scene.live(h)?;
        let i = scene.ix(list);
        let res = match name {
            "splice" => {
                let pos = int(args.first(), "splice pos")?.max(0) as usize;
                let removed = int(args.get(1), "splice removed")?.max(0) as usize;
                let ids: Vec<u64> = match args.get(2) {
                    Some(Value::List(v)) => v.iter().map(|x| int(Some(x), "splice id").map(|x| x as u64)).collect::<Result<_, _>>()?,
                    None | Some(Value::Null) => Vec::new(),
                    Some(other) => return Err(format!("splice ids: expected a list, got {other:?}")),
                };
                let labels = item_keys::labels_of(args.get(3), "splice labels")?;
                let W::VirtualList(v) = scene.w_mut(i) else { unreachable!() };
                if v.rust.is_some() {
                    return Err("splice: this list shows a Rust row source".into());
                }
                v.splice(pos, removed, &ids, labels.as_deref())?;
                scene.invalidate_content(list);
                Value::Null
            }
            "count" => {
                let W::VirtualList(v) = scene.w(i) else { unreachable!() };
                Value::I64(v.len() as i64)
            }
            "scrollTo" => {
                let pos = int(args.first(), "scrollTo pos")?;
                let hint = match args.get(1) {
                    None | Some(Value::Null) => Reveal::Nearest,
                    Some(Value::Str(h)) => Reveal::parse(h).ok_or_else(|| format!("scrollTo: `{h}` is not one of nearest, top, bottom, center"))?,
                    Some(other) => return Err(format!("scrollTo hint: expected a string, got {other:?}")),
                };
                let W::VirtualList(v) = scene.w_mut(i) else { unreachable!() };
                if pos >= 0 {
                    v.reveal = Some((pos as usize, hint));
                }
                Value::Null
            }
            "rowAt" => {
                let pos = int(args.first(), "rowAt pos")?;
                let W::VirtualList(v) = scene.w(i) else { unreachable!() };
                Value::I64(if pos < 0 { -1 } else { v.row_at_pos(pos as usize) })
            }
            "setRow" => {
                let row = int(args.first(), "setRow row")?;
                let row = usize::try_from(row).map_err(|_| format!("setRow: no row slot {row}"))?;
                let outlet = match args.get(1) {
                    Some(Value::Handle(w)) => Some(scene.live(*w)?),
                    Some(Value::I64(w)) if *w > 0 => Some(scene.live(*w as u64)?),
                    None | Some(Value::Null) => None,
                    Some(other) => return Err(format!("setRow: expected a widget, got {other:?}")),
                };
                set_row(&mut scene, list, row, outlet)?;
                Value::Null
            }
            "setNodes" | "setSelection" | "selection" => {
                let W::VirtualList(v) = scene.w_mut(i) else { unreachable!() };
                let Some(t) = v.tree.as_deref_mut() else { return Err(format!("method {name} is not implemented")) };
                match name {
                    "setNodes" => {
                        let ints = |k: usize, what: &str| -> Result<Vec<i64>, String> { list_arg(args.get(k), what)?.iter().map(|x| int(Some(x), what)).collect() };
                        let strs = |k: usize| -> Vec<String> { list_arg(args.get(k), "").unwrap_or_default().iter().map(|x| if let Value::Str(s) = x { s.clone() } else { String::new() }).collect() };
                        let ids: Vec<u64> = ints(0, "setNodes id")?.into_iter().map(|x| x as u64).collect();
                        t.set_nodes(&ids, &ints(1, "setNodes level")?, &ints(2, "setNodes flags")?, &strs(3), &strs(4));
                    }
                    "setSelection" => {
                        let ids: Vec<u64> = list_arg(args.first(), "setSelection ids")?.iter().map(|x| int(Some(x), "setSelection id").map(|x| x as u64)).collect::<Result<_, _>>()?;
                        if t.sel.mode != Mode::None {
                            t.sel.set(ids);
                        }
                    }
                    _ => return Ok(Value::List(t.sel.selected.iter().map(|x| Value::I64(*x as i64)).collect())),
                }
                // the inset or the disclosure changed: lay the rows out again
                scene.invalidate_content(list);
                Value::Null
            }
            other => return Err(format!("method {other} is not implemented")),
        };
        let r = scene.core[i].root;
        drop(scene);
        self.touched(Some(r));
        Ok(res)
    }

    /// Step 5b of `logic` (after the checkpoint and rAF): every shown `virtual-list` with JS rows
    /// decides this frame's offset and binds the rows it will draw (`call_sync`), so the pass
    /// draws no unbound row (R22).
    pub(crate) fn realize_lists(&self, ctx: &egui::Context) {
        let kinds: Vec<usize> = ["virtual-list", "tree-view"].into_iter().filter_map(super::tag_index).collect();
        let lists: Vec<NodeId> = {
            let scene = self.0.scene.borrow();
            kinds
                .iter()
                .flat_map(|&k| scene.index.of_kind(k).iter().copied())
                .filter(|&l| {
                    let i = scene.ix(l);
                    let r = scene.core[i].root;
                    !r.is_detached() && scene.roots.get(r).is_some_and(|e| e.visible) && matches!(scene.w(i), W::VirtualList(v) if v.rust.is_none())
                })
                .collect()
        };
        for l in lists {
            self.realize_list(ctx, l);
        }
    }

    fn realize_list(&self, ctx: &egui::Context, l: NodeId) {
        let Some(host) = self.host() else { return };
        // two rounds: the second binds the slots the first one set up
        for _ in 0..2 {
            let (sig, plan, root) = {
                let mut scene = self.0.scene.borrow_mut();
                let Some(i) = scene.idx(l) else { return };
                let kind = scene.core[i].kind as usize;
                let r = scene.core[i].root;
                let mine = self.viewport_of(r) == Some(ctx.viewport_id());
                let sig = Sigs::of(&scene, i, kind);
                let W::VirtualList(v) = scene.w_mut(i) else { return };
                let moved = mine && v.advance(ctx);
                let plan = plan(v, &sig);
                if moved || !plan.is_empty() {
                    scene.invalidate_content(l);
                }
                (sig, plan, r)
            };
            if plan.is_empty() {
                return;
            }
            self.touched(Some(root));
            let h = l.to_raw();
            for (row, generation) in plan.unbind {
                if sig.unbind != 0 {
                    let _ = host.call_sync(h, sig.unbind, &[Value::I64(row as i64), Value::I64(generation)]);
                }
            }
            for row in plan.setup {
                if sig.setup != 0 {
                    let _ = host.call_sync(h, sig.setup, &[Value::I64(row as i64)]);
                }
            }
            for (row, pos, id, generation) in plan.bind {
                let ok = sig.bind == 0 || host.call_sync(h, sig.bind, &[Value::I64(row as i64), Value::I64(pos as i64), Value::I64(id as i64), Value::I64(generation)]).is_ok();
                let mut scene = self.0.scene.borrow_mut();
                let Some(i) = scene.idx(l) else { return };
                if let W::VirtualList(v) = scene.w_mut(i)
                    && let Some(sl) = v.slots.get_mut(row)
                    && sl.generation == generation
                {
                    sl.deferred = !ok;
                }
            }
        }
    }
}

/// The list's signal ids, 0 when not listened.
struct Sigs {
    setup: u32,
    bind: u32,
    unbind: u32,
}

impl Sigs {
    fn of(s: &Scene, i: usize, kind: usize) -> Sigs {
        let id = |n: &str| {
            let x = super::signal_id(kind, n);
            if s.listens(i, x) { x } else { 0 }
        };
        Sigs { setup: id("setup"), bind: id("bind"), unbind: id("unbind") }
    }
}

#[derive(Default)]
struct Plan {
    unbind: Vec<(usize, i64)>,
    setup: Vec<usize>,
    bind: Vec<(usize, usize, u64, i64)>,
}

impl Plan {
    fn is_empty(&self) -> bool {
        self.unbind.is_empty() && self.setup.is_empty() && self.bind.is_empty()
    }
}

/// What the list must tell JS to show this frame's window: unbinds of slots that left it, setups
/// of slots it lacks, binds of the positions that entered it (into free slots with content). The
/// slots' state is updated as if every call succeeded.
fn plan(v: &mut VList, sig: &Sigs) -> Plan {
    let mut p = Plan::default();
    let want = v.window();
    for (k, sl) in v.slots.iter_mut().enumerate() {
        if sl.bound && sl.pos.is_none_or(|x| !want.contains(&x)) {
            p.unbind.push((k, sl.generation));
            sl.bound = false;
            sl.pos = None;
            sl.deferred = false;
        }
    }
    let mut have = vec![false; want.len()];
    for sl in &mut v.slots {
        if sl.bound
            && let Some(x) = sl.pos
        {
            have[x - want.start] = true;
            // a bind that ran deferred: bind again now
            if sl.deferred {
                sl.generation += 1;
                sl.deferred = false;
                p.bind.push((usize::MAX, x, sl.id, sl.generation));
            }
        }
    }
    // fix up the slot numbers of the rebinds
    for b in &mut p.bind {
        b.0 = v.slots.iter().position(|sl| sl.bound && sl.pos == Some(b.1)).expect("a bound slot");
    }
    let mut free = v.slots.iter().enumerate().filter(|(_, sl)| !sl.bound && sl.outlet.is_some()).map(|(k, _)| k).collect::<Vec<_>>().into_iter();
    let mut pending_setup = v.slots.iter().filter(|sl| sl.setup && sl.outlet.is_none()).count();
    for x in want.clone() {
        if have[x - want.start] {
            continue;
        }
        match free.next() {
            Some(k) => {
                let sl = &mut v.slots[k];
                sl.bound = true;
                sl.pos = Some(x);
                sl.id = v.ids[x];
                sl.generation += 1;
                p.bind.push((k, x, sl.id, sl.generation));
            }
            None if pending_setup > 0 => pending_setup -= 1, // set up earlier, waiting for setRow
            None if sig.setup != 0 => {
                v.slots.push(Slot { setup: true, ..Slot::default() });
                p.setup.push(v.slots.len() - 1);
            }
            None => {}
        }
    }
    v.stats.setups += p.setup.len() as u64;
    v.stats.binds += p.bind.len() as u64;
    v.stats.unbinds += p.unbind.len() as u64;
    p
}

/// `setRow(row, outlet)`: the outlet becomes a child of the list (a row of its own, never in JS's
/// tree); the slot's previous outlet, if another, leaves it.
fn set_row(s: &mut Scene, list: NodeId, row: usize, outlet: Option<NodeId>) -> Result<(), String> {
    let i = s.ix(list);
    let old = {
        let W::VirtualList(v) = s.w_mut(i) else { unreachable!() };
        if row >= v.slots.len() {
            v.slots.resize_with(row + 1, || Slot { setup: true, ..Slot::default() });
        }
        std::mem::replace(&mut v.slots[row].outlet, outlet)
    };
    if let Some(o) = old
        && Some(o) != outlet
        && s.idx(o).is_some_and(|oi| s.core[oi].parent == Some(list))
    {
        s.remove(list, o, Origin::Js).map_err(|e| e.to_string())?;
    }
    if let Some(o) = outlet {
        s.adopt(list, o, Origin::Js).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ---- Rust rows ----

impl SceneMut<'_> {
    /// Makes `list` (a `virtual-list`) show rows from a Rust model (ARCH-SCENE §10.2): rows are
    /// built and bound by the list before layout, recycled through `pool` (pass `RowPool::new()`
    /// with the `clears` the rows need).
    pub fn set_row_source(&mut self, list: NodeId, source: impl RowSource, pool: RowPool) -> Result<(), SceneError> {
        let i = self.s.idx(list).ok_or(SceneError::Stale(list.to_raw()))?;
        let W::VirtualList(v) = self.s.w_mut(i) else { return Err(SceneError::Invalid("not a <virtual-list>".into())) };
        if !v.ids.is_empty() {
            return Err(SceneError::Invalid("the list already shows JS items".into()));
        }
        v.rust = Some(RustRows { source: Box::new(source), pool, version: None, bound: Vec::new(), offset: f64::NAN });
        self.s.invalidate_content(list);
        Ok(())
    }

    /// The row source of `list`, typed (to change its model; the list binds again when the
    /// source's `version` changes).
    pub fn with_row_source<T: RowSource, R>(&mut self, list: NodeId, f: impl FnOnce(&mut T) -> R) -> Option<R> {
        let i = self.s.idx(list)?;
        let W::VirtualList(v) = self.s.w_mut(i) else { return None };
        let src: &mut dyn RowSource = &mut *v.rust.as_mut()?.source;
        let any: &mut dyn std::any::Any = src;
        Some(f(any.downcast_mut::<T>()?))
    }

    /// Scrolls `list` so that position `pos` is in view (applied before the next layout).
    pub fn scroll_list_to(&mut self, list: NodeId, pos: usize) -> Result<(), SceneError> {
        let i = self.s.idx(list).ok_or(SceneError::Stale(list.to_raw()))?;
        let W::VirtualList(v) = self.s.w_mut(i) else { return Err(SceneError::Invalid("not a <virtual-list>".into())) };
        v.reveal = Some((pos, Reveal::Nearest));
        self.s.invalidate_content(list);
        Ok(())
    }
}

/// Step b of a root's frame (ARCH-SCENE §7.2), before its layout: lists of root `r` with a Rust row
/// source decide this frame's offset and bind their window of rows (R22: no frame lag).
pub(crate) fn prepare_root(s: &mut Scene, ctx: &egui::Context, r: RootIx) {
    let Some(k) = super::tag_index("virtual-list") else { return };
    let lists: Vec<NodeId> = s.index.of_kind(k).iter().copied().filter(|&l| s.core[s.ix(l)].root == r).collect();
    for l in lists {
        let i = s.ix(l);
        let W::VirtualList(v) = s.w_mut(i) else { continue };
        let Some(rows) = v.rust.take() else { continue };
        let len = rows.source.len();
        if v.heights.len() != len || rows.source.uniform_height().is_some_and(|h| h != v.heights.estimate()) {
            let est = rows.source.uniform_height().unwrap_or(v.row_height as f32);
            v.heights = RowHeights::uniform(len, est);
        }
        v.rust = Some(rows);
        v.advance(ctx);
        let want = v.window();
        let mut rows = v.rust.take().expect("taken above");
        let version = rows.source.version();
        let same = rows.version == Some(version) && rows.offset == v.scroll.offset && rows.bound.len() == want.len() && rows.bound.first().map(|b| b.1) == Some(want.start);
        if same {
            let W::VirtualList(v) = s.w_mut(i) else { unreachable!() };
            v.rust = Some(rows);
            continue;
        }
        let offset = v.scroll.offset;
        let before = rows.pool.created();
        let mut m = SceneMut { s, origin: Origin::Rust(ProviderId::named("virtual-list")) };
        let mut bound = Vec::with_capacity(want.len());
        let keys: std::collections::HashSet<u64> = want.clone().map(|p| rows.source.key(p)).collect();
        rows.pool.release_except(&mut m, |k| keys.contains(&k));
        for pos in want {
            let key = rows.source.key(pos);
            let t = rows.source.template(pos);
            let src = &mut rows.source;
            let n = rows.pool.row(&mut m, key, t, |m| src.build(m, t));
            if m.s.idx(n).and_then(|ni| m.s.core[ni].parent) != Some(l) {
                let _ = m.s.adopt(l, n, m.origin);
            }
            rows.source.bind(&mut m, n, pos);
            bound.push((n, pos));
        }
        rows.pool.release_unused(&mut m);
        rows.bound = bound;
        rows.version = Some(version);
        rows.offset = offset;
        let built = rows.pool.created() - before;
        s.invalidate_content(l);
        let W::VirtualList(v) = s.w_mut(i) else { unreachable!() };
        v.stats.rows_built += built;
        v.rust = Some(rows);
    }
}

// ---- layout (called by `layout.rs` with the measured rows) ----

/// After layout measured the bound rows at the list's width: their heights into the model,
/// anchored (a height change above the first visible row keeps that row in place). Returns each
/// row's place relative to the list.
pub(crate) fn place_rows(v: &mut VList, size: egui::Vec2, measured: &[(NodeId, usize, f32)]) -> Vec<(NodeId, egui::Pos2, egui::Vec2)> {
    v.view = size;
    let first = v.heights.row_at(v.scroll.offset);
    let mut shift = 0.0;
    for &(_, pos, h) in measured {
        let old = v.heights.height(pos);
        if v.heights.set(pos, h) && pos < first && v.scroll.offset > 0.0 {
            shift += (h - old) as f64;
        }
    }
    if shift != 0.0 {
        v.scroll.offset = (v.scroll.offset + shift).clamp(0.0, v.max_offset());
    }
    measured
        .iter()
        .map(|&(n, pos, h)| {
            let x = v.row_inset(pos).min(size.x);
            (n, egui::pos2(x, (v.heights.offset(pos) - v.scroll.offset) as f32), egui::vec2(size.x - x, h))
        })
        .collect()
}

/// The fixed row height of a list (no measuring), if it has one.
pub(crate) fn uniform_height(v: &VList) -> Option<f32> {
    v.rust.as_ref().and_then(|r| r.source.uniform_height()).or(v.uniform.then_some(v.row_height as f32))
}

// ---- the walk ----

/// Draws list `i` (its rect from layout, offset by `origin`): rows at their places in a clipped
/// child `Ui`, a11y nodes with set size, selection and activation, placeholders for unbound
/// positions, the scroll bar. OW5a: every row is the list's `::part(row)` in its own state (DESIGN
/// R6, [`list_rows`]), the bar is OW2's Scrollable ([`list_rows::ListBar`]); nothing reads egui's
/// `Visuals`.
pub(crate) fn paint(s: &mut Scene, i: usize, origin: egui::Vec2, ui: &mut egui::Ui, out: &mut Vec<Out>) -> Option<egui::Response> {
    let r = s.core[i].rect.translate(origin);
    if !ui.is_rect_visible(r) {
        return None;
    }
    let list = s.slots.id_at(i).expect("live list");
    let h = list.to_raw();
    let kind = s.core[i].kind as usize;
    let list_id = s.core[i].egui_id;
    let ctx = ui.ctx().clone();
    let placed = {
        let W::VirtualList(v) = s.w(i) else { return None };
        v.placed(s)
    };
    // WG5b: a `tree-view` (its tree's selection mode, drag and drop types)
    let tree = match s.w(i) {
        W::VirtualList(v) => v.tree.as_deref().map(|t| (t.sel.mode, t.indent)),
        _ => return None,
    };
    // WG5a: keyboard focus is on the cursor row (EV3: a row is the list's focus stop), or on the
    // list itself while the cursor row is not drawn (scrolling to it); the rows are drawn over it.
    // WG5b: a tree keeps focus on the list node itself (one tab stop, `focus::Role::Tree`)
    let resp = ui.interact(r, list_id, egui::Sense::focusable_noninteractive());
    let placed_keys: Vec<u64> = {
        let W::VirtualList(v) = s.w(i) else { return None };
        placed.iter().filter(|&&(_, p)| p < v.len()).map(|&(_, p)| v.key_at(p)).collect()
    };
    let mut tree_input = None;
    let mut had_keys = false;
    let focused = if tree.is_some() {
        let focused = resp.has_focus();
        // EV3: focused, the tree takes its keys (read here; `record_with` takes them out)
        tree_input = focused.then(|| super::treeview::tree_input(list_id, ui));
        had_keys = tree_input.as_ref().is_some_and(|(k, t)| !k.is_empty() || !t.is_empty());
        crate::focus::record_with(s, i, ui, &resp, crate::focus::Role::Tree, true);
        focused
    } else {
        let now_focused = ctx.memory(|m| m.focused());
        let focused = resp.has_focus() || now_focused.is_some_and(|f| placed_keys.iter().any(|&k| list_id.with(("row", k)) == f));
        if focused {
            if let Some(f) = now_focused {
                item_keys::lock_arrows(ui, f);
            }
            // OW5a1b: Ctrl+C raises `copy(ids)` with the selection
            if item_keys::take_copy(ui) {
                let ids = match s.w(i) {
                    W::VirtualList(v) if !v.sel.selected.is_empty() => Some(item_keys::ids_value(v.sel.selected.iter().copied())),
                    _ => None,
                };
                let sig = super::signal_id(kind, "copy");
                if let Some(ids) = ids
                    && s.listens(i, sig)
                {
                    out.push(Out::Event(h, sig, vec![ids]));
                }
            }
            let keys = item_keys::read_keys(ui, true);
            if !keys.is_empty() {
                had_keys = true;
                keys_input(s, i, &keys, &ctx, out);
                ctx.request_repaint();
            }
        }
        focused
    };
    let (len, selectable, multi, cursor, offset, total, view, taken, decided, kbd) = {
        let W::VirtualList(v) = s.w_mut(i) else { return None };
        v.scroll.last_rect = r;
        v.scroll.layer = Some(ui.layer_id());
        v.stats.frames += 1;
        // R8: the cursor row's ring is for keyboard focus (Tab in, or keys); a click clears it
        if focused && (had_keys || !v.was_focused) {
            v.kbd = true;
        }
        // a tree's selection and cursor are its own (`Tree::sel`, drawn by `tree_row`)
        let (selectable, multi, cursor) = match &tree {
            Some((mode, ..)) => (*mode != Mode::None, mode.is_multi(), None),
            None => (v.sel.mode != Mode::None, v.sel.mode.is_multi(), v.sel.cursor),
        };
        (v.len(), selectable, multi, cursor, v.scroll.offset, v.heights.total(), v.view, v.scroll.taken(), v.scroll.decided_in(&ctx), v.kbd)
    };
    ctx.accesskit_node_builder(list_id, |n| {
        n.set_role(if tree.is_some() {
            egui::accesskit::Role::Tree
        } else if selectable {
            egui::accesskit::Role::ListBox
        } else {
            egui::accesskit::Role::List
        });
        n.set_size_of_set(len);
        n.set_clips_children(); // AX1: rows partly scrolled out are clipped
        if multi {
            n.set_multiselectable();
        }
        // the cursor row is the focused descendant while the list has focus
        if focused && let Some(k) = cursor {
            n.set_active_descendant(list_id.with(("row", k)).accesskit_id());
        }
    });
    // wheel input this frame's offset took (the rest goes to the bar's Scrollable, then chains)
    if decided && taken != 0.0 {
        ui.input_mut(|inp| inp.smooth_scroll_delta.y -= taken);
    }
    s.core[i].drawn = r;
    crate::scene::hit::record(s, i, ui);
    let clip = ui.clip_rect().intersect(r);
    let mut rows_ui = ui.new_child(egui::UiBuilder::new().max_rect(r).id_salt(("vlist", h)).accessibility_parent(list_id));
    rows_ui.set_clip_rect(clip);
    let mut drawn = 0u64;
    let mut covered: Vec<(f32, f32)> = Vec::with_capacity(placed.len());
    let sig_activate = super::signal_id(kind, "activate");
    let emit = |s: &Scene, out: &mut Vec<Out>, name: &str, args: Vec<Value>| {
        let sig = super::signal_id(kind, name);
        if s.listens(i, sig) {
            out.push(Out::Event(h, sig, args));
        }
    };
    // WG5b (OW5a1b: lists too): the row a drag would drop on, decided before the rows so
    // targets inside them win
    let drop_at = row_drop(s, i, &placed, origin, r, clip, ui, out, &emit);
    let (drags, context_selects) = match s.w(i) {
        W::VirtualList(v) => (v.dnd.drags(), crate::native::policy_of(&ctx).context_click_selects),
        _ => return None,
    };
    let mut hovered: Option<(usize, u64)> = None;
    let mut first_row = true; // EV3
    let mut clicked = false;
    let cursor_drawn = cursor.is_some_and(|c| placed_keys.contains(&c));
    for (n, pos) in placed {
        let Some(ni) = s.idx(n) else { continue };
        let rr = s.core[ni].rect.translate(origin);
        covered.push((rr.min.y, rr.max.y));
        // AX1: a `scroll-into-view` on the row or inside it (automation's `scroll_into_view`)
        // scrolls the list to the row when it is not wholly in view
        if s.core[ni].flags.intersects(NodeFlags::WANT_SCROLL | NodeFlags::SCROLL_BELOW) {
            s.core[ni].flags.remove(NodeFlags::SCROLL_BELOW);
            if !clip.contains_rect(rr) {
                if let W::VirtualList(v) = s.w_mut(i) {
                    v.reveal = Some((pos, Reveal::Nearest));
                }
                ctx.request_repaint();
            }
        }
        if !rr.intersects(clip) {
            crate::scene::hit::record_culled(s, ni, &rows_ui); // AX1: realised, not shown
            continue;
        }
        let key = {
            let W::VirtualList(v) = s.w(i) else { return None };
            if pos >= v.len() {
                continue;
            }
            v.key_at(pos)
        };
        let row_id = list_id.with(("row", key));
        let at = (pos, len);
        let full = egui::Rect::from_x_y_ranges(r.x_range(), rr.y_range());
        let drop_into = matches!(drop_at, Some((DropAt::Into, Some(d))) if (d.min.y - rr.min.y).abs() < 0.5);
        let row = if let Some((_, indent)) = &tree {
            let rs = tree_row(s, i, key, at, full, *indent, focused, kbd, drop_into, row_id, list_id, &mut rows_ui, out, &emit);
            clicked |= rs.1;
            rs.0
        } else {
            let rresp = rows_ui.interact(rr, row_id, if drags { egui::Sense::click_and_drag() } else { egui::Sense::click() });
            let is_sel = selectable && matches!(s.w(i), W::VirtualList(v) if v.sel.is_selected(key));
            crate::scene::virt::a11y_row(&ctx, row_id, pos, len, |node| {
                node.set_role(if selectable { egui::accesskit::Role::ListBoxOption } else { egui::accesskit::Role::ListItem });
                if selectable {
                    node.set_selected(is_sel);
                }
            });
            let row = RowState {
                hovered: rresp.hovered(),
                pressed: rresp.is_pointer_button_down_on(),
                selected: is_sel,
                current: cursor == Some(key),
                list_focused: focused,
                keyboard: kbd,
                drop_into,
                disabled: !rows_ui.is_enabled(),
                ..RowState::default()
            };
            let painted = list_rows::paint_row(s, i, &row, at, rr, &rows_ui);
            if row.hovered
                && let Some(c) = painted.cursor
            {
                ctx.set_cursor_icon(c);
            }
            // WG5a: focus follows the cursor row (from the list itself: Tab in, or after a jump);
            // with no cursor yet, the list's tab stop takes it
            let stop = if cursor_drawn { cursor == Some(key) } else { first_row };
            if focused && !rresp.has_focus() && (cursor == Some(key) || (cursor.is_none() && stop)) {
                rresp.request_focus();
            }
            if rresp.clicked() {
                // WG5a: a click focuses the row and goes through the keyed selection (Ctrl/Shift)
                rresp.request_focus();
                clicked = true;
                let m = item_keys::mods(rows_ui.input(|inp| inp.modifiers));
                click_row(s, i, key, m, &ctx, out);
                emit(s, out, "row-clicked", vec![Value::I64(pos as i64), Value::I64(key as i64)]);
                ctx.request_repaint();
            }
            if rresp.secondary_clicked() {
                // OW5a1b (Policy): the selection follows the context click to an unselected row
                if context_selects && selectable && !is_sel {
                    rresp.request_focus();
                    click_row(s, i, key, crate::views::selection::Mods::default(), &ctx, out);
                }
                emit(s, out, "row-context", vec![Value::I64(pos as i64), Value::I64(key as i64)]);
            }
            if rresp.double_clicked() && s.listens(i, sig_activate) && list_rows::allowed(s, i, "activate", &ctx, out) {
                out.push(Out::Event(h, sig_activate, vec![Value::I64(pos as i64)]));
            }
            if drags {
                let label = match s.w(i) {
                    W::VirtualList(v) => v.label_at(pos).unwrap_or_default(),
                    _ => String::new(),
                };
                row_drag(s, i, key, &rresp, &rows_ui, &label);
            }
            {
                // EV3: a row takes focus and the list's keys; WG5a: the cursor row (else the first
                // drawn) is the list's tab stop
                first_row = false;
                crate::focus::record_with(s, i, &rows_ui, &rresp, crate::focus::Role::List, stop);
            }
            row
        };
        if row.hovered {
            hovered = Some((pos, key));
        }
        // the row node mirrors its row's state (`virtual-list:focus-within > :selected`)
        list_rows::mirror(s, ni, &row);
        let mut row_ui = rows_ui.new_child(egui::UiBuilder::new().max_rect(rr).id_salt(("row", key)).accessibility_parent(row_id));
        row_ui.set_clip_rect(clip);
        let mark = crate::scene::virt::widgets_in_layer(&ctx, row_ui.layer_id());
        crate::walk::paint_at(s, ni, origin, &mut row_ui, out);
        // OW5a: a row whose contents are clipped away (partly scrolled out) is still named, by
        // its type-ahead label or its position
        if !crate::scene::virt::label_row_by_contents(&ctx, row_id, row_ui.layer_id(), mark) {
            let name = match s.w(i) {
                W::VirtualList(v) => v.label_at(pos).unwrap_or_else(|| format!("Row {}", pos + 1)),
                _ => format!("Row {}", pos + 1),
            };
            ctx.accesskit_node_builder(row_id, |n| {
                if n.label().is_none_or(|l| l.trim().is_empty()) {
                    n.set_label(name);
                }
            });
        }
        s.core[ni].drawn = rr;
        crate::scene::hit::record(s, ni, &row_ui);
        // R8: the cursor row's focus ring, over its contents
        list_rows::paint_row_outline(s, i, &row, at, if tree.is_some() { full } else { rr }, &rows_ui);
        drawn += 1;
    }
    match drop_at {
        Some((_, None)) => list_rows::paint_drop_line(s, i, r.intersect(clip), None, false, &rows_ui),
        Some((DropAt::Before, Some(rr))) => list_rows::paint_drop_line(s, i, r, Some(rr), false, &rows_ui),
        Some((DropAt::After, Some(rr))) => list_rows::paint_drop_line(s, i, r, Some(rr), true, &rows_ui),
        // `into`: the row's `:drop-target` fill (`list-drop`)
        Some((DropAt::Into, Some(_))) | None => {}
    }
    if let Some((keys, text)) = tree_input {
        tree_keys(s, i, keys, text, &ctx, ui, out, &emit);
    }
    // WG5a: the cursor row is not drawn this frame (a jump scrolls to it next frame): the list
    // itself keeps focus meanwhile (egui drops focus from a widget that is not drawn)
    if focused && cursor.is_some() && !cursor_drawn {
        ctx.memory_mut(|m| m.request_focus(list_id));
    }
    // positions in view that no bound row covers: placeholders (R22's gate counts them)
    let mut unbound = 0u64;
    if view.y > 0.0 {
        let mut gaps = Vec::new();
        {
            let W::VirtualList(v) = s.w(i) else { return None };
            for pos in v.heights.range(offset, offset + view.y as f64) {
                let y = r.min.y + (v.heights.offset(pos) - offset) as f32;
                let yr = y..y + v.heights.height(pos);
                if !covered.iter().any(|c| c.0 <= yr.start + 0.5 && c.1 >= yr.end - 0.5) {
                    gaps.push(egui::Rect::from_x_y_ranges(r.x_range(), yr.start.max(r.min.y)..=yr.end.min(r.max.y)));
                }
            }
        }
        unbound = gaps.len() as u64;
        for pr in gaps {
            list_rows::paint_placeholder(s, i, pr, &rows_ui);
        }
    }
    if unbound > 0 {
        ctx.request_repaint();
    }
    // R15: the edge shadows of a list scrolled away from its top (`:state(sv.overflow-top)`)
    list_rows::paint_shadows(s, i, r, &rows_ui);
    // the scroll bar (OW2's Scrollable): what it moves applies at the next offset decision
    let moved = {
        let W::VirtualList(v) = s.w_mut(i) else { return None };
        let mut bar = std::mem::take(&mut v.bar);
        let d = bar.run(s, i, ui, &resp, r, offset, total);
        if let W::VirtualList(v) = s.w_mut(i) {
            v.bar = bar;
        }
        d
    };
    // OW5a1b: `row-hovered` when the pointer enters a row; `bind:current` when the cursor moved
    let (entered, current) = match s.w_mut(i) {
        W::VirtualList(v) => {
            let entered = hovered.filter(|&(_, k)| v.hovered != Some(k));
            v.hovered = hovered.map(|(_, k)| k);
            let c = v.cursor();
            let current = (c != v.current_seen).then(|| {
                v.current_seen = c;
                c
            });
            (entered, current)
        }
        _ => (None, None),
    };
    if let Some((pos, key)) = entered {
        emit(s, out, "row-hovered", vec![Value::I64(pos as i64), Value::I64(key as i64)]);
    }
    if let Some(c) = current {
        let v = Value::I64(c.map_or(-1, |k| k as i64));
        crate::leaf::widget_changed(s, i, &[P::CurrentItem], vec![v], out);
    }
    if let W::VirtualList(v) = s.w_mut(i) {
        if moved.abs() > 1e-3 {
            v.scroll.add_pending(moved);
            ctx.request_repaint();
        }
        if clicked {
            v.kbd = false;
        }
        v.was_focused = focused || clicked;
        v.stats.rows_drawn += drawn;
        v.stats.unbound_drawn += unbound;
    }
    // WG1: every kind shows its `tooltip` on hover
    let tip = &s.common(i).tooltip;
    let resp = if tip.is_empty() { resp } else { resp.on_hover_text(tip.clone()) };
    Some(resp)
}

// ---- WG5a: keyboard and keyed selection ----

/// After the selection changed by user input: `selected-ids` (bind), `selectionchange(ids)`, and
/// M5's `selected`/`select(pos)` for the cursor row when it is selected.
fn selection_changed(s: &mut Scene, i: usize, out: &mut Vec<Out>) {
    let h = s.slots.id_at(i).expect("live list").to_raw();
    let kind = s.core[i].kind as usize;
    let W::VirtualList(v) = s.w_mut(i) else { return };
    let ids = item_keys::ids_value(v.sel.selected.iter().copied());
    let cur = v.sel.cursor.filter(|&k| v.sel.is_selected(k)).and_then(|k| v.row_of(k));
    let sel_pos = cur.map(|p| p as i64);
    let pos_changed = sel_pos.is_some_and(|p| p != v.selected);
    v.selected = sel_pos.unwrap_or(if v.sel.selected.is_empty() { -1 } else { v.selected });
    let sig = super::signal_id(kind, "selectionchange");
    if s.listens(i, sig) {
        out.push(Out::Event(h, sig, vec![ids.clone()]));
    }
    let sig = super::signal_id(kind, "select");
    if let Some(p) = sel_pos.filter(|_| pos_changed)
        && s.listens(i, sig)
    {
        out.push(Out::Event(h, sig, vec![Value::I64(p)]));
    }
    crate::leaf::widget_changed(s, i, &[P::SelectedIds], vec![ids], out);
}

fn click_row(s: &mut Scene, i: usize, key: u64, m: crate::views::selection::Mods, ctx: &egui::Context, out: &mut Vec<Out>) {
    let W::VirtualList(v) = s.w_mut(i) else { return };
    let ids = v.all_ids().into_owned();
    // EV1: a filter that prevents `selectionchange` keeps the selection (the cursor still moves)
    let mut sel = v.sel.clone();
    let c = sel.click(&ids, key, m);
    if c.selection && !list_rows::allowed(s, i, "selectionchange", ctx, out) {
        if let W::VirtualList(v) = s.w_mut(i) {
            v.sel.cursor = sel.cursor;
        }
        return;
    }
    if let W::VirtualList(v) = s.w_mut(i) {
        v.sel = sel;
    }
    if c.selection {
        selection_changed(s, i, out);
    }
}

/// The focused list's keys: cursor moves (scrolled into view), selection, Enter (`activate`).
fn keys_input(s: &mut Scene, i: usize, keys: &[item_keys::Input], ctx: &egui::Context, out: &mut Vec<Out>) {
    let h = s.slots.id_at(i).expect("live list").to_raw();
    let kind = s.core[i].kind as usize;
    let W::VirtualList(v) = s.w_mut(i) else { return };
    let ids = v.all_ids().into_owned();
    let page = v.page();
    let before = v.sel.clone();
    let mut sel = std::mem::take(&mut v.sel);
    let o = item_keys::apply(&mut sel, &ids, keys, page, |p| v.label_at(p));
    v.sel = sel;
    // EV1: a filter that prevents `selectionchange` keeps the selection (the cursor moves)
    if o.selection && !list_rows::allowed(s, i, "selectionchange", ctx, out) {
        if let W::VirtualList(v) = s.w_mut(i) {
            let cursor = v.sel.cursor;
            v.sel.set(before.selected.iter().copied());
            v.sel.cursor = cursor;
            v.sel.anchor = before.anchor;
        }
        return keys_rest(s, i, h, kind, &ids, o.cursor, None, ctx, out);
    }
    keys_rest(s, i, h, kind, &ids, o.cursor, Some(o), ctx, out)
}

#[allow(clippy::too_many_arguments)]
fn keys_rest(s: &mut Scene, i: usize, h: u64, kind: usize, ids: &[u64], cursor: bool, o: Option<item_keys::Outcome>, ctx: &egui::Context, out: &mut Vec<Out>) {
    let W::VirtualList(v) = s.w_mut(i) else { return };
    if cursor
        && let Some(p) = v.sel.cursor.and_then(|k| ids.iter().position(|&x| x == k))
    {
        v.reveal = Some((p, Reveal::Nearest));
        s.invalidate_content(s.slots.id_at(i).expect("live list"));
    }
    let Some(o) = o else { return };
    if o.selection {
        selection_changed(s, i, out);
    }
    let sig = super::signal_id(kind, "activate");
    if let Some(p) = o.activate
        && s.listens(i, sig)
        && list_rows::allowed(s, i, "activate", ctx, out)
    {
        out.push(Out::Event(h, sig, vec![Value::I64(p as i64)]));
    }
}

// ---- reads ----

impl<'a> crate::scene::SceneRef<'a> {
    /// The state of a `virtual-list` node.
    pub fn virtual_list(self, id: NodeId) -> Option<&'a VList> {
        match self.get(id)?.builtin() {
            W::VirtualList(v) => Some(v),
            _ => None,
        }
    }

    /// The rows a `virtual-list` shows now, by position: (row node, position).
    pub fn list_rows(self, id: NodeId) -> Vec<(NodeId, usize)> {
        let s = self.s;
        self.virtual_list(id).map(|v| v.placed(s)).unwrap_or_default()
    }
}

// ---- OW5a1b: row drag and drop (lists and trees) ----

fn mac(ctx: &egui::Context) -> bool {
    ctx.os() == egui::os::OperatingSystem::Mac
}

/// The payload of a row drag starting on item `key` of list `v`: a tree's rows (`Tree::mime`),
/// or a list's: `row-drag-type` with the dragged ids (a line each) and `text/plain` with their
/// labels; with `internal-move`, also [`ROW_MOVE`] with `key` (the list takes only its own).
fn row_payload(v: &VList, key: u64) -> svelters_host::platform::MimeData {
    if let Some(t) = v.tree.as_deref() {
        return t.mime(&v.dnd.drag_type, &t.drag_rows(&v.ids, key));
    }
    let ids = v.all_ids();
    let rows: Vec<u64> = if !v.dnd.internal_move && v.sel.is_selected(key) { ids.iter().copied().filter(|x| v.sel.is_selected(*x)).collect() } else { vec![key] };
    let label = |k: u64| v.row_of(k).and_then(|p| v.label_at(p)).unwrap_or_default();
    let mut m = svelters_host::platform::MimeData::new();
    if v.dnd.internal_move {
        m = m.with(ROW_MOVE, key.to_string());
    }
    if !v.dnd.drag_type.is_empty() {
        m = m.with(v.dnd.drag_type.clone(), rows.iter().map(|k| k.to_string()).collect::<Vec<_>>().join("\n"));
    }
    m.with("text/plain", rows.iter().map(|&k| label(k)).collect::<Vec<_>>().join("\n"))
}

/// A row's drag: the rows' [`DragSource`](crate::interact::DragSource) on the row's response
/// `rresp` (item `key`, ghost text `label`). Sets the payload on `Begin`, the cursor, and paints
/// the list's `::part(ghost)`.
pub(crate) fn row_drag(s: &mut Scene, i: usize, key: u64, rresp: &egui::Response, ui: &egui::Ui, label: &str) {
    use crate::interact::{DragIntent, DragSourceParams, Intents, PartResponses, Unit};
    let ctx = ui.ctx().clone();
    let list_id = s.core[i].egui_id;
    let W::VirtualList(v) = s.w_mut(i) else { return };
    if !v.dnd.drags() {
        return;
    }
    let mut src = std::mem::take(&mut v.dnd.source);
    let params = DragSourceParams { on: true, mac: mac(&ctx), over_target: super::dnd::over_target(&ctx) };
    let mut state = crate::provider::StateBits::NONE;
    let mut fired = Intents::default();
    crate::interact::run(ui, rresp, &mut state, &PartResponses::default(), |cx| src.input(&params, cx, &mut fired));
    if fired.iter().any(|f| *f == DragIntent::Begin) {
        let p = src.payload(row_payload(v, key), Some(list_id));
        egui::DragAndDrop::set_payload(&ctx, p);
    }
    if let Some(c) = src.cursor() {
        ctx.set_cursor_icon(c);
    }
    let dragging = src.dragging();
    if src.next_frame().is_some() {
        ctx.request_repaint();
    }
    v.dnd.source = src;
    if dragging {
        let fallback = if v.dnd.drag_type.is_empty() { "row" } else { v.dnd.drag_type.as_str() };
        let text = if label.is_empty() { fallback.to_string() } else { label.to_string() };
        super::dnd::ghost(s, i, ui, list_id.with(("row", key)), &text);
    }
}

/// Where a row drop lands: the row (`id`, position, `where`, its rect from the inset); `None`
/// below the last row.
type RowHit = Option<(u64, usize, DropAt, egui::Rect)>;

/// The rows' drop zone (`row-drop-types`, `internal-move`): while an accepted drag is over the
/// rows, where it would land; on release `drop-row(id, where, type, data)` (or, for the list's
/// own `internal-move` drag, `move(from, to)`). Runs the list's
/// [`DropTarget`](crate::interact::DropTarget) (`:drop-target` on the list) and, on trees, DnD
/// auto-expand: a drag held `into` a collapsed row for `Policy::drag_expand_ms` raises `expand`.
/// Runs before the rows are painted, so drop targets inside rows (painted later) win the pointer.
#[allow(clippy::too_many_arguments)]
pub(crate) fn row_drop(s: &mut Scene, i: usize, placed: &[(NodeId, usize)], origin: egui::Vec2, r: egui::Rect, clip: egui::Rect, ui: &egui::Ui, out: &mut Vec<Out>, emit: &super::treeview::Emit<'_>) -> Option<(DropAt, Option<egui::Rect>)> {
    use crate::interact::{DropIntent, DropParams, Offer};
    use egui::DragAndDrop;
    let ctx = ui.ctx().clone();
    let list_id = s.core[i].egui_id;
    let zone = list_id.with("row-drop");
    let (drop_types, internal) = {
        let W::VirtualList(v) = s.w(i) else { return None };
        if !v.dnd.drops() {
            return None;
        }
        (v.dnd.drop_types.clone(), v.dnd.internal_move)
    };
    let payload = DragAndDrop::payload::<super::dnd::DragPayload>(&ctx);
    // the list's own reorder drag, else the entry `row-drop-types` takes
    let own = internal && payload.as_ref().is_some_and(|p| p.source == Some(list_id) && p.best(ROW_MOVE).is_some());
    let offer: Option<(String, String)> = payload.as_ref().and_then(|p| if own { p.best(ROW_MOVE) } else if drop_types.is_empty() { None } else { p.best(&drop_types) }).map(|(t, d)| (t.to_string(), d));
    let inside = offer.is_some() && ctx.rect_contains_pointer(ui.layer_id(), clip);
    if inside {
        super::dnd::claim(&ctx, zone);
        ctx.request_repaint(); // the claim decides next pass
    }
    let claimed = inside && super::dnd::claimed(&ctx, zone);
    let ptr = ctx.pointer_latest_pos();
    let hit: RowHit = {
        let W::VirtualList(v) = s.w(i) else { return None };
        let mut hit = None;
        if claimed && let Some(ptr) = ptr {
            for &(n, pos) in placed {
                let Some(ni) = s.idx(n) else { continue };
                let rr = s.core[ni].rect.translate(origin);
                if pos < v.len() && rr.y_range().contains(ptr.y) {
                    let t = (ptr.y - rr.min.y) / rr.height().max(1.0);
                    // a reorder lands between rows
                    let at = if own { if t < 0.5 { DropAt::Before } else { DropAt::After } } else { DropAt::of(t) };
                    let indent = v.tree.as_ref().map_or(0.0, |t| t.indent);
                    let x = (r.min.x + v.row_inset(pos) - indent).max(r.min.x);
                    hit = Some((v.key_at(pos), pos, at, egui::Rect::from_x_y_ranges(x..=r.max.x, rr.y_range())));
                }
            }
        }
        hit
    };
    let released = ctx.input(|inp| inp.pointer.any_released());
    let action = super::dnd::drop_action(ctx.input(|inp| inp.modifiers), mac(&ctx));
    let params = DropParams {
        offer: offer.map(|(ty, d)| Offer { ty, data: released.then_some(d) }),
        inside: claimed,
        released,
        at: ptr.map_or(egui::Vec2::ZERO, |p| p - r.min),
        action,
    };
    let mut tgt = match s.w(i) {
        W::VirtualList(v) => v.dnd.target,
        _ => return None,
    };
    let mut state = s.core[i].state;
    let fired = crate::interact::dnd::run_bare(&ctx, zone, clip, ui.is_enabled(), &mut state, &mut tgt, &params);
    if let W::VirtualList(v) = s.w_mut(i) {
        v.dnd.target = tgt;
    }
    crate::style::set_own_state(s, i, state);
    for f in fired {
        let DropIntent::Drop { ty, data, action, .. } = f else { continue };
        if let Some(p) = DragAndDrop::take_payload::<super::dnd::DragPayload>(&ctx) {
            p.accept_as(action);
        }
        if own {
            let W::VirtualList(v) = s.w(i) else { return None };
            let from = data.trim().parse::<u64>().ok().and_then(|k| v.row_of(k));
            let to = match hit {
                Some((_, pos, DropAt::Before, _)) => pos,
                Some((_, pos, _, _)) => pos + 1,
                None => v.len(),
            };
            // dropping a row into its own gap moves nothing
            if let Some(from) = from.filter(|&f| to != f && to != f + 1) {
                emit(s, out, "move", vec![Value::I64(from as i64), Value::I64(to as i64)]);
            }
        } else {
            let (id, at) = hit.map_or((-1, DropAt::Into), |h| (h.0 as i64, h.2));
            emit(s, out, "drop-row", vec![Value::I64(id), Value::Str(at.name().into()), Value::Str(ty), Value::Str(data)]);
        }
    }
    auto_expand(s, i, if claimed && !released { hit } else { None }, &ctx, out, emit);
    claimed.then(|| hit.map_or((DropAt::Into, None), |h| (h.2, Some(h.3))))
}

/// DnD auto-expand (Qt `autoExpandDelay`): a drag held `into` a collapsed tree row for
/// `Policy::drag_expand_ms` raises `expand` for it, once (EV1-gated).
fn auto_expand(s: &mut Scene, i: usize, hit: RowHit, ctx: &egui::Context, out: &mut Vec<Out>, emit: &super::treeview::Emit<'_>) {
    let delay = crate::native::policy_of(ctx).drag_expand_ms;
    let now = ctx.input(|inp| inp.time);
    let W::VirtualList(v) = s.w_mut(i) else { return };
    let Some(t) = v.tree.as_deref() else { return };
    let target = match hit {
        Some((key, _, DropAt::Into, _)) if delay > 0 && t.nodes.get(&key).is_some_and(|n| n.expandable && !n.expanded) => Some(key),
        _ => None,
    };
    let Some(key) = target else {
        v.dnd.expand_hover = None;
        return;
    };
    let since = match v.dnd.expand_hover {
        Some((k, at)) if k == key && at.is_infinite() => return,
        Some((k, at)) if k == key => at,
        _ => {
            v.dnd.expand_hover = Some((key, now));
            now
        }
    };
    let wait = delay as f64 / 1000.0 - (now - since);
    if wait > 0.0 {
        ctx.request_repaint_after(std::time::Duration::from_secs_f64(wait));
        return;
    }
    // once: until the drag leaves the row (JS splices the children in meanwhile)
    v.dnd.expand_hover = Some((key, f64::INFINITY));
    if list_rows::allowed(s, i, "expand", ctx, out) {
        emit(s, out, "expand", vec![Value::I64(key as i64)]);
    }
}
