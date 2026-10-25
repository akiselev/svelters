//! `table` and `table-column` as skelter-core kinds over the [`Table`] model. They implement
//! `skelter_core::Kind` directly (not through `host::Hosted`): the table reads its `table-column`
//! children's props, which the hosted kind interface does not carry.
//!
//! - The columns are data: `table` folds its children (`KindSpec::folds_children`), so skelter-core
//!   measures it as a leaf, never lays out, records or hits a column, and repaints the table when a
//!   column's prop changes. The columns are re-read in `measure` and `prepare`; a width the user
//!   dragged stays until the column's own `width` prop changes.
//! - Props go to [`TableState`](super::state::TableState); the props the table changes itself
//!   (`selected`, `current`, `sort-column`, …) are written back as native state for `bind:`.
//! - JS methods (`set-rows`, `set-ids`, `clear-rows`, `scroll-to-row`, `fit-columns`) are
//!   skelter-core methods ([`Kind::call`]).
//! - Scrolling: until the views lane's scroll layer hosts the table, the kind keeps the body's offset
//!   itself (the wheel scrolls it, clamped; keyboard moves and `scroll-to-row` reveal rows), with no
//!   scroll bars or fling.
//!
//! skelter-core gaps this kind works around (listed in `plans/milestones/SKELTER-widgets-tier1.md`):
//! - `prepare` cannot emit, so the model's `range` (and a restored `header-state`'s events) are held
//!   and delivered with the table's next event or method call;
//! - an event can write only its own node's props, so a user resize stays in the table and is not
//!   written back to the column's `width` (`column-resized` reports it; JS writes it if it binds it);
//! - `record` has no clock, so the resize edges' hover delay uses the last event's time.

use super::column::{ALIGNS, ColumnState, ELIDES, HEADER_ALIGNS, SORTS};
use super::selection::Mode;
use super::shim::{self as tshim, Emit, Platform};
use super::text_painter::{record_core, with_text};
use super::{Table, TableLook};
use crate::theme::Theme;
use skelter_base::accesskit::Role;
use skelter_base::change::ChangeClass;
use skelter_base::event::{KeyState, Modifiers, PointerEvent, PointerPhase, PointerType, ScrollDelta};
use skelter_base::geom::{Point, Size, Vec2};
use skelter_base::time::Time;
use skelter_core::{
    A11yCx, ArgSpec, Event, EventCx, FocusReason, Handled, Kind, KindRegistry, KindSpec, MeasureCx, MethodId, MethodSpec, NodeView, PrepareCx, PropCx, PropDefault, PropError, PropId,
    PropSpec, RecordCx, RegistryError, SignalFlags, SignalSpec, Value, ValueType,
};
use std::sync::Arc;

static TABLE_PROPS: &[PropSpec] = &[
    PropSpec::new("row-count", ValueType::I64, ChangeClass::Layout).default(PropDefault::I64(0)).doc("Rows in the table when `set-ids` gave none."),
    PropSpec::new("row-height", ValueType::F64, ChangeClass::Layout).default(PropDefault::F64(20.0)).doc("Every row's height in points."),
    PropSpec::new("sticky-columns", ValueType::I64, ChangeClass::Paint).default(PropDefault::I64(0)).doc("The first shown columns that stay at the left."),
    PropSpec::new("striped", ValueType::Bool, ChangeClass::Paint).default(PropDefault::Bool(true)).doc("Odd rows get the stripe fill."),
    PropSpec::new("overscan", ValueType::I64, ChangeClass::Property).default(PropDefault::I64(20)).doc("Rows asked for beyond the visible ones, on each side."),
    PropSpec::new("selected", ValueType::List, ChangeClass::Paint).native().doc("The selected rows' ids."),
    PropSpec::new("selection", ValueType::Enum, ChangeClass::Paint).enum_values(Mode::NAMES).default(PropDefault::Enum(1)).doc("How clicks and keys select."),
    PropSpec::new("current", ValueType::I64, ChangeClass::Paint).default(PropDefault::I64(-1)).native().doc("The cursor row's id, -1 for none."),
    PropSpec::new("current-column", ValueType::I64, ChangeClass::Paint).default(PropDefault::I64(0)).native().doc("The cursor column (counts shown columns)."),
    PropSpec::new("sortable", ValueType::Bool, ChangeClass::Property).default(PropDefault::Bool(false)).doc("A header click sorts."),
    PropSpec::new("sort-column", ValueType::I64, ChangeClass::Paint).default(PropDefault::I64(-1)).native().doc("The sorted column (child index), -1 for none."),
    PropSpec::new("sort-order", ValueType::Enum, ChangeClass::Paint).enum_values(SORTS).default(PropDefault::Enum(0)).native().doc("The sort order."),
    PropSpec::new("hidden-columns", ValueType::List, ChangeClass::Layout).native().doc("The keys of the hidden columns."),
    PropSpec::new("header-state", ValueType::Str, ChangeClass::Layout).default(PropDefault::Str("")).native().doc("Widths, hidden columns and sort as JSON; setting it restores them."),
    PropSpec::new("stretch-last", ValueType::Bool, ChangeClass::Paint).default(PropDefault::Bool(true)).doc("The last column fills the rest of the width."),
];

const P: SignalFlags = SignalFlags::PASSIVE;

static TABLE_SIGNALS: &[SignalSpec] = &[
    SignalSpec::new("range", Some(ValueType::Record), P)
        .args(&[ArgSpec::new("first", ValueType::I64), ArgSpec::new("last", ValueType::I64)])
        .doc("The rows the table needs data for changed (visible plus `overscan`); answer with `set-rows`."),
    SignalSpec::new("changed", Some(ValueType::List), P).args(&[ArgSpec::new("ids", ValueType::List)]).doc("The selection changed; carries the selected ids."),
    SignalSpec::new("selectionchange", Some(ValueType::List), P).args(&[ArgSpec::new("ids", ValueType::List)]).doc("The selection changed (DOM spelling)."),
    SignalSpec::new("activated", Some(ValueType::I64), P).args(&[ArgSpec::new("row", ValueType::I64)]).doc("A row was double-clicked or Enter pressed on the cursor row; carries its position."),
    SignalSpec::new("header-clicked", Some(ValueType::I64), P).args(&[ArgSpec::new("column", ValueType::I64)]).doc("A header cell was clicked; carries the column."),
    SignalSpec::new("header-double-clicked", Some(ValueType::I64), P).args(&[ArgSpec::new("column", ValueType::I64)]).doc("A header cell was double-clicked."),
    SignalSpec::new("header-context", Some(ValueType::I64), P).args(&[ArgSpec::new("column", ValueType::I64)]).doc("A secondary click on the header; carries the column under the pointer."),
    SignalSpec::new("column-resized", Some(ValueType::Record), P)
        .args(&[ArgSpec::new("column", ValueType::I64), ArgSpec::new("width", ValueType::F64)])
        .doc("The user resized a column (or `fit-columns` did)."),
    SignalSpec::new("sort-changed", Some(ValueType::Record), P)
        .args(&[ArgSpec::new("column", ValueType::I64), ArgSpec::new("order", ValueType::Str)])
        .doc("A header click on a sortable table changed the sort; sort the rows in JS."),
    SignalSpec::new("columns-changed", Some(ValueType::List), P).args(&[ArgSpec::new("hidden", ValueType::List)]).doc("The hidden columns changed."),
    SignalSpec::new("header-state-changed", Some(ValueType::Str), P).args(&[ArgSpec::new("state", ValueType::Str)]).doc("The header's widths, hidden columns or sort changed by the user."),
    SignalSpec::new("copy", Some(ValueType::List), P).args(&[ArgSpec::new("ids", ValueType::List)]).doc("Ctrl+C (Cmd+C) on the focused table; carries the selected ids."),
];

static TABLE_METHODS: &[MethodSpec] = &[
    MethodSpec::new("set-rows", &[ArgSpec::new("first", ValueType::I64), ArgSpec::new("rows", ValueType::List)], None).doc("Replaces the data window: rows `first..` (each a list of cells)."),
    MethodSpec::new("set-ids", &[ArgSpec::new("ids", ValueType::List), ArgSpec { name: "labels", ty: ValueType::List, nullable: true }], None).doc("Every row's id (and label) by position."),
    MethodSpec::new("clear-rows", &[], None).doc("Drops the data window and asks again for what shows."),
    MethodSpec::new("scroll-to-row", &[ArgSpec::new("row", ValueType::I64)], None).doc("Scrolls the row (position) into view."),
    MethodSpec::new("fit-columns", &[ArgSpec { name: "column", ty: ValueType::I64, nullable: true }], None).doc("Fits one column (child index) or every one to its contents."),
];

/// `table`: a virtual table over its `table-column` children.
pub static TABLE: KindSpec = KindSpec {
    props: TABLE_PROPS,
    signals: TABLE_SIGNALS,
    methods: TABLE_METHODS,
    role: Role::Table,
    focusable: true,
    container: true,
    folds_children: true,
    doc: "A virtual table: rows are a JS data window (`range` → `set-rows`), columns its `table-column` children.",
    ..KindSpec::new("table")
};

static COLUMN_PROPS: &[PropSpec] = &[
    PropSpec::new("title", ValueType::Str, ChangeClass::Paint).default(PropDefault::Str("")),
    PropSpec::new("width", ValueType::F64, ChangeClass::Layout).nullable(),
    PropSpec::new("min-width", ValueType::F64, ChangeClass::Layout).nullable(),
    PropSpec::new("max-width", ValueType::F64, ChangeClass::Layout).nullable(),
    PropSpec::new("visible", ValueType::Bool, ChangeClass::Layout).default(PropDefault::Bool(true)),
    PropSpec::new("resizable", ValueType::Bool, ChangeClass::Property).default(PropDefault::Bool(true)),
    PropSpec::new("align", ValueType::Enum, ChangeClass::Paint).enum_values(ALIGNS).default(PropDefault::Enum(0)),
    PropSpec::new("header-align", ValueType::Enum, ChangeClass::Paint).enum_values(HEADER_ALIGNS).default(PropDefault::Enum(0)),
    PropSpec::new("sort", ValueType::Enum, ChangeClass::Paint).enum_values(SORTS).default(PropDefault::Enum(0)),
    PropSpec::new("editable", ValueType::Bool, ChangeClass::Property).default(PropDefault::Bool(false)),
    PropSpec::new("name", ValueType::Str, ChangeClass::Property).default(PropDefault::Str("")),
    PropSpec::new("hideable", ValueType::Bool, ChangeClass::Property).default(PropDefault::Bool(true)),
    PropSpec::new("elide", ValueType::Enum, ChangeClass::Paint).enum_values(ELIDES).default(PropDefault::Enum(0)),
    PropSpec::new("tooltip", ValueType::Str, ChangeClass::Property).default(PropDefault::Str("")),
];

/// `table-column`: one column of a `table` (data: never laid out or drawn as a node).
pub static TABLE_COLUMN: KindSpec = KindSpec { props: COLUMN_PROPS, role: Role::ColumnHeader, doc: "A column of a table (its header title, width and behaviour).", ..KindSpec::new("table-column") };

/// Registers `table` and `table-column`.
pub fn register(reg: &mut KindRegistry) -> Result<(), RegistryError> {
    reg.register::<TableKind>()?;
    reg.register::<TableColumnKind>()?;
    Ok(())
}

impl TableLook {
    /// The look from `theme`'s tokens (the defaults where a token is missing).
    pub fn from_theme(theme: &Theme) -> TableLook {
        let d = TableLook::default();
        let c = |name: &str, fallback| theme.color(name).unwrap_or(fallback);
        let mut text = d.text.clone();
        text.color = c("fg", text.color);
        text.size = theme.px("font-size").unwrap_or(text.size);
        if let Some(f) = theme.token("font-family") {
            text.family = f.to_owned();
        }
        let hover = c("list-hover", d.row_hover);
        TableLook {
            background: c("bg", d.background),
            header_text: text.with_weight(600),
            text,
            header_background: c("bg", d.header_background),
            header_hover: hover,
            stripe: hover.with_alpha(hover.a * 0.5),
            row_hover: hover,
            selected: c("list-sel", d.selected),
            selected_text: c("list-sel-fg", d.selected_text),
            selected_inactive: c("list-sel-inactive", d.selected_inactive),
            focus_ring: c("focus", d.focus_ring),
            line: c("border", d.line),
            resize_hover: c("focus", d.resize_hover),
            sort_arrow: c("icon", d.sort_arrow),
            hover_width: theme.px("sash-w").unwrap_or(d.hover_width),
            ..d
        }
    }
}

/// A skelter-core value as the model's (an enum by its name in `names`).
fn to_model(v: &Value, names: &[&str]) -> Option<tshim::Value> {
    Some(match v {
        Value::Bool(b) => tshim::Value::Bool(*b),
        Value::I64(i) => tshim::Value::I64(*i),
        Value::F64(f) => tshim::Value::F64(*f),
        Value::Str(s) => tshim::Value::Str(s.to_string()),
        Value::Enum(i) => tshim::Value::Str(names.get(*i as usize)?.to_string()),
        Value::List(l) => tshim::Value::List(l.iter().filter_map(|x| to_model(x, &[])).collect()),
        Value::Color(_) | Value::Record(_) => return None,
    })
}

/// The model's value as skelter-core's for prop `p` (an enum name as its index).
fn to_core(v: &tshim::Value, p: Option<&PropSpec>) -> Value {
    match v {
        tshim::Value::Bool(b) => Value::Bool(*b),
        tshim::Value::I64(i) => Value::I64(*i),
        tshim::Value::F64(f) => Value::F64(*f),
        tshim::Value::Str(s) => match p.filter(|p| p.ty == ValueType::Enum).and_then(|p| p.enum_values.iter().position(|n| n == s)) {
            Some(i) => Value::Enum(i as u32),
            None => Value::Str(Arc::from(s.as_str())),
        },
        tshim::Value::List(l) => Value::List(l.iter().map(|x| to_core(x, None)).collect::<Vec<_>>().into()),
    }
}

/// A model event as skelter-core's signal argument: none, the one value, or a record named by the spec.
pub fn emit_arg(spec: &SignalSpec, args: &[tshim::Value]) -> Option<Value> {
    match args {
        [] => None,
        [v] => Some(to_core(v, None)),
        many => Some(Value::Record(spec.args.iter().zip(many).map(|(a, v)| (Arc::from(a.name), to_core(v, None))).collect::<Vec<_>>().into())),
    }
}

/// Column `c`'s state from its node.
fn column_of(c: NodeView<'_>) -> ColumnState {
    let mut col = ColumnState::default();
    for (i, p) in COLUMN_PROPS.iter().enumerate() {
        let Some(v) = c.prop(TABLE_COLUMN.prop_id(i)) else { continue };
        if let Some(mv) = to_model(v, p.enum_values) {
            let _ = col.set_prop(p.name, &mv);
        }
    }
    col
}

/// The `table-column` children of `node`, in child order.
fn columns_of(node: NodeView<'_>) -> Vec<ColumnState> {
    (0..node.children().len()).filter_map(|k| node.child(k)).filter(|c| c.kind().spec.name == TABLE_COLUMN.name).map(column_of).collect()
}

/// The `table` kind.
#[derive(Default)]
pub struct TableKind {
    pub table: Table,
    look: TableLook,
    /// Events the model raised in `prepare`, delivered with the next event or method call.
    pending: Vec<Emit>,
    /// Each column's `width` prop as last read (a user resize stays until it changes).
    read_widths: Vec<Option<f32>>,
    off: Vec2,
    /// The laid-out size (as of the last prepare).
    size: Size,
    /// The last event's time (the clock `record` lacks).
    now: Time,
}

impl TableKind {
    /// Re-reads the columns, keeping a width the user dragged while the column's prop is unchanged.
    fn read_columns(&mut self, node: NodeView<'_>) {
        let cols = columns_of(node);
        let mut next = Vec::with_capacity(cols.len());
        for (k, mut c) in cols.into_iter().enumerate() {
            let prop = c.width;
            if self.read_widths.get(k).copied() == Some(prop)
                && let Some(old) = self.table.columns.get(k)
            {
                c.width = old.width;
            }
            next.push((c, prop));
        }
        self.read_widths = next.iter().map(|(_, p)| *p).collect();
        self.table.columns = next.into_iter().map(|(c, _)| c).collect();
    }

    fn clamp_off(&mut self) {
        let content = self.table.content_size();
        let g = self.table.geom();
        let view_w = g.body.width().max(0.0);
        let view_h = g.body.height().max(0.0);
        self.off.x = self.off.x.clamp(0.0, (content.w - view_w).max(0.0));
        self.off.y = self.off.y.clamp(0.0, (content.h - view_h).max(0.0));
    }

    /// Delivers `out` and the held events, writes back what the model changed itself, and applies
    /// its focus and scroll requests.
    fn deliver(&mut self, cx: &mut EventCx, mut out: Vec<Emit>) {
        let mut all = std::mem::take(&mut self.pending);
        all.append(&mut out);
        for name in self.table.take_changed() {
            let Some(i) = TABLE_PROPS.iter().position(|p| p.name == name) else { continue };
            if let Some(v) = self.table.state.get_prop(name) {
                let _ = cx.set_native(TABLE.prop_id(i), to_core(&v, Some(&TABLE_PROPS[i])));
            }
        }
        // a user resize stays in the table (see the module doc)
        self.table.take_changed_columns();
        for e in &all {
            if let Some(i) = TABLE_SIGNALS.iter().position(|s| s.name == e.signal) {
                cx.emit(TABLE.signal_id(i), emit_arg(&TABLE_SIGNALS[i], &e.args));
            }
        }
        if self.table.take_focus_request() {
            cx.focus(FocusReason::Pointer);
        }
        if let Some(off) = self.table.take_scroll_request() {
            self.off = off;
            self.clamp_off();
            self.table.set_viewport(self.size, self.off);
        }
        if let Some(t) = self.table.next_wakeup(self.now) {
            cx.wake_at(t);
        }
        cx.set_cursor(self.table.cursor_icon());
        cx.request_paint();
    }

    fn pointer_event(phase: PointerPhase, p: &skelter_core::PointerInput) -> PointerEvent {
        PointerEvent { phase, pointer: p.pointer, kind: p.kind, button: p.button, buttons: p.buttons, pos: p.local, modifiers: p.modifiers, pressure: p.pressure, tilt: Vec2::ZERO, history: Default::default() }
    }

    fn bare(phase: PointerPhase) -> PointerEvent {
        PointerEvent { phase, pointer: 0, kind: PointerType::Mouse, button: None, buttons: 0, pos: Point::new(-1.0, -1.0), modifiers: Modifiers::default(), pressure: 0.0, tilt: Vec2::ZERO, history: Default::default() }
    }

    fn mac() -> bool {
        Platform::current().is_mac()
    }
}

impl Kind for TableKind {
    fn spec() -> &'static KindSpec {
        &TABLE
    }

    fn prop_changed(&mut self, cx: &mut PropCx, prop: PropId, value: Option<&Value>) -> Result<(), PropError> {
        let Some(i) = (0..TABLE_PROPS.len()).find(|&i| TABLE.prop_id(i) == prop) else { return Ok(()) };
        let p = &TABLE_PROPS[i];
        let v = match value.cloned().or_else(|| p.default.to_value()) {
            Some(v) => to_model(&v, p.enum_values),
            None => Some(tshim::Value::List(Vec::new())),
        };
        let Some(v) = v else { return Err(PropError::Rejected(format!("{}: not a value the table takes", p.name))) };
        let class = self.table.state.set_prop(p.name, &v).map_err(PropError::Rejected)?;
        if class >= ChangeClass::Layout {
            cx.request_layout();
        } else {
            cx.request_paint();
        }
        Ok(())
    }

    fn measure(&self, cx: &mut MeasureCx, known: taffy::Size<Option<f32>>, _avail: taffy::Size<taffy::AvailableSpace>) -> taffy::Size<f32> {
        let t = Table { columns: columns_of(cx.node), state: self.table.state.clone(), ..Table::default() };
        let width: f32 = t.shown().iter().map(|&d| t.columns[d].width()).sum();
        // the header and up to ten rows
        let header = t.metrics.header_height.unwrap_or(t.state.row_height + 4.0);
        let rows = (t.state.len().min(10)) as f32;
        taffy::Size { width: known.width.unwrap_or(width), height: known.height.unwrap_or(header + rows * t.state.row_height) }
    }

    fn prepare(&mut self, cx: &mut PrepareCx) {
        let node = cx.node;
        self.read_columns(node);
        if let Some(t) = cx.service::<Theme>() {
            self.look = TableLook::from_theme(t);
        }
        self.size = node.size();
        self.table.set_viewport(self.size, self.off);
        self.clamp_off();
        self.table.set_viewport(self.size, self.off);
        let (table, look, pending) = (&mut self.table, &self.look, &mut self.pending);
        with_text(cx.service::<skelter_text::TextSystem>(), |p| table.prepare(look, p, pending));
        if let Some(off) = self.table.take_scroll_request() {
            self.off = off;
            self.clamp_off();
            self.table.set_viewport(self.size, self.off);
        }
    }

    fn record(&self, cx: &mut RecordCx) {
        record_core(cx, |p, list| self.table.record(&self.look, p, list, self.now));
    }

    fn event(&mut self, cx: &mut EventCx, ev: &Event) -> Handled {
        self.now = cx.time();
        let mut out = Vec::new();
        let handled = match ev {
            Event::PointerDown(p) => {
                cx.capture_pointer();
                self.table.pointer(&Self::pointer_event(PointerPhase::Down, p), self.now, Self::mac(), &mut out)
            }
            Event::PointerUp(p) => {
                cx.release_pointer();
                self.table.pointer(&Self::pointer_event(PointerPhase::Up, p), self.now, Self::mac(), &mut out)
            }
            Event::PointerMove(p) => self.table.pointer(&Self::pointer_event(PointerPhase::Move, p), self.now, Self::mac(), &mut out),
            Event::PointerCancel => self.table.pointer(&Self::bare(PointerPhase::Cancel), self.now, Self::mac(), &mut out),
            Event::HoverLeave => self.table.pointer(&Self::bare(PointerPhase::Leave), self.now, Self::mac(), &mut out),
            Event::Wheel(w) => {
                let row_h = self.table.state.row_height;
                let d = match w.delta {
                    ScrollDelta::Lines(v) => Vec2::new(v.x * row_h * 3.0, v.y * row_h * 3.0),
                    ScrollDelta::Pixels(v) => v,
                    ScrollDelta::Pages(v) => Vec2::new(v.x * self.size.w, v.y * self.size.h),
                };
                let before = self.off;
                self.off = self.off - d;
                self.clamp_off();
                self.table.set_viewport(self.size, self.off);
                self.off != before
            }
            Event::Key(k) => {
                let used = self.table.key(k, self.now, Self::mac(), &mut out);
                let plain = !k.modifiers.contains(Modifiers::CTRL) && !k.modifiers.contains(Modifiers::SUPER);
                match (&k.text, used) {
                    (Some(t), false) if k.state == KeyState::Pressed && plain && !t.chars().any(char::is_control) => {
                        self.table.text(t, self.now, &mut out);
                        true
                    }
                    _ => used,
                }
            }
            Event::FocusIn(reason) => {
                self.table.set_focus(true, *reason == FocusReason::Keyboard);
                true
            }
            Event::FocusOut => {
                self.table.set_focus(false, false);
                true
            }
            Event::Timer => true,
            _ => false,
        };
        self.deliver(cx, out);
        if handled && !matches!(ev, Event::FocusIn(_) | Event::FocusOut | Event::Timer) { Handled::Yes } else { Handled::No }
    }

    fn call(&mut self, cx: &mut EventCx, method: MethodId, args: &[Value]) -> Result<Option<Value>, PropError> {
        let Some(i) = (0..TABLE_METHODS.len()).find(|&i| TABLE.method_id(i) == method) else {
            return Err(PropError::Rejected(format!("{method:?} is not a method of <table>")));
        };
        let args: Vec<tshim::Value> = args.iter().filter_map(|v| to_model(v, &[])).collect();
        match TABLE_METHODS[i].name {
            "set-rows" => self.table.state.set_rows(&args).map_err(PropError::Rejected)?,
            "set-ids" => self.table.state.set_ids(&args).map_err(PropError::Rejected)?,
            "clear-rows" => self.table.state.clear_rows(),
            "scroll-to-row" => match args.first() {
                Some(tshim::Value::I64(r)) => self.table.state.scroll_to_row(*r),
                _ => return Err(PropError::Rejected("scroll-to-row: expected a row".into())),
            },
            "fit-columns" => self.table.state.fit_columns(match args.first() {
                Some(tshim::Value::I64(c)) if *c >= 0 => Some(*c as usize),
                _ => None,
            }),
            _ => {}
        }
        self.deliver(cx, Vec::new());
        cx.request_layout();
        Ok(None)
    }

    fn a11y(&self, cx: &mut A11yCx) {
        cx.default_a11y();
        cx.out.set_role(Role::Table);
        cx.out.set_row_count(self.table.state.len() as usize);
        cx.out.set_column_count(self.table.shown().len());
    }
}

/// The `table-column` kind: data its table reads; it draws nothing itself.
#[derive(Default)]
pub struct TableColumnKind;

impl Kind for TableColumnKind {
    fn spec() -> &'static KindSpec {
        &TABLE_COLUMN
    }

    fn record(&self, _cx: &mut RecordCx) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn specs_match_the_model_and_register() {
        // every table prop is one the model takes, and reads back
        let state = super::super::state::TableState::default();
        for p in TABLE_PROPS {
            assert!(state.get_prop(p.name).is_some(), "table.{}", p.name);
        }
        let col = ColumnState::default();
        for p in COLUMN_PROPS {
            assert!(col.get_prop(p.name).is_some(), "table-column.{}", p.name);
        }
        let names: Vec<&str> = TABLE_SIGNALS.iter().map(|s| s.name).collect();
        assert_eq!(names, super::super::SIGNALS, "the spec lists the model's signals in order");
        assert_ne!(TABLE.id(), TABLE_COLUMN.id());
        assert!(TABLE.folds_children, "columns are data");
        let mut reg = KindRegistry::new();
        register(&mut reg).expect("table and table-column register");
        assert!(reg.by_name("table").is_some() && reg.by_name("table-column").is_some());
    }

    #[test]
    fn values_convert_both_ways() {
        let order = &TABLE_PROPS[11];
        assert_eq!(order.name, "sort-order");
        assert_eq!(to_model(&Value::Enum(2), order.enum_values), Some(tshim::Value::Str("descending".into())));
        assert_eq!(to_core(&tshim::Value::Str("ascending".into()), Some(order)), Value::Enum(1));
        let range = &TABLE_SIGNALS[0];
        let arg = emit_arg(range, &[tshim::Value::I64(3), tshim::Value::I64(40)]);
        assert_eq!(arg, Some(Value::Record(vec![(Arc::from("first"), Value::I64(3)), (Arc::from("last"), Value::I64(40))].into())));
        assert_eq!(emit_arg(&TABLE_SIGNALS[3], &[tshim::Value::I64(7)]), Some(Value::I64(7)));
    }
}
