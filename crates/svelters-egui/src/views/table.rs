//! `table` and `table-column` (WG4a), on `egui_table` 0.11 (virtual rows and columns).
//!
//! Columns are the table's `table-column` children (data only: hidden by the table's layout).
//! Rows are a JS data window, the data half of the virtual-list protocol (M5 / N3): each frame the
//! table works out the rows it shows plus `overscan` on each side and, when that range changes,
//! emits `range(first, last)`; JS answers with `set-rows(first, rows)`, which replaces the window.
//! Rows outside the window draw empty until their data arrives (with the default overscan of 20
//! rows, a scroll of up to a page shows data already sent). A small table can send everything
//! once: `set-rows(0, all)`.
//!
//! Rich cells (components per cell) are not here: they need row slots recycled as the table
//! scrolls, which is LISTS' `RowPool` (ARCH-SCENE §8.2). The window keeps the same `range`
//! request, so rich rows can take over the request side unchanged.

use super::{arg, arity};
use crate::kinds::{self, P};
use crate::layout::Metrics;
use crate::scene::props::{as_bool, as_enum, as_f64, as_i64, as_str, as_str_list};
use crate::scene::{Arena, Dirty, W};
use crate::walk::Out;
use egui::{Align, Id, Layout, Rect, Role, Sense, Ui, UiBuilder, Vec2, vec2};
use egui_table::{CellInfo, Column, HeaderCellInfo, HeaderRow, PrefetchInfo, TableDelegate};
use svelters_abi::{StrResult, Value};

/// A table node's props and data.
#[derive(Clone, Debug, PartialEq)]
pub struct TableState {
    pub row_count: i64,
    pub row_height: f64,
    pub sticky: i64,
    pub striped: bool,
    pub overscan: i64,
    pub selected: i64,
    /// The data window: rows `first..first + rows.len()`.
    pub first: u64,
    pub rows: Vec<Vec<String>>,
    /// The last `range` asked for (inclusive), so it is emitted once per change.
    pub asked: Option<(u64, u64)>,
    /// `scroll-to-row`, applied next frame.
    pub scroll_to: Option<u64>,
    /// Per column (by its node's egui id): the `width` prop last written into egui_table's state
    /// (a user resize stays until the prop changes).
    pub applied: Vec<(Id, f64)>,
    // FM1 (QW4): the header (plans/milestones/FM1-form-widgets.md)
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
}

impl Default for TableState {
    fn default() -> Self {
        TableState {
            row_count: 0,
            row_height: 20.0,
            sticky: 0,
            striped: true,
            overscan: 20,
            selected: -1,
            first: 0,
            rows: Vec::new(),
            asked: None,
            scroll_to: None,
            applied: Vec::new(),
            sortable: false,
            sort_column: -1,
            sort_order: "none",
            column_menu: true,
            hidden: Vec::new(),
            header_state: String::new(),
            restore: None,
            seen: Vec::new(),
            resized: false,
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
            P::SelectedIndex => self.selected = as_i64(v)?.max(-1),
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
            P::SelectedIndex => Value::I64(self.selected),
            P::Sortable => Value::Bool(self.sortable),
            P::SortColumn => Value::I64(self.sort_column),
            P::SortOrder => Value::Str(self.sort_order.to_string()),
            P::ColumnMenu => Value::Bool(self.column_menu),
            P::HiddenColumns => Value::List(self.hidden.iter().map(|h| Value::Str(h.clone())).collect()),
            P::HeaderState => Value::Str(self.header_state.clone()),
            _ => return None,
        })
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

    /// The text of cell (`row`, `col`), if its row is in the window.
    pub fn cell(&self, row: u64, col: usize) -> Option<&str> {
        let k = row.checked_sub(self.first)? as usize;
        self.rows.get(k)?.get(col).map(String::as_str)
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

/// One column, from its `table-column` node.
#[derive(Clone)]
struct Col {
    id: Id,
    title: String,
    width: f64,
    resizable: bool,
    align: &'static str,
    sort: &'static str,
    /// FM1: its child index (the data column, `header-clicked`'s and `sort-column`'s number)
    data: usize,
    /// FM1: its `name`, else its child index
    key: String,
    hideable: bool,
}

struct Delegate<'a> {
    /// The shown columns.
    cols: &'a [Col],
    /// FM1: every column, for the header's column menu.
    all: &'a [Col],
    /// FM1: a column the column menu hid or showed (its key).
    toggled: Option<String>,
    t: &'a TableState,
    row_h: f32,
    enabled: bool,
    visible: Option<std::ops::Range<u64>>,
    clicked: Option<u64>,
    activated: Option<u64>,
    header: Option<usize>,
}

const CELL_MARGIN: f32 = 4.0;

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
        let v = ui.visuals();
        if row as i64 == self.t.selected {
            ui.painter().rect_filled(rect, 0.0, v.selection.bg_fill);
        } else if self.t.striped && row % 2 == 1 {
            ui.painter().rect_filled(rect, 0.0, v.faint_bg_color);
        }
        if self.enabled {
            let resp = ui.interact(rect, ui.scope_id().with("click"), Sense::click());
            if resp.hovered() && row as i64 != self.t.selected {
                ui.painter().rect_filled(rect, 0.0, v.widgets.hovered.weak_bg_fill.gamma_multiply(0.5));
            }
            if resp.clicked() {
                self.clicked = Some(row);
            }
            if resp.double_clicked() {
                self.activated = Some(row);
            }
        }
    }

    fn cell_ui(&mut self, ui: &mut Ui, cell: &CellInfo) {
        // FM1: hidden columns are left out, so the shown column says which data column it is
        let Some(text) = self.cols.get(cell.col_nr).and_then(|c| self.t.cell(cell.row_nr, c.data)) else {
            return;
        };
        let align = self.cols.get(cell.col_nr).map_or("start", |c| c.align);
        let layout = match align {
            "center" => Layout::centered_and_justified(egui::Direction::LeftToRight),
            "end" => Layout::right_to_left(Align::Center),
            _ => Layout::left_to_right(Align::Center),
        };
        let r = ui.max_rect().shrink2(vec2(CELL_MARGIN, 0.0));
        let selected = cell.row_nr as i64 == self.t.selected;
        ui.scope_builder(UiBuilder::new().max_rect(r).layout(layout), |ui| {
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

/// Paints table `i` at `r`: egui_table with the columns of its children and its data window,
/// then its events (`range`, `changed`, `activated`, `header-clicked`).
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
                W::TableColumn { title, resizable, align, sort, name, hideable } => Some((ci, title, *resizable, *align, *sort, name, *hideable)),
                _ => None,
            }
        })
        .enumerate()
        .map(|(k, (ci, title, resizable, align, sort, name, hideable))| Col {
            id: arena.core[ci].egui_id,
            title: title.clone(),
            width: arena.common(ci).width.unwrap_or(100.0).max(1.0),
            resizable,
            align,
            sort,
            data: k,
            key: if name.is_empty() { k.to_string() } else { name.clone() },
            hideable,
        })
        .collect();
    let W::Table(t) = arena.w_mut(i) else { unreachable!() };
    let t: &mut TableState = t;
    // FM1 (QW4): a `header-state` the host set: hidden columns, sort and widths
    let mut restored: Vec<P> = Vec::new();
    let mut restore_widths: Vec<(Id, f32)> = Vec::new();
    if let Some(st) = t.restore.take() {
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
            for c in &all {
                if let Some(px) = w.get(&c.key).and_then(|x| x.as_f64()) {
                    restore_widths.push((c.id, px.max(16.0) as f32));
                }
            }
        }
    }
    let cols: Vec<Col> = all.iter().filter(|c| !t.hidden.contains(&c.key)).cloned().collect();
    let row_h = t.row_height as f32;
    let mut b = UiBuilder::new().max_rect(r).scope_id(id.with("scope")).layout(Layout::top_down(Align::Min));
    if !enabled {
        b = b.disabled();
    }
    let mut tui = ui.new_child(b);
    tui.set_clip_rect(ui.clip_rect().intersect(r));
    let columns: Vec<Column> = cols.iter().map(|c| Column::new(c.width as f32).id(c.id).resizable(c.resizable).range(16.0..=f32::INFINITY)).collect();
    let mut table = egui_table::Table::new()
        .id_salt(id.with("table"))
        .num_rows(t.row_count as u64)
        .columns(columns)
        .num_sticky_cols((t.sticky as usize).min(cols.len()))
        .headers([HeaderRow::new(row_h + 4.0)]);
    if let Some(row) = t.scroll_to.take() {
        table = table.scroll_to_row(row.min((t.row_count.max(1) - 1) as u64), None);
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
    for &(id, w) in &restore_widths {
        state.col_widths.insert(id, w);
    }
    t.applied = all.iter().map(|c| (c.id, c.width)).collect();
    t.seen = all.iter().filter_map(|c| Some((c.id, *state.col_widths.get(&c.id)?))).collect();
    state.store(tui.ctx(), tid);
    let mut d = Delegate { cols: &cols, all: &all, toggled: None, t, row_h, enabled, visible: None, clicked: None, activated: None, header: None };
    let resp = if cols.is_empty() { tui.allocate_rect(r, Sense::hover()) } else { table.show(&mut tui, &mut d) };
    let Delegate { visible, clicked, activated, header, toggled, .. } = d;
    // FM1: a user resize shows as egui_table widths that differ from the ones it was given
    let widths = egui_table::TableState::load(tui.ctx(), tid).map(|s| s.col_widths).unwrap_or_default();
    let resized = t.seen.iter().any(|(id, w)| widths.get(id).is_some_and(|x| (x - w).abs() > 0.25));
    let name = arena.common(i).accessible_name.clone();
    tui.ctx().accesskit_node_builder(resp.id, |b| {
        b.set_role(Role::Table);
        if !name.is_empty() {
            b.set_label(name);
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
        && t.row_count > 0
    {
        let last_row = t.row_count as u64 - 1;
        let o = t.overscan.max(0) as u64;
        let first = v.start.saturating_sub(o);
        let last = (v.end.max(v.start + 1) - 1 + o).min(last_row);
        if t.asked != Some((first, last)) {
            t.asked = Some((first, last));
            events.push((sig("range"), vec![Value::I64(first as i64), Value::I64(last as i64)]));
        }
    }
    let sel = clicked.filter(|&row| row as i64 != t.selected);
    if let Some(row) = sel {
        t.selected = row as i64;
    }
    if let Some(row) = activated {
        events.push((sig("activated"), vec![Value::I64(row as i64)]));
    }
    if let Some(col) = header {
        events.push((sig("header-clicked"), vec![Value::I64(col as i64)]));
    }
    // FM1 (QW4): sorting from the header, the column menu, the saved state
    let mut sorted = restored.contains(&P::SortColumn);
    if let Some(col) = header.filter(|_| t.sortable) {
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
    let ctx = ui.ctx().clone();
    let released = !ctx.input(|inp| inp.pointer.any_down());
    // the host's own `header-state` is not echoed; a resize counts once the pointer is released
    let done_resizing = t.resized && released;
    let mut state = None;
    if header.is_some_and(|_| t.sortable) || menu || done_resizing {
        if done_resizing {
            t.resized = false;
        }
        let s = header_state(&all, &widths, t);
        if s != t.header_state {
            t.header_state = s.clone();
            state = Some(s);
        }
    }
    let (sort_args, hidden_arg) = (
        vec![Value::I64(t.sort_column), Value::Str(t.sort_order.to_string())],
        Value::List(t.hidden.iter().map(|x| Value::Str(x.clone())).collect()),
    );
    for (s, args) in events {
        if arena.listens(i, s) {
            out.push(Out::Event(h, s, args));
        }
    }
    if let Some(row) = sel {
        crate::leaf::widget_changed(arena, i, &[P::SelectedIndex], vec![Value::I64(row as i64)], out);
    }
    if sorted {
        crate::leaf::widget_changed_via(arena, i, &[P::SortColumn, P::SortOrder], "sort-changed", sort_args, out);
    }
    if hid {
        crate::leaf::widget_changed_via(arena, i, &[P::HiddenColumns], "columns-changed", vec![hidden_arg], out);
    }
    if let Some(s) = state {
        crate::leaf::widget_changed_via(arena, i, &[P::HeaderState], "header-state-changed", vec![Value::Str(s)], out);
    }
    if sorted || hid {
        ctx.request_repaint();
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
