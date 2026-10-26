//! `table` and `table-column` (svelters-egui `views/table.rs`, OW5b; ARCH-WIDGETS §6.1): a virtual
//! table drawn from skelter display-list primitives. Rows and columns are virtual on both axes: only
//! the rows and columns in view are recorded. The header row is frozen at the top and the first
//! `sticky-columns` columns at the left.
//!
//! - **Data** ([`state`]): a JS window of rows (`range` → `set-rows`) keyed by `set-ids` ids.
//! - **Columns** ([`column`]): the table's `table-column` children, read into [`Table::columns`] by
//!   the kind; hidden ones (`visible={false}`, `hidden-columns`) are left out of the shown columns,
//!   and the cursor column counts shown columns.
//! - **Geometry** ([`geom`]) in table-local points. Scrolling is the views lane's scroll layer: the
//!   kind passes its offset in ([`Table::set_viewport`]) and applies [`Table::take_scroll_request`].
//! - **Header**: a click raises `header-clicked` and, `sortable`, sorts (`sort-changed`); a double
//!   click `header-double-clicked`; a secondary click `header-context`. Each edge resizes its
//!   column ([`resize`]): `column-resized` on release, a double click fits the column to its
//!   header and the window's cells. User changes rewrite `header-state` (`header-state-changed`).
//! - **Keyboard** (focused): Up/Down/PageUp/PageDown/Home/End move the cursor row (Shift extends,
//!   Ctrl moves alone), Left/Right the cursor column, Space and Ctrl+A select, Enter activates,
//!   Ctrl+C raises `copy(ids)`, typing is type-ahead; Escape cancels a column drag.
//!
//! The cell editor (F2 and typing on `editable` columns, the text-edit field) is the kind's
//! ([`kinds`]). Not here yet: double clicks starting an edit, the column menu (a views-lane popup), the Rust row model
//! (`svelters_host::rows`), the scroll bars and edge shadows (the scroll layer's). AccessKit is the
//! kind's: the header row and the realised rows with their cells, as virtual children (FQ-66).

pub mod column;
pub mod text_painter;
pub mod geom;
pub mod item_keys;
pub mod kinds;
pub mod resize;
pub mod selection;
pub mod shim;
pub mod state;

use column::ColumnState;
use geom::Geom;
use item_keys::Input;
use skelter_base::color::Color;
use skelter_base::cursor::CursorIcon;
use skelter_base::display::{DisplayList, Prim};
use skelter_base::event::{KeyEvent, KeyState, Modifiers, MouseButton, PointerEvent, PointerPhase};
use skelter_base::geom::{Corners, Point, Rect, RoundedRect, Size, Vec2};
use skelter_base::key::Key;
use skelter_base::time::{Span, Time};
use resize::{ColumnResize, ResizeIntent};
use selection::{Mods, next_by};
use shim::{Emit, TextPainter, TextStyle, Value};
use state::TableState;

/// The signals a table raises (its spec's list, in order).
pub const SIGNALS: &[&str] = &[
    "range",
    "changed",
    "selectionchange",
    "activated",
    "header-clicked",
    "header-double-clicked",
    "header-context",
    "column-resized",
    "sort-changed",
    "columns-changed",
    "header-state-changed",
    "copy",
    "commit",
];

/// Sizes that place the table's parts and decide its hit zones (from the computed style).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TableMetrics {
    /// The header's height; `None`: the row height + 4.
    pub header_height: Option<f32>,
    /// The width of a header edge's grab zone (`::part(resize)` `--hit`, the `sash-w` token).
    pub resize_hit: f32,
    /// The pointer travel that makes a press a drag (no click).
    pub drag_threshold: f32,
    /// The longest gap between the clicks of a double click.
    pub double_click: Span,
}

impl Default for TableMetrics {
    fn default() -> Self {
        TableMetrics { header_height: None, resize_hit: 4.0, drag_threshold: 4.0, double_click: Span::from_millis(500) }
    }
}

/// The colours and text styles a table records with (its computed style and parts; the defaults
/// are VS Code Light Modern's tokens, as svelters-egui's UA tables resolve them).
#[derive(Clone, Debug, PartialEq)]
pub struct TableLook {
    pub background: Color,
    /// `::part(cell)` text.
    pub text: TextStyle,
    /// `::part(header-cell)` text (weight 600).
    pub header_text: TextStyle,
    pub header_background: Color,
    pub header_hover: Color,
    /// Odd rows at rest with `striped` (`list-hover`, half alpha).
    pub stripe: Color,
    pub row_hover: Color,
    /// Selected rows while the table has focus (`list-sel`), and their text (`list-sel-fg`).
    pub selected: Color,
    pub selected_text: Color,
    /// Selected rows without focus (`list-sel-inactive`).
    pub selected_inactive: Color,
    /// The cursor cell's ring with keyboard focus (`focus`).
    pub focus_ring: Color,
    /// The header edges' 1 pt line and the header's bottom border (`border`).
    pub line: Color,
    /// An edge's hover bar (`focus`).
    pub resize_hover: Color,
    pub sort_arrow: Color,
    /// `::part(cell)` and `::part(header-cell)` side padding.
    pub cell_padding: f32,
    /// `::part(sort)` width.
    pub sort_width: f32,
    pub line_width: f32,
    /// An edge's hover bar width (`sash-w`).
    pub hover_width: f32,
}

impl Default for TableLook {
    fn default() -> Self {
        let rgb = |v: u32| Color::from_rgba8((v >> 16) as u8, (v >> 8) as u8, v as u8, 255);
        let text = TextStyle { color: rgb(0x3B3B3B), ..TextStyle::default() };
        TableLook {
            background: rgb(0xFFFFFF),
            header_text: text.with_weight(600),
            text,
            header_background: rgb(0xF8F8F8),
            header_hover: rgb(0xF2F2F2),
            stripe: rgb(0xF2F2F2).with_alpha(0.5),
            row_hover: rgb(0xF2F2F2),
            selected: rgb(0xE8E8E8),
            selected_text: rgb(0x000000),
            selected_inactive: rgb(0xE4E6F1),
            focus_ring: rgb(0x005FB8),
            line: rgb(0xE5E5E5),
            resize_hover: rgb(0x005FB8),
            sort_arrow: rgb(0x3B3B3B),
            cell_padding: 4.0,
            sort_width: 16.0,
            line_width: 1.0,
            hover_width: 4.0,
        }
    }
}

/// What a primary press landed on.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Target {
    Row { row: u64, col: Option<usize>, mods: Mods },
    /// A header cell (data column).
    Head(usize),
    /// A header edge (data column).
    Edge(usize),
}

impl Target {
    /// Whether two presses are on the same thing (for double clicks).
    fn same(&self, o: &Target) -> bool {
        match (self, o) {
            (Target::Row { row: a, .. }, Target::Row { row: b, .. }) => a == b,
            (Target::Head(a), Target::Head(b)) | (Target::Edge(a), Target::Edge(b)) => a == b,
            _ => false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Press {
    target: Target,
    at: Point,
    /// The pointer moved past the drag threshold: no click at the release.
    dragged: bool,
}

/// One table: its props and data, its columns, and its interaction state.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Table {
    pub state: TableState,
    /// The `table-column` children's props, in child order.
    pub columns: Vec<ColumnState>,
    pub metrics: TableMetrics,
    size: Size,
    /// The body's scroll offset (the scroll layer's).
    off: Vec2,
    scroll_request: Option<Vec2>,
    focused: bool,
    /// Focus came from the keyboard (the cursor cell's ring); a click clears it.
    kbd: bool,
    wants_focus: bool,
    /// The row (and shown column) under the pointer.
    hover: Option<(u64, Option<usize>)>,
    /// The header cell under the pointer (data column).
    head_hover: Option<usize>,
    press: Option<Press>,
    /// The last click: its target and time, for double clicks.
    last_click: Option<(Target, Time, Point)>,
    resize: ColumnResize,
    /// Props the table changed itself since the last [`Table::take_changed`] (for `bind:`).
    changed: Vec<&'static str>,
    /// Columns (child index) whose `width` the table changed since the last
    /// [`Table::take_changed_columns`].
    changed_columns: Vec<usize>,
}

impl Table {
    pub fn new(columns: Vec<ColumnState>) -> Table {
        Table { columns, ..Table::default() }
    }

    /// The data column of each shown column.
    pub fn shown(&self) -> Vec<usize> {
        self.state.shown_columns(&self.columns)
    }

    fn header_h(&self) -> f32 {
        self.metrics.header_height.unwrap_or(self.state.row_height + 4.0)
    }

    /// Column `data`'s drawn width: a live resize, else its prop.
    fn width_of(&self, data: usize) -> f32 {
        self.resize.live(data).unwrap_or_else(|| self.columns[data].width())
    }

    /// The table's geometry at its size and scroll offset.
    pub fn geom(&self) -> Geom {
        let shown = self.shown();
        let widths: Vec<f32> = shown.iter().map(|&d| self.width_of(d)).collect();
        let r = Rect::from_origin_size(Point::ZERO, self.size);
        let mut g = Geom::new(r, self.header_h(), &widths, self.state.sticky.max(0) as usize, self.state.stretch_last, self.state.row_height);
        g.off = self.off;
        g
    }

    /// The body's content size (what the scroll layer scrolls): the shown columns' width and every row.
    pub fn content_size(&self) -> Size {
        let g = self.geom();
        Size::new(g.total, self.state.len() as f32 * g.row_h)
    }

    /// The table's laid-out size and the body's scroll offset.
    pub fn set_viewport(&mut self, size: Size, off: Vec2) {
        self.size = size;
        self.off = off;
    }

    /// The offset the table wants the scroll layer at (a keyboard move, `scroll-to-row`).
    pub fn take_scroll_request(&mut self) -> Option<Vec2> {
        self.scroll_request.take()
    }

    /// Whether a press asked for keyboard focus since the last call.
    pub fn take_focus_request(&mut self) -> bool {
        std::mem::take(&mut self.wants_focus)
    }

    /// The props the table changed itself (`selected`, `current`, `current-column`,
    /// `sort-column`, `sort-order`, `hidden-columns`, `header-state`) since the last call.
    pub fn take_changed(&mut self) -> Vec<&'static str> {
        std::mem::take(&mut self.changed)
    }

    /// The columns (child index) whose `width` the table wrote since the last call.
    pub fn take_changed_columns(&mut self) -> Vec<usize> {
        std::mem::take(&mut self.changed_columns)
    }

    /// Keyboard focus came or went; `keyboard`: it came by Tab (the cursor cell shows its ring).
    pub fn set_focus(&mut self, on: bool, keyboard: bool) {
        self.focused = on;
        if on && keyboard {
            self.kbd = true;
        }
        if !on {
            self.kbd = false;
        }
    }

    pub fn focused(&self) -> bool {
        self.focused
    }

    fn mark(&mut self, props: &[&'static str]) {
        for p in props {
            if !self.changed.contains(p) {
                self.changed.push(p);
            }
        }
    }

    fn set_width(&mut self, data: usize, w: f32) -> f32 {
        let c = &mut self.columns[data];
        let w = ((w.clamp(c.min(), c.max()) as f64 * 100.0).round() / 100.0) as f32;
        if c.width != Some(w) {
            c.width = Some(w);
            if !self.changed_columns.contains(&data) {
                self.changed_columns.push(data);
            }
        }
        w
    }

    fn header_state_changed(&mut self, out: &mut Vec<Emit>) {
        if let Some(s) = self.state.refresh_header_state(&self.columns) {
            self.mark(&["header-state"]);
            out.push(Emit::new("header-state-changed", vec![Value::Str(s)]));
        }
    }

    fn selection_changed(&mut self, out: &mut Vec<Emit>) {
        let ids = item_keys::ids_value(self.state.sel.selected.iter().copied());
        self.mark(&["selected"]);
        out.push(Emit::new("changed", vec![ids.clone()]));
        out.push(Emit::new("selectionchange", vec![ids]));
    }

    fn reveal_cursor(&mut self) {
        if let Some(p) = self.state.sel.cursor.and_then(|c| self.state.pos_of(c)) {
            self.state.scroll_to = Some(p);
        }
    }

    /// Whether this release completes a double click on `target` (and remembers it otherwise).
    fn count_click(&mut self, target: Target, at: Point, now: Time) -> bool {
        let double = self.last_click.is_some_and(|(t, when, p)| {
            t.same(&target) && now - when <= self.metrics.double_click && (at - p).length() <= self.metrics.drag_threshold
        });
        self.last_click = if double { None } else { Some((target, now, at)) };
        double
    }

    /// A pointer event in table-local points at time `now`; `mac` picks the platform's modifiers.
    /// True when the table handled it.
    pub fn pointer(&mut self, ev: &PointerEvent, now: Time, mac: bool, out: &mut Vec<Emit>) -> bool {
        let g = self.geom();
        let shown = self.shown();
        let len = self.state.len();
        let p = ev.pos;
        match ev.phase {
            PointerPhase::Move | PointerPhase::Enter => {
                let edge = g.edge_at(p, self.metrics.resize_hit).map(|k| shown[k]).filter(|&d| self.columns[d].resizable);
                if let Some(ResizeIntent::Live { .. }) = self.resize.moved(p.x, edge, now) {
                    return true;
                }
                if let Some(pr) = self.press.as_mut()
                    && (p - pr.at).length() > self.metrics.drag_threshold
                {
                    pr.dragged = true;
                }
                self.hover = g.row_at(p.y, len).map(|r| (r, g.col_at(p.x)));
                self.head_hover = if g.head.contains(p) && edge.is_none() { g.col_at(p.x).map(|k| shown[k]) } else { None };
                false
            }
            PointerPhase::Leave => {
                self.hover = None;
                self.head_hover = None;
                self.resize.leave();
                false
            }
            PointerPhase::Down if ev.button == Some(MouseButton::Primary) => {
                self.wants_focus = true;
                self.kbd = false;
                let edge = g.edge_at(p, self.metrics.resize_hit).map(|k| shown[k]).filter(|&d| self.columns[d].resizable);
                let target = if let Some(d) = edge {
                    let c = &self.columns[d];
                    let (w, min, max) = (self.width_of(d), c.min(), c.max());
                    self.resize.press(d, p.x, w, min, max);
                    Target::Edge(d)
                } else if g.head.contains(p) {
                    match g.col_at(p.x) {
                        Some(k) => Target::Head(shown[k]),
                        None => return true,
                    }
                } else {
                    match g.row_at(p.y, len) {
                        Some(row) => Target::Row { row, col: g.col_at(p.x), mods: item_keys::mods(ev.modifiers, mac) },
                        None => return true,
                    }
                };
                self.press = Some(Press { target, at: p, dragged: false });
                true
            }
            PointerPhase::Down if ev.button == Some(MouseButton::Secondary) => {
                if g.head.contains(p)
                    && let Some(k) = g.col_at(p.x)
                {
                    out.push(Emit::new("header-context", vec![Value::I64(shown[k] as i64)]));
                    return true;
                }
                false
            }
            PointerPhase::Up if ev.button == Some(MouseButton::Primary) => {
                let Some(pr) = self.press.take() else { return false };
                match self.resize.release() {
                    Some(ResizeIntent::Commit { data, width }) => {
                        let w = self.set_width(data, width);
                        out.push(Emit::new("column-resized", vec![Value::I64(data as i64), Value::F64(w as f64)]));
                        self.header_state_changed(out);
                        self.last_click = None;
                        return true;
                    }
                    Some(_) => {}
                    None => {}
                }
                if pr.dragged {
                    return true;
                }
                // the release must be on what the press was on
                let here = match pr.target {
                    Target::Edge(_) => true,
                    Target::Head(d) => g.head.contains(p) && g.col_at(p.x).map(|k| shown[k]) == Some(d),
                    Target::Row { row, .. } => g.row_at(p.y, len) == Some(row),
                };
                if !here {
                    return true;
                }
                let double = self.count_click(pr.target, p, now);
                match pr.target {
                    Target::Edge(d) => {
                        if double {
                            self.state.fit_columns(Some(d));
                        }
                    }
                    Target::Head(d) => {
                        if double {
                            out.push(Emit::new("header-double-clicked", vec![Value::I64(d as i64)]));
                        } else {
                            self.click_header(d, out);
                        }
                    }
                    Target::Row { row, col, mods } => {
                        if double {
                            out.push(Emit::new("activated", vec![Value::I64(row as i64)]));
                            return true;
                        }
                        self.click_row(row, col, mods, out);
                    }
                }
                true
            }
            PointerPhase::Cancel => {
                self.resize.cancel();
                self.press = None;
                false
            }
            _ => false,
        }
    }

    /// A click on the header of data column `d` (a pointer's, or AccessKit's `Click`): `header-clicked`
    /// and, `sortable`, the sort.
    pub fn click_header(&mut self, d: usize, out: &mut Vec<Emit>) {
        out.push(Emit::new("header-clicked", vec![Value::I64(d as i64)]));
        if self.state.sort_click(d) {
            self.mark(&["sort-column", "sort-order"]);
            out.push(Emit::new("sort-changed", vec![Value::I64(self.state.sort_column), Value::Str(self.state.sort_order.into())]));
            self.header_state_changed(out);
        }
    }

    /// A click on row `row` (in shown column `col`) with `mods` (a pointer's, or AccessKit's
    /// `Click` on a row or cell): the cursor, the cursor column and the selection, as a click sets them.
    pub fn click_row(&mut self, row: u64, col: Option<usize>, mods: Mods, out: &mut Vec<Emit>) {
        let ids = self.state.all_ids();
        let key = self.state.key_of(row);
        let ch = self.state.sel.click(&ids, key, mods);
        if ch.cursor {
            self.mark(&["current"]);
        }
        if let Some(k) = col
            && k != self.state.cur_col
        {
            self.state.cur_col = k;
            self.mark(&["current-column"]);
        }
        if ch.selection {
            self.selection_changed(out);
        }
    }

    /// A key event while the table has focus, at time `now`. True when the table handled it.
    pub fn key(&mut self, ev: &KeyEvent, now: Time, mac: bool, out: &mut Vec<Emit>) -> bool {
        if ev.state != KeyState::Pressed {
            return false;
        }
        if ev.key == Some(Key::Escape) && self.resize.cancel().is_some() {
            self.press = None;
            return true;
        }
        let ncols = self.shown().len();
        let primary = ev.modifiers.primary(mac);
        let handled = match ev.key {
            Some(k @ (Key::ArrowLeft | Key::ArrowRight)) if !ev.modifiers.contains(Modifiers::ALT) => {
                let dir = if k == Key::ArrowLeft { -1 } else { 1 };
                if let Some(c) = next_by(ncols, Some(self.state.cur_col), dir, false, |_| true)
                    && c != self.state.cur_col
                {
                    self.state.cur_col = c;
                    self.state.scroll_to_col = Some(c);
                    self.mark(&["current-column"]);
                }
                true
            }
            Some(Key::C) if primary && !ev.modifiers.contains(Modifiers::SHIFT) => {
                if !self.state.sel.selected.is_empty() {
                    out.push(Emit::new("copy", vec![item_keys::ids_value(self.state.sel.selected.iter().copied())]));
                }
                true
            }
            _ => match item_keys::input_of(ev, now.as_secs_f64(), mac, true) {
                Some(input) => {
                    self.apply(&[input], out);
                    true
                }
                None => false,
            },
        };
        if handled {
            self.kbd = true;
        }
        handled
    }

    /// Typed text that reached the table outside a key event (an IME commit): type-ahead.
    pub fn text(&mut self, text: &str, now: Time, out: &mut Vec<Emit>) {
        self.apply(&[Input::Text(text.to_string(), now.as_secs_f64())], out);
    }

    fn apply(&mut self, inputs: &[Input], out: &mut Vec<Emit>) {
        let ids = self.state.all_ids();
        let mut sel = std::mem::take(&mut self.state.sel);
        let o = item_keys::apply(&mut sel, &ids, inputs, self.state.page, |p| self.state.label(p));
        self.state.sel = sel;
        if o.cursor {
            self.mark(&["current"]);
            self.reveal_cursor();
        }
        if o.selection {
            self.selection_changed(out);
        }
        if let Some(p) = o.activate {
            out.push(Emit::new("activated", vec![Value::I64(p as i64)]));
        }
    }

    /// The width that fits column `data`'s header and the window's cells.
    pub fn fit_width(&self, data: usize, look: &TableLook, text: &mut dyn TextPainter) -> f32 {
        let c = &self.columns[data];
        let pad = 2.0 * look.cell_padding;
        let mut w = text.measure(&c.title, &look.header_text).w + pad;
        if self.state.shown_sort(data, c) != "none" {
            w += look.sort_width;
        }
        for row in &self.state.rows {
            if let Some(t) = row.get(data) {
                w = w.max(text.measure(t, &look.text).w + pad);
            }
        }
        w.ceil().clamp(c.min(), c.max())
    }

    /// Before recording (after layout): applies a restored `header-state`, `fit-columns` and
    /// `scroll-to-row`, keeps the cursor column in range, and asks for the rows it shows (`range`).
    pub fn prepare(&mut self, look: &TableLook, text: &mut dyn TextPainter, out: &mut Vec<Emit>) {
        let restored = self.state.restore_header(&self.columns);
        for &(k, w) in &restored.widths {
            if k < self.columns.len() {
                self.set_width(k, w);
            }
        }
        if restored.props.contains(&"sort-column") {
            self.mark(&["sort-column", "sort-order"]);
            out.push(Emit::new("sort-changed", vec![Value::I64(self.state.sort_column), Value::Str(self.state.sort_order.into())]));
        }
        if restored.props.contains(&"hidden-columns") {
            self.mark(&["hidden-columns"]);
            out.push(Emit::new("columns-changed", vec![Value::List(self.state.hidden.iter().map(|h| Value::Str(h.clone())).collect())]));
        }
        if let Some(f) = self.state.fit.take() {
            let shown = self.shown();
            let fits: Vec<(usize, f32)> = shown
                .iter()
                .copied()
                .filter(|&d| self.columns[d].resizable && f.is_none_or(|k| k == d))
                .map(|d| (d, self.fit_width(d, look, text)))
                .collect();
            for (d, w) in &fits {
                let w = self.set_width(*d, *w);
                out.push(Emit::new("column-resized", vec![Value::I64(*d as i64), Value::F64(w as f64)]));
            }
            if !fits.is_empty() {
                self.header_state_changed(out);
            }
        }
        let ncols = self.shown().len();
        if ncols > 0 && self.state.cur_col >= ncols {
            self.state.cur_col = ncols - 1;
        }
        let len = self.state.len();
        let g = self.geom();
        let row = self.state.scroll_to.take();
        let col = self.state.scroll_to_col.take();
        if row.is_some() || col.is_some() {
            let off = g.reveal(self.off, row, col, len);
            if off != self.off {
                self.off = off;
                self.scroll_request = Some(off);
            }
        }
        let g = self.geom();
        let (first, end) = g.visible_rows(len);
        self.state.page = ((end - first).saturating_sub(1) as usize).max(1);
        if let Some((a, b)) = self.state.wanted_range(first, end) {
            out.push(Emit::new("range", vec![Value::I64(a as i64), Value::I64(b as i64)]));
        }
    }

    /// The cursor over the table: `col-resize` over an edge or while resizing, `pointer` over rows.
    pub fn cursor_icon(&self) -> CursorIcon {
        if self.resize.dragging().is_some() || self.resize.hovered().is_some() {
            CursorIcon::ColResize
        } else if self.hover.is_some() {
            CursorIcon::Pointer
        } else {
            CursorIcon::Default
        }
    }

    /// When the table needs a frame without input (an edge's hover bar turning on).
    pub fn next_wakeup(&self, now: Time) -> Option<Time> {
        self.resize.next_wakeup(now)
    }

    /// Records the table at `now`: the visible rows and cells under the frozen header (sticky
    /// columns over the scrolled ones), then the header with its sort arrows and resize edges.
    pub fn record(&self, look: &TableLook, text: &mut dyn TextPainter, list: &mut DisplayList, now: Time) {
        let g = self.geom();
        let bounds = Rect::from_origin_size(Point::ZERO, self.size);
        if bounds.is_empty() {
            return;
        }
        let shown = self.shown();
        let len = self.state.len();
        let clip = |list: &mut DisplayList, r: Rect| list.push(Prim::PushClip(RoundedRect { rect: r, radii: Corners::ZERO }), r);
        clip(list, bounds);
        shim::fill(list, bounds, look.background, 0.0);
        // ---- rows ----
        let (first, end) = g.visible_rows(len);
        clip(list, g.body);
        for pos in first..end {
            let key = self.state.key_of(pos);
            let selected = self.state.sel.mode != selection::Mode::None && self.state.sel.is_selected(key);
            let rr = g.row_rect(pos);
            let bg = if selected {
                if self.focused { look.selected } else { look.selected_inactive }
            } else if self.hover.is_some_and(|(r, _)| r == pos) {
                look.row_hover
            } else if self.state.striped && pos % 2 == 1 {
                look.stripe
            } else {
                Color::TRANSPARENT
            };
            shim::fill(list, rr, bg, 0.0);
        }
        // the scrolled columns, then the sticky ones over them
        let visible: Vec<usize> = g.visible_cols().collect();
        for sticky in [false, true] {
            let cols: Vec<usize> = visible.iter().copied().filter(|&k| (k < g.sticky) == sticky).collect();
            let Some(&k0) = cols.first() else { continue };
            let (l, r) = g.clip_x(k0);
            let area = Rect::new(l, g.body.min.y, r, g.body.max.y);
            clip(list, area);
            for pos in first..end {
                let key = self.state.key_of(pos);
                let selected = self.state.sel.is_selected(key) && self.state.sel.mode != selection::Mode::None;
                let current = self.state.sel.cursor == Some(key);
                let style = if selected && self.focused { look.text.with_color(look.selected_text) } else { look.text.clone() };
                for &k in &cols {
                    let (cr, _) = g.cell_rect(pos, k);
                    if sticky {
                        // a sticky cell hides the scrolled cells under it
                        let rr = g.row_rect(pos);
                        let under = if selected {
                            if self.focused { look.selected } else { look.selected_inactive }
                        } else if self.hover.is_some_and(|(r, _)| r == pos) {
                            look.row_hover
                        } else if self.state.striped && pos % 2 == 1 {
                            look.stripe
                        } else {
                            Color::TRANSPARENT
                        };
                        shim::fill(list, Rect::new(cr.min.x, rr.min.y, cr.max.x, rr.max.y), look.background, 0.0);
                        shim::fill(list, Rect::new(cr.min.x, rr.min.y, cr.max.x, rr.max.y), under, 0.0);
                    }
                    let d = shown[k];
                    let c = &self.columns[d];
                    if let Some(t) = self.state.cell(pos, d).filter(|t| !t.is_empty()) {
                        let content = Rect::new(cr.min.x + look.cell_padding, cr.min.y, cr.max.x - look.cell_padding, cr.max.y);
                        if content.width() > 0.0 {
                            text.draw(list, t, &style, content, c.cell_align(), c.elide == "end");
                        }
                    }
                    if self.focused && self.kbd && current && self.state.cur_col == k {
                        shim::frame(list, cr, Color::TRANSPARENT, look.focus_ring, 1.0, 0.0);
                    }
                }
            }
            list.push(Prim::PopClip, area);
        }
        list.push(Prim::PopClip, g.body);
        // ---- the header ----
        if g.head.height() > 0.0 {
            clip(list, g.head);
            shim::fill(list, g.head, look.header_background, 0.0);
            for &k in &visible {
                let d = shown[k];
                let c = &self.columns[d];
                let (cr, cclip) = g.head_rect(k);
                clip(list, cclip);
                if k < g.sticky {
                    shim::fill(list, cr, look.header_background, 0.0);
                }
                if self.head_hover == Some(d) {
                    shim::fill(list, cr, look.header_hover, 0.0);
                }
                let mut content = Rect::new(cr.min.x + look.cell_padding, cr.min.y, cr.max.x - look.cell_padding, cr.max.y);
                let sort = self.state.shown_sort(d, c);
                if sort != "none" && content.width() > 0.0 {
                    let sr = Rect::new((content.max.x - look.sort_width).max(content.min.x), cr.min.y, content.max.x, cr.max.y);
                    sort_arrow(list, sr, sort == "ascending", look.sort_arrow);
                    content.max.x = sr.min.x;
                }
                if content.width() > 0.0 && !c.title.is_empty() {
                    text.draw(list, &c.title, &look.header_text, content, c.head_align(), c.elide == "end");
                }
                list.push(Prim::PopClip, cclip);
            }
            // the edges: a line at rest, the hover bar after the delay or while dragged
            for &k in &visible {
                let d = shown[k];
                let (_, x) = g.col_x(k);
                let (l, r) = g.clip_x(k);
                if x < l || x > r + 0.5 {
                    continue;
                }
                shim::fill(list, Rect::new(x - look.line_width, g.head.min.y, x, g.head.max.y), look.line, 0.0);
                if self.columns[d].resizable && self.resize.bar_shown(d, now) {
                    let hw = look.hover_width / 2.0;
                    shim::fill(list, Rect::new(x - hw, g.head.min.y, x + hw, g.head.max.y), look.resize_hover, 0.0);
                }
            }
            shim::line(list, Point::new(g.head.min.x, g.head.max.y - 0.5), Point::new(g.head.max.x, g.head.max.y - 0.5), look.line_width, look.line);
            list.push(Prim::PopClip, g.head);
        }
        list.push(Prim::PopClip, bounds);
    }
}

/// A sort arrow centred in `r`: a chevron pointing up (ascending) or down.
fn sort_arrow(list: &mut DisplayList, r: Rect, up: bool, color: Color) {
    let c = Point::new((r.min.x + r.max.x) / 2.0, (r.min.y + r.max.y) / 2.0);
    let (w, h) = (4.0, 2.5);
    let (tip, base) = if up { (c.y - h, c.y + h) } else { (c.y + h, c.y - h) };
    shim::line(list, Point::new(c.x - w, base), Point::new(c.x, tip), 1.5, color);
    shim::line(list, Point::new(c.x, tip), Point::new(c.x + w, base), 1.5, color);
}

#[cfg(test)]
mod tests;
