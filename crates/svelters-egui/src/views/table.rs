//! `table` and `table-column`, svelters' own (OW5b; ARCH-WIDGETS §6.1, WIDGET-PARITY §2.18.4-6,
//! DESIGN R6, R8, R10, R11, R15). No `egui_table`, no egui widgets: rows, cells, the header and
//! its parts are painted from CSS and UA tables; scrolling is OW2's [`Scrollable`] on both axes,
//! column widths OW2's [`Resizable`] per header edge.
//!
//! Columns are the table's `table-column` children (data only: hidden by the table's layout).
//! Rows are a JS data window, the data half of the virtual-list protocol (M5 / N3): each frame the
//! table works out the rows it shows plus `overscan` on each side and, when that range changes,
//! emits `range(first, last)`; JS answers with `set-rows(first, rows)`, which replaces the window.
//! Rows outside the window draw empty until their data arrives. **Or a Rust model** (LS2, SV41):
//! [`SceneMut::set_table_model`] gives the table a [`RowModel`] (`svelters_host::rows`); the table
//! reads its cells directly (no `range`, no JS call per page), sorts it on `sort-column`/`sort-order`
//! and filters it and expands its tree rows through the model's [`RowView`]. Only the visible rows **and**
//! columns are drawn (virtual on both axes); the header row is frozen at the top, the first
//! `sticky-columns` columns at the left.
//!
//! **Parts** (`table::part(name)`), styled per item through [`Scene::part_at`] exactly as OW5a's
//! list rows (the host's state replaced by the item's, `:nth-child` by logical position, P1):
//!
//! | Part | Kind | States |
//! |---|---|---|
//! | `row` | P, per row (position = row) | R6: `:hover`, `:active`, `:selected` (`table:focus-within::part(row):selected` active, `:not(:focus-within)` inactive), `:current` (the cursor row) |
//! | `cell` | P, per cell (position = shown column) | the row's, `:hover` (the cell), `:current` + `:focus-visible` (the cursor cell, R8 ring, keyboard only) |
//! | `error` | P, the cell of a rejected commit | outline in `invalid-border`; its reason is an OW3 tooltip |
//! | `header` | P, the frozen header row | |
//! | `header-cell` | P, per shown column | `:hover`, `:active` |
//! | `sort` | P, the sort arrow of a header cell (`content: icon(…)` replaces the codicon) | `:checked` when descending |
//! | `resize` | N, a header cell's right edge (R11: 1 px line, `focus` hover bar after the delay) | `:hover`, `:dragging` |
//! | `scrollbar-x`/`-y`, `thumb-x`/`-y` | P, OW2's overlay bars | `:hover`, `:active`, `:dragging` (thumb) |
//! | `menu`, `menu-item` | P, the column menu (`column-menu`) | `:current`, `:checked`, `:disabled` |
//! | `expander` | P, a Rust model's tree row chevron (first shown column; `content: icon(…)` replaces it) | the row's, `:hover`, `:checked` when expanded |
//!
//! **Rows have ids (WG5a).** `set-ids(ids, labels?)` gives every row (by position) an id; selection
//! (`selected`), the cursor (`current`) and edits are keyed by id ([`KeySelect`]), so a re-sort or a
//! filter keeps them on the same rows.
//!
//! **Keyboard.** A click focuses the table. Focused: Up/Down/PageUp/PageDown/Home/End move the
//! cursor row (Shift extends, Ctrl moves alone), Left/Right the cursor column ([`RovingList`]),
//! Space and Ctrl+A select, Enter activates, Ctrl+C raises `copy(ids)`, F2 or typing edits the
//! cursor cell when its column is `editable` (typing elsewhere is type-ahead).
//!
//! **The header (FM1's QW4, Qt's QHeaderView).** `sortable` sorts on a header click
//! (`sort-column`/`sort-order`, `sort-changed`); a right click raises `header-context` and opens the
//! column menu (`hideable` columns, `hidden-columns`); `header-state` (JSON) saves and restores
//! widths, hidden columns and the sort. The table never writes files (ZU3): the app persists
//! `header-state` with DX1's `uiState()` (UiStateBackend). Hidden columns (and `visible={false}`
//! ones) are left out of the shown columns; WG5a's `current-column` and edits count shown
//! columns, and [`TableState::data_col`] maps them to data columns.
//!
//! **Editing** is WG5a's: the editor is the column's `editor` snippet or a built-in text field.
//! **Seam (until OW4's own text field lands on master):** the built-in field is still egui's
//! `TextEdit`, in [`paint_editor`] only; everything else is own.
//!
//! **OW5a2** (multi-column tree) reuses [`ColumnState`], [`columns`], [`header`] and the header
//! parts: a tree with `table-column` children lays its rows on the same widths.

use super::item_keys::{self, Input, RowKeys};
use super::selection::{KeySelect, Mode, Mods};
use super::{arg, arity};
use crate::backend::EguiBackend;
use crate::interact::{DismissParams, Intents, PartResponses, Resizable, Resize, ResizeParams, RovingList, ScrollParams, Scrollable, Unit};
use crate::kinds::{self, P};
use crate::layout::Metrics;
use crate::paint::{self, IconKey, PaintCx, StyleView, TraceKind, Ua, UaRule, UaValue::*};
use crate::provider::StateBits;
use crate::scene::props::{as_bool, as_enum, as_f64, as_i64, as_str, as_str_list};
use crate::scene::{Arena, Dirty, NodeId, Origin, Scene, SceneError, SceneMut, SceneRef, W};
use crate::walk::Out;
use egui::{Align, Color32, Id, Layout, Pos2, Rect, Role, Sense, Stroke, Ui, UiBuilder, Vec2, pos2, vec2};
use std::borrow::Cow;
use svelters_abi::{StrResult, Value};
use svelters_host::rows::{CellKind, RowModel, RowView, SortOrder, TreeRow};

/// How long a rejected commit's reason stays on its cell, in seconds.
const ERROR_SECONDS: f64 = 5.0;
/// The narrowest a column gets without a `min-width` (Qt's `minimumSectionSize` is 16-ish).
const MIN_COL: f32 = 16.0;

const SEL_ACTIVE: StateBits = StateBits(StateBits::SELECTED.0 | StateBits::FOCUS_WITHIN.0);

/// `::part(row)`: DESIGN R6's four states (the R8 ring is on the cursor cell), R10's pointer.
pub const ROW_UA: Ua = Ua(&[
    UaRule::base(&[("cursor", Keyword("pointer"))]),
    UaRule::when(StateBits::HOVERED, &[("background-color", Token("list-hover"))]),
    UaRule::when(StateBits::SELECTED, &[("background-color", Token("list-sel-inactive"))]),
    UaRule::when(SEL_ACTIVE, &[("background-color", Token("list-sel")), ("color", Token("list-sel-fg"))]),
    UaRule::when(StateBits::DISABLED, &[("cursor", Keyword("default"))]),
]);

/// `striped`: the fill of odd rows at rest (CSS: `table::part(row):nth-child(even)`).
pub const STRIPE_UA: Ua = Ua(&[UaRule::base(&[("background-color", Token("list-hover"))])]);

/// `::part(cell)`: 4 px sides; R8's 1 px ring inside the cursor cell, keyboard focus only.
pub const CELL_UA: Ua = Ua(&[
    UaRule::base(&[
        ("padding-left", Px(4.0)),
        ("padding-right", Px(4.0)),
        ("outline-style", Keyword("none")),
        ("outline-color", Token("focus")),
        ("outline-width", Px(1.0)),
        ("outline-offset", Px(-1.0)),
    ]),
    UaRule::when(StateBits::FOCUS_VISIBLE, &[("outline-style", Keyword("solid"))]),
]);

/// `::part(error)`: a cell whose commit was rejected (DESIGN §2.4: `invalid-border`).
pub const ERROR_UA: Ua = Ua(&[UaRule::base(&[
    ("outline-style", Keyword("solid")),
    ("outline-color", Token("invalid-border")),
    ("outline-width", Px(1.0)),
    ("outline-offset", Px(-1.0)),
])]);

/// `::part(header)`: the frozen header row.
pub const HEADER_UA: Ua = Ua(&[UaRule::base(&[("background-color", Token("bg"))])]);

/// `::part(header-cell)`: a column's header (QSS `QHeaderView::section`).
pub const HEADER_CELL_UA: Ua = Ua(&[
    UaRule::base(&[("padding-left", Px(4.0)), ("padding-right", Px(4.0)), ("font-weight", Number(600.0)), ("color", Token("fg"))]),
    UaRule::when(StateBits::HOVERED, &[("background-color", Token("list-hover"))]),
    UaRule::when(StateBits::DISABLED, &[("color", Token("disabled-fg"))]),
]);

/// `::part(sort)`: the sort arrow (QSS `::up-arrow`/`::down-arrow`); a codicon unless `content:`.
pub const SORT_UA: Ua = Ua(&[UaRule::base(&[("width", Px(16.0)), ("font-size", Px(14.0)), ("color", Token("icon"))])]);

/// `::part(expander)`: a tree row's disclosure chevron (codicon `chevron-right`/`chevron-down`;
/// `:checked` while expanded), in the first shown column.
pub const EXPANDER_UA: Ua = Ua(&[UaRule::base(&[("width", Px(16.0)), ("font-size", Px(14.0)), ("color", Token("icon"))])]);

/// LS2: how far each tree level indents the first shown column (VS Code's tree: 8 + 16 chevron).
pub const INDENT: f32 = 16.0;

/// `::part(resize)`: R11, the split view's sash (1 px `border` line, `sash-w` hit area and hover bar).
pub const RESIZE_UA: Ua = crate::widgets::split_view::SASH_UA;

/// An edit in progress.
#[derive(Clone, Debug, PartialEq)]
pub struct Edit {
    /// The row's id and the column's index and node.
    pub row: u64,
    pub col: usize,
    pub column: NodeId,
    /// The cell's text when the edit started; the built-in editor's text.
    pub value: String,
    pub draft: String,
    /// What was typed to start it ("" for F2, a double click or `edit`).
    pub typed: String,
    /// The column's `editor` snippet edits (else the built-in text field).
    pub snippet: bool,
    /// `editor-bind` ran.
    pub bound: bool,
    /// Focus goes into the editor at its next paint.
    pub focus: bool,
    pub had_focus: bool,
    /// The cell's size last drawn (the snippet editor's layout size).
    pub size: Vec2,
    /// The editor's focusable widgets last pass.
    pub widgets: Vec<Id>,
    /// Asked to end: commit (else revert) and then move by `step` editable cells (Tab).
    pub end: Option<(bool, i32)>,
}

/// A rejected commit's reason, shown under its cell.
#[derive(Clone, Debug, PartialEq)]
pub struct EditError {
    pub row: u64,
    pub col: usize,
    pub reason: String,
    /// egui time it goes away (set when first shown).
    pub until: Option<f64>,
    /// OW5b: where its tooltip shows (OW3's `tooltip::show`), once per change.
    pub shown_at: Option<Pos2>,
}

/// A `table-column`'s own props (`width`, `min-width`, `max-width`, `visible` and `tooltip` are
/// Widget's, in `Common`). OW5a2's multi-column tree reads the same.
#[derive(Clone, Debug, PartialEq)]
pub struct ColumnState {
    pub title: String,
    pub resizable: bool,
    pub align: &'static str,
    pub sort: &'static str,
    pub editable: bool,
    pub editor: bool,
    /// FM1: its name in `hidden-columns` and `header-state` (empty: its child index).
    pub name: String,
    pub hideable: bool,
    /// `auto`: `align`.
    pub header_align: &'static str,
    pub elide: &'static str,
}

impl Default for ColumnState {
    fn default() -> Self {
        ColumnState {
            title: String::new(),
            resizable: true,
            align: "start",
            sort: "none",
            editable: false,
            editor: false,
            name: String::new(),
            hideable: true,
            header_align: "auto",
            elide: "end",
        }
    }
}

impl ColumnState {
    pub(crate) fn set(&mut self, key: P, v: &Value) -> Result<(), String> {
        match key {
            P::Title => self.title = as_str(v)?,
            P::Resizable => self.resizable = as_bool(v)?,
            P::CellAlign => self.align = as_enum(v, kinds::CELL_ALIGN)?,
            P::Sort => self.sort = as_enum(v, kinds::SORT)?,
            P::Editable => self.editable = as_bool(v)?,
            // WG5a: the runtime writes `true` while a snippet is set (a function never reaches the backend)
            P::Editor => self.editor = !matches!(v, Value::Null | Value::Bool(false)),
            P::Name => self.name = as_str(v)?,
            P::Hideable => self.hideable = as_bool(v)?,
            P::HeaderAlign => self.header_align = as_enum(v, kinds::HEADER_ALIGN)?,
            P::Elide => self.elide = as_enum(v, kinds::ELIDE)?,
            _ => return Err(format!("{key:?} is not a prop of <table-column>")),
        }
        Ok(())
    }

    pub(crate) fn get(&self, key: P) -> Option<Value> {
        let s = |x: &str| Value::Str(x.to_string());
        Some(match key {
            P::Title => s(&self.title),
            P::Resizable => Value::Bool(self.resizable),
            P::CellAlign => s(self.align),
            P::Sort => s(self.sort),
            P::Editable => Value::Bool(self.editable),
            P::Editor => Value::Bool(self.editor),
            P::Name => s(&self.name),
            P::Hideable => Value::Bool(self.hideable),
            P::HeaderAlign => s(self.header_align),
            P::Elide => s(self.elide),
            _ => return None,
        })
    }
}

/// The column menu, open at a screen point.
#[derive(Clone, Copy, Debug)]
struct ColumnMenu {
    at: Pos2,
    /// The pass it opened in (the opening click is not an outside press).
    pass: u64,
}

/// A table's interaction state that is not a prop (a clone starts fresh).
#[derive(Default)]
pub struct TableRt {
    scroll: Scrollable,
    /// Per column node: its header edge's unit and the edge's state bits.
    edges: Vec<(NodeId, Resizable, StateBits)>,
    thumbs: [StateBits; 2],
    menu: Option<ColumnMenu>,
    /// A live resize: (column node, width) until the drag commits.
    live: Option<(NodeId, f32)>,
}

impl Clone for TableRt {
    fn clone(&self) -> TableRt {
        TableRt::default()
    }
}

impl PartialEq for TableRt {
    fn eq(&self, _: &TableRt) -> bool {
        true
    }
}

impl std::fmt::Debug for TableRt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TableRt").field("offset", &self.scroll.offset()).field("menu", &self.menu.is_some()).finish()
    }
}

/// LS2 (SV41): a table's Rust model and what it shows of it (sort, filter, expanded tree rows).
pub(crate) struct RustTable {
    pub model: Box<dyn RowModel>,
    pub view: RowView,
    /// The model column of each data column (by `name` = [`Column::key`], else by index).
    pub cols: Vec<usize>,
}

impl RustTable {
    fn model_col(&self, data_col: usize) -> usize {
        self.cols.get(data_col).copied().unwrap_or(data_col)
    }

    fn cell(&self, pos: u64, data_col: usize) -> Option<Cow<'_, str>> {
        let row = self.view.row(pos as usize)?;
        Some(self.model.cell(row, self.model_col(data_col)).into_text())
    }

    fn tree_row(&self, pos: u64) -> Option<TreeRow> {
        self.view.is_tree().then(|| self.view.tree_row(&*self.model, pos as usize))
    }
}

/// The Rust model slot of a table (`None`: rows come from the JS window). Like [`TableRt`], a
/// clone starts empty.
#[derive(Default)]
pub struct ModelSlot(pub(crate) Option<Box<RustTable>>);

impl ModelSlot {
    pub fn is_some(&self) -> bool {
        self.0.is_some()
    }
}

impl Clone for ModelSlot {
    fn clone(&self) -> ModelSlot {
        ModelSlot(None)
    }
}

impl PartialEq for ModelSlot {
    fn eq(&self, o: &ModelSlot) -> bool {
        self.0.is_none() && o.0.is_none()
    }
}

impl std::fmt::Debug for ModelSlot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            Some(m) => f.debug_struct("RustTable").field("rows", &m.model.len()).field("shown", &m.view.len()).finish(),
            None => f.write_str("None"),
        }
    }
}

/// A table node's props and data.
#[derive(Clone, Debug, PartialEq)]
pub struct TableState {
    pub row_count: i64,
    pub row_height: f64,
    pub sticky: i64,
    pub striped: bool,
    pub overscan: i64,
    /// The data window: rows `first..first + rows.len()`.
    pub first: u64,
    pub rows: Vec<Vec<String>>,
    /// The last `range` asked for (inclusive), so it is emitted once per change.
    pub asked: Option<(u64, u64)>,
    /// `scroll-to-row`, applied next frame.
    pub scroll_to: Option<u64>,
    pub scroll_to_col: Option<usize>,
    // ---- WG5a ----
    /// `set-ids`: row ids (and labels) by position; `None`: a row's id is its position.
    pub keys: Option<RowKeys>,
    pub selection: &'static str,
    pub sel: KeySelect,
    pub cur_col: usize,
    /// Rows the last pass showed (a page move).
    pub page: usize,
    pub edit: Option<Edit>,
    /// `edit(row-id, column)` or the next cell after a Tab, started at the next paint.
    pub pending_edit: Option<(u64, usize)>,
    /// (column node, editor outlet): each column's realised `editor` snippet.
    pub editors: Vec<(NodeId, NodeId)>,
    pub error: Option<EditError>,
    /// Focus back on the table after an edit ended.
    pub refocus: bool,
    // ---- OW5b: the header (FM1's QW4) ----
    pub sortable: bool,
    pub sort_column: i64,
    pub sort_order: &'static str,
    pub column_menu: bool,
    /// `hidden-columns`: column names (or child indices).
    pub hidden: Vec<String>,
    pub header_state: String,
    /// A `header-state` the host set, applied at the next paint (it needs the columns).
    pub restore: Option<serde_json::Value>,
    /// The data column of each shown column (hidden ones are left out; WG5a's `cur_col` and edits
    /// count shown columns).
    pub col_map: Vec<usize>,
    pub stretch_last: bool,
    /// `fit-columns(column?)`, applied at the next paint.
    pub fit: Option<Option<usize>>,
    /// R8: focus came from the keyboard (the cursor cell's ring); a click clears it.
    pub kbd: bool,
    pub was_focused: bool,
    /// The error tooltip showed last pass (hidden once the error is gone).
    pub tip_shown: bool,
    pub rt: TableRt,
    /// LS2 (SV41): rows from a Rust model ([`SceneMut::set_table_model`]) instead of the JS window.
    pub model: ModelSlot,
}

impl Default for TableState {
    fn default() -> Self {
        TableState {
            row_count: 0,
            row_height: 20.0,
            sticky: 0,
            striped: true,
            overscan: 20,
            first: 0,
            rows: Vec::new(),
            asked: None,
            scroll_to: None,
            scroll_to_col: None,
            keys: None,
            selection: "single",
            sel: KeySelect::new(Mode::Single),
            cur_col: 0,
            page: 10,
            edit: None,
            pending_edit: None,
            editors: Vec::new(),
            error: None,
            refocus: false,
            sortable: false,
            sort_column: -1,
            sort_order: "none",
            column_menu: true,
            hidden: Vec::new(),
            header_state: String::new(),
            restore: None,
            col_map: Vec::new(),
            stretch_last: true,
            fit: None,
            kbd: false,
            was_focused: false,
            tip_shown: false,
            rt: TableRt::default(),
            model: ModelSlot::default(),
        }
    }
}

impl TableState {
    pub(crate) fn set(&mut self, key: P, v: &Value) -> Result<Dirty, String> {
        match key {
            P::RowCount => self.row_count = as_i64(v)?.max(0),
            P::RowHeight => self.row_height = as_f64(v)?.max(1.0),
            P::StickyColumns => self.sticky = as_i64(v)?.max(0),
            P::Striped => self.striped = as_bool(v)?,
            P::Overscan => self.overscan = as_i64(v)?.max(0),
            P::SelectedIds => {
                self.sel.set(item_keys::ids_of(v, "selected")?);
            }
            P::ListSelection => {
                self.selection = as_enum(v, kinds::TABLE_SELECTION)?;
                let m = Mode::parse(self.selection).unwrap_or_default();
                let keep: Vec<u64> = self.sel.selected.iter().copied().collect();
                self.sel.mode = m;
                self.sel.set(keep);
            }
            P::CurrentRow => {
                let k = as_i64(v)?;
                self.sel.cursor = (k >= 0).then_some(k as u64);
                self.sel.anchor = self.sel.cursor;
            }
            P::CurrentColumn => self.cur_col = as_i64(v)?.max(0) as usize,
            P::Sortable => self.sortable = as_bool(v)?,
            P::SortColumn => self.sort_column = as_i64(v)?.max(-1),
            P::SortOrder => self.sort_order = as_enum(v, kinds::SORT)?,
            P::ColumnMenu => self.column_menu = as_bool(v)?,
            P::HiddenColumns => self.hidden = if matches!(v, Value::Null) { Vec::new() } else { as_str_list(v)? },
            P::HeaderState => {
                let s = as_str(v)?;
                self.restore = if s.is_empty() || s == self.header_state {
                    None
                } else {
                    Some(serde_json::from_str::<serde_json::Value>(&s).map_err(|e| format!("header-state: {e}"))?)
                };
                self.header_state = s;
            }
            P::StretchLast => self.stretch_last = as_bool(v)?,
            _ => return Err(format!("{key:?} is not a prop of <table>")),
        }
        Ok(if key == P::RowHeight { Dirty::MEASURE } else { Dirty::EMPTY })
    }

    pub(crate) fn get(&self, key: P) -> Option<Value> {
        Some(match key {
            P::RowCount => Value::I64(self.row_count),
            P::RowHeight => Value::F64(self.row_height),
            P::StickyColumns => Value::I64(self.sticky),
            P::Striped => Value::Bool(self.striped),
            P::Overscan => Value::I64(self.overscan),
            P::SelectedIds => item_keys::ids_value(self.sel.selected.iter().copied()),
            P::ListSelection => Value::Str(self.selection.to_string()),
            P::CurrentRow => Value::I64(self.sel.cursor.map_or(-1, |k| k as i64)),
            P::CurrentColumn => Value::I64(self.cur_col as i64),
            P::Sortable => Value::Bool(self.sortable),
            P::SortColumn => Value::I64(self.sort_column),
            P::SortOrder => Value::Str(self.sort_order.to_string()),
            P::ColumnMenu => Value::Bool(self.column_menu),
            P::HiddenColumns => Value::List(self.hidden.iter().map(|h| Value::Str(h.clone())).collect()),
            P::HeaderState => Value::Str(self.header_state.clone()),
            P::StretchLast => Value::Bool(self.stretch_last),
            _ => return None,
        })
    }

    /// The data column of shown column `col` (the same while no column is hidden).
    pub fn data_col(&self, col: usize) -> usize {
        self.col_map.get(col).copied().unwrap_or(col)
    }

    /// `fit-columns(column?)`.
    pub(crate) fn fit_columns(&mut self, args: &[Value]) -> StrResult<Value> {
        arity("fit-columns", args, 0, 1)?;
        self.fit = Some(match args.first() {
            None | Some(Value::Null) => None,
            Some(v) => Some(as_i64(v).map_err(|e| format!("fit-columns column: {e}"))?.max(0) as usize),
        });
        Ok(Value::Null)
    }

    /// Rows in the table: a Rust model's shown rows, else the ids given by `set-ids`, else
    /// `row-count`.
    pub fn len(&self) -> u64 {
        if let Some(m) = &self.model.0 {
            return m.view.len() as u64;
        }
        self.keys.as_ref().map_or(self.row_count.max(0) as u64, |k| k.ids.len() as u64)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The id of the row at `pos`.
    pub fn key_of(&self, pos: u64) -> u64 {
        if let Some(m) = &self.model.0 {
            return m.view.row(pos as usize).map_or(pos, |r| m.model.key(r));
        }
        self.keys.as_ref().and_then(|k| k.ids.get(pos as usize).copied()).unwrap_or(pos)
    }

    /// The position of the row with id `key`.
    pub fn pos_of(&self, key: u64) -> Option<u64> {
        if let Some(m) = &self.model.0 {
            return m.view.pos_of(key).map(|p| p as u64);
        }
        match &self.keys {
            Some(k) => k.ids.iter().position(|&x| x == key).map(|p| p as u64),
            None => (key < self.len()).then_some(key),
        }
    }

    fn all_ids(&self) -> Cow<'_, [u64]> {
        if let Some(m) = &self.model.0 {
            return Cow::Owned(m.view.keys(&*m.model));
        }
        match &self.keys {
            Some(k) => Cow::Borrowed(&k.ids),
            None => Cow::Owned((0..self.len()).collect()),
        }
    }

    /// Type-ahead text of the row at `pos`: its `set-ids` label, else its first cell (window only).
    fn label(&self, pos: usize) -> Option<String> {
        if let Some(m) = &self.model.0 {
            return m.view.row(pos).and_then(|r| m.model.label(r)).map(Cow::into_owned);
        }
        self.keys.as_ref().and_then(|k| k.label(pos)).or_else(|| self.cell(pos as u64, 0).map(Cow::into_owned))
    }

    /// `set-rows(first, rows)`.
    pub(crate) fn set_rows(&mut self, args: &[Value]) -> StrResult<Value> {
        arity("set-rows", args, 2, 2)?;
        let first = as_i64(&args[0]).map_err(|e| format!("set-rows first: {e}"))?;
        if first < 0 {
            return Err(format!("set-rows first: {first} is negative"));
        }
        let rows = match &args[1] {
            Value::List(rows) => rows,
            Value::Null => &Vec::new(),
            other => {
                return Err(format!("set-rows rows: expected a list of rows, got {other:?}"));
            }
        };
        let mut out = Vec::with_capacity(rows.len());
        for (k, r) in rows.iter().enumerate() {
            match r {
                Value::List(cells) => out.push(cells.iter().map(cell_text).collect()),
                Value::Null => out.push(Vec::new()),
                other => {
                    return Err(format!("set-rows row {k}: expected a list of cells, got {other:?}"));
                }
            }
        }
        self.first = first as u64;
        self.rows = out;
        Ok(Value::Null)
    }

    pub(crate) fn clear_rows(&mut self, args: &[Value]) -> StrResult<Value> {
        arity("clear-rows", args, 0, 0)?;
        self.rows.clear();
        self.first = 0;
        // ask again for what is shown
        self.asked = None;
        Ok(Value::Null)
    }

    pub(crate) fn scroll_to_row(&mut self, args: &[Value]) -> StrResult<Value> {
        arity("scroll-to-row", args, 1, 1)?;
        let row = as_i64(arg(args, 0))?;
        self.scroll_to = Some(row.max(0) as u64);
        Ok(Value::Null)
    }

    /// `set-ids(ids, labels?)`: the id of every row by position (null: positions again).
    pub(crate) fn set_ids(&mut self, args: &[Value]) -> StrResult<Value> {
        arity("set-ids", args, 1, 2)?;
        if matches!(args[0], Value::Null) {
            self.keys = None;
        } else {
            let ids = item_keys::ids_of(&args[0], "set-ids ids")?;
            let labels = item_keys::labels_of(args.get(1), "set-ids labels")?.unwrap_or_default();
            if !labels.is_empty() && labels.len() != ids.len() {
                return Err(format!("set-ids labels: {} labels for {} ids", labels.len(), ids.len()));
            }
            self.keys = Some(RowKeys { ids, labels });
        }
        // the rows moved: ask for the window again
        self.asked = None;
        Ok(Value::Null)
    }

    /// `edit(row-id, column)`: starts editing that cell (at the next frame).
    pub(crate) fn edit_cell(&mut self, args: &[Value]) -> StrResult<Value> {
        arity("edit", args, 2, 2)?;
        let row = as_i64(&args[0]).map_err(|e| format!("edit row: {e}"))?;
        let col = as_i64(&args[1]).map_err(|e| format!("edit column: {e}"))?;
        if row < 0 || col < 0 {
            return Err(format!("edit: no cell ({row}, {col})"));
        }
        self.pending_edit = Some((row as u64, col as usize));
        Ok(Value::Null)
    }

    /// `end-edit(commit)`: ends the open edit (true commits, false reverts). Returns whether one was open.
    pub(crate) fn end_edit(&mut self, args: &[Value]) -> StrResult<Value> {
        arity("end-edit", args, 0, 1)?;
        let commit = !matches!(args.first(), Some(Value::Bool(false)));
        Ok(Value::Bool(match &mut self.edit {
            Some(e) => {
                e.end.get_or_insert((commit, 0));
                true
            }
            None => false,
        }))
    }

    /// The text of cell (`row`, `col`) (data column `col`): from the Rust model, else if its row
    /// is in the JS window.
    pub fn cell(&self, row: u64, col: usize) -> Option<Cow<'_, str>> {
        if let Some(m) = &self.model.0 {
            return m.cell(row, col);
        }
        let k = row.checked_sub(self.first)? as usize;
        self.rows.get(k)?.get(col).map(|s| Cow::Borrowed(s.as_str()))
    }

    /// A committed edit of cell (`row`, `col`): into the Rust model (which may reject it), else
    /// into the window.
    pub(crate) fn set_cell(&mut self, row: u64, col: usize, text: String) -> Result<(), String> {
        if let Some(m) = &mut self.model.0 {
            let Some(r) = m.view.row(row as usize) else { return Ok(()) };
            let mc = m.model_col(col);
            return m.model.commit(r, mc, &text);
        }
        let Some(k) = row.checked_sub(self.first) else { return Ok(()) };
        if let Some(r) = self.rows.get_mut(k as usize) {
            if r.len() <= col {
                r.resize(col + 1, String::new());
            }
            r[col] = text;
        }
        Ok(())
    }

    /// LS2: brings a Rust model's view up to date with the columns `all` (model column of each)
    /// and the sort (`sort-column`/`sort-order` on a sortable model column). True if the shown
    /// rows were rebuilt.
    pub(crate) fn sync_model(&mut self, all: &[Col]) -> bool {
        let Some(m) = self.model.0.as_deref_mut() else { return false };
        let specs = m.model.columns();
        m.cols = all.iter().map(|c| specs.iter().position(|s| s.key == c.key).unwrap_or(c.data)).collect();
        let order = match self.sort_order {
            "ascending" => Some(SortOrder::Ascending),
            "descending" => Some(SortOrder::Descending),
            _ => None,
        };
        let sort = (self.sort_column >= 0).then(|| m.model_col(self.sort_column as usize)).filter(|&mc| specs.get(mc).is_none_or(|s| s.sortable)).zip(order);
        m.view.set_sort(sort);
        m.view.sync(&*m.model)
    }

    /// LS2: the tree facts of the row at `pos` (`None`: not a tree).
    pub fn tree_row(&self, pos: u64) -> Option<TreeRow> {
        self.model.0.as_ref()?.tree_row(pos)
    }

    /// LS2: expands or collapses the tree row with key `key`. True if it changed.
    pub(crate) fn set_expanded(&mut self, key: u64, on: bool) -> bool {
        self.model.0.as_mut().is_some_and(|m| m.view.set_expanded(key, on))
    }
}

fn cell_text(v: &Value) -> String {
    match v {
        Value::Null => String::new(),
        Value::Str(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
        Value::I64(i) => i.to_string(),
        Value::U64(u) => u.to_string(),
        Value::F64(f) => {
            if f.fract() == 0.0 && f.abs() < 1e15 {
                format!("{}", *f as i64)
            } else {
                f.to_string()
            }
        }
        other => format!("{other:?}"),
    }
}
/// Natural size: a few columns' worth, a header and eight rows.
pub(crate) fn intrinsic(m: &Metrics, t: &TableState) -> Vec2 {
    let _ = m;
    vec2(300.0, (t.row_height as f32) * 9.0)
}

/// The snippet editor outlet to lay out, and its size (the edited cell's), if any.
pub(crate) fn editor_place(t: &TableState) -> Option<(NodeId, Vec2)> {
    let e = t.edit.as_ref().filter(|e| e.snippet && e.bound && e.size.x > 0.0)?;
    let (_, outlet) = t.editors.iter().find(|(c, _)| *c == e.column)?;
    Some((*outlet, e.size))
}

/// The keys of a focused table that are not selection keys: Left/Right (cursor column), F2 (edit)
/// and typed text (edit, or type-ahead). Consumed from egui's input.
enum TableKey {
    Column(i32),
    F2,
    Text(String, f64),
}

fn table_keys(ui: &Ui) -> Vec<TableKey> {
    use egui::{Event, Key};
    let now = ui.input(|i| i.time);
    ui.ctx().input_mut(|i| {
        let mut out = Vec::new();
        i.events.retain(|e| {
            let k = match e {
                Event::Key { key: Key::ArrowLeft, pressed: true, .. } => TableKey::Column(-1),
                Event::Key { key: Key::ArrowRight, pressed: true, .. } => TableKey::Column(1),
                Event::Key { key: Key::F2, pressed: true, .. } => TableKey::F2,
                Event::Text(t) if t != " " => TableKey::Text(t.clone(), now),
                _ => return true,
            };
            out.push(k);
            false
        });
        out
    })
}

/// Starts editing cell (`pos`, `col`) when its column is editable.
fn start_edit(t: &mut TableState, cols: &[Col], pos: u64, col: usize, typed: String) -> bool {
    let Some(c) = cols.get(col).filter(|c| c.editable) else { return false };
    if pos >= t.len() {
        return false;
    }
    let row = t.key_of(pos);
    let value = t.cell(pos, t.data_col(col)).map(Cow::into_owned).unwrap_or_default();
    let draft = if typed.is_empty() { value.clone() } else { typed.clone() };
    t.edit = Some(Edit { row, col, column: c.node, value, draft, typed, snippet: c.editor, bound: false, focus: true, had_focus: false, size: Vec2::ZERO, widgets: Vec::new(), end: None });
    t.error = None;
    t.sel.cursor = Some(row);
    t.cur_col = col;
    t.scroll_to = Some(pos);
    t.scroll_to_col = Some(col);
    true
}

/// The editable cell `step` cells after (`pos`, `col`), row-major, if any.
fn next_editable(editable: &[bool], len: u64, pos: u64, col: usize, step: i32) -> Option<(u64, usize)> {
    let n = editable.len() as i64;
    if n == 0 || step == 0 {
        return None;
    }
    let mut at = pos as i64 * n + col as i64;
    let end = len as i64 * n;
    loop {
        at += step.signum() as i64;
        if at < 0 || at >= end {
            return None;
        }
        let c = (at % n) as usize;
        if editable[c] {
            return Some(((at / n) as u64, c));
        }
    }
}

/// One column, from its `table-column` node.
#[derive(Clone)]
pub(crate) struct Col {
    pub node: NodeId,
    /// Its scene index.
    pub ci: usize,
    pub title: String,
    /// Its width (the `width` prop, a user resize, clamped to `min`..`max`).
    pub width: f32,
    pub min: f32,
    pub max: f32,
    pub resizable: bool,
    pub align: &'static str,
    pub header_align: &'static str,
    pub sort: &'static str,
    pub editable: bool,
    pub editor: bool,
    /// Its child index (the data column, `header-clicked`'s and `sort-column`'s number).
    pub data: usize,
    /// Its `name`, else its child index.
    pub key: String,
    pub hideable: bool,
    /// `visible` (Widget's).
    pub shown: bool,
    pub tooltip: String,
    pub elide: Option<paint::Elide>,
}

/// The `table-column` children of node `i` (every one; [`Col::shown`] and `hidden-columns` pick the
/// shown ones). OW5a2's multi-column tree reads its columns with this.
pub(crate) fn columns(s: &Scene, i: usize) -> Vec<Col> {
    s.children[i]
        .iter()
        .filter_map(|&c| {
            let ci = s.ix(c);
            let W::TableColumn(st) = s.w(ci) else { return None };
            let cm = s.common(ci);
            let min = cm.min_width.map_or(MIN_COL, |m| m as f32).max(1.0);
            let max = cm.max_width.map_or(f32::INFINITY, |m| m as f32).max(min);
            Some(Col {
                node: c,
                ci,
                title: st.title.clone(),
                width: (cm.width.unwrap_or(100.0) as f32).clamp(min, max),
                min,
                max,
                resizable: st.resizable,
                align: st.align,
                header_align: if st.header_align == "auto" { st.align } else { st.header_align },
                sort: st.sort,
                editable: st.editable,
                editor: st.editor,
                data: 0,
                key: st.name.clone(),
                hideable: st.hideable,
                shown: cm.visible,
                tooltip: cm.tooltip.clone(),
                elide: paint::Elide::parse(st.elide),
            })
        })
        .enumerate()
        .map(|(k, c)| Col { data: k, key: if c.key.is_empty() { k.to_string() } else { c.key.clone() }, ..c })
        .collect()
}

/// Where the shown columns are: content x and drawn width (the last one stretched), the sticky
/// ones fixed at the left, the rest scrolled by `off.x`.
pub(crate) struct Geom {
    pub body: Rect,
    pub head: Rect,
    pub xs: Vec<f32>,
    pub ws: Vec<f32>,
    pub sticky: usize,
    pub sticky_w: f32,
    pub total: f32,
    pub off: Vec2,
    pub row_h: f32,
}

impl Geom {
    pub(crate) fn new(r: Rect, header_h: f32, widths: &[f32], sticky: usize, stretch_last: bool, row_h: f32) -> Geom {
        let head = Rect::from_min_size(r.min, vec2(r.width(), header_h.min(r.height())));
        let body = Rect::from_min_max(pos2(r.left(), head.bottom()), r.max);
        let mut ws = widths.to_vec();
        let sum: f32 = ws.iter().sum();
        if stretch_last && sum < body.width() && let Some(l) = ws.last_mut() {
            *l += body.width() - sum;
        }
        let mut xs = Vec::with_capacity(ws.len());
        let mut x = 0.0;
        for w in &ws {
            xs.push(x);
            x += w;
        }
        let sticky = sticky.min(ws.len());
        Geom { body, head, sticky_w: ws[..sticky].iter().sum(), xs, ws, sticky, total: x, off: Vec2::ZERO, row_h }
    }

    /// Column `k`'s screen x range.
    pub(crate) fn col_x(&self, k: usize) -> (f32, f32) {
        let x = self.body.left() + self.xs[k] - if k < self.sticky { 0.0 } else { self.off.x };
        (x, x + self.ws[k])
    }

    /// The x range column `k` may paint in (scrolled columns stay right of the sticky ones).
    pub(crate) fn clip_x(&self, k: usize) -> (f32, f32) {
        if k < self.sticky { (self.body.left(), self.body.right()) } else { ((self.body.left() + self.sticky_w).min(self.body.right()), self.body.right()) }
    }

    /// The columns with something in view.
    pub(crate) fn visible_cols(&self) -> impl Iterator<Item = usize> + '_ {
        (0..self.ws.len()).filter(|&k| {
            let ((a, b), (l, r)) = (self.col_x(k), self.clip_x(k));
            b > l && a < r
        })
    }

    pub(crate) fn col_at(&self, x: f32) -> Option<usize> {
        self.visible_cols().find(|&k| {
            let ((a, b), (l, r)) = (self.col_x(k), self.clip_x(k));
            x >= a.max(l) && x < b.min(r)
        })
    }

    pub(crate) fn row_at(&self, y: f32, len: u64) -> Option<u64> {
        let p = ((y - self.body.top() + self.off.y) / self.row_h).floor();
        (p >= 0.0 && (p as u64) < len && y >= self.body.top() && y < self.body.bottom()).then_some(p as u64)
    }

    /// Row `pos`'s screen rect (full width of the shown columns).
    pub(crate) fn row_rect(&self, pos: u64) -> Rect {
        let y = self.body.top() + pos as f32 * self.row_h - self.off.y;
        let right = (self.body.left() + self.total - self.off.x).max(self.body.left() + self.sticky_w).min(self.body.right());
        Rect::from_min_max(pos2(self.body.left(), y), pos2(right, y + self.row_h))
    }

    pub(crate) fn cell_rect(&self, pos: u64, k: usize) -> (Rect, Rect) {
        let y = self.body.top() + pos as f32 * self.row_h - self.off.y;
        let (a, b) = self.col_x(k);
        let (l, r) = self.clip_x(k);
        let cell = Rect::from_min_max(pos2(a, y), pos2(b, y + self.row_h));
        (cell, Rect::from_x_y_ranges(l..=r, self.body.y_range()))
    }
}

/// A click on a row this pass.
struct Click {
    row: u64,
    mods: Mods,
    double: bool,
    /// The column under the pointer.
    col: Option<usize>,
    at: Pos2,
}

/// An AccessKit node's bounds (egui fills them only for focusable widgets; rows and cells are
/// hover-only).
fn bounds(b: &mut egui::accesskit::Node, r: Rect) {
    b.set_bounds(egui::accesskit::Rect { x0: r.min.x.into(), y0: r.min.y.into(), x1: r.max.x.into(), y1: r.max.y.into() });
}

/// A cell's a11y and hover id.
fn cell_id(table: Id, row_key: u64, col: usize) -> Id {
    table.with(("cell", row_key, col))
}

fn dark(ui: &Ui) -> bool {
    ui.ctx().theme() == egui::Theme::Dark
}

fn text_align(a: &str) -> paint::Align {
    match a {
        "center" => paint::Align::Center,
        "end" => paint::Align::End,
        _ => paint::Align::Start,
    }
}

/// Paints `text` in `content` under `s`, cut by `elide` when too wide.
fn paint_cell_text(pc: &mut PaintCx, s: &StyleView, text: &str, content: Rect, align: &str, elide: Option<paint::Elide>, ctx: &egui::Context) {
    if text.is_empty() || content.width() <= 0.0 {
        return;
    }
    let shown = match elide {
        Some(m) => Cow::Owned(paint::elide(ctx, s, text, m, content.width())),
        None => Cow::Borrowed(text),
    };
    let g = paint::text_galley(ctx, s, &shown, None);
    paint::paint_text(pc, s, &g, content, text_align(align));
}

/// The sort the header shows on column `c`: the table's `sort-column`, else the column's `sort`.
fn shown_sort(t: &TableState, c: &Col) -> &'static str {
    if t.sort_column >= 0 {
        if t.sort_column as usize == c.data { t.sort_order } else { "none" }
    } else {
        c.sort
    }
}

/// Emits event `name` of node `i` when listened.
fn emit(arena: &Arena, i: usize, name: &str, args: Vec<Value>, out: &mut Vec<Out>) {
    let s = kinds::signal_id(arena.core[i].kind as usize, name);
    if arena.listens(i, s) {
        out.push(Out::Event(arena.slots.id_at(i).expect("live node").to_raw(), s, args));
    }
}

/// A user change of props `keys` announced (`bind:`) and through event `name`.
fn changed_via(arena: &mut Arena, i: usize, keys: &[P], name: &str, args: Vec<Value>, out: &mut Vec<Out>) {
    crate::leaf::props_changed(arena, i, keys, out);
    emit(arena, i, name, args, out);
}

/// Writes a user width into column `c`'s `width` and announces it (`bind:width`).
fn set_width(arena: &mut Arena, c: &mut Col, w: f32, out: &mut Vec<Out>) {
    let w = w.clamp(c.min, c.max);
    c.width = w;
    let wv = (w as f64 * 100.0).round() / 100.0;
    if arena.common(c.ci).width != Some(wv) {
        arena.set_common(c.ci, |cm| cm.width = Some(wv));
        crate::leaf::props_changed(arena, c.ci, &[P::Width], out);
    }
}

/// The width that fits column `c`'s header and the cells of the data window.
fn fit_width(arena: &Arena, i: usize, t: &TableState, c: &Col, ctx: &egui::Context, d: bool) -> f32 {
    let node = arena.slots.id_at(i).expect("live node");
    let host_css = arena.style.computed(node).cloned();
    let host = StyleView::new(host_css.as_deref(), Ua(&[]), StateBits::NONE, d);
    let hc = arena.style.part_computed(node, "header-cell").cloned();
    let hv = host.part(hc.as_deref(), HEADER_CELL_UA);
    let mut w = paint::measure_text(ctx, &hv, &c.title, crate::provider::AvailableSpace::MaxContent).x + paint::box_extra(&hv).x;
    if shown_sort(t, c) != "none" {
        w += hv.part(None, SORT_UA).px("width").unwrap_or(16.0);
    }
    let cc = arena.style.part_computed(node, "cell").cloned();
    let cv = host.part(cc.as_deref(), CELL_UA);
    let pad = paint::box_extra(&cv).x;
    let fit = |w: f32, text: &str| w.max(paint::measure_text(ctx, &cv, text, crate::provider::AvailableSpace::MaxContent).x + pad);
    if t.model.is_some() {
        // a Rust model: the rows in view and as many after (LS2)
        let first = (t.rt.scroll.offset().y / t.row_height as f32).max(0.0) as u64;
        let indent = |p: u64| t.tree_row(p).filter(|_| c.data == t.data_col(0)).map_or(0.0, |r| (r.depth + 1) as f32 * INDENT);
        for p in first..(first + 100).min(t.len()) {
            if let Some(text) = t.cell(p, c.data) {
                w = w.max(fit(0.0, &text) + indent(p));
            }
        }
    } else {
        for row in &t.rows {
            if let Some(text) = row.get(c.data) {
                w = fit(w, text);
            }
        }
    }
    w.ceil().clamp(c.min, c.max)
}

/// What the header did this pass.
#[derive(Default)]
pub(crate) struct HeaderOut {
    pub clicked: Option<usize>,
    pub double: Option<usize>,
    pub context: Option<(usize, Pos2)>,
    /// (column index in `all`, width): resizes that committed (drag released, a fit).
    pub committed: Vec<(usize, f32)>,
}

/// Paints table `i` at `r`: its frozen header, the visible rows and columns of its data window,
/// the open editor, the bars and the column menu, then its events (`range`, `selectionchange`,
/// `activated`, `header-clicked`, …).
pub(crate) fn paint(arena: &mut Arena, i: usize, r: Rect, ui: &mut Ui, out: &mut Vec<Out>) {
    let node = arena.slots.id_at(i).expect("live node");
    let h = node.to_raw();
    let id = arena.core[i].egui_id;
    let enabled = arena.common(i).enabled;
    let ctx = ui.ctx().clone();
    let d = dark(ui);
    let mut all = columns(arena, i);
    // FM1 (QW4): a `header-state` the host set: hidden columns, sort, widths (into the columns)
    let restored = {
        let W::Table(t) = arena.w_mut(i) else { unreachable!() };
        restore_header(t, &all)
    };
    for &(k, w) in &restored.widths {
        set_width(arena, &mut all[k], w, out);
    }
    let W::Table(t) = arena.w_mut(i) else { unreachable!() };
    // a live resize shows before it commits
    if let Some((n, w)) = t.rt.live
        && let Some(c) = all.iter_mut().find(|c| c.node == n)
    {
        c.width = w.clamp(c.min, c.max);
    }
    let cols: Vec<Col> = all.iter().filter(|c| c.shown && !t.hidden.contains(&c.key)).cloned().collect();
    t.col_map = cols.iter().map(|c| c.data).collect();
    // LS2: a Rust model's shown rows (sort, filter, expanded tree rows), before anything reads them
    t.sync_model(&all);
    if !cols.is_empty() && t.cur_col >= cols.len() {
        t.cur_col = cols.len() - 1;
    }
    let row_h = t.row_height as f32;
    let len = t.len();
    let mut tui = ui.new_child({
        let b = UiBuilder::new().max_rect(r).id_salt(("table", id)).layout(Layout::top_down(Align::Min));
        if enabled { b } else { b.disabled() }
    });
    tui.set_clip_rect(ui.clip_rect().intersect(r));
    // the table's a11y node (Table, scroll actions) and its keyboard focus (Grid)
    let tresp = tui.interact(r, id.with("table"), Sense::hover());
    let focus_id = id.with("focus");
    let fresp = tui.interact(r, focus_id, Sense::focusable_noninteractive());
    let mut focused = enabled && fresp.has_focus();
    let mut sel_changed = false;
    let mut cur_changed = false;
    let mut activate: Option<u64> = None;
    let mut copy = false;
    // LS2: tree rows expanded or collapsed this pass (key, expanded)
    let mut toggled_rows: Vec<(u64, bool)> = Vec::new();
    let W::Table(t) = arena.w_mut(i) else { unreachable!() };
    let t: &mut TableState = t;
    let selectable = t.sel.mode != Mode::None;
    if std::mem::take(&mut t.refocus) && enabled {
        ctx.memory_mut(|m| m.request_focus(focus_id));
        focused = true;
    }
    let mut had_keys = false;
    if focused && t.edit.is_none() {
        item_keys::lock_arrows(&tui, focus_id);
        copy = item_keys::take_copy(&tui);
        let mut inputs: Vec<Input> = Vec::new();
        for k in table_keys(&tui) {
            had_keys = true;
            let cur = t.sel.cursor.and_then(|c| t.pos_of(c));
            match k {
                TableKey::Column(dir) => {
                    // LS2: in the first column of a tree row, Right expands, Left collapses or goes
                    // to the parent row (Qt's QTreeView, VS Code's tree)
                    let tree = cur.filter(|_| t.cur_col == 0).and_then(|p| Some((p, t.tree_row(p)?)));
                    if let Some((p, tr)) = tree
                        && (dir > 0 && tr.has_children && !tr.expanded || dir < 0 && (tr.expanded || tr.depth > 0))
                    {
                        let key = t.key_of(p);
                        if dir > 0 || tr.expanded {
                            if t.set_expanded(key, dir > 0) {
                                toggled_rows.push((key, dir > 0));
                            }
                        } else if let Some(pp) = t.model.0.as_ref().and_then(|m| m.view.parent_pos(&*m.model, p as usize)) {
                            let ids = t.all_ids().into_owned();
                            let ch = t.sel.click(&ids, t.key_of(pp as u64), Mods::default());
                            sel_changed |= ch.selection;
                            cur_changed |= ch.cursor;
                            t.scroll_to = Some(pp as u64);
                        }
                        continue;
                    }
                    if let Some(c) = RovingList::next_by(cols.len(), Some(t.cur_col), dir, false, |_| true) {
                        cur_changed |= c != t.cur_col;
                        t.cur_col = c;
                        t.scroll_to_col = Some(c);
                    }
                }
                TableKey::F2 => {
                    if let Some(p) = cur {
                        start_edit(t, &cols, p, t.cur_col, String::new());
                    }
                }
                TableKey::Text(s, now) => {
                    let started = cur.is_some_and(|p| t.edit.is_none() && start_edit(t, &cols, p, t.cur_col, s.clone()));
                    if !started && t.edit.is_none() {
                        inputs.push(Input::Text(s, now));
                    }
                }
            }
        }
        if t.edit.is_none() {
            inputs.extend(item_keys::read_keys(&tui, false));
        }
        if !inputs.is_empty() {
            had_keys = true;
            let ids = t.all_ids().into_owned();
            let mut sel = std::mem::take(&mut t.sel);
            let o = item_keys::apply(&mut sel, &ids, &inputs, t.page, |p| t.label(p));
            t.sel = sel;
            sel_changed |= o.selection;
            cur_changed |= o.cursor;
            if o.cursor
                && let Some(p) = t.sel.cursor.and_then(|c| t.pos_of(c))
            {
                t.scroll_to = Some(p);
            }
            activate = o.activate.map(|p| p as u64);
        }
        if cur_changed || sel_changed || activate.is_some() || !toggled_rows.is_empty() {
            t.error = None;
            ctx.request_repaint();
        }
    }
    // R8: the ring is for keyboard focus (Tab in, keys); a click clears it (below)
    if focused && (had_keys || !t.was_focused) {
        t.kbd = true;
    }
    t.was_focused = focused;
    if let Some((key, col)) = t.pending_edit.take()
        && t.edit.is_none()
        && let Some(p) = t.pos_of(key)
    {
        start_edit(t, &cols, p, col, String::new());
    }
    // ---- geometry and scrolling (OW2's Scrollable on both axes) ----
    let host_css = arena.style.computed(node).cloned();
    let header_css = arena.style.part_computed(node, "header").cloned();
    let host0 = StyleView::new(host_css.as_deref(), Ua(&[]), arena.core[i].state, d);
    let header_v = host0.part(header_css.as_deref(), HEADER_UA);
    let header_h = header_v.px("height").unwrap_or(row_h + 4.0);
    let W::Table(t) = arena.w_mut(i) else { unreachable!() };
    let widths: Vec<f32> = cols.iter().map(|c| c.width).collect();
    let mut g = Geom::new(r, header_h, &widths, t.sticky.max(0) as usize, t.stretch_last, row_h);
    let content = vec2(g.total, len as f32 * row_h);
    let view = g.body.size();
    let bar_w = StyleView::ua_only(crate::widgets::scroll_area::TRACK_UA, StateBits::NONE, d).px("width").unwrap_or(10.0);
    let min_thumb = StyleView::ua_only(crate::widgets::scroll_area::THUMB_UA, StateBits::NONE, d).px("min-height").unwrap_or(20.0);
    let sc = &mut t.rt.scroll;
    sc.set_geometry(content, view, [None, None]);
    let mut off = sc.offset();
    if let Some(row) = t.scroll_to.take() {
        let y = row.min(len.max(1) - 1) as f32 * row_h;
        if y < off.y {
            off.y = y;
        } else if y + row_h > off.y + view.y {
            off.y = y + row_h - view.y;
        }
    }
    if let Some(k) = t.scroll_to_col.take()
        && k < cols.len()
        && k >= g.sticky
    {
        let (x0, x1) = (g.xs[k] - g.sticky_w, g.xs[k] + g.ws[k] - g.sticky_w);
        let room = view.x - g.sticky_w;
        if x0 < off.x {
            off.x = x0;
        } else if x1 > off.x + room {
            off.x = (x1 - room).min(x0);
        }
    }
    sc.set_offset(off);
    let bars = sc.bar_geometry(g.body, bar_w, min_thumb, [sc.overflows(0), sc.overflows(1)]);
    sc.set_geometry(content, view, bars);
    let policy = crate::native::policy_of(&ctx);
    let now = ctx.input(|inp| inp.time);
    let mode = crate::interact::scroll::bar_mode(None, policy.scrollbars);
    let alpha = match mode {
        crate::native::Scrollbars::Overlay => sc.bar_alpha(now, &policy.scroll),
        crate::native::Scrollbars::Classic => 1.0,
    };
    let mut parts = PartResponses::default();
    for (ax, b) in bars.iter().enumerate() {
        let Some(b) = b else { continue };
        let (bn, tn) = if ax == 0 { ("scrollbar-x", "thumb-x") } else { ("scrollbar-y", "thumb-y") };
        if !(enabled && (alpha > 0.0 || tresp.contains_pointer())) {
            // the bar's AccessKit node needs a widget rect (`paint_bars`)
            let _ = tui.interact(b.track, id.with(bn), Sense::hover());
        } else {
            let tr = tui.interact(b.track, id.with(bn), Sense::click_and_drag());
            let th = tui.interact(b.thumb, id.with(tn), Sense::drag());
            parts.0.push((bn, crate::interact::Resp::of(&tr)));
            parts.0.push((tn, crate::interact::Resp::of(&th)));
        }
    }
    let params = ScrollParams { axes: [true, true], chain: true, page_keys: false, arrow_keys: false };
    let mut fired = Intents::default();
    let mut state = arena.core[i].state;
    {
        let W::Table(t) = arena.w_mut(i) else { unreachable!() };
        let sc = &mut t.rt.scroll;
        crate::interact::run(&tui, &tresp, &mut state, &parts, |icx| sc.input(&params, icx, &mut fired));
    }
    crate::style::set_own_state(arena, i, state);
    let W::Table(t) = arena.w_mut(i) else { unreachable!() };
    let sc = &mut t.rt.scroll;
    let consumed = sc.consumed();
    if consumed != Vec2::ZERO {
        ctx.input_mut(|inp| inp.smooth_scroll_delta -= consumed);
    }
    for (ax, (tn, _)) in [("thumb-x", 0), ("thumb-y", 1)].iter().enumerate() {
        let p = parts.get(tn);
        t.rt.thumbs[ax].set(StateBits::HOVERED, p.is_some_and(|p| p.hovered));
        t.rt.thumbs[ax].set(StateBits::PRESSED, p.is_some_and(|p| p.down_on));
        t.rt.thumbs[ax].set(StateBits::DRAGGING, t.rt.scroll.dragging(ax));
    }
    let sc = &t.rt.scroll;
    if !fired.is_empty() {
        ctx.request_repaint();
    }
    if let Some(w) = sc.next_frame() {
        ctx.request_repaint_after(w);
    }
    if mode == crate::native::Scrollbars::Overlay
        && let Some(w) = sc.bar_repaint(now, &policy.scroll)
    {
        ctx.request_repaint_after(std::time::Duration::from_secs_f32(w.max(0.0)));
    }
    g.off = sc.offset();
    let first = ((g.off.y / row_h).floor().max(0.0) as u64).min(len);
    let end = (((g.off.y + view.y) / row_h).ceil().max(0.0) as u64).min(len);
    t.page = ((end - first).saturating_sub(1) as usize).max(1);
    // ---- the body: clicks and hover by position (rows and cells are painted, not widgets) ----
    let bresp = tui.interact(g.body, id.with("body"), if enabled { Sense::click() } else { Sense::hover() });
    let hover_at = bresp.hover_pos().filter(|_| enabled && bresp.hovered());
    let hover = hover_at.and_then(|p| Some((g.row_at(p.y, len)?, g.col_at(p.x))));
    let pressed = bresp.is_pointer_button_down_on();
    let mut clicks: Vec<Click> = Vec::new();
    if enabled
        && (bresp.clicked() || bresp.double_clicked())
        && let Some(p) = bresp.interact_pointer_pos()
        && let Some(row) = g.row_at(p.y, len)
    {
        let mods = item_keys::mods(tui.input(|inp| inp.modifiers));
        clicks.push(Click { row, mods, double: bresp.double_clicked(), col: g.col_at(p.x), at: p });
    }
    // ---- rows and cells ----
    let mut cells: Vec<(u64, usize, Rect, Rect)> = Vec::new();
    let visible: Vec<usize> = g.visible_cols().collect();
    let ncols = cols.len();
    let painter = tui.painter_at(g.body);
    let scroll_painter = painter.with_clip_rect(painter.clip_rect().intersect(Rect::from_x_y_ranges((g.body.left() + g.sticky_w)..=g.body.right(), g.body.y_range())));
    let stripe = StyleView::ua_only(STRIPE_UA, StateBits::NONE, d).color("background-color").unwrap_or(Color32::TRANSPARENT).gamma_multiply(0.5);
    let error_at = {
        let W::Table(t) = arena.w(i) else { unreachable!() };
        t.error.as_ref().map(|e| (e.row, e.col))
    };
    let mut cursor_icon = None;
    let mut error_rect: Option<Rect> = None;
    // LS2: the expander of each shown tree row with children (position, its clipped rect)
    let mut expanders: Vec<(u64, Rect)> = Vec::new();
    for pos in first..end {
        let W::Table(t) = arena.w(i) else { unreachable!() };
        let key = t.key_of(pos);
        let tree = t.tree_row(pos);
        let selected = selectable && t.sel.is_selected(key);
        let current = t.sel.cursor == Some(key);
        let (cur_col, kbd, striped) = (t.cur_col, t.kbd, t.striped);
        let rr = g.row_rect(pos);
        let row_hovered = hover.is_some_and(|(p, _)| p == pos);
        let mut rbits = StateBits::NONE;
        rbits.set(StateBits::HOVERED, row_hovered);
        rbits.set(StateBits::PRESSED, row_hovered && pressed);
        rbits.set(StateBits::SELECTED, selected);
        rbits.set(StateBits::CURRENT, current);
        rbits.set(StateBits::FOCUS_WITHIN, focused);
        rbits.set(StateBits::DISABLED, !enabled);
        let row_css = arena.part_at(node, "row", rbits, (pos as usize, len as usize));
        let host = StyleView::new(host_css.as_deref(), Ua(&[]), rbits, d);
        let row_v = host.part(row_css.as_deref(), ROW_UA);
        {
            let mut pc = PaintCx::for_node(&painter, h);
            pc.tag = "table";
            let mut pr = pc.part("row");
            if striped && pos % 2 == 1 && row_v.color("background-color").is_none_or(|c| c.a() == 0) {
                painter.rect_filled(rr, 0.0, stripe);
                pr.record(TraceKind::Box, rr, stripe, Stroke::NONE, 0.0);
            }
            paint::paint_box(&mut pr, &row_v, rr);
        }
        if row_hovered {
            cursor_icon = row_v.cursor();
        }
        let row_id = id.with(("row", key));
        let rresp = tui.interact(rr.intersect(g.body), row_id, Sense::hover());
        let rb = rresp.rect;
        tui.ctx().accesskit_node_builder(rresp.id, |b| {
            bounds(b, rb);
            b.set_role(Role::Row);
            b.set_row_index(pos as usize);
            if selectable {
                b.set_selected(selected);
            }
            if let Some(tr) = tree {
                b.set_level(tr.depth as usize + 1);
                if tr.has_children {
                    b.set_expanded(tr.expanded);
                }
            }
        });
        let cui = tui.new_child(UiBuilder::new().max_rect(rr).accessibility_parent(rresp.id));
        for &k in &visible {
            let c = &cols[k];
            let (cr, clip) = g.cell_rect(pos, k);
            cells.push((pos, k, cr, clip));
            let is_cur = focused && current && cur_col == k;
            let mut cbits = rbits;
            cbits.set(StateBits::HOVERED, hover == Some((pos, Some(k))));
            cbits.set(StateBits::PRESSED, hover == Some((pos, Some(k))) && pressed);
            cbits.set(StateBits::CURRENT, current && cur_col == k);
            cbits.set(StateBits::FOCUSED, is_cur);
            cbits.set(StateBits::FOCUS_VISIBLE, is_cur && kbd);
            let cell_css = arena.part_at(node, "cell", cbits, (k, ncols));
            let cv = row_v.part(cell_css.as_deref(), CELL_UA).in_state(cbits);
            let W::Table(t) = arena.w(i) else { unreachable!() };
            let editing = t.edit.as_ref().is_some_and(|e| e.row == key && e.col == k);
            // owned: the expander's style below borrows the arena mutably
            let text = t.cell(pos, t.data_col(k)).map(Cow::into_owned).unwrap_or_default();
            let p = if k < g.sticky { &painter } else { &scroll_painter };
            let mut pc = PaintCx::for_node(p, h);
            pc.tag = "table";
            let geom = paint::paint_box(&mut pc.part("cell"), &cv, cr);
            let mut content = geom.content;
            // LS2: tree rows indent the first shown column and draw their expander there
            if k == 0
                && let Some(tr) = tree
            {
                let er = Rect::from_min_size(pos2(content.left() + tr.depth as f32 * INDENT, cr.top()), vec2(INDENT, cr.height()));
                if tr.has_children {
                    let mut ebits = rbits;
                    ebits.set(StateBits::CHECKED, tr.expanded);
                    ebits.set(StateBits::HOVERED, hover_at.is_some_and(|hp| er.contains(hp)));
                    let ecss = arena.part_at(node, "expander", ebits, (pos as usize, len as usize));
                    let ev = row_v.part(ecss.as_deref(), EXPANDER_UA).in_state(ebits);
                    let icon = if tr.expanded { "chevron-down" } else { "chevron-right" };
                    paint::paint_icon(&mut pc.part("expander"), &ev, &IconKey::new("codicon", icon), er);
                    expanders.push((pos, er.intersect(clip)));
                }
                content = content.with_min_x(er.right().min(content.right()));
            }
            if !editing {
                paint_cell_text(&mut pc.part("cell"), &cv, &text, content, c.align, c.elide, &ctx);
            }
            paint::paint_outline(&mut pc.part("cell"), &cv, cr);
            if error_at == Some((key, k)) {
                let ev = cv.part(None, ERROR_UA);
                paint::paint_outline(&mut pc.part("error"), &ev, cr);
                error_rect = Some(cr.intersect(clip));
            }
            let cid = cell_id(id, key, k);
            let cresp = cui.interact(cr.intersect(clip), cid, Sense::hover());
            let (label, editable, cb) = (text, c.editable, cresp.rect);
            cui.ctx().accesskit_node_builder(cresp.id, |b| {
                bounds(b, cb);
                b.set_role(Role::Cell);
                b.set_column_index(k);
                b.set_row_index(pos as usize);
                b.set_label(label);
                if selected {
                    b.set_selected(true);
                }
                if !editable {
                    b.set_read_only();
                }
            });
        }
    }
    if let Some(ci) = cursor_icon
        && hover.is_some()
    {
        ctx.set_cursor_icon(ci);
    }
    // R15: edge shadows when scrolled away from the top or the left (base.css)
    {
        let host = StyleView::new(host_css.as_deref(), Ua(&[]), arena.core[i].state, d);
        let mut pc = PaintCx::for_node(&painter, h);
        pc.tag = "table";
        paint::paint_inset_shadows(&mut pc, &host, g.body);
    }
    // ---- the header (frozen), its sort arrows and resize edges ----
    let hout = header(arena, i, &all, &cols, &g, enabled, &mut tui, out);
    // ---- clicks: focus, keyed selection, edits on double clicks ----
    let W::Table(t) = arena.w_mut(i) else { unreachable!() };
    for c in &clicks {
        ctx.memory_mut(|m| m.request_focus(focus_id));
        t.kbd = false;
        // LS2: a click on a tree row's expander toggles it (the double click's second press too)
        if expanders.iter().any(|&(p, er)| p == c.row && er.contains(c.at)) {
            let key = t.key_of(c.row);
            let on = !t.tree_row(c.row).is_some_and(|tr| tr.expanded);
            if !c.double && t.set_expanded(key, on) {
                toggled_rows.push((key, on));
            }
            continue;
        }
        if c.double {
            let col = c.col.unwrap_or(t.cur_col);
            if t.edit.is_none() && start_edit(t, &cols, c.row, col, String::new()) {
                continue;
            }
            activate = Some(c.row);
            continue;
        }
        let key = t.key_of(c.row);
        if let Some(e) = &mut t.edit
            && (e.row != key || Some(e.col) != c.col)
        {
            // a click elsewhere commits the open edit
            e.end.get_or_insert((true, 0));
        }
        let ids = t.all_ids().into_owned();
        let ch = t.sel.click(&ids, key, c.mods);
        sel_changed |= ch.selection;
        cur_changed |= ch.cursor;
        if let Some(col) = c.col {
            cur_changed |= col != t.cur_col;
            t.cur_col = col;
        }
        t.error = None;
    }
    // ---- the editor (seam: egui's TextEdit until OW4) ----
    let editor = paint_editor(arena, i, &cols, &cells, &mut tui, out);
    if editor {
        ctx.request_repaint();
    }
    // ---- a rejected commit's reason: OW3's tooltip at its cell ----
    let W::Table(t) = arena.w_mut(i) else { unreachable!() };
    let mut hide_tip = false;
    if let Some(e) = &mut t.error {
        let until = *e.until.get_or_insert(now + ERROR_SECONDS);
        let at = error_rect.filter(|_| now < until).map(|er| er.left_bottom());
        if at != e.shown_at {
            match at {
                Some(p) => crate::widgets::tooltip::show(&ctx, &e.reason, p),
                None => crate::widgets::tooltip::hide(&ctx),
            }
            e.shown_at = at;
        }
        if now >= until {
            hide_tip = e.shown_at.is_some();
            t.error = None;
        } else {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64((until - now).max(0.05)));
        }
    } else if std::mem::take(&mut t.tip_shown) {
        hide_tip = true;
    }
    t.tip_shown = t.error.as_ref().is_some_and(|e| e.shown_at.is_some());
    if hide_tip {
        crate::widgets::tooltip::hide(&ctx);
    }
    // ---- the bars (over the rows) ----
    paint_bars(arena, i, &tui, &bars, alpha, d);
    // ---- the column menu ----
    let toggled = column_menu(arena, i, &all, &cols, &hout, &mut tui, out);
    // ---- fit-columns (the method), double clicks on edges ----
    let mut committed = hout.committed.clone();
    let fit = {
        let W::Table(t) = arena.w_mut(i) else { unreachable!() };
        t.fit.take()
    };
    if let Some(f) = fit {
        let W::Table(t) = arena.w(i) else { unreachable!() };
        let fits: Vec<(usize, f32)> = all.iter().enumerate().filter(|(_, c)| c.shown && c.resizable && f.is_none_or(|k| k == c.data)).map(|(k, c)| (k, fit_width(arena, i, t, c, &ctx, d))).collect();
        committed.extend(fits);
    }
    for &(k, w) in &committed {
        set_width(arena, &mut all[k], w, out);
    }
    // ---- a11y ----
    let name = arena.common(i).accessible_name.clone();
    let W::Table(t) = arena.w(i) else { unreachable!() };
    let active = t.sel.cursor.map(|k| cell_id(id, k, t.cur_col));
    let multi = t.sel.mode == Mode::Multi;
    let sc = &t.rt.scroll;
    let tb = tresp.rect;
    tui.ctx().accesskit_node_builder(tresp.id, |b| {
        bounds(b, tb);
        b.set_role(Role::Table);
        if !name.is_empty() {
            b.set_label(name.clone());
        }
        if multi {
            b.set_multiselectable();
        }
        b.set_row_count(len as usize);
        b.set_column_count(ncols);
        sc.a11y(&params, b);
    });
    tui.ctx().accesskit_node_builder(focus_id, |b| {
        b.set_role(Role::Grid);
        if !name.is_empty() {
            b.set_label(format!("{name} keyboard"));
        }
        if focused && let Some(a) = active {
            b.set_active_descendant(a.accesskit_id());
        }
    });
    let resp = crate::a11y::decorate(tresp, &arena.n(h));
    let tip = arena.common(i).tooltip.clone();
    if !tip.is_empty() {
        let _ = crate::widgets::tooltip::attach(&tui, &resp, &tip, true);
    }
    arena.core[i].drawn = r;
    crate::scene::hit::record(arena, i, ui);
    // ---- events ----
    let W::Table(t) = arena.w_mut(i) else { unreachable!() };
    let mut events: Vec<(&str, Vec<Value>)> = Vec::new();
    // the data the table needs: the visible rows and `overscan` on each side
    if len > 0 && end > first && !t.model.is_some() {
        let o = t.overscan.max(0) as u64;
        let (a, b) = (first.saturating_sub(o), (end - 1 + o).min(len - 1));
        if t.asked != Some((a, b)) {
            t.asked = Some((a, b));
            events.push(("range", vec![Value::I64(a as i64), Value::I64(b as i64)]));
        }
    }
    let ids = item_keys::ids_value(t.sel.selected.iter().copied());
    if sel_changed {
        events.push(("selectionchange", vec![ids.clone()]));
    }
    if let Some(row) = activate {
        events.push(("activated", vec![Value::I64(row as i64)]));
    }
    if !toggled_rows.is_empty() {
        ctx.request_repaint();
    }
    for &(key, on) in &toggled_rows {
        events.push(("row-toggled", vec![Value::I64(key as i64), Value::Bool(on)]));
    }
    if let Some(col) = hout.clicked {
        events.push(("header-clicked", vec![Value::I64(col as i64)]));
    }
    if let Some(col) = hout.double {
        events.push(("header-double-clicked", vec![Value::I64(col as i64)]));
    }
    if copy && !t.sel.selected.is_empty() {
        events.push(("copy", vec![ids.clone()]));
    }
    for &(k, w) in &committed {
        events.push(("column-resized", vec![Value::I64(all[k].data as i64), Value::F64((w as f64 * 100.0).round() / 100.0)]));
    }
    // FM1 (QW4): sorting from the header, the column menu, the saved state
    let W::Table(t) = arena.w_mut(i) else { unreachable!() };
    let hev = header_after(t, &all, hout.clicked, toggled, !committed.is_empty(), &restored.props, &ctx);
    for (name, args) in events {
        emit(arena, i, name, args, out);
    }
    if cur_changed {
        crate::leaf::props_changed(arena, i, &[P::CurrentRow, P::CurrentColumn], out);
    }
    if sel_changed {
        crate::leaf::widget_changed(arena, i, &[P::SelectedIds], vec![ids], out);
    }
    hev.announce(arena, i, out);
}

/// The header row: `::part(header)`, a `::part(header-cell)` per visible column (clicks, the
/// tooltip, a11y ColumnHeader with the sort), its `::part(sort)` arrow, and the `::part(resize)`
/// edges (OW2's Resizable, R11). Widths that change live are kept in `TableRt::live` until the drag
/// commits.
#[allow(clippy::too_many_arguments)]
pub(crate) fn header(arena: &mut Arena, i: usize, all: &[Col], cols: &[Col], g: &Geom, enabled: bool, ui: &mut Ui, out: &mut Vec<Out>) -> HeaderOut {
    let _ = out;
    let node = arena.slots.id_at(i).expect("live node");
    let h = node.to_raw();
    let id = arena.core[i].egui_id;
    let ctx = ui.ctx().clone();
    let d = dark(ui);
    let mut o = HeaderOut::default();
    if g.head.height() <= 0.0 {
        return o;
    }
    let painter = ui.painter_at(g.head);
    let host_css = arena.style.computed(node).cloned();
    let host_state = arena.core[i].state;
    let host = StyleView::new(host_css.as_deref(), Ua(&[]), host_state, d);
    let hcss = arena.part_at(node, "header", host_state, (0, 1));
    let hv = host.part(hcss.as_deref(), HEADER_UA);
    let mut pc = PaintCx::for_node(&painter, h);
    pc.tag = "table";
    paint::paint_box(&mut pc.part("header"), &hv, g.head);
    let n = cols.len();
    let visible: Vec<usize> = g.visible_cols().collect();
    for &k in &visible {
        let c = &cols[k];
        let (a, b) = g.col_x(k);
        let (l, r) = g.clip_x(k);
        let cr = Rect::from_x_y_ranges(a..=b, g.head.y_range());
        let clip = Rect::from_x_y_ranges(l..=r, g.head.y_range());
        let resp = ui.interact(cr.intersect(clip), id.with(("head", c.data)), if enabled { Sense::click() } else { Sense::hover() });
        if !c.tooltip.is_empty() {
            let _ = crate::widgets::tooltip::attach(ui, &resp, &c.tooltip, true);
        }
        let mut bits = StateBits::NONE;
        bits.set(StateBits::HOVERED, enabled && resp.hovered());
        bits.set(StateBits::PRESSED, enabled && resp.is_pointer_button_down_on());
        bits.set(StateBits::DISABLED, !enabled);
        let W::Table(t) = arena.w(i) else { unreachable!() };
        let sort = shown_sort(t, c);
        let css = arena.part_at(node, "header-cell", bits, (k, n));
        let cv = hv.part(css.as_deref(), HEADER_CELL_UA).in_state(bits);
        let cp = painter.with_clip_rect(painter.clip_rect().intersect(clip));
        let mut pc = PaintCx::for_node(&cp, h);
        pc.tag = "table";
        let geom = paint::paint_box(&mut pc.part("header-cell"), &cv, cr);
        let mut content = geom.content;
        if sort != "none" {
            let mut sbits = bits;
            sbits.set(StateBits::CHECKED, sort == "descending");
            let scss = arena.part_at(node, "sort", sbits, (k, n));
            let sv = cv.part(scss.as_deref(), SORT_UA).in_state(sbits);
            let sw = sv.px("width").unwrap_or(16.0);
            let sr = Rect::from_min_max(pos2(content.right() - sw, content.top()), content.max);
            let key = IconKey::new("codicon", if sort == "ascending" { "arrow-up" } else { "arrow-down" });
            if let Some(key) = paint::content_icon(&sv, &key) {
                paint::paint_icon(&mut pc.part("sort"), &sv, &key, sr);
            }
            content.max.x -= sw;
        }
        paint_cell_text(&mut pc.part("header-cell"), &cv, &c.title, content, c.header_align, c.elide, &ctx);
        let title = c.title.clone();
        ctx.accesskit_node_builder(resp.id, |b| {
            b.set_role(Role::ColumnHeader);
            b.set_label(title);
            b.set_column_index(k);
            match sort {
                "ascending" => b.set_sort_direction(egui::accesskit::SortDirection::Ascending),
                "descending" => b.set_sort_direction(egui::accesskit::SortDirection::Descending),
                _ => {}
            }
        });
        if enabled {
            if resp.double_clicked() {
                o.double = Some(c.data);
            } else if resp.clicked() {
                o.clicked = Some(c.data);
            }
            if resp.secondary_clicked() {
                o.context = Some((c.data, resp.interact_pointer_pos().unwrap_or(cr.left_bottom())));
            }
        }
    }
    // the edges, over the cells: a line at rest, R11's hover bar, the resize
    let sash = StyleView::ua_only(RESIZE_UA, StateBits::NONE, d);
    let rcss = arena.style.part_computed(node, "resize").cloned();
    let policy = crate::native::policy_of(&ctx);
    for &k in &visible {
        let c = &cols[k];
        let (_, x) = g.col_x(k);
        let (l, r) = g.clip_x(k);
        if x < l || x > r + 0.5 {
            continue;
        }
        let rv = hv.part(rcss.as_deref(), RESIZE_UA);
        let line_w = rv.px("width").unwrap_or(1.0);
        let hit = rv.px("--hit").or_else(|| sash.px("--hit")).unwrap_or(4.0).max(line_w);
        let er = Rect::from_x_y_ranges((x - hit / 2.0)..=(x + hit / 2.0), g.head.y_range());
        let live = enabled && c.resizable;
        let resp = ui.interact(er, id.with(("resize", c.data)), if live { Sense::click_and_drag() } else { Sense::hover() });
        let W::Table(t) = arena.w_mut(i) else { unreachable!() };
        let slot = match t.rt.edges.iter().position(|e| e.0 == c.node) {
            Some(p) => p,
            None => {
                t.rt.edges.push((c.node, Resizable::default(), StateBits::NONE));
                t.rt.edges.len() - 1
            }
        };
        let p = ResizeParams { axis: 0, sign: 1.0, min: c.min, max: c.max, step: 10.0, collapsible: false, double_click: false };
        let mut fired = Intents::default();
        let mut shown = false;
        if live {
            let (_, unit, bits) = &mut t.rt.edges[slot];
            unit.set_size(c.width);
            crate::interact::run(ui, &resp, bits, &PartResponses::default(), |icx| unit.input(&p, icx, &mut fired));
            shown = unit.hover_shown();
            if unit.hovered() {
                ctx.set_cursor_icon(egui::CursorIcon::ResizeColumn);
            }
            if let Some(w) = unit.next_frame() {
                ctx.request_repaint_after(w);
            }
            let title = c.title.clone();
            ctx.accesskit_node_builder(resp.id, |nd| {
                unit.a11y(&p, nd);
                nd.set_label(format!("Resize column {title}"));
            });
        }
        let ai = all.iter().position(|a| a.node == c.node).unwrap_or(0);
        for f in fired.iter() {
            match *f {
                Resize::Start => {}
                Resize::Resize { size } => {
                    t.rt.live = Some((c.node, size));
                    ctx.request_repaint();
                }
                Resize::Commit { size } => {
                    t.rt.live = None;
                    o.committed.push((ai, size));
                }
                Resize::Cancel { .. } => t.rt.live = None,
            }
        }
        if live && resp.double_clicked() {
            t.rt.live = None;
            let W::Table(t) = arena.w(i) else { unreachable!() };
            o.committed.push((ai, fit_width(arena, i, t, &all[ai], &ctx, d)));
        }
        // paint: the 1 px line, and the hover bar once shown (R11)
        let W::Table(t) = arena.w(i) else { unreachable!() };
        let ebits = t.rt.edges[slot].2;
        let rv = hv.part(rcss.as_deref(), RESIZE_UA).in_state(ebits);
        let mut pc = PaintCx::for_node(&painter, h);
        pc.tag = "table";
        let mut pr = pc.part("resize");
        let lw = pr.snap_width(line_w);
        let line = pr.snap_rect(Rect::from_x_y_ranges((x - lw)..=x, g.head.y_range()));
        let col = rv.color("background-color").unwrap_or(Color32::GRAY);
        pr.painter.rect_filled(line, 0.0, col);
        pr.record(TraceKind::Box, line, col, Stroke::NONE, 0.0);
        if shown {
            let hw = rv.px("--hover-width").unwrap_or(4.0);
            let hc = rv.color("--hover-color").unwrap_or(Color32::LIGHT_BLUE);
            let bar = pr.snap_rect(Rect::from_x_y_ranges((x - hw / 2.0)..=(x + hw / 2.0), g.head.y_range()));
            pr.painter.rect_filled(bar, 0.0, hc);
            pr.record(TraceKind::Box, bar, hc, Stroke::NONE, 0.0);
        }
        let _ = policy.scroll;
    }
    o
}

/// The overlay bars (`::part(scrollbar-x|y)`, `::part(thumb-x|y)`), as a scroll area's, and their
/// AccessKit `ScrollBar`s.
fn paint_bars(arena: &mut Arena, i: usize, ui: &Ui, bars: &[Option<crate::interact::BarGeom>; 2], alpha: f32, d: bool) {
    use crate::widgets::scroll_area::{THUMB_UA, TRACK_UA};
    let node = arena.slots.id_at(i).expect("live node");
    let id = arena.core[i].egui_id;
    let host_css = arena.style.computed(node).cloned();
    for (ax, b) in bars.iter().enumerate() {
        let Some(b) = b else { continue };
        let (bn, tn) = if ax == 0 { ("scrollbar-x", "thumb-x") } else { ("scrollbar-y", "thumb-y") };
        let W::Table(t) = arena.w(i) else { unreachable!() };
        let (v, max, tbits) = (t.rt.scroll.offset()[ax] as f64, t.rt.scroll.max_offset()[ax] as f64, t.rt.thumbs[ax]);
        let track = b.track;
        ui.ctx().accesskit_node_builder(id.with(bn), |n| {
            bounds(n, track);
            n.set_role(Role::ScrollBar);
            n.set_orientation(if ax == 0 { egui::accesskit::Orientation::Horizontal } else { egui::accesskit::Orientation::Vertical });
            n.set_numeric_value(v);
            n.set_min_numeric_value(0.0);
            n.set_max_numeric_value(max);
        });
        if alpha <= 0.0 {
            continue;
        }
        let track_css = arena.part_at(node, bn, StateBits::NONE, (0, 1));
        let thumb_css = arena.part_at(node, tn, tbits, (0, 1));
        let host = StyleView::new(host_css.as_deref(), Ua(&[]), StateBits::NONE, d);
        let mut pc = PaintCx::for_node(ui.painter(), node.to_raw());
        pc.tag = "table";
        let track = host.part(track_css.as_deref(), TRACK_UA);
        let fill = track.color("background-color").unwrap_or(Color32::TRANSPARENT).gamma_multiply(alpha);
        let mut tp = pc.part(bn);
        if fill.a() > 0 {
            tp.painter.rect_filled(b.track, 0.0, fill);
        }
        tp.record(TraceKind::Box, b.track, fill, Stroke::NONE, 0.0);
        let thumb = host.part(thumb_css.as_deref(), THUMB_UA).in_state(tbits);
        let fill = thumb.color("background-color").unwrap_or(Color32::TRANSPARENT).gamma_multiply(alpha);
        let radius = thumb.px("border-radius").unwrap_or(0.0);
        let mut tp = pc.part(tn);
        tp.painter.rect_filled(b.thumb, radius, fill);
        tp.record(TraceKind::Box, b.thumb, fill, Stroke::NONE, 0.0);
    }
}

/// FM1 (QW4): the column menu (`column-menu`): a right click on the header raises
/// `header-context` (EV1 filters first) and, unless prevented, opens a menu of the `hideable`
/// columns as check items; the last shown column cannot be hidden. Own paint (the menu's UA
/// tables: `::part(menu)`, `::part(menu-item)`). Returns the key of a column it toggled.
fn column_menu(arena: &mut Arena, i: usize, all: &[Col], cols: &[Col], hout: &HeaderOut, ui: &mut Ui, out: &mut Vec<Out>) -> Option<String> {
    use crate::widgets::menu::{CHECK_UA, ITEM_UA, MENU_UA};
    let ctx = ui.ctx().clone();
    let id = arena.core[i].egui_id;
    let node = arena.slots.id_at(i).expect("live node");
    let pass = ctx.cumulative_pass_nr();
    if let Some((col, at)) = hout.context {
        let allowed = crate::widgets::list_rows::allowed(arena, i, "header-context", &ctx, out);
        emit(arena, i, "header-context", vec![Value::I64(col as i64)], out);
        let W::Table(t) = arena.w_mut(i) else { unreachable!() };
        if allowed && t.column_menu && all.iter().any(|c| c.hideable) {
            t.rt.menu = Some(ColumnMenu { at, pass });
        }
    }
    let W::Table(t) = arena.w(i) else { unreachable!() };
    let menu = t.rt.menu?;
    let hidden = t.hidden.clone();
    let d = dark(ui);
    let items: Vec<&Col> = all.iter().filter(|c| c.hideable).collect();
    let mcss = arena.style.part_computed(node, "menu").cloned();
    let host_css = arena.style.computed(node).cloned();
    let host = StyleView::new(host_css.as_deref(), Ua(&[]), StateBits::NONE, d);
    let mv = host.part(mcss.as_deref(), MENU_UA);
    let iv0 = mv.part(None, ITEM_UA);
    let check_w = mv.part(None, CHECK_UA).px("width").unwrap_or(16.0);
    let gap = iv0.px("gap").unwrap_or(6.0);
    let item_h = iv0.px("line-height").unwrap_or(22.0) + iv0.px("padding-top").unwrap_or(2.0) + iv0.px("padding-bottom").unwrap_or(2.0);
    let label = |c: &Col| if c.title.is_empty() { c.key.clone() } else { c.title.clone() };
    let text_w = items.iter().map(|c| paint::measure_text(&ctx, &iv0, &label(c), crate::provider::AvailableSpace::MaxContent).x).fold(0.0, f32::max);
    let extra = paint::box_extra(&mv);
    let w = (check_w + gap + text_w + paint::box_extra(&iv0).x + extra.x).max(mv.px("min-width").unwrap_or(160.0));
    let size = vec2(w, item_h * items.len() as f32 + extra.y);
    let screen = ctx.content_rect();
    let min = pos2(menu.at.x.min(screen.right() - size.x).max(screen.left()), menu.at.y.min(screen.bottom() - size.y).max(screen.top()));
    let rect = Rect::from_min_size(min, size);
    let mut toggled = None;
    let mut close = false;
    let shown_n = cols.len();
    crate::widgets::popup::area(&ctx, id.with("column-menu"), egui::Order::Foreground, rect, |mui| {
        let mresp = mui.interact(rect, id.with("column-menu-surface"), Sense::hover());
        let mpainter = mui.painter().clone();
        let mut pc = PaintCx::for_node(&mpainter, node.to_raw());
        pc.tag = "table";
        let geom = paint::paint_box(&mut pc.part("menu"), &mv, rect);
        mui.ctx().accesskit_node_builder(mresp.id, |b| {
            bounds(b, rect);
            b.set_role(Role::Menu);
            b.set_label("Columns");
        });
        let mi = mui.new_child(UiBuilder::new().max_rect(rect).accessibility_parent(mresp.id));
        for (j, c) in items.iter().enumerate() {
            let ir = Rect::from_min_size(pos2(geom.content.left(), geom.content.top() + j as f32 * item_h), vec2(geom.content.width(), item_h));
            let on = !hidden.contains(&c.key) && c.shown;
            let last = on && shown_n <= 1;
            let iresp = mi.interact(ir, id.with(("column-menu", j)), if last { Sense::hover() } else { Sense::click() });
            let mut bits = StateBits::NONE;
            bits.set(StateBits::CURRENT, iresp.hovered());
            bits.set(StateBits::CHECKED, on);
            bits.set(StateBits::DISABLED, last);
            let icss = arena.part_at(node, "menu-item", bits, (j, items.len()));
            let iv = mv.part(icss.as_deref(), ITEM_UA).in_state(bits);
            let ig = paint::paint_box(&mut pc.part("menu-item"), &iv, ir);
            let cv = iv.part(None, CHECK_UA);
            if on {
                let cr = Rect::from_min_size(ig.content.min, vec2(check_w, ig.content.height()));
                paint::paint_icon(&mut pc.part("menu-item"), &cv, &IconKey::new("codicon", "check"), cr);
            }
            let tr = ig.content.with_min_x(ig.content.left() + check_w + gap);
            paint_cell_text(&mut pc.part("menu-item"), &iv, &label(c), tr, "start", None, &ctx);
            let name = label(c);
            mi.ctx().accesskit_node_builder(iresp.id, |b| {
                b.set_role(Role::MenuItemCheckBox);
                b.set_label(name);
                b.set_toggled(if on { egui::accesskit::Toggled::True } else { egui::accesskit::Toggled::False });
                if last {
                    b.set_disabled();
                }
            });
            if iresp.clicked() {
                toggled = Some(c.key.clone());
                close = true;
            }
        }
    });
    if pass > menu.pass
        && crate::widgets::popup::dismissed(&ctx, &DismissParams { inside: vec![rect], on_outside: crate::interact::dismiss::Outside::Close, escape: true }).is_some()
    {
        close = true;
    }
    if close {
        let W::Table(t) = arena.w_mut(i) else { unreachable!() };
        t.rt.menu = None;
        ctx.request_repaint();
    }
    toggled
}

/// FM1 (QW4): a `header-state` the host set, applied to `t`.
#[derive(Default)]
struct Restored {
    /// The props it changed.
    props: Vec<P>,
    /// (column index, width).
    widths: Vec<(usize, f32)>,
}

/// FM1 (QW4): applies a `header-state` the host set to `t` (hidden columns and sort; the widths
/// are returned for the columns).
fn restore_header(t: &mut TableState, all: &[Col]) -> Restored {
    let mut out = Restored::default();
    let Some(st) = t.restore.take() else { return out };
    if let Some(h) = st.get("hidden").and_then(|h| h.as_array()) {
        let hidden: Vec<String> = h.iter().filter_map(|x| x.as_str().map(str::to_string)).collect();
        if hidden != t.hidden {
            t.hidden = hidden;
            out.props.push(P::HiddenColumns);
        }
    }
    let sort = st.get("sort").and_then(|x| {
        let col = all.iter().find(|c| Some(c.key.as_str()) == x.get("column").and_then(|c| c.as_str()))?;
        let order = kinds::SORT.iter().copied().find(|o| Some(*o) == x.get("order").and_then(|o| o.as_str()))?;
        Some((col.data as i64, order))
    });
    let (sc, so) = sort.unwrap_or((-1, "none"));
    if (sc, so) != (t.sort_column, t.sort_order) {
        (t.sort_column, t.sort_order) = (sc, so);
        out.props.extend([P::SortColumn, P::SortOrder]);
    }
    if let Some(w) = st.get("widths").and_then(|w| w.as_object()) {
        for (k, c) in all.iter().enumerate() {
            if let Some(px) = w.get(&c.key).and_then(|x| x.as_f64()) {
                out.widths.push((k, px as f32));
            }
        }
    }
    out
}

/// FM1 (QW4): what the header changed this pass, announced after the table's other events.
struct HeaderEvents {
    sorted: Option<Vec<Value>>,
    hidden: Option<Value>,
    state: Option<String>,
}

/// FM1 (QW4): after the table ran: a header click on a `sortable` table sorts (or flips), a column
/// menu toggle hides or shows, a committed resize; each user change writes `header-state` (a
/// restored one is not echoed).
fn header_after(t: &mut TableState, all: &[Col], header: Option<usize>, toggled: Option<String>, resized: bool, restored: &[P], ctx: &egui::Context) -> HeaderEvents {
    let mut sorted = restored.contains(&P::SortColumn);
    let sort_click = header.filter(|_| t.sortable);
    if let Some(col) = sort_click {
        let flip = t.sort_column == col as i64 && t.sort_order == "ascending";
        t.sort_column = col as i64;
        t.sort_order = if flip { "descending" } else { "ascending" };
        sorted = true;
    }
    let mut hid = restored.contains(&P::HiddenColumns);
    let menu = toggled.is_some();
    if let Some(key) = toggled {
        if let Some(k) = t.hidden.iter().position(|x| *x == key) {
            t.hidden.remove(k);
        } else {
            t.hidden.push(key);
        }
        hid = true;
    }
    let mut state = None;
    if sort_click.is_some() || menu || resized {
        let s = header_state(all, t);
        if s != t.header_state {
            t.header_state = s.clone();
            state = Some(s);
        }
    }
    if sorted || hid {
        ctx.request_repaint();
    }
    HeaderEvents {
        sorted: sorted.then(|| vec![Value::I64(t.sort_column), Value::Str(t.sort_order.to_string())]),
        hidden: hid.then(|| Value::List(t.hidden.iter().map(|x| Value::Str(x.clone())).collect())),
        state,
    }
}

impl HeaderEvents {
    fn announce(self, arena: &mut Arena, i: usize, out: &mut Vec<Out>) {
        if let Some(args) = self.sorted {
            changed_via(arena, i, &[P::SortColumn, P::SortOrder], "sort-changed", args, out);
        }
        if let Some(h) = self.hidden {
            changed_via(arena, i, &[P::HiddenColumns], "columns-changed", vec![h], out);
        }
        if let Some(s) = self.state {
            changed_via(arena, i, &[P::HeaderState], "header-state-changed", vec![Value::Str(s)], out);
        }
    }
}

/// FM1 (QW4): the `header-state` JSON of the columns `all`, their hidden ones and the sort.
fn header_state(all: &[Col], t: &TableState) -> String {
    let w: serde_json::Map<String, serde_json::Value> = all.iter().map(|c| (c.key.clone(), serde_json::json!((c.width as f64 * 100.0).round() / 100.0))).collect();
    let sort = all
        .iter()
        .find(|c| t.sort_column >= 0 && c.data == t.sort_column as usize)
        .map_or(serde_json::Value::Null, |c| serde_json::json!({"column": c.key, "order": t.sort_order}));
    serde_json::json!({"widths": w, "hidden": t.hidden, "sort": sort}).to_string()
}


/// The built-in editor's text inset (the seam's egui `TextEdit`).
const CELL_MARGIN: f32 = 4.0;

#[allow(clippy::disallowed_types)] // OW: wrapper until OW4 (the built-in cell editor is the one egui widget left; OW5b seam)
/// Draws the open edit's editor over its cell (the built-in text field, or the column's snippet
/// outlet), moves focus into it once, and turns Enter/Escape/Tab and focus leaving it into an end
/// request (handled in logic). Returns whether another frame is needed for it.
fn paint_editor(arena: &mut Arena, i: usize, cols: &[Col], cells: &[(u64, usize, Rect, Rect)], ui: &mut Ui, out: &mut Vec<Out>) -> bool {
    let table = arena.slots.id_at(i).expect("live node");
    let id = arena.core[i].egui_id;
    let ctx = ui.ctx().clone();
    let (place, outlet, title) = {
        let W::Table(t) = arena.w_mut(i) else { unreachable!() };
        let Some(e) = &t.edit else { return false };
        if e.snippet && !e.bound {
            return true;
        }
        let (row, col) = (e.row, e.col);
        let pos = t.pos_of(row);
        let cell = pos.and_then(|p| cells.iter().find(|c| c.0 == p && c.1 == col)).map(|c| (c.2, c.3));
        let outlet = t.editors.iter().find(|(c, _)| Some(*c) == cols.get(col).map(|c| c.node)).map(|x| x.1);
        let e = t.edit.as_mut().expect("an edit");
        if let Some((cr, _)) = cell
            && (cr.size() - e.size).length() > 0.5
        {
            e.size = cr.size();
            arena.invalidate_content(table);
        }
        (cell, outlet, cols.get(col).map_or(String::new(), |c| c.title.clone()))
    };
    // the cell is not drawn (scrolled away): the edit waits
    let Some((cr, clip)) = place else { return false };
    let mut eui = ui.new_child(UiBuilder::new().max_rect(cr).id_salt(("cell-editor", id)).layout(Layout::left_to_right(Align::Center)));
    eui.set_clip_rect(clip.intersect(ui.clip_rect()));
    let W::Table(t) = arena.w_mut(i) else { unreachable!() };
    let e = t.edit.as_mut().expect("an edit");
    let mut widgets: Vec<Id> = Vec::new();
    if !e.snippet {
        let eid = id.with(("cell-editor", e.row, e.col));
        if e.focus {
            ctx.memory_mut(|m| m.request_focus(eid));
            e.focus = false;
        }
        egui::Frame::new().fill(eui.visuals().extreme_bg_color).show(&mut eui, |ui| {
            let te = egui::TextEdit::singleline(&mut e.draft).id(eid).desired_width(f32::INFINITY).margin(vec2(CELL_MARGIN, 1.0)).frame(egui::Frame::NONE);
            let r = ui.add_sized(cr.size(), te);
            ui.ctx().accesskit_node_builder(r.id, |b| b.set_label(title.clone()));
        });
        widgets.push(eid);
    } else if e.bound
        && let Some(o) = outlet
        && let Some(oi) = arena.idx(o)
    {
        let layer = eui.layer_id();
        let mark = crate::scene::virt::widgets_in_layer(&ctx, layer);
        let laid = arena.core[oi].rect;
        if laid.width() > 0.0 {
            let at = cr.min.to_vec2() - laid.min.to_vec2();
            // the editor's own events and prop changes (its `bind:`s) go out with the table's
            crate::walk::paint_at(arena, oi, at, &mut eui, out);
            arena.core[oi].drawn = cr;
        }
        widgets = ctx.viewport(|vp| vp.this_pass.widgets.get_layer(layer).skip(mark).filter(|w| w.sense.is_focusable() && w.enabled).map(|w| w.id).collect());
        let W::Table(t) = arena.w_mut(i) else { unreachable!() };
        let e = t.edit.as_mut().expect("an edit");
        if e.focus
            && let Some(&first) = widgets.first()
        {
            ctx.memory_mut(|m| m.request_focus(first));
            e.focus = false;
        }
    }
    let W::Table(t) = arena.w_mut(i) else { unreachable!() };
    let e = t.edit.as_mut().expect("an edit");
    let has = widgets.iter().any(|w| ctx.memory(|m| m.has_focus(*w)));
    let mine = has || widgets.iter().any(|w| ctx.memory(|m| m.had_focus_last_frame(*w)));
    e.had_focus |= has;
    e.widgets = widgets;
    if mine && e.end.is_none() {
        use egui::{Event, Key};
        let key = ctx.input(|inp| {
            inp.events.iter().find_map(|ev| match ev {
                Event::Key { key: Key::Enter, pressed: true, .. } => Some((true, 0)),
                Event::Key { key: Key::Escape, pressed: true, .. } => Some((false, 0)),
                Event::Key { key: Key::Tab, pressed: true, modifiers, .. } => Some((true, if modifiers.shift { -1 } else { 1 })),
                _ => None,
            })
        });
        if let Some(k) = key {
            e.end = Some(k);
            // the keys belong to the edit
            ctx.input_mut(|inp| inp.events.retain(|ev| !matches!(ev, Event::Key { key: Key::Enter | Key::Escape | Key::Tab, .. })));
        }
    }
    if e.end.is_none() && e.had_focus && !has && !e.focus {
        // focus left the editor: commit (Qt's default)
        e.end = Some((true, 0));
    }
    // another frame when logic has JS work (a bind, an end) or focus is still to move in
    e.end.is_some() || (e.snippet && !e.bound) || e.focus
}

// ---- LS2: rows from a Rust model (SV41) ----

impl SceneMut<'_> {
    /// Makes `table` show rows from a Rust model (LS2, SV41) instead of the JS window: no `range`
    /// events and no JS call per page. `view` holds the filter and the tree rows' expansion (pass
    /// `RowView::new()`); the table's sort (`sort-column`/`sort-order`, a header click on a
    /// `sortable` table) sorts it in Rust. Selection, the cursor and edits stay keyed by
    /// [`RowModel::key`]. A table with no `table-column` children gets one per
    /// [`RowModel::columns`] (Rust-owned, `name` = the column's key); declared columns map to
    /// model columns by `name`, else by index.
    pub fn set_table_model(&mut self, table: NodeId, model: impl RowModel, view: RowView) -> Result<(), SceneError> {
        let i = self.s.idx(table).ok_or(SceneError::Stale(table.to_raw()))?;
        if !matches!(self.s.w(i), W::Table(_)) {
            return Err(SceneError::Invalid("not a <table>".into()));
        }
        let has_cols = self.s.children[i].iter().any(|&c| matches!(self.s.w(self.s.ix(c)), W::TableColumn(_)));
        if !has_cols {
            for c in model.columns() {
                let n = self.create_tag("table-column")?;
                let mut nm = self.node(n).expect("created");
                let set = |nm: &mut crate::scene::NodeMut<'_>, k: &str, v: Value| nm.set_by_name(k, &v).map_err(|e| SceneError::Invalid(e.to_string()));
                set(&mut nm, "title", Value::Str(c.header.clone()))?;
                set(&mut nm, "name", Value::Str(c.key.clone()))?;
                if let Some(w) = c.width {
                    set(&mut nm, "width", Value::F64(w as f64))?;
                }
                if c.kind == CellKind::Number {
                    set(&mut nm, "align", Value::Str("end".into()))?;
                }
                // the table may be the runtime's: its model columns are the model's (as list rows)
                self.s.adopt(table, n, self.origin)?;
            }
        }
        let W::Table(t) = self.s.w_mut(i) else { unreachable!() };
        t.model = ModelSlot(Some(Box::new(RustTable { model: Box::new(model), view, cols: Vec::new() })));
        t.rows.clear();
        t.first = 0;
        t.asked = None;
        self.s.invalidate_content(table);
        Ok(())
    }

    /// The Rust model of `table`, typed, and its view (to change rows, the filter or expansion):
    /// the table redraws; a model whose [`RowModel::version`] changed is re-sorted and filtered.
    pub fn with_table_model<T: RowModel, R>(&mut self, table: NodeId, f: impl FnOnce(&mut T, &mut RowView) -> R) -> Option<R> {
        let i = self.s.idx(table)?;
        let W::Table(t) = self.s.w_mut(i) else { return None };
        let m = t.model.0.as_deref_mut()?;
        let any: &mut dyn std::any::Any = &mut *m.model;
        let r = f(any.downcast_mut::<T>()?, &mut m.view);
        self.s.invalidate_content(table);
        Some(r)
    }

    /// Back to the JS window (`range` events start again).
    pub fn clear_table_model(&mut self, table: NodeId) -> Result<(), SceneError> {
        let i = self.s.idx(table).ok_or(SceneError::Stale(table.to_raw()))?;
        let W::Table(t) = self.s.w_mut(i) else { return Err(SceneError::Invalid("not a <table>".into())) };
        t.model = ModelSlot(None);
        t.asked = None;
        self.s.invalidate_content(table);
        Ok(())
    }
}

impl SceneRef<'_> {
    /// Reads the Rust model of `table` and its view (positions as shown: `view.row(pos)`,
    /// `view.pos_of(key)`).
    pub fn table_model<T: RowModel, R>(self, table: NodeId, f: impl FnOnce(&T, &RowView) -> R) -> Option<R> {
        let W::Table(t) = self.s.w(self.s.idx(table)?) else { return None };
        let m = t.model.0.as_deref()?;
        let any: &dyn std::any::Any = &*m.model;
        Some(f(any.downcast_ref::<T>()?, &m.view))
    }
}

// ---- table-column methods (the JS runtime's editor protocol) ----

/// `set-editor(outlet)` and `end-edit(commit)` on a `table-column` (`views::call`).
pub(crate) fn column_call(s: &mut Scene, column: NodeId, name: &str, args: &[Value]) -> StrResult<Value> {
    let ci = s.ix(column);
    let table = s.core[ci].parent.filter(|&p| s.idx(p).is_some_and(|pi| matches!(s.w(pi), W::Table(_))));
    match name {
        "set-editor" => {
            arity("set-editor", args, 1, 1)?;
            let Some(table) = table else { return Err("set-editor: the column is not in a <table>".into()) };
            let outlet = match &args[0] {
                Value::Handle(w) => Some(s.live(*w)?),
                Value::I64(w) if *w > 0 => Some(s.live(*w as u64)?),
                Value::Null => None,
                other => return Err(format!("set-editor: expected a widget, got {other:?}")),
            };
            let ti = s.ix(table);
            if let Some(o) = outlet
                && s.idx(o).and_then(|oi| s.core[oi].parent) != Some(table)
            {
                s.adopt(table, o, Origin::Js).map_err(|e| e.to_string())?;
            }
            let W::Table(t) = s.w_mut(ti) else { unreachable!() };
            t.editors.retain(|(c, _)| *c != column);
            if let Some(o) = outlet {
                t.editors.push((column, o));
            }
            s.invalidate_content(table);
            Ok(Value::Null)
        }
        "end-edit" => {
            let Some(table) = table else { return Ok(Value::Bool(false)) };
            let ti = s.ix(table);
            let W::Table(t) = s.w_mut(ti) else { unreachable!() };
            if t.edit.as_ref().is_some_and(|e| e.column != column) {
                return Ok(Value::Bool(false));
            }
            t.end_edit(args)
        }
        other => Err(format!("method {other} is not implemented")),
    }
}

// ---- logic: editor binds and commits (sync JS calls, never in the pass) ----

/// What a table's edit needs from JS this logic.
enum Step {
    Bind { column: NodeId, row: u64, value: String, typed: String },
    End { column: NodeId, row: u64, col: usize, snippet: bool, draft: String, commit: bool, step: i32 },
}

impl EguiBackend {
    /// Step 5c of `logic` (after `realize_lists`): every table with an edit to bind (its column's
    /// `editor-bind`) or to end (`editor-value`, the table's `commit` and `editor-unbind`).
    pub(crate) fn realize_edits(&self) {
        let Some(k) = kinds::tag_index("table") else { return };
        let tables: Vec<NodeId> = {
            let scene = self.0.scene.borrow();
            scene
                .index
                .of_kind(k)
                .iter()
                .copied()
                .filter(|&n| matches!(scene.w(scene.ix(n)), W::Table(t) if t.edit.as_ref().is_some_and(|e| e.end.is_some() || (e.snippet && !e.bound))))
                .collect()
        };
        for n in tables {
            // a bind, then (the same logic) an end that was already asked for
            for _ in 0..2 {
                if !self.edit_step(n) {
                    break;
                }
            }
        }
    }

    /// One JS round of table `n`'s edit; false when there is nothing to do.
    fn edit_step(&self, n: NodeId) -> bool {
        let Some(host) = self.host() else { return false };
        let (step, root, table_kind) = {
            let scene = self.0.scene.borrow();
            let Some(i) = scene.idx(n) else { return false };
            let W::Table(t) = scene.w(i) else { return false };
            let Some(e) = &t.edit else { return false };
            let step = if let Some((commit, step)) = e.end {
                Step::End { column: e.column, row: e.row, col: e.col, snippet: e.snippet && e.bound, draft: e.draft.clone(), commit, step }
            } else if e.snippet && !e.bound {
                Step::Bind { column: e.column, row: e.row, value: e.value.clone(), typed: e.typed.clone() }
            } else {
                return false;
            };
            (step, scene.core[i].root, scene.core[i].kind as usize)
        };
        let col_kind = kinds::tag_index("table-column").unwrap_or(0);
        let listened = |node: NodeId, kind: usize, name: &str| {
            let s = kinds::signal_id(kind, name);
            let scene = self.0.scene.borrow();
            scene.idx(node).is_some_and(|i| scene.listens(i, s)).then_some(s)
        };
        match step {
            Step::Bind { column, row, value, typed } => {
                if let Some(s) = listened(column, col_kind, "editor-bind") {
                    let _ = host.call_sync(column.to_raw(), s, &[Value::I64(row as i64), Value::Str(value), Value::Str(typed)]);
                }
                let mut scene = self.0.scene.borrow_mut();
                if let Some(i) = scene.idx(n)
                    && let W::Table(t) = scene.w_mut(i)
                    && let Some(e) = &mut t.edit
                {
                    e.bound = true;
                }
                scene.invalidate_content(n);
            }
            Step::End { column, row, col, snippet, draft, commit, step } => {
                let mut verdict: Option<String> = None;
                let mut value = Value::Str(draft);
                if commit {
                    if snippet
                        && let Some(s) = listened(column, col_kind, "editor-value")
                        && let Ok(v) = host.call_sync(column.to_raw(), s, &[])
                    {
                        value = v;
                    }
                    if let Some(s) = listened(n, table_kind, "commit") {
                        // null (or nothing) accepts; a string rejects with that reason, true without one
                        match host.call_sync(n.to_raw(), s, &[Value::I64(row as i64), Value::I64(col as i64), value.clone()]) {
                            Ok(Value::Str(reason)) => verdict = Some(reason),
                            Ok(Value::Bool(true)) => verdict = Some("The value was not accepted.".into()),
                            _ => {}
                        }
                    }
                }
                if snippet && let Some(s) = listened(column, col_kind, "editor-unbind") {
                    let _ = host.call_sync(column.to_raw(), s, &[]);
                }
                let mut scene = self.0.scene.borrow_mut();
                let Some(i) = scene.idx(n) else { return false };
                let W::Table(t) = scene.w_mut(i) else { return false };
                t.edit = None;
                t.refocus = true;
                let pos = t.pos_of(row);
                match verdict {
                    Some(reason) => t.error = Some(EditError { row, col, reason, until: None, shown_at: None }),
                    None if commit => {
                        if let Some(p) = pos
                            && let Err(reason) = t.set_cell(p, t.data_col(col), cell_text(&value))
                        {
                            t.error = Some(EditError { row, col, reason, until: None, shown_at: None });
                        }
                    }
                    None => {}
                }
                // Tab: the next editable cell (its columns are read from the scene)
                if commit && t.error.is_none() && step != 0 && let Some(p) = pos {
                    let len = t.len();
                    let editable: Vec<bool> = scene.children[i]
                        .iter()
                        .filter_map(|&c| match scene.w(scene.ix(c)) {
                            W::TableColumn(c) => Some(c.editable),
                            _ => None,
                        })
                        .collect();
                    // the shown columns only (Tab skips hidden ones; `col` counts shown columns)
                    let W::Table(t) = scene.w(i) else { return false };
                    let shown: Vec<bool> = if t.col_map.is_empty() { editable } else { t.col_map.iter().map(|&k| editable.get(k).copied().unwrap_or(false)).collect() };
                    if let Some((np, nc)) = next_editable(&shown, len, p, col, step) {
                        let W::Table(t) = scene.w_mut(i) else { return false };
                        t.pending_edit = Some((t.key_of(np), nc));
                    }
                }
                scene.invalidate_content(n);
            }
        }
        self.touched(Some(root));
        true
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tab_order_walks_editable_cells_row_major() {
        let cols = [false, true, true];
        assert_eq!(next_editable(&cols, 3, 0, 1, 1), Some((0, 2)));
        assert_eq!(next_editable(&cols, 3, 0, 2, 1), Some((1, 1)));
        assert_eq!(next_editable(&cols, 3, 1, 1, -1), Some((0, 2)));
        assert_eq!(next_editable(&cols, 3, 0, 1, -1), None);
        assert_eq!(next_editable(&cols, 3, 2, 2, 1), None);
    }

    #[test]
    fn ids_keep_selection_across_set_ids() {
        let mut t = TableState { row_count: 4, ..TableState::default() };
        assert_eq!((t.key_of(2), t.pos_of(3)), (2, Some(3)));
        t.set_ids(&[item_keys::ids_value([40, 30, 20, 10]), Value::List(["d", "c", "b", "a"].map(|s| Value::Str(s.into())).to_vec())]).unwrap();
        t.set(P::SelectedIds, &item_keys::ids_value([20])).unwrap();
        assert_eq!(t.pos_of(20), Some(2));
        // re-sorted: same id, new position
        t.set_ids(&[item_keys::ids_value([10, 20, 30, 40])]).unwrap();
        assert!(t.sel.is_selected(20));
        assert_eq!((t.pos_of(20), t.len()), (Some(1), 4));
        assert!(t.set_ids(&[item_keys::ids_value([1, 2]), Value::List(vec![Value::Str("x".into())])]).is_err());
    }

    #[test]
    fn geometry_stretches_the_last_column_and_keeps_sticky_ones() {
        let r = Rect::from_min_size(pos2(0.0, 0.0), vec2(300.0, 124.0));
        let mut g = Geom::new(r, 24.0, &[100.0, 80.0], 1, true, 20.0);
        assert_eq!((g.ws[1], g.total), (200.0, 300.0), "stretch-last fills the width");
        let g2 = Geom::new(r, 24.0, &[100.0, 80.0], 1, false, 20.0);
        assert_eq!(g2.total, 180.0);
        // scrolled: the sticky column stays, the other moves under it
        g.ws = vec![100.0, 400.0];
        g.total = 500.0;
        g.off = vec2(50.0, 30.0);
        assert_eq!(g.col_x(0), (0.0, 100.0));
        assert_eq!(g.col_x(1), (50.0, 450.0));
        assert_eq!(g.clip_x(1).0, 100.0);
        assert_eq!(g.col_at(80.0), Some(0), "the sticky column covers the scrolled one");
        assert_eq!(g.col_at(120.0), Some(1));
        // rows: 30 points scrolled, row 1 starts 10 below the header
        assert_eq!(g.row_at(24.0 + 5.0, 100), Some(1));
        assert_eq!(g.row_rect(1).top(), 24.0 - 10.0);
        assert_eq!(g.row_at(10.0, 100), None, "the header is not a row");
    }

    #[test]
    fn header_state_round_trips_widths_hidden_and_sort() {
        let col = |k: usize, key: &str, w: f32| Col {
            node: NodeId::from_raw(k as u64 + 1).unwrap(),
            ci: k,
            title: key.to_uppercase(),
            width: w,
            min: MIN_COL,
            max: f32::INFINITY,
            resizable: true,
            align: "start",
            header_align: "start",
            sort: "none",
            editable: false,
            editor: false,
            data: k,
            key: key.to_string(),
            hideable: true,
            shown: true,
            tooltip: String::new(),
            elide: None,
        };
        let all = [col(0, "name", 120.0), col(1, "1", 80.5)];
        let mut t = TableState { sort_column: 1, sort_order: "descending", hidden: vec!["name".into()], ..TableState::default() };
        let s = header_state(&all, &t);
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["widths"]["name"], 120.0);
        assert_eq!(v["sort"]["column"], "1");
        // restoring it elsewhere: hidden, sort and widths come back (the props it changed listed)
        let mut u = TableState::default();
        u.set(P::HeaderState, &Value::Str(s)).unwrap();
        let r = restore_header(&mut u, &all);
        assert_eq!((u.hidden.clone(), u.sort_column, u.sort_order), (vec!["name".to_string()], 1, "descending"));
        assert_eq!(r.widths, vec![(0, 120.0), (1, 80.5)]);
        assert!(r.props.contains(&P::HiddenColumns) && r.props.contains(&P::SortColumn));
        // a header click on a sortable table sorts, then flips
        t.sortable = true;
        t.sort_column = -1;
        let ctx = egui::Context::default();
        let ev = header_after(&mut t, &all, Some(0), None, false, &[], &ctx);
        assert_eq!((t.sort_column, t.sort_order), (0, "ascending"));
        assert!(ev.sorted.is_some() && ev.state.is_some());
        header_after(&mut t, &all, Some(0), None, false, &[], &ctx);
        assert_eq!(t.sort_order, "descending");
        // the column menu shows a hidden column again
        let ev = header_after(&mut t, &all, None, Some("name".into()), false, &[], &ctx);
        assert!(t.hidden.is_empty() && ev.hidden.is_some());
    }
}
