//! `table` and `table-column` (WG4a), on `egui_table` 0.11 (virtual rows and columns), with WG5a's
//! keyboard, keyed selection and cell editing.
//!
//! Columns are the table's `table-column` children (data only: hidden by the table's layout).
//! Rows are a JS data window, the data half of the virtual-list protocol (M5 / N3): each frame the
//! table works out the rows it shows plus `overscan` on each side and, when that range changes,
//! emits `range(first, last)`; JS answers with `set-rows(first, rows)`, which replaces the window.
//! Rows outside the window draw empty until their data arrives (with the default overscan of 20
//! rows, a scroll of up to a page shows data already sent). A small table can send everything
//! once: `set-rows(0, all)`.
//!
//! **Rows have ids (WG5a).** `set-ids(ids, labels?)` gives every row (by position) an id and,
//! optionally, a type-ahead label; without it a row's id is its position. Selection (`selected`,
//! bindable), the cursor (`current`) and edits are keyed by id ([`KeySelect`], WG5b's shared
//! helper), so a re-sort or a filter (a new `set-ids`) keeps them on the same rows.
//!
//! **Keyboard.** A click focuses the table. Focused: Up/Down/PageUp/PageDown/Home/End move the
//! cursor row (Shift extends, Ctrl moves alone), Left/Right the cursor column, Space and Ctrl+A
//! select, Enter activates, F2 or typing edits the cursor cell when its column is `editable`
//! (typing elsewhere is type-ahead on the labels, else on the first column's window text).
//!
//! **Editing.** An edit starts on F2, a typed key, a double click on an editable cell or the
//! `edit(row-id, column)` method. The editor is the column's `editor` snippet (realised once per
//! column by the JS runtime into an outlet the table adopts, then rebound per edit:
//! `editor-bind`/`editor-value`/`editor-unbind` on the column), or a built-in text field. Enter
//! commits, Escape reverts, Tab/Shift+Tab commit and edit the next/previous editable cell, focus
//! leaving the editor commits. A commit is the sync `commit(row-id, column, value)` the host can
//! reject (`e.result = 'reason'` or `e.preventDefault()`): the edit then reverts and the reason
//! shows under the cell. The JS calls run in `logic` ([`EguiBackend::realize_edits`]), never in
//! the pass.

use super::item_keys::{self, Input, RowKeys};
use super::selection::{KeySelect, Mode, Mods};
use super::{arg, arity};
use crate::backend::EguiBackend;
use crate::kinds::{self, P};
use crate::layout::Metrics;
use crate::scene::props::{as_bool, as_enum, as_f64, as_i64, as_str, as_str_list};
use crate::scene::{Arena, Dirty, NodeId, Origin, Scene, W};
use crate::walk::Out;
use egui::{Align, Id, Layout, Rect, Role, Sense, Ui, UiBuilder, Vec2, vec2};
use egui_table::{CellInfo, Column, HeaderCellInfo, HeaderRow, PrefetchInfo, TableDelegate};
use std::borrow::Cow;
use svelters_abi::{StrResult, Value};

/// How long a rejected commit's reason stays under its cell, in seconds.
const ERROR_SECONDS: f64 = 5.0;

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
    /// Per column (by its node's egui id): the `width` prop last written into egui_table's state
    /// (a user resize stays until the prop changes).
    pub applied: Vec<(Id, f64)>,
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
    // ---- FM1 (QW4): the header (plans/milestones/FM1-form-widgets.md) ----
    pub sortable: bool,
    pub sort_column: i64,
    pub sort_order: &'static str,
    pub column_menu: bool,
    /// `hidden-columns`: column names (or child indices).
    pub hidden: Vec<String>,
    pub header_state: String,
    /// A `header-state` the host set, applied at the next paint (it needs the columns).
    pub restore: Option<serde_json::Value>,
    /// The widths egui_table held before the last `show`, by column id (a user resize differs).
    pub seen: Vec<(Id, f32)>,
    /// A resize happened; `header-state-changed` once the pointer is released.
    pub resized: bool,
    /// The data column of each shown column (hidden columns are left out of egui_table; WG5a's
    /// `cur_col` and edits count shown columns).
    pub col_map: Vec<usize>,
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
            applied: Vec::new(),
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
            seen: Vec::new(),
            resized: false,
            col_map: Vec::new(),
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
                self.selection = crate::scene::props::as_enum(v, kinds::TABLE_SELECTION)?;
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
            P::HiddenColumns => self.hidden = as_str_list(v)?,
            P::HeaderState => {
                let s = as_str(v)?;
                self.restore = if s.is_empty() {
                    None
                } else {
                    Some(serde_json::from_str::<serde_json::Value>(&s).map_err(|e| format!("header-state: {e}"))?)
                };
                self.header_state = s;
            }
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
            _ => return None,
        })
    }

    /// Rows in the table: the ids given by `set-ids`, else `row-count`.
    pub fn len(&self) -> u64 {
        self.keys.as_ref().map_or(self.row_count.max(0) as u64, |k| k.ids.len() as u64)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The id of the row at `pos`.
    pub fn key_of(&self, pos: u64) -> u64 {
        self.keys.as_ref().and_then(|k| k.ids.get(pos as usize).copied()).unwrap_or(pos)
    }

    /// The position of the row with id `key`.
    pub fn pos_of(&self, key: u64) -> Option<u64> {
        match &self.keys {
            Some(k) => k.ids.iter().position(|&x| x == key).map(|p| p as u64),
            None => (key < self.len()).then_some(key),
        }
    }

    fn all_ids(&self) -> Cow<'_, [u64]> {
        match &self.keys {
            Some(k) => Cow::Borrowed(&k.ids),
            None => Cow::Owned((0..self.len()).collect()),
        }
    }

    /// Type-ahead text of the row at `pos`: its `set-ids` label, else its first cell (window only).
    fn label(&self, pos: usize) -> Option<String> {
        self.keys.as_ref().and_then(|k| k.label(pos)).or_else(|| self.cell(pos as u64, 0).map(str::to_string))
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

    /// The text of cell (`row`, `col`), if its row is in the window.
    /// FM1: the data column of shown column `col` (the same while no column is hidden).
    pub fn data_col(&self, col: usize) -> usize {
        self.col_map.get(col).copied().unwrap_or(col)
    }

    pub fn cell(&self, row: u64, col: usize) -> Option<&str> {
        let k = row.checked_sub(self.first)? as usize;
        self.rows.get(k)?.get(col).map(String::as_str)
    }

    fn set_cell(&mut self, row: u64, col: usize, text: String) {
        let Some(k) = row.checked_sub(self.first) else { return };
        if let Some(r) = self.rows.get_mut(k as usize) {
            if r.len() <= col {
                r.resize(col + 1, String::new());
            }
            r[col] = text;
        }
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

/// One column, from its `table-column` node.
#[derive(Clone)]
struct Col {
    node: NodeId,
    id: Id,
    title: String,
    width: f64,
    resizable: bool,
    align: &'static str,
    sort: &'static str,
    editable: bool,
    editor: bool,
    /// FM1: its child index (the data column, `header-clicked`'s and `sort-column`'s number)
    data: usize,
    /// FM1: its `name`, else its child index
    key: String,
    hideable: bool,
}

/// A click on a row this pass.
struct Click {
    row: u64,
    mods: Mods,
    double: bool,
    /// The column under the pointer.
    col: Option<usize>,
}

struct Delegate<'a> {
    /// The shown columns.
    cols: &'a [Col],
    /// FM1: every column, for the header's column menu.
    all: &'a [Col],
    /// FM1: a column the column menu hid or showed (its key).
    toggled: Option<String>,
    t: &'a TableState,
    table_id: Id,
    row_h: f32,
    enabled: bool,
    selectable: bool,
    focused: bool,
    visible: Option<std::ops::Range<u64>>,
    clicks: Vec<Click>,
    header: Option<usize>,
    /// Drawn cells: (row, column, rect, clip).
    cells: Vec<(u64, usize, Rect, Rect)>,
    pointer: Option<egui::Pos2>,
}

const CELL_MARGIN: f32 = 4.0;

/// A cell's a11y and hover id.
fn cell_id(table: Id, row_key: u64, col: usize) -> Id {
    table.with(("cell", row_key, col))
}

impl TableDelegate for Delegate<'_> {
    fn prepare(&mut self, info: &PrefetchInfo) {
        self.visible = Some(info.visible_rows.clone());
    }

    fn header_cell_ui(&mut self, ui: &mut Ui, cell: &HeaderCellInfo) {
        let Some(c) = self.cols.get(cell.col_range.start) else {
            return;
        };
        let rect = ui.max_rect();
        let resp = ui.interact(rect, ui.scope_id().with("head"), if self.enabled { Sense::click() } else { Sense::hover() });
        if resp.hovered() {
            ui.painter().rect_filled(rect, 0.0, ui.visuals().widgets.hovered.weak_bg_fill);
        }
        ui.add_space(CELL_MARGIN);
        let label = ui.add(egui::Label::new(egui::RichText::new(c.title.as_str()).strong()).selectable(false));
        // the sort arrow, painted (the default fonts have no arrow glyphs)
        // FM1: the table's sort-column, else the column's own `sort`
        let shown_sort = if self.t.sort_column >= 0 { if self.t.sort_column as usize == c.data { self.t.sort_order } else { "none" } } else { c.sort };
        let dir = match shown_sort {
            "ascending" => Some(egui::accesskit::SortDirection::Ascending),
            "descending" => Some(egui::accesskit::SortDirection::Descending),
            _ => None,
        };
        if let Some(d) = dir {
            let s = 4.0;
            let ctr = egui::pos2(label.rect.right() + 4.0 + s, rect.center().y);
            let (tip, l, r) = if d == egui::accesskit::SortDirection::Ascending {
                (ctr - vec2(0.0, s), ctr + vec2(-s, s * 0.6), ctr + vec2(s, s * 0.6))
            } else {
                (ctr + vec2(0.0, s), ctr - vec2(s, s * 0.6), ctr + vec2(s, -s * 0.6))
            };
            let color = ui.visuals().strong_text_color();
            ui.painter().add(egui::Shape::convex_polygon(vec![tip, l, r], color, egui::Stroke::NONE));
        }
        let title = c.title.clone();
        ui.ctx().accesskit_node_builder(resp.id, |b| {
            b.set_role(Role::ColumnHeader);
            b.set_label(title);
            if let Some(d) = dir {
                b.set_sort_direction(d);
            }
        });
        if resp.clicked() {
            self.header = Some(c.data);
        }
        // FM1 (QW4): the column menu
        if self.enabled && self.t.column_menu && self.all.iter().any(|c| c.hideable) {
            let shown = self.cols.len();
            resp.context_menu(|ui| {
                for c in self.all.iter().filter(|c| c.hideable) {
                    let mut on = !self.t.hidden.contains(&c.key);
                    // the last shown column stays (the header is where the menu opens)
                    let last = on && shown <= 1;
                    let title = if c.title.is_empty() { c.key.as_str() } else { c.title.as_str() };
                    if ui.add_enabled(!last, egui::Checkbox::new(&mut on, title)).changed() {
                        self.toggled = Some(c.key.clone());
                    }
                }
            });
        }
    }

    fn row_ui(&mut self, ui: &mut Ui, row: u64) {
        let rect = ui.max_rect();
        // egui_table calls this once per column region (sticky, scrolled): an empty region is skipped
        if rect.width() <= 0.0 {
            return;
        }
        let v = ui.visuals();
        let key = self.t.key_of(row);
        let selected = self.selectable && self.t.sel.is_selected(key);
        if selected {
            ui.painter().rect_filled(rect, 0.0, v.selection.bg_fill);
        } else if self.t.striped && row % 2 == 1 {
            ui.painter().rect_filled(rect, 0.0, v.faint_bg_color);
        }
        // per region (a fixed id would be registered twice with sticky columns)
        let resp = ui.interact(rect, ui.scope_id().with("click"), if self.enabled { Sense::click() } else { Sense::hover() });
        let selectable = self.selectable;
        ui.ctx().accesskit_node_builder(resp.id, |b| {
            b.set_role(Role::Row);
            b.set_row_index(row as usize);
            if selectable {
                b.set_selected(selected);
            }
        });
        if self.enabled {
            if resp.hovered() && !selected {
                ui.painter().rect_filled(rect, 0.0, v.widgets.hovered.weak_bg_fill.gamma_multiply(0.5));
            }
            if resp.clicked() || resp.double_clicked() {
                let mods = item_keys::mods(ui.input(|i| i.modifiers));
                self.clicks.push(Click { row, mods, double: resp.double_clicked(), col: None });
            }
        }
    }

    fn cell_ui(&mut self, ui: &mut Ui, cell: &CellInfo) {
        let full = ui.max_rect();
        self.cells.push((cell.row_nr, cell.col_nr, full, ui.clip_rect()));
        if let Some(p) = self.pointer
            && full.contains(p)
            && let Some(c) = self.clicks.last_mut()
            && c.row == cell.row_nr
        {
            c.col = Some(cell.col_nr);
        }
        let key = self.t.key_of(cell.row_nr);
        let col = self.cols.get(cell.col_nr);
        let selected = self.selectable && self.t.sel.is_selected(key);
        let editing = self.t.edit.as_ref().is_some_and(|e| e.row == key && e.col == cell.col_nr);
        let cid = cell_id(self.table_id, key, cell.col_nr);
        let _ = ui.interact(full, cid, Sense::hover());
        let editable = col.is_some_and(|c| c.editable);
        ui.ctx().accesskit_node_builder(cid, |b| {
            b.set_role(Role::Cell);
            b.set_column_index(cell.col_nr);
            if selected {
                b.set_selected(true);
            }
            if !editable {
                b.set_read_only();
            }
        });
        // the cursor cell, while the table has focus
        if self.focused && self.t.sel.cursor == Some(key) && self.t.cur_col == cell.col_nr {
            ui.painter().rect_stroke(full.shrink(1.0), 0.0, ui.visuals().selection.stroke, egui::StrokeKind::Inside);
        }
        if editing {
            return;
        }
        let Some(text) = self.t.cell(cell.row_nr, self.t.data_col(cell.col_nr)) else {
            return;
        };
        let align = col.map_or("start", |c| c.align);
        let layout = match align {
            "center" => Layout::centered_and_justified(egui::Direction::LeftToRight),
            "end" => Layout::right_to_left(Align::Center),
            _ => Layout::left_to_right(Align::Center),
        };
        let r = full.shrink2(vec2(CELL_MARGIN, 0.0));
        ui.scope_builder(UiBuilder::new().max_rect(r).layout(layout).accessibility_parent(cid), |ui| {
            let mut rt = egui::RichText::new(text);
            if selected {
                rt = rt.color(ui.visuals().selection.stroke.color);
            }
            ui.add(egui::Label::new(rt).selectable(false).truncate());
        });
    }

    fn default_row_height(&self) -> f32 {
        self.row_h
    }
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
    let value = t.cell(pos, t.data_col(col)).unwrap_or("").to_string();
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
fn next_editable(cols: &[Col], len: u64, pos: u64, col: usize, step: i32) -> Option<(u64, usize)> {
    let n = cols.len() as i64;
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
        if cols[c].editable {
            return Some(((at / n) as u64, c));
        }
    }
}

/// Paints table `i` at `r`: egui_table with the columns of its children and its data window, the
/// open editor, then its events (`range`, `selectionchange`, `activated`, `header-clicked`).
pub(crate) fn paint(arena: &mut Arena, i: usize, r: Rect, ui: &mut Ui, out: &mut Vec<Out>) {
    let node = arena.slots.id_at(i).expect("live node");
    let h = node.to_raw();
    let kind = arena.core[i].kind as usize;
    let id = arena.core[i].egui_id;
    let enabled = arena.common(i).enabled;
    let all: Vec<Col> = arena.children[i]
        .iter()
        .filter_map(|&c| {
            let ci = arena.ix(c);
            match arena.w(ci) {
                // the column's width is its Widget `width` (null: 100)
                W::TableColumn { title, resizable, align, sort, editable, editor, name, hideable } => Some(Col {
                    node: c,
                    id: arena.core[ci].egui_id,
                    title: title.clone(),
                    width: arena.common(ci).width.unwrap_or(100.0).max(1.0),
                    resizable: *resizable,
                    align,
                    sort,
                    editable: *editable,
                    editor: *editor,
                    data: 0,
                    key: name.clone(),
                    hideable: *hideable,
                }),
                _ => None,
            }
        })
        .enumerate()
        .map(|(k, c)| Col { data: k, key: if c.key.is_empty() { k.to_string() } else { c.key.clone() }, ..c })
        .collect();
    // FM1 (QW4): a `header-state` the host set (hidden columns, sort, widths), then the shown columns
    let (cols, restored, restore_widths) = {
        let W::Table(t) = arena.w_mut(i) else { unreachable!() };
        let (restored, widths) = restore_header(t, &all);
        let cols: Vec<Col> = all.iter().filter(|c| !t.hidden.contains(&c.key)).cloned().collect();
        t.col_map = cols.iter().map(|c| c.data).collect();
        (cols, restored, widths)
    };
    let ctx = ui.ctx().clone();
    let row_h = {
        let W::Table(t) = arena.w(i) else { unreachable!() };
        t.row_height as f32
    };
    let mut b = UiBuilder::new().max_rect(r).scope_id(id.with("scope")).layout(Layout::top_down(Align::Min));
    if !enabled {
        b = b.disabled();
    }
    let mut tui = ui.new_child(b);
    tui.set_clip_rect(ui.clip_rect().intersect(r));
    // WG5a: the table takes keyboard focus (rows and cells are drawn over it)
    let focus_id = id.with("focus");
    let fresp = tui.interact(r, focus_id, Sense::focusable_noninteractive());
    let mut focused = enabled && fresp.has_focus();
    let mut sel_changed = false;
    let mut cur_changed = false;
    let mut activate: Option<u64> = None;
    let W::Table(t) = arena.w_mut(i) else { unreachable!() };
    let t: &mut TableState = t;
    let selectable = t.sel.mode != Mode::None;
    if std::mem::take(&mut t.refocus) && enabled {
        ctx.memory_mut(|m| m.request_focus(focus_id));
        focused = true;
    }
    if focused && t.edit.is_none() {
        item_keys::lock_arrows(&tui, focus_id);
        let mut inputs: Vec<Input> = Vec::new();
        for k in table_keys(&tui) {
            let cur = t.sel.cursor.and_then(|c| t.pos_of(c));
            match k {
                TableKey::Column(d) if !cols.is_empty() => {
                    let c = (t.cur_col as i64 + d as i64).clamp(0, cols.len() as i64 - 1) as usize;
                    cur_changed |= c != t.cur_col;
                    t.cur_col = c;
                    t.scroll_to_col = Some(c);
                }
                TableKey::Column(_) => {}
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
        if cur_changed || sel_changed || activate.is_some() {
            t.error = None;
            ctx.request_repaint();
        }
    }
    if let Some((key, col)) = t.pending_edit.take()
        && t.edit.is_none()
        && let Some(p) = t.pos_of(key)
    {
        start_edit(t, &cols, p, col, String::new());
    }
    let len = t.len();
    let mut table = egui_table::Table::new()
        .id_salt(id.with("table"))
        .num_rows(len)
        .columns(cols.iter().map(|c| Column::new(c.width as f32).id(c.id).resizable(c.resizable).range(16.0..=f32::INFINITY)).collect::<Vec<_>>())
        .num_sticky_cols((t.sticky as usize).min(cols.len()))
        .headers([HeaderRow::new(row_h + 4.0)]);
    if let Some(row) = t.scroll_to.take() {
        table = table.scroll_to_row(row.min(len.max(1) - 1), None);
    }
    if let Some(c) = t.scroll_to_col.take()
        && c < cols.len()
    {
        table = table.scroll_to_column(c, None);
    }
    // the column widths: the props, unless the user resized since the prop last changed (and no
    // first-frame sizing pass: egui_table only auto-sizes a table it has no state for)
    let tid = table.get_id(&tui);
    let mut state = egui_table::TableState::load(tui.ctx(), tid).unwrap_or_default();
    for c in &all {
        let known = t.applied.iter().find(|(k, _)| *k == c.id).map(|(_, w)| *w);
        if known != Some(c.width) || !state.col_widths.contains_key(&c.id) {
            state.col_widths.insert(c.id, c.width as f32);
        }
    }
    for &(cid, w) in &restore_widths {
        state.col_widths.insert(cid, w);
    }
    t.applied = all.iter().map(|c| (c.id, c.width)).collect();
    t.seen = all.iter().filter_map(|c| Some((c.id, *state.col_widths.get(&c.id)?))).collect();
    state.store(tui.ctx(), tid);
    let pointer = ctx.input(|i| i.pointer.interact_pos());
    let mut d = Delegate {
        cols: &cols,
        all: &all,
        toggled: None,
        t,
        table_id: id,
        row_h,
        enabled,
        selectable,
        focused,
        visible: None,
        clicks: Vec::new(),
        header: None,
        cells: Vec::new(),
        pointer,
    };
    let resp = if cols.is_empty() { tui.allocate_rect(r, Sense::hover()) } else { table.show(&mut tui, &mut d) };
    let Delegate { visible, clicks, header, cells, toggled, .. } = d;
    // FM1: a user resize shows as egui_table widths that differ from the ones it was given
    let widths = egui_table::TableState::load(tui.ctx(), tid).map(|s| s.col_widths).unwrap_or_default();
    // ---- clicks: focus, keyed selection, edits on double clicks ----
    let W::Table(t) = arena.w_mut(i) else { unreachable!() };
    if let Some(v) = &visible {
        t.page = ((v.end - v.start).saturating_sub(1) as usize).max(1);
    }
    for c in &clicks {
        ctx.memory_mut(|m| m.request_focus(focus_id));
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
    // ---- the editor ----
    let editor = paint_editor(arena, i, &cols, &cells, &mut tui, out);
    let W::Table(t) = arena.w_mut(i) else { unreachable!() };
    if editor {
        ctx.request_repaint();
    }
    // a rejected commit's reason, under its cell
    if let Some(e) = &mut t.error {
        let now = ctx.input(|i| i.time);
        let until = *e.until.get_or_insert(now + ERROR_SECONDS);
        let at = t.keys.as_ref().map_or(Some(e.row), |k| k.ids.iter().position(|&x| x == e.row).map(|p| p as u64));
        let cell = at.and_then(|p| cells.iter().find(|c| c.0 == p && c.1 == e.col));
        if now >= until {
            t.error = None;
        } else if let Some(&(_, _, cr, clip)) = cell
            && clip.intersects(cr)
        {
            let reason = e.reason.clone();
            egui::Area::new(id.with("edit-error")).order(egui::Order::Tooltip).fixed_pos(cr.left_bottom()).interactable(false).show(&ctx, |ui| {
                egui::Frame::popup(ui.style()).fill(ui.visuals().error_fg_color.gamma_multiply(0.15)).show(ui, |ui| {
                    // a plain label: its text is its name (TK3+ makes it an announcement)
                    ui.label(egui::RichText::new(reason).color(ui.visuals().error_fg_color));
                });
            });
            ctx.request_repaint_after(std::time::Duration::from_secs_f64((until - now).max(0.05)));
        }
    }
    // ---- a11y ----
    let name = arena.common(i).accessible_name.clone();
    let W::Table(t) = arena.w(i) else { unreachable!() };
    let active = t.sel.cursor.map(|k| cell_id(id, k, t.cur_col));
    let multi = t.sel.mode == Mode::Multi;
    tui.ctx().accesskit_node_builder(resp.id, |b| {
        b.set_role(Role::Table);
        if !name.is_empty() {
            b.set_label(name.clone());
        }
        if multi {
            b.set_multiselectable();
        }
        b.set_row_count(len as usize);
        b.set_column_count(cols.len());
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
    let mut resp = crate::a11y::decorate(resp, &arena.n(h));
    let tip = arena.common(i).tooltip.clone();
    if !tip.is_empty() {
        resp = resp.on_hover_text(tip);
    }
    let _ = resp;
    arena.core[i].drawn = r;
    crate::scene::hit::record(arena, i, ui);
    let sig = |name: &str| kinds::signal_id(kind, name);
    let W::Table(t) = arena.w_mut(i) else { unreachable!() };
    let mut events: Vec<(u32, Vec<Value>)> = Vec::new();
    // the data the table needs: the visible rows and `overscan` on each side
    if let Some(v) = visible
        && len > 0
    {
        let last_row = len - 1;
        let o = t.overscan.max(0) as u64;
        let first = v.start.saturating_sub(o);
        let last = (v.end.max(v.start + 1) - 1 + o).min(last_row);
        if t.asked != Some((first, last)) {
            t.asked = Some((first, last));
            events.push((sig("range"), vec![Value::I64(first as i64), Value::I64(last as i64)]));
        }
    }
    let ids = item_keys::ids_value(t.sel.selected.iter().copied());
    if sel_changed {
        events.push((sig("selectionchange"), vec![ids.clone()]));
    }
    if let Some(row) = activate {
        events.push((sig("activated"), vec![Value::I64(row as i64)]));
    }
    if let Some(col) = header {
        events.push((sig("header-clicked"), vec![Value::I64(col as i64)]));
    }
    // FM1 (QW4): sorting from the header, the column menu, the saved state
    let fm1 = header_after(t, &all, &widths, header, toggled, &restored, &ctx);
    for (s, args) in events {
        if arena.listens(i, s) {
            out.push(Out::Event(h, s, args));
        }
    }
    if cur_changed {
        crate::leaf::props_changed(arena, i, &[P::CurrentRow, P::CurrentColumn], out);
    }
    if sel_changed {
        crate::leaf::widget_changed(arena, i, &[P::SelectedIds], vec![ids], out);
    }
    fm1.announce(arena, i, out);
}

/// FM1 (QW4): applies a `header-state` the host set to `t` (hidden columns and sort; the widths
/// are returned for egui_table's state). Returns the props it changed and the widths.
fn restore_header(t: &mut TableState, all: &[Col]) -> (Vec<P>, Vec<(Id, f32)>) {
    let mut restored = Vec::new();
    let mut widths = Vec::new();
    let Some(st) = t.restore.take() else { return (restored, widths) };
    if let Some(h) = st.get("hidden").and_then(|h| h.as_array()) {
        let hidden: Vec<String> = h.iter().filter_map(|x| x.as_str().map(str::to_string)).collect();
        if hidden != t.hidden {
            t.hidden = hidden;
            restored.push(P::HiddenColumns);
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
        restored.extend([P::SortColumn, P::SortOrder]);
    }
    if let Some(w) = st.get("widths").and_then(|w| w.as_object()) {
        for c in all {
            if let Some(px) = w.get(&c.key).and_then(|x| x.as_f64()) {
                widths.push((c.id, px.max(16.0) as f32));
            }
        }
    }
    (restored, widths)
}

/// FM1 (QW4): what the header did this pass, announced after the table's other events.
struct HeaderEvents {
    sorted: Option<Vec<Value>>,
    hidden: Option<Value>,
    state: Option<String>,
}

/// FM1 (QW4): after the table ran: a header click on a `sortable` table sorts (or flips), a column
/// menu toggle hides or shows, a finished resize; each user change writes `header-state` (a
/// restored one is not echoed).
fn header_after(t: &mut TableState, all: &[Col], widths: &egui::IdMap<f32>, header: Option<usize>, toggled: Option<String>, restored: &[P], ctx: &egui::Context) -> HeaderEvents {
    let resized = t.seen.iter().any(|(id, w)| widths.get(id).is_some_and(|x| (x - w).abs() > 0.25));
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
    t.resized |= resized;
    let done_resizing = t.resized && !ctx.input(|inp| inp.pointer.any_down());
    let mut state = None;
    if sort_click.is_some() || menu || done_resizing {
        if done_resizing {
            t.resized = false;
        }
        let s = header_state(all, widths, t);
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
            crate::leaf::widget_changed_via(arena, i, &[P::SortColumn, P::SortOrder], "sort-changed", args, out);
        }
        if let Some(h) = self.hidden {
            crate::leaf::widget_changed_via(arena, i, &[P::HiddenColumns], "columns-changed", vec![h], out);
        }
        if let Some(s) = self.state {
            crate::leaf::widget_changed_via(arena, i, &[P::HeaderState], "header-state-changed", vec![Value::Str(s)], out);
        }
    }
}

/// FM1 (QW4): the `header-state` JSON of the columns `all` (egui_table's `widths` where it has
/// them), their hidden ones and the sort.
fn header_state(all: &[Col], widths: &egui::IdMap<f32>, t: &TableState) -> String {
    let w: serde_json::Map<String, serde_json::Value> = all
        .iter()
        .map(|c| (c.key.clone(), serde_json::json!(widths.get(&c.id).map_or(c.width, |&x| (x as f64 * 100.0).round() / 100.0))))
        .collect();
    let sort = all
        .iter()
        .find(|c| t.sort_column >= 0 && c.data == t.sort_column as usize)
        .map_or(serde_json::Value::Null, |c| serde_json::json!({"column": c.key, "order": t.sort_order}));
    serde_json::json!({"widths": w, "hidden": t.hidden, "sort": sort}).to_string()
}

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
                    if snippet && let Some(s) = listened(column, col_kind, "editor-value") {
                        if let Ok(v) = host.call_sync(column.to_raw(), s, &[]) {
                            value = v;
                        }
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
                    Some(reason) => t.error = Some(EditError { row, col, reason, until: None }),
                    None if commit => {
                        if let Some(p) = pos {
                            t.set_cell(p, t.data_col(col), cell_text(&value));
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
                            W::TableColumn { editable, .. } => Some(*editable),
                            _ => None,
                        })
                        .collect();
                    // FM1: the shown columns only (Tab skips hidden ones; `col` counts shown columns)
                    let W::Table(t) = scene.w(i) else { return false };
                    let shown: Vec<bool> = if t.col_map.is_empty() { editable } else { t.col_map.iter().map(|&k| editable.get(k).copied().unwrap_or(false)).collect() };
                    let cols: Vec<Col> = shown
                        .into_iter()
                        .enumerate()
                        .map(|(k, editable)| Col {
                            node: n,
                            id: Id::NULL,
                            title: String::new(),
                            width: 0.0,
                            resizable: false,
                            align: "start",
                            sort: "none",
                            editable,
                            editor: false,
                            data: k,
                            key: String::new(),
                            hideable: false,
                        })
                        .collect();
                    if let Some((np, nc)) = next_editable(&cols, len, p, col, step) {
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

    fn col(editable: bool) -> Col {
        Col { node: NodeId::from_raw(1).unwrap(), id: Id::NULL, title: String::new(), width: 0.0, resizable: false, align: "start", sort: "none", editable, editor: false, data: 0, key: String::new(), hideable: false }
    }

    #[test]
    fn tab_order_walks_editable_cells_row_major() {
        let cols = [col(false), col(true), col(true)];
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
}
