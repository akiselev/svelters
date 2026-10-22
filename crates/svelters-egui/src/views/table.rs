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
use crate::scene::props::{as_bool, as_f64, as_i64};
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
struct Col {
    id: Id,
    title: String,
    width: f64,
    resizable: bool,
    align: &'static str,
    sort: &'static str,
}

struct Delegate<'a> {
    cols: &'a [Col],
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
        let dir = match c.sort {
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
            self.header = Some(cell.col_range.start);
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
        let Some(text) = self.t.cell(cell.row_nr, cell.col_nr) else {
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
    let cols: Vec<Col> = arena.children[i]
        .iter()
        .filter_map(|&c| {
            let ci = arena.ix(c);
            match arena.w(ci) {
                // the column's width is its Widget `width` (null: 100)
                W::TableColumn { title, resizable, align, sort } => Some(Col {
                    id: arena.core[ci].egui_id,
                    title: title.clone(),
                    width: arena.common(ci).width.unwrap_or(100.0).max(1.0),
                    resizable: *resizable,
                    align,
                    sort,
                }),
                _ => None,
            }
        })
        .collect();
    let W::Table(t) = arena.w_mut(i) else { unreachable!() };
    let t: &mut TableState = t;
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
    for c in &cols {
        let known = t.applied.iter().find(|(k, _)| *k == c.id).map(|(_, w)| *w);
        if known != Some(c.width) || !state.col_widths.contains_key(&c.id) {
            state.col_widths.insert(c.id, c.width as f32);
        }
    }
    t.applied = cols.iter().map(|c| (c.id, c.width)).collect();
    state.store(tui.ctx(), tid);
    let mut d = Delegate { cols: &cols, t, row_h, enabled, visible: None, clicked: None, activated: None, header: None };
    let resp = if cols.is_empty() { tui.allocate_rect(r, Sense::hover()) } else { table.show(&mut tui, &mut d) };
    let Delegate { visible, clicked, activated, header, .. } = d;
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
        resp = crate::tips::text(resp, &tip);
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
    for (s, args) in events {
        if arena.listens(i, s) {
            out.push(Out::Event(h, s, args));
        }
    }
    if let Some(row) = sel {
        crate::leaf::widget_changed(arena, i, &[P::SelectedIndex], vec![Value::I64(row as i64)], out);
    }
}
