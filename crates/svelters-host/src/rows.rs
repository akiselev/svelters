//! LS2 (SV41, SV44/SV52): Rust row models for lists, tables and trees, engine-neutral.
//!
//! A [`RowModel`] is the app's data: a row count, a stable key per row, typed [`Cell`]s per
//! column, and optionally tree depth (rows in pre-order). It knows nothing of a toolkit: the egui
//! `table` reads it directly (`svelters_egui` `SceneMut::set_table_model`), and GTK's `GListModel`
//! or Qt's `QAbstractItemModel` adapters can wrap the same trait.
//!
//! A [`RowView`] is what a view shows of a model: the rows in display order after
//! [`RowView::set_sort`], [`RowView::set_filter`] and the collapsed tree rows are applied, with
//! key ↔ position lookups. It is plain data (no toolkit), rebuilt when the model's
//! [`RowModel::version`] or length changes, so the same sort, filter and expansion logic runs on
//! every backend.
//!
//! **Trees.** A tree model returns its rows in pre-order with [`RowModel::depth`]: a row's
//! children are the rows after it with a greater depth, up to the next row at its depth or less.
//! Sorting orders siblings (children stay under their parent); filtering keeps a row that
//! matches and the ancestors of every match, shown expanded while the filter is on.

use std::any::Any;
use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// One cell's value. Sorting compares numbers numerically and text in natural order
/// (case-insensitive, digit runs by value: `N2` < `N10`).
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Cell<'a> {
    Empty,
    Text(Cow<'a, str>),
    Int(i64),
    Float(f64),
    Bool(bool),
}

impl<'a> Cell<'a> {
    /// Borrowed text.
    pub fn text(s: &'a str) -> Cell<'a> {
        Cell::Text(Cow::Borrowed(s))
    }

    /// The text a view shows (integral floats without a fraction, as JS prints them).
    pub fn display(&self) -> Cow<'_, str> {
        match self {
            Cell::Text(s) => Cow::Borrowed(s),
            other => other.clone().into_text(),
        }
    }

    /// [`Cell::display`], keeping borrowed text borrowed.
    pub fn into_text(self) -> Cow<'a, str> {
        match self {
            Cell::Empty => Cow::Borrowed(""),
            Cell::Text(s) => s,
            Cell::Int(i) => Cow::Owned(i.to_string()),
            Cell::Float(f) if f.fract() == 0.0 && f.abs() < 1e15 => Cow::Owned(format!("{}", f as i64)),
            Cell::Float(f) => Cow::Owned(f.to_string()),
            Cell::Bool(b) => Cow::Borrowed(if b { "true" } else { "false" }),
        }
    }

    fn rank(&self) -> u8 {
        match self {
            Cell::Empty => 0,
            Cell::Bool(_) => 1,
            Cell::Int(_) | Cell::Float(_) => 2,
            Cell::Text(_) => 3,
        }
    }

    /// The sort order: empty cells first, then booleans, numbers and text.
    pub fn sort_cmp(&self, other: &Cell<'_>) -> Ordering {
        match (self, other) {
            (Cell::Int(a), Cell::Int(b)) => a.cmp(b),
            (Cell::Int(_) | Cell::Float(_), Cell::Int(_) | Cell::Float(_)) => num(self).total_cmp(&num(other)),
            (Cell::Bool(a), Cell::Bool(b)) => a.cmp(b),
            (Cell::Text(a), Cell::Text(b)) => natural_cmp(a, b),
            _ => self.rank().cmp(&other.rank()),
        }
    }
}

fn num(c: &Cell<'_>) -> f64 {
    match c {
        Cell::Int(i) => *i as f64,
        Cell::Float(f) => *f,
        _ => 0.0,
    }
}

/// Natural order: case-insensitive, runs of ASCII digits compared by value; ties broken by the
/// plain byte order so the order is total.
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (x, y) = (a.as_bytes(), b.as_bytes());
    let (mut i, mut j) = (0, 0);
    while i < x.len() && j < y.len() {
        if x[i].is_ascii_digit() && y[j].is_ascii_digit() {
            let (si, sj) = (i, j);
            while i < x.len() && x[i].is_ascii_digit() {
                i += 1;
            }
            while j < y.len() && y[j].is_ascii_digit() {
                j += 1;
            }
            // by value: strip leading zeros, then longer is larger, then digit by digit
            let (da, db) = (trim_zeros(&x[si..i]), trim_zeros(&y[sj..j]));
            let o = da.len().cmp(&db.len()).then_with(|| da.cmp(db));
            if o != Ordering::Equal {
                return o;
            }
            continue;
        }
        let (ca, la) = next_char(a, i);
        let (cb, lb) = next_char(b, j);
        let o = ca.to_lowercase().cmp(cb.to_lowercase());
        if o != Ordering::Equal {
            return o;
        }
        i += la;
        j += lb;
    }
    (x.len() - i).cmp(&(y.len() - j)).then_with(|| a.cmp(b))
}

fn trim_zeros(d: &[u8]) -> &[u8] {
    let k = d.iter().position(|&c| c != b'0').unwrap_or(d.len());
    &d[k..]
}

fn next_char(s: &str, at: usize) -> (char, usize) {
    let c = s[at..].chars().next().unwrap_or('\0');
    (c, c.len_utf8().max(1))
}

/// What a column holds: a view's default alignment and editor follow it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum CellKind {
    #[default]
    Text,
    /// Right-aligned by default.
    Number,
    Bool,
}

/// A model column: its key (the view's column `name`), header, width, whether it sorts, and its
/// kind. Build with [`Column::new`] and the setters.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct Column {
    pub key: String,
    pub header: String,
    /// Points; `None`: the view's default.
    pub width: Option<f32>,
    pub sortable: bool,
    pub kind: CellKind,
}

impl Column {
    pub fn new(key: impl Into<String>, header: impl Into<String>) -> Column {
        Column { key: key.into(), header: header.into(), width: None, sortable: true, kind: CellKind::Text }
    }

    pub fn width(mut self, w: f32) -> Column {
        self.width = Some(w);
        self
    }

    pub fn sortable(mut self, on: bool) -> Column {
        self.sortable = on;
        self
    }

    pub fn kind(mut self, k: CellKind) -> Column {
        self.kind = k;
        self
    }
}

/// An app's rows, read by a view with no JS call (SV41). Rows are positions `0..len()` in model
/// order; [`RowModel::key`] is the row's stable identity (selection, the cursor, expansion and
/// realised rows follow keys across sorts, filters and model changes).
pub trait RowModel: Any {
    fn len(&self) -> usize;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The row's stable key (unique among the model's rows).
    fn key(&self, row: usize) -> u64;

    /// The columns (a view with no columns of its own makes them from these).
    fn columns(&self) -> &[Column] {
        &[]
    }

    /// Cell (`row`, `col`), `col` indexing [`RowModel::columns`].
    fn cell(&self, row: usize, col: usize) -> Cell<'_>;

    /// The value sorting compares (default: the cell).
    fn sort_key(&self, row: usize, col: usize) -> Cell<'_> {
        self.cell(row, col)
    }

    /// Tree depth (0: a top-level row); rows are in pre-order.
    fn depth(&self, _row: usize) -> u32 {
        0
    }

    /// Bumped by the app when rows or cells change: views rebuild their order and redraw.
    fn version(&self) -> u64 {
        0
    }

    /// The text keyboard type-ahead matches (default: the first cell).
    fn label(&self, row: usize) -> Option<Cow<'_, str>> {
        Some(self.cell(row, 0).into_text())
    }

    /// The row's anchor (`"row[net:GND]"`), for paths and tutorials.
    fn anchor(&self, _key: u64) -> Option<String> {
        None
    }

    fn find_anchor(&self, _anchor: &str) -> Option<u64> {
        None
    }

    /// An edit of cell (`row`, `col`) was committed: `Err(reason)` rejects it (shown on the cell).
    /// The default accepts and changes nothing; a model that stores the value bumps its version.
    fn commit(&mut self, _row: usize, _col: usize, _text: &str) -> Result<(), String> {
        Ok(())
    }
}

/// Sort direction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortOrder {
    Ascending,
    Descending,
}

/// A row filter.
#[derive(Clone)]
#[non_exhaustive]
pub enum RowFilter {
    /// Rows with a cell containing `needle` (case-insensitive) in `columns` (empty: every
    /// [`RowModel::columns`] column, or column 0 when the model names none).
    Text { needle: String, columns: Vec<usize> },
    /// Rows for which the function is true.
    Custom(RowPredicate),
}

/// A [`RowFilter::Custom`] predicate: (model, model row) → shown.
pub type RowPredicate = Arc<dyn Fn(&dyn RowModel, usize) -> bool>;

impl RowFilter {
    pub fn text(needle: impl Into<String>) -> RowFilter {
        RowFilter::Text { needle: needle.into(), columns: Vec::new() }
    }

    pub fn custom(f: impl Fn(&dyn RowModel, usize) -> bool + 'static) -> RowFilter {
        RowFilter::Custom(Arc::new(f))
    }

    fn matcher<'m>(&'m self, m: &'m dyn RowModel) -> Box<dyn Fn(usize) -> bool + 'm> {
        match self {
            RowFilter::Text { needle, columns } => {
                let needle = needle.to_lowercase();
                let cols: Vec<usize> = if !columns.is_empty() {
                    columns.clone()
                } else if m.columns().is_empty() {
                    vec![0]
                } else {
                    (0..m.columns().len()).collect()
                };
                Box::new(move |row| needle.is_empty() || cols.iter().any(|&c| contains_ci(&m.cell(row, c).display(), &needle)))
            }
            RowFilter::Custom(f) => Box::new(move |row| f(m, row)),
        }
    }
}

impl std::fmt::Debug for RowFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RowFilter::Text { needle, columns } => f.debug_struct("Text").field("needle", needle).field("columns", columns).finish(),
            RowFilter::Custom(_) => f.write_str("Custom(..)"),
        }
    }
}

/// `hay` contains `needle_lower` ignoring case (no allocation for ASCII text).
fn contains_ci(hay: &str, needle_lower: &str) -> bool {
    if needle_lower.is_empty() {
        return true;
    }
    if hay.is_ascii() && needle_lower.is_ascii() {
        let (h, n) = (hay.as_bytes(), needle_lower.as_bytes());
        return h.len() >= n.len() && h.windows(n.len()).any(|w| w.eq_ignore_ascii_case(n));
    }
    hay.to_lowercase().contains(needle_lower)
}

const NONE: u32 = u32::MAX;

/// A shown row's tree facts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TreeRow {
    pub depth: u32,
    pub has_children: bool,
    pub expanded: bool,
}

/// What a view shows of a [`RowModel`]: the rows in display order (sorted, filtered, collapsed
/// subtrees left out), with key ↔ position maps. Call [`RowView::sync`] before reading; it
/// rebuilds when the model's version or length, the sort, the filter or an expansion changed.
#[derive(Debug, Default)]
pub struct RowView {
    /// Shown model rows, in display order.
    order: Vec<u32>,
    /// Key → display position.
    pos: HashMap<u64, u32>,
    /// Per model row (trees only): the end of its subtree (exclusive).
    end: Vec<u32>,
    /// Per model row (trees only, while filtering): a descendant matches.
    keep_desc: Vec<bool>,
    tree: bool,
    sort: Option<(usize, SortOrder)>,
    filter: Option<RowFilter>,
    /// Keys whose children show (`expand_all` flips the meaning: keys collapsed).
    toggled: HashSet<u64>,
    expand_all: bool,
    built: Option<(u64, usize)>,
    dirty: bool,
    rebuilds: u64,
}

impl RowView {
    pub fn new() -> RowView {
        RowView { dirty: true, ..RowView::default() }
    }

    /// Tree rows start expanded (default: collapsed).
    pub fn expanded_by_default(mut self, on: bool) -> RowView {
        self.expand_all = on;
        self.dirty = true;
        self
    }

    /// Rebuilds the order if anything changed. True if it did.
    pub fn sync(&mut self, m: &dyn RowModel) -> bool {
        let stamp = (m.version(), m.len());
        if !self.dirty && self.built == Some(stamp) {
            return false;
        }
        self.rebuild(m);
        self.built = Some(stamp);
        self.dirty = false;
        self.rebuilds += 1;
        true
    }

    /// The view must rebuild at the next [`RowView::sync`].
    pub fn invalidate(&mut self) {
        self.dirty = true;
    }

    /// How many times the order was rebuilt.
    pub fn rebuilds(&self) -> u64 {
        self.rebuilds
    }

    pub fn len(&self) -> usize {
        self.order.len()
    }

    pub fn is_empty(&self) -> bool {
        self.order.is_empty()
    }

    /// The model row shown at display position `pos`.
    pub fn row(&self, pos: usize) -> Option<usize> {
        self.order.get(pos).map(|&r| r as usize)
    }

    /// The display position of the row with key `key` (`None`: filtered out or collapsed away).
    pub fn pos_of(&self, key: u64) -> Option<usize> {
        self.pos.get(&key).map(|&p| p as usize)
    }

    /// The shown rows' keys in display order.
    pub fn keys(&self, m: &dyn RowModel) -> Vec<u64> {
        self.order.iter().map(|&r| m.key(r as usize)).collect()
    }

    pub fn sort(&self) -> Option<(usize, SortOrder)> {
        self.sort
    }

    /// Sorts by model column `col` (`None`: model order).
    pub fn set_sort(&mut self, sort: Option<(usize, SortOrder)>) {
        if self.sort != sort {
            self.sort = sort;
            self.dirty = true;
        }
    }

    pub fn filter(&self) -> Option<&RowFilter> {
        self.filter.as_ref()
    }

    pub fn set_filter(&mut self, filter: Option<RowFilter>) {
        self.filter = filter;
        self.dirty = true;
    }

    /// Whether the tree row `key` shows its children (a filter shows the ancestors of matches
    /// expanded regardless).
    pub fn is_expanded(&self, key: u64) -> bool {
        self.toggled.contains(&key) != self.expand_all
    }

    /// Expands or collapses tree row `key`. True if it changed.
    pub fn set_expanded(&mut self, key: u64, on: bool) -> bool {
        if self.is_expanded(key) == on {
            return false;
        }
        if on != self.expand_all {
            self.toggled.insert(key);
        } else {
            self.toggled.remove(&key);
        }
        self.dirty = true;
        true
    }

    /// Expands (or collapses) every row.
    pub fn set_all_expanded(&mut self, on: bool) {
        self.expand_all = on;
        self.toggled.clear();
        self.dirty = true;
    }

    /// Whether the model has tree rows (any depth above 0), as of the last rebuild.
    pub fn is_tree(&self) -> bool {
        self.tree
    }

    /// The tree facts of the row shown at `pos` (all `false`/0 for a flat model).
    pub fn tree_row(&self, m: &dyn RowModel, pos: usize) -> TreeRow {
        let Some(r) = self.row(pos) else { return TreeRow { depth: 0, has_children: false, expanded: false } };
        if !self.tree {
            return TreeRow { depth: 0, has_children: false, expanded: false };
        }
        let has_children = self.end[r] as usize > r + 1;
        let forced = self.filter.is_some() && self.keep_desc.get(r).copied().unwrap_or(false);
        TreeRow { depth: m.depth(r), has_children, expanded: has_children && (forced || self.is_expanded(m.key(r))) }
    }

    /// The display position of the parent of the row shown at `pos`.
    pub fn parent_pos(&self, m: &dyn RowModel, pos: usize) -> Option<usize> {
        let r = self.row(pos)?;
        let d = m.depth(r);
        if !self.tree || d == 0 {
            return None;
        }
        (0..pos).rev().find(|&p| self.row(p).is_some_and(|q| m.depth(q) < d))
    }

    fn rebuild(&mut self, m: &dyn RowModel) {
        let n = m.len();
        self.order.clear();
        self.pos.clear();
        self.keep_desc.clear();
        self.tree = (0..n).any(|r| m.depth(r) > 0);
        let matcher = self.filter.as_ref().map(|f| f.matcher(m));
        if !self.tree {
            self.end.clear();
            self.order.extend((0..n as u32).filter(|&r| matcher.as_ref().is_none_or(|f| f(r as usize))));
            if let Some((col, o)) = self.sort {
                sort_rows(m, &mut self.order, col, o);
            }
        } else {
            // subtree ends and parents from the pre-order depths
            self.end = vec![0; n];
            let mut parent = vec![NONE; n];
            let mut stack: Vec<(u32, u32)> = Vec::new();
            for r in 0..n {
                let d = m.depth(r);
                while let Some(&(p, pd)) = stack.last() {
                    if pd < d {
                        break;
                    }
                    self.end[p as usize] = r as u32;
                    stack.pop();
                }
                parent[r] = stack.last().map_or(NONE, |&(p, _)| p);
                stack.push((r as u32, d));
            }
            for (p, _) in stack {
                self.end[p as usize] = n as u32;
            }
            // filter: a row stays when it matches or a descendant does
            let keep: Option<Vec<bool>> = matcher.as_ref().map(|f| {
                let mut keep = vec![false; n];
                self.keep_desc = vec![false; n];
                for r in (0..n).rev() {
                    keep[r] |= f(r) || self.keep_desc[r];
                    if keep[r] && parent[r] != NONE {
                        self.keep_desc[parent[r] as usize] = true;
                    }
                }
                keep
            });
            let shown = |r: u32| keep.as_ref().is_none_or(|k| k[r as usize]);
            let children = |end: &[u32], r: Option<u32>| -> Vec<u32> {
                let (mut c, e) = match r {
                    Some(r) => (r + 1, end[r as usize]),
                    None => (0, n as u32),
                };
                let mut out = Vec::new();
                while c < e {
                    if shown(c) {
                        out.push(c);
                    }
                    c = end[c as usize];
                }
                out
            };
            // depth-first, siblings sorted, into expanded rows only
            let mut level = children(&self.end, None);
            if let Some((col, o)) = self.sort {
                sort_rows(m, &mut level, col, o);
            }
            let mut stack: Vec<std::vec::IntoIter<u32>> = vec![level.into_iter()];
            while let Some(it) = stack.last_mut() {
                let Some(r) = it.next() else {
                    stack.pop();
                    continue;
                };
                self.order.push(r);
                let open = self.end[r as usize] > r + 1
                    && (self.is_expanded(m.key(r as usize)) || (matcher.is_some() && self.keep_desc[r as usize]));
                if open {
                    let mut kids = children(&self.end, Some(r));
                    if let Some((col, o)) = self.sort {
                        sort_rows(m, &mut kids, col, o);
                    }
                    stack.push(kids.into_iter());
                }
            }
        }
        self.pos.reserve(self.order.len());
        for (p, &r) in self.order.iter().enumerate() {
            self.pos.insert(m.key(r as usize), p as u32);
        }
    }
}

/// Stable sort of model rows by column `col` (ties keep model order).
fn sort_rows(m: &dyn RowModel, rows: &mut [u32], col: usize, o: SortOrder) {
    rows.sort_by(|&a, &b| {
        let c = m.sort_key(a as usize, col).sort_cmp(&m.sort_key(b as usize, col));
        match o {
            SortOrder::Ascending => c,
            SortOrder::Descending => c.reverse(),
        }
    });
}

/// A ready [`RowModel`] over owned rows of cells (tests, small tables, examples).
#[derive(Clone, Debug, Default)]
pub struct VecModel {
    pub columns: Vec<Column>,
    /// (key, depth, cells).
    pub rows: Vec<(u64, u32, Vec<Cell<'static>>)>,
    pub version: u64,
}

impl VecModel {
    pub fn new(columns: Vec<Column>) -> VecModel {
        VecModel { columns, rows: Vec::new(), version: 0 }
    }

    /// Appends a row (depth 0) keyed by its position.
    pub fn push(&mut self, cells: Vec<Cell<'static>>) {
        let k = self.rows.len() as u64;
        self.rows.push((k, 0, cells));
        self.version += 1;
    }
}

impl RowModel for VecModel {
    fn len(&self) -> usize {
        self.rows.len()
    }
    fn key(&self, row: usize) -> u64 {
        self.rows[row].0
    }
    fn columns(&self) -> &[Column] {
        &self.columns
    }
    fn cell(&self, row: usize, col: usize) -> Cell<'_> {
        self.rows.get(row).and_then(|r| r.2.get(col)).cloned().unwrap_or(Cell::Empty)
    }
    fn depth(&self, row: usize) -> u32 {
        self.rows[row].1
    }
    fn version(&self) -> u64 {
        self.version
    }
    fn commit(&mut self, row: usize, col: usize, text: &str) -> Result<(), String> {
        let cells = &mut self.rows.get_mut(row).ok_or("no such row")?.2;
        if cells.len() <= col {
            cells.resize(col + 1, Cell::Empty);
        }
        cells[col] = match cells[col] {
            Cell::Int(_) => Cell::Int(text.trim().parse().map_err(|_| format!("{text:?} is not a whole number"))?),
            Cell::Float(_) => Cell::Float(text.trim().parse().map_err(|_| format!("{text:?} is not a number"))?),
            _ => Cell::Text(Cow::Owned(text.to_string())),
        };
        self.version += 1;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nets(n: usize) -> VecModel {
        let mut m = VecModel::new(vec![Column::new("name", "Net"), Column::new("class", "Class"), Column::new("pads", "Pads").kind(CellKind::Number)]);
        for i in 0..n {
            m.push(vec![Cell::Text(format!("N{i}").into()), Cell::text(if i.is_multiple_of(3) { "power" } else { "signal" }), Cell::Int((i % 7) as i64)]);
        }
        m
    }

    #[test]
    fn natural_order() {
        let mut v = vec!["N10", "n2", "N1", "GND", "N02", "a"];
        v.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(v, ["a", "GND", "N1", "N02", "n2", "N10"]);
        assert_eq!(Cell::Int(3).sort_cmp(&Cell::Float(2.5)), Ordering::Greater);
        assert_eq!(Cell::Empty.sort_cmp(&Cell::text("x")), Ordering::Less);
    }

    #[test]
    fn sort_and_filter_keep_keys() {
        let m = nets(20);
        let mut v = RowView::new();
        v.sync(&m);
        assert_eq!(v.len(), 20);
        assert_eq!(v.pos_of(12), Some(12));
        v.set_sort(Some((0, SortOrder::Descending)));
        assert!(v.sync(&m));
        assert_eq!(m.key(v.row(0).unwrap()), 19);
        assert_eq!(v.pos_of(19), Some(0));
        // pads ascending, ties in model order
        v.set_sort(Some((2, SortOrder::Ascending)));
        v.sync(&m);
        assert_eq!(v.keys(&m)[..3], [0, 7, 14]);
        v.set_filter(Some(RowFilter::Text { needle: "POW".into(), columns: vec![1] }));
        v.sync(&m);
        assert_eq!(v.len(), 7);
        assert_eq!(v.pos_of(1), None);
        assert!(!v.sync(&m), "nothing changed");
    }

    fn tree() -> VecModel {
        // a
        //   a1
        //     a1x
        //   a2
        // b
        //   b1
        let mut m = VecModel::new(vec![Column::new("name", "Name")]);
        for (k, (d, s)) in [(0, "a"), (1, "a2"), (2, "a2x"), (1, "a1"), (0, "b"), (1, "b1")].into_iter().enumerate() {
            m.rows.push((k as u64, d, vec![Cell::text(s)]));
        }
        m
    }

    #[test]
    fn tree_rows_expand_sort_and_filter() {
        let m = tree();
        let mut v = RowView::new();
        v.sync(&m);
        assert!(v.is_tree());
        assert_eq!(v.keys(&m), [0, 4], "collapsed");
        assert_eq!(v.tree_row(&m, 0), TreeRow { depth: 0, has_children: true, expanded: false });
        v.set_expanded(0, true);
        v.sync(&m);
        assert_eq!(v.keys(&m), [0, 1, 3, 4]);
        v.set_sort(Some((0, SortOrder::Ascending)));
        v.sync(&m);
        assert_eq!(v.keys(&m), [0, 3, 1, 4], "siblings sorted under their parent");
        assert_eq!(v.parent_pos(&m, 2), Some(0));
        v.set_sort(Some((0, SortOrder::Descending)));
        v.sync(&m);
        assert_eq!(v.keys(&m), [4, 0, 1, 3]);
        // a filter shows the ancestors of matches, expanded
        v.set_sort(None);
        v.set_filter(Some(RowFilter::text("2x")));
        v.sync(&m);
        assert_eq!(v.keys(&m), [0, 1, 2]);
        assert!(v.tree_row(&m, 1).expanded);
        v.set_filter(None);
        v.set_all_expanded(true);
        v.sync(&m);
        assert_eq!(v.len(), 6);
        v.set_expanded(1, false);
        v.sync(&m);
        assert_eq!(v.keys(&m), [0, 1, 3, 4, 5]);
    }

    #[test]
    fn commit_types_the_value() {
        let mut m = nets(3);
        assert!(m.commit(1, 2, "x").is_err());
        m.commit(1, 2, "42").unwrap();
        assert_eq!(m.cell(1, 2), Cell::Int(42));
    }
}
