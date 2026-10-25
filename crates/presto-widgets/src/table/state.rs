//! A table's props and data (svelters-egui `views/table.rs` `TableState`).
//!
//! Rows are a JS data window, the data half of the virtual-list protocol: the table works out the
//! rows it shows plus `overscan` on each side and, when that range changes, emits
//! `range(first, last)`; JS answers with `set-rows(first, rows)`, which replaces the window. Rows
//! outside the window draw empty until their data arrives.
//!
//! Rows have ids: `set-ids(ids, labels?)` gives every row (by position) an id; selection
//! (`selected`), the cursor (`current-row`) are keyed by id ([`KeySelect`]), so a re-sort or a
//! filter keeps them on the same rows. Without ids a row's id is its position.
//!
//! `header-state` (JSON) saves and restores widths, hidden columns and the sort; the table never
//! writes files: the app persists it.

use super::column::{ColumnState, SORTS};
use super::item_keys::{self, RowKeys};
use super::selection::{KeySelect, Mode};
use super::shim::Value;
use presto_base::change::ChangeClass;
use serde_json::json;

pub(crate) fn as_bool(v: &Value) -> Result<bool, String> {
    match v {
        Value::Bool(b) => Ok(*b),
        other => Err(format!("expected a boolean, got {other:?}")),
    }
}

pub(crate) fn as_i64(v: &Value) -> Result<i64, String> {
    match v {
        Value::I64(i) => Ok(*i),
        Value::F64(f) if f.fract() == 0.0 && f.is_finite() => Ok(*f as i64),
        other => Err(format!("expected an integer, got {other:?}")),
    }
}

pub(crate) fn as_f64(v: &Value) -> Result<f64, String> {
    match v {
        Value::F64(f) => Ok(*f),
        Value::I64(i) => Ok(*i as f64),
        other => Err(format!("expected a number, got {other:?}")),
    }
}

pub(crate) fn as_str(v: &Value) -> Result<String, String> {
    match v {
        Value::Str(s) => Ok(s.clone()),
        other => Err(format!("expected a string, got {other:?}")),
    }
}

/// `v` as one of `names` (the `&'static` spelling).
pub(crate) fn as_enum(v: &Value, names: &[&'static str]) -> Result<&'static str, String> {
    let s = as_str(v)?;
    names.iter().copied().find(|n| *n == s).ok_or_else(|| format!("expected one of {names:?}, got {s:?}"))
}

pub(crate) fn as_str_list(v: &Value) -> Result<Vec<String>, String> {
    match v {
        Value::List(xs) => xs.iter().map(as_str).collect(),
        other => Err(format!("expected a list of strings, got {other:?}")),
    }
}

/// A cell's text from a `set-rows` value.
fn cell_text(v: &Value) -> String {
    match v {
        Value::Str(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
        Value::I64(i) => i.to_string(),
        Value::F64(f) if f.fract() == 0.0 && f.abs() < 1e15 => format!("{}", *f as i64),
        Value::F64(f) => f.to_string(),
        Value::List(_) => String::new(),
    }
}

/// What a restored `header-state` changed.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Restored {
    /// The table props it changed (`hidden-columns`, `sort-column`, `sort-order`).
    pub props: Vec<&'static str>,
    /// (column child index, width).
    pub widths: Vec<(usize, f32)>,
}

/// A table node's props and data.
#[derive(Clone, Debug, PartialEq)]
pub struct TableState {
    pub row_count: i64,
    pub row_height: f32,
    pub sticky: i64,
    pub striped: bool,
    pub overscan: i64,
    /// The data window: rows `first..first + rows.len()`.
    pub first: u64,
    pub rows: Vec<Vec<String>>,
    /// The last `range` asked for (inclusive), so it is emitted once per change.
    pub asked: Option<(u64, u64)>,
    /// `scroll-to-row`, applied at the next prepare.
    pub scroll_to: Option<u64>,
    pub scroll_to_col: Option<usize>,
    /// `set-ids`: row ids (and labels) by position; `None`: a row's id is its position.
    pub keys: Option<RowKeys>,
    pub sel: KeySelect,
    /// The cursor column (counts shown columns).
    pub cur_col: usize,
    /// Rows the last prepare showed (a page move).
    pub page: usize,
    pub sortable: bool,
    pub sort_column: i64,
    pub sort_order: &'static str,
    /// `hidden-columns`: column keys.
    pub hidden: Vec<String>,
    pub header_state: String,
    /// A `header-state` the host set, applied at the next prepare (it needs the columns).
    pub restore: Option<serde_json::Value>,
    pub stretch_last: bool,
    /// `fit-columns(column?)`, applied at the next prepare (`Some(None)`: every column).
    pub fit: Option<Option<usize>>,
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
            sel: KeySelect::new(Mode::Single),
            cur_col: 0,
            page: 10,
            sortable: false,
            sort_column: -1,
            sort_order: "none",
            hidden: Vec::new(),
            header_state: String::new(),
            restore: None,
            stretch_last: true,
            fit: None,
        }
    }
}

impl TableState {
    /// Writes prop `name`; the change class of the write.
    pub fn set_prop(&mut self, name: &str, v: &Value) -> Result<ChangeClass, String> {
        match name {
            "row-count" => {
                self.row_count = as_i64(v)?.max(0);
                return Ok(ChangeClass::Layout);
            }
            "row-height" => {
                self.row_height = (as_f64(v)? as f32).max(1.0);
                return Ok(ChangeClass::Layout);
            }
            "sticky-columns" => self.sticky = as_i64(v)?.max(0),
            "striped" => self.striped = as_bool(v)?,
            "overscan" => {
                self.overscan = as_i64(v)?.max(0);
                return Ok(ChangeClass::Property);
            }
            "selected" => {
                self.sel.set(item_keys::ids_of(v, "selected")?);
            }
            "selection" => {
                let m = Mode::parse(&as_str(v)?).ok_or_else(|| format!("selection: expected one of {:?}", Mode::NAMES))?;
                let keep: Vec<u64> = self.sel.selected.iter().copied().collect();
                self.sel.mode = m;
                self.sel.set(keep);
            }
            "current" => {
                let k = as_i64(v)?;
                self.sel.cursor = (k >= 0).then_some(k as u64);
                self.sel.anchor = self.sel.cursor;
            }
            "current-column" => self.cur_col = as_i64(v)?.max(0) as usize,
            "sortable" => {
                self.sortable = as_bool(v)?;
                return Ok(ChangeClass::Property);
            }
            "sort-column" => self.sort_column = as_i64(v)?.max(-1),
            "sort-order" => self.sort_order = as_enum(v, SORTS)?,
            "hidden-columns" => {
                self.hidden = as_str_list(v)?;
                return Ok(ChangeClass::Layout);
            }
            "header-state" => {
                let s = as_str(v)?;
                self.restore = if s.is_empty() || s == self.header_state {
                    None
                } else {
                    Some(serde_json::from_str::<serde_json::Value>(&s).map_err(|e| format!("header-state: {e}"))?)
                };
                self.header_state = s;
                return Ok(ChangeClass::Layout);
            }
            "stretch-last" => self.stretch_last = as_bool(v)?,
            _ => return Err(format!("{name} is not a prop of <table>")),
        }
        Ok(ChangeClass::Paint)
    }

    pub fn get_prop(&self, name: &str) -> Option<Value> {
        Some(match name {
            "row-count" => Value::I64(self.row_count),
            "row-height" => Value::F64(self.row_height as f64),
            "sticky-columns" => Value::I64(self.sticky),
            "striped" => Value::Bool(self.striped),
            "overscan" => Value::I64(self.overscan),
            "selected" => item_keys::ids_value(self.sel.selected.iter().copied()),
            "selection" => Value::Str(self.sel.mode.name().to_string()),
            "current" => Value::I64(self.sel.cursor.map_or(-1, |k| k as i64)),
            "current-column" => Value::I64(self.cur_col as i64),
            "sortable" => Value::Bool(self.sortable),
            "sort-column" => Value::I64(self.sort_column),
            "sort-order" => Value::Str(self.sort_order.to_string()),
            "hidden-columns" => Value::List(self.hidden.iter().map(|h| Value::Str(h.clone())).collect()),
            "header-state" => Value::Str(self.header_state.clone()),
            "stretch-last" => Value::Bool(self.stretch_last),
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

    /// Every row id in order.
    pub fn all_ids(&self) -> Vec<u64> {
        match &self.keys {
            Some(k) => k.ids.clone(),
            None => (0..self.len()).collect(),
        }
    }

    /// Type-ahead text of the row at `pos`: its `set-ids` label, else its first cell (window only).
    pub fn label(&self, pos: usize) -> Option<String> {
        self.keys.as_ref().and_then(|k| k.label(pos)).or_else(|| self.cell(pos as u64, 0).map(str::to_string))
    }

    /// The text of cell (`row`, data column `col`), if its row is in the window.
    pub fn cell(&self, row: u64, col: usize) -> Option<&str> {
        let k = row.checked_sub(self.first)? as usize;
        self.rows.get(k)?.get(col).map(String::as_str)
    }

    /// `set-rows(first, rows)`: replaces the data window.
    pub fn set_rows(&mut self, args: &[Value]) -> Result<(), String> {
        let [first, rows] = args else { return Err(format!("set-rows: expected 2 arguments, got {}", args.len())) };
        let first = as_i64(first).map_err(|e| format!("set-rows first: {e}"))?;
        if first < 0 {
            return Err(format!("set-rows first: {first} is negative"));
        }
        let Value::List(rows) = rows else { return Err(format!("set-rows rows: expected a list of rows, got {rows:?}")) };
        let mut out = Vec::with_capacity(rows.len());
        for (k, r) in rows.iter().enumerate() {
            match r {
                Value::List(cells) => out.push(cells.iter().map(cell_text).collect()),
                other => return Err(format!("set-rows row {k}: expected a list of cells, got {other:?}")),
            }
        }
        self.first = first as u64;
        self.rows = out;
        Ok(())
    }

    /// `clear-rows()`: drops the window and asks again for what is shown.
    pub fn clear_rows(&mut self) {
        self.rows.clear();
        self.first = 0;
        self.asked = None;
    }

    /// `scroll-to-row(row)`.
    pub fn scroll_to_row(&mut self, row: i64) {
        self.scroll_to = Some(row.max(0) as u64);
    }

    /// `set-ids(ids, labels?)`: the id of every row by position (an empty call: positions again).
    pub fn set_ids(&mut self, args: &[Value]) -> Result<(), String> {
        match args {
            [] => self.keys = None,
            [ids, rest @ ..] if rest.len() <= 1 => {
                let ids = item_keys::ids_of(ids, "set-ids ids")?;
                let labels = item_keys::labels_of(rest.first(), "set-ids labels")?.unwrap_or_default();
                if !labels.is_empty() && labels.len() != ids.len() {
                    return Err(format!("set-ids labels: {} labels for {} ids", labels.len(), ids.len()));
                }
                self.keys = Some(RowKeys { ids, labels });
            }
            _ => return Err(format!("set-ids: expected 1 or 2 arguments, got {}", args.len())),
        }
        // the rows moved: ask for the window again
        self.asked = None;
        Ok(())
    }

    /// `fit-columns(column?)`: fits one column (child index) or every one at the next prepare.
    pub fn fit_columns(&mut self, column: Option<usize>) {
        self.fit = Some(column);
    }

    /// The `range(first, last)` to ask for when rows `first..end` show: the shown rows and
    /// `overscan` on each side; `None` when it was asked already (or nothing shows).
    pub fn wanted_range(&mut self, first: u64, end: u64) -> Option<(u64, u64)> {
        let len = self.len();
        if len == 0 || end <= first {
            return None;
        }
        let o = self.overscan.max(0) as u64;
        let r = (first.saturating_sub(o), (end - 1 + o).min(len - 1));
        if self.asked == Some(r) {
            return None;
        }
        self.asked = Some(r);
        Some(r)
    }

    /// The sort the header shows on column `data` (`col` its props): the table's `sort-column`,
    /// else the column's `sort`.
    pub fn shown_sort(&self, data: usize, col: &ColumnState) -> &'static str {
        if self.sort_column >= 0 {
            if self.sort_column as usize == data { self.sort_order } else { "none" }
        } else {
            col.sort
        }
    }

    /// The data column of each shown column: visible and not in `hidden-columns`.
    pub fn shown_columns(&self, all: &[ColumnState]) -> Vec<usize> {
        all.iter().enumerate().filter(|(k, c)| c.visible && !self.hidden.contains(&c.key(*k))).map(|(k, _)| k).collect()
    }

    /// A header click on a `sortable` table: sorts on column `data` ascending, or flips an
    /// ascending sort on it. True if it sorted.
    pub fn sort_click(&mut self, data: usize) -> bool {
        if !self.sortable {
            return false;
        }
        let flip = self.sort_column == data as i64 && self.sort_order == "ascending";
        self.sort_column = data as i64;
        self.sort_order = if flip { "descending" } else { "ascending" };
        true
    }

    /// Applies a `header-state` the host set (hidden columns and sort); the widths are returned for
    /// the columns `all`.
    pub fn restore_header(&mut self, all: &[ColumnState]) -> Restored {
        let mut out = Restored::default();
        let Some(st) = self.restore.take() else { return out };
        if let Some(h) = st.get("hidden").and_then(|h| h.as_array()) {
            let hidden: Vec<String> = h.iter().filter_map(|x| x.as_str().map(str::to_string)).collect();
            if hidden != self.hidden {
                self.hidden = hidden;
                out.props.push("hidden-columns");
            }
        }
        let sort = st.get("sort").and_then(|x| {
            let key = x.get("column").and_then(|c| c.as_str())?;
            let data = all.iter().enumerate().position(|(k, c)| c.key(k) == key)?;
            let order = SORTS.iter().copied().find(|o| Some(*o) == x.get("order").and_then(|o| o.as_str()))?;
            Some((data as i64, order))
        });
        let (sc, so) = sort.unwrap_or((-1, "none"));
        if (sc, so) != (self.sort_column, self.sort_order) {
            (self.sort_column, self.sort_order) = (sc, so);
            out.props.extend(["sort-column", "sort-order"]);
        }
        if let Some(w) = st.get("widths").and_then(|w| w.as_object()) {
            for (k, c) in all.iter().enumerate() {
                if let Some(px) = w.get(&c.key(k)).and_then(|x| x.as_f64()) {
                    out.widths.push((k, px as f32));
                }
            }
        }
        out
    }

    /// The `header-state` JSON of the columns `all`, the hidden ones and the sort.
    pub fn header_state_of(&self, all: &[ColumnState]) -> String {
        let w: serde_json::Map<String, serde_json::Value> = all.iter().enumerate().map(|(k, c)| (c.key(k), json!((c.width() as f64 * 100.0).round() / 100.0))).collect();
        let sort = all
            .iter()
            .enumerate()
            .find(|(k, _)| self.sort_column >= 0 && *k == self.sort_column as usize)
            .map_or(serde_json::Value::Null, |(k, c)| json!({"column": c.key(k), "order": self.sort_order}));
        json!({"widths": w, "hidden": self.hidden, "sort": sort}).to_string()
    }

    /// Recomputes `header-state` after a user change; the new value if it changed.
    pub fn refresh_header_state(&mut self, all: &[ColumnState]) -> Option<String> {
        let s = self.header_state_of(all);
        (s != self.header_state).then(|| {
            self.header_state = s.clone();
            s
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_keep_selection_across_set_ids() {
        let mut t = TableState { row_count: 4, ..TableState::default() };
        assert_eq!((t.key_of(2), t.pos_of(3)), (2, Some(3)));
        t.set_ids(&[item_keys::ids_value([40, 30, 20, 10]), Value::List(["d", "c", "b", "a"].map(|s| Value::Str(s.into())).to_vec())]).unwrap();
        t.set_prop("selected", &item_keys::ids_value([20])).unwrap();
        assert_eq!(t.pos_of(20), Some(2));
        // re-sorted: same id, new position
        t.set_ids(&[item_keys::ids_value([10, 20, 30, 40])]).unwrap();
        assert!(t.sel.is_selected(20));
        assert_eq!((t.pos_of(20), t.len()), (Some(1), 4));
        assert!(t.set_ids(&[item_keys::ids_value([1, 2]), Value::List(vec![Value::Str("x".into())])]).is_err());
    }

    #[test]
    fn the_window_holds_rows_and_others_are_empty() {
        let mut t = TableState { row_count: 1000, overscan: 5, ..TableState::default() };
        let row = |a: &str, b: i64| Value::List(vec![Value::Str(a.into()), Value::I64(b)]);
        t.set_rows(&[Value::I64(10), Value::List(vec![row("ten", 10), row("eleven", 11)])]).unwrap();
        assert_eq!((t.cell(10, 0), t.cell(11, 1)), (Some("ten"), Some("11")));
        assert_eq!((t.cell(9, 0), t.cell(12, 0), t.cell(10, 5)), (None, None, None), "outside the window: empty");
        assert!(t.set_rows(&[Value::I64(-1), Value::List(vec![])]).is_err());
        // the range: the shown rows plus overscan, asked once
        assert_eq!(t.wanted_range(20, 30), Some((15, 34)));
        assert_eq!(t.wanted_range(20, 30), None, "asked already");
        assert_eq!(t.wanted_range(0, 10), Some((0, 14)), "clamped at the top");
        assert_eq!(t.wanted_range(995, 1000), Some((990, 999)), "and at the bottom");
        t.clear_rows();
        assert_eq!(t.wanted_range(995, 1000), Some((990, 999)), "cleared: asked again");
    }

    #[test]
    fn header_state_round_trips_widths_hidden_and_sort() {
        let mut name = ColumnState::new("Name", 120.0);
        name.name = "name".into();
        let all = [name, ColumnState::new("Value", 80.5)];
        let mut t = TableState { sort_column: 1, sort_order: "descending", hidden: vec!["name".into()], ..TableState::default() };
        let s = t.header_state_of(&all);
        let v: serde_json::Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["widths"]["name"], 120.0);
        assert_eq!(v["sort"]["column"], "1");
        // restoring it elsewhere: hidden, sort and widths come back (the props it changed listed)
        let mut u = TableState::default();
        u.set_prop("header-state", &Value::Str(s)).unwrap();
        let r = u.restore_header(&all);
        assert_eq!((u.hidden.clone(), u.sort_column, u.sort_order), (vec!["name".to_string()], 1, "descending"));
        assert_eq!(r.widths, vec![(0, 120.0), (1, 80.5)]);
        assert!(r.props.contains(&"hidden-columns") && r.props.contains(&"sort-column"));
        assert_eq!(u.shown_columns(&all), vec![1]);
        // a header click on a sortable table sorts, then flips
        assert!(!t.sort_click(0), "not sortable");
        t.sortable = true;
        t.sort_column = -1;
        assert!(t.sort_click(0));
        assert_eq!((t.sort_column, t.sort_order), (0, "ascending"));
        t.sort_click(0);
        assert_eq!(t.sort_order, "descending");
        assert!(t.refresh_header_state(&all).is_some());
        assert!(t.refresh_header_state(&all).is_none(), "unchanged");
    }
}
