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
//! - Cells of `editable` columns are edited in place by the text-edit field ([`CellEditor`] over
//!   [`TextField`](crate::text::field::TextField), the document's text system laying it out): F2 or
//!   typing on the cursor cell starts an edit (typing replaces the value), the field takes the keys,
//!   IME and presses in the cell, and Enter, Tab/Shift+Tab (moving to the next or previous editable
//!   cell) or focus leaving commit (`commit(row, column, value)`; the window shows the value until
//!   JS answers) while Escape reverts. The caret goes out through core's fast channel.
//! - Scrolling: the body's offset is a views [`Scroller`]'s: its bars (`::part(thumb-x)`,
//!   `::part(thumb-y)`, and for classic bars `::part(scrollbar-x)`, `::part(scrollbar-y)`,
//!   `::part(corner)`, the scroll-area's parts and UA tables) sit over the body's right and bottom
//!   edges, with thumb drags, track pages (held: repeated) and overlay fading per the platform
//!   policy; the wheel, keyboard moves and `scroll-to-row` move the same offset, and each move emits
//!   the common `scroll` event. The offset stays the kind's and a scroll re-records the table: the
//!   frozen header and sticky columns would move with a `self_scroll` list (core's scroll-offset
//!   fast path wants a fixed list beside the scrolled one first).
//!
//! - The model's `range` (and a restored `header-state`'s events) come out of `prepare` and are
//!   emitted from there (`PrepareCx::emit`); a user resize writes the `table-column`'s `width` back
//!   as native state (`EventCx::set_native_on`) and reports it (`column-resized`).
//!
//! skelter-core gap this kind works around (listed in `plans/milestones/SKELTER-widgets-tier1.md`):
//! `record` has no clock, so the resize edges' hover delay uses the last event's time.

use super::column::{ALIGNS, ColumnState, ELIDES, HEADER_ALIGNS, SORTS};
use super::selection::Mode;
use super::shim::{self as tshim, Emit, Platform};
use super::text_painter::{record_core, with_text};
use super::{Table, TableLook};
use crate::theme::Theme;
use skelter_base::accesskit::Role;
use skelter_base::change::ChangeClass;
use skelter_base::event::{KeyState, Modifiers, PointerEvent, PointerPhase, PointerType, ScrollDelta};
use crate::interact::{Policy, Scrollbars, StateBits};
use crate::paint::PaintCx;
use crate::shim::core::Styles;
use crate::table::text_painter::{CollectRes, NoShaper};
use crate::text::cell::CellEditor;
use crate::views::scroll::Scroller;
use skelter_base::display::DisplayList;
use crate::text::field::{FieldColors, FieldPointer};
use crate::text::geometry::SystemLayouter;
use crate::text::kind::text_style;
use crate::text::policy::{Platform as TextPlatform, TextPolicy};
use crate::text::shim::{Layouter, MonoLayouter};
use skelter_base::geom::{Point, Rect, Size, Vec2};
use skelter_base::ids::NodeId;
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
    SignalSpec::new("commit", Some(ValueType::Record), P)
        .args(&[ArgSpec::new("row", ValueType::I64), ArgSpec::new("column", ValueType::I64), ArgSpec::new("value", ValueType::Str)])
        .doc("A cell edit was committed (Enter, Tab or focus leaving the editor); carries the row position, the column (child index) and the text."),
];

static TABLE_METHODS: &[MethodSpec] = &[
    MethodSpec::new("set-rows", &[ArgSpec::new("first", ValueType::I64), ArgSpec::new("rows", ValueType::List)], None).doc("Replaces the data window: rows `first..` (each a list of cells)."),
    MethodSpec::new("set-ids", &[ArgSpec::new("ids", ValueType::List), ArgSpec { name: "labels", ty: ValueType::List, nullable: true }], None).doc("Every row's id (and label) by position."),
    MethodSpec::new("clear-rows", &[], None).doc("Drops the data window and asks again for what shows."),
    MethodSpec::new("scroll-to-row", &[ArgSpec::new("row", ValueType::I64)], None).doc("Scrolls the row (position) into view."),
    MethodSpec::new("fit-columns", &[ArgSpec { name: "column", ty: ValueType::I64, nullable: true }], None).doc("Fits one column (child index) or every one to its contents."),
];

/// The scroll bars' paint parts (the scroll-area's).
const BAR_PARTS: &[&str] = &["thumb-x", "thumb-y", "scrollbar-x", "scrollbar-y", "corner"];
/// The bars' thickness and their shortest thumb (points).
const BAR_WIDTH: f32 = 10.0;
const MIN_THUMB: f32 = 20.0;

/// `table`: a virtual table over its `table-column` children.
pub static TABLE: KindSpec = KindSpec {
    props: TABLE_PROPS,
    signals: TABLE_SIGNALS,
    methods: TABLE_METHODS,
    role: Role::Table,
    focusable: true,
    container: true,
    folds_children: true,
    conceals_children: true,
    parts: BAR_PARTS,
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

/// `table-column`'s `width`, which a user resize writes back.
fn column_width() -> PropId {
    TABLE_COLUMN.prop_id(1)
}

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

/// The ids of [`columns_of`]'s nodes.
fn column_nodes_of(node: NodeView<'_>) -> Vec<NodeId> {
    (0..node.children().len()).filter_map(|k| node.child(k)).filter(|c| c.kind().spec.name == TABLE_COLUMN.name).map(|c| c.id()).collect()
}

/// The `table` kind.
#[derive(Default)]
pub struct TableKind {
    pub table: Table,
    look: TableLook,
    /// Each column's `width` prop as last read (a user resize stays until it changes).
    read_widths: Vec<Option<f32>>,
    /// The `table-column` nodes, by data column.
    column_nodes: Vec<NodeId>,
    off: Vec2,
    /// The laid-out size (as of the last prepare).
    size: Size,
    /// The last event's time (the clock `record` lacks).
    now: Time,
    /// The cell being edited.
    edit: Option<Edit>,
    /// The body's scroll offset, geometry and bars.
    scroller: Scroller,
    /// The platform policy (bars, track clicks), from the document's service.
    policy: Policy,
    /// The theme is dark (the bars' UA colours).
    dark: bool,
    /// What each virtual child of the last a11y node is, by index (FQ-66): actions on them.
    a11y_targets: std::cell::RefCell<Vec<A11yTarget>>,
}

/// A virtual child of the table's a11y node: the header row, a column header (data column), a
/// realised row (position) or one of its cells (position, shown column).
#[derive(Clone, Copy, Debug, PartialEq)]
enum A11yTarget {
    HeaderRow,
    Header(usize),
    Row(u64),
    Cell(u64, usize),
}

fn ax_rect(r: Rect) -> skelter_base::accesskit::Rect {
    skelter_base::accesskit::Rect { x0: r.min.x as f64, y0: r.min.y as f64, x1: r.max.x as f64, y1: r.max.y as f64 }
}

/// A cell edit: the cell (row position, data column), its rect (from the last prepare) and the
/// text-edit field over it.
struct Edit {
    pos: u64,
    data: usize,
    rect: Rect,
    editor: CellEditor,
}

impl TableKind {
    /// Re-reads the columns, keeping a width the user dragged while the column's prop is unchanged.
    fn read_columns(&mut self, node: NodeView<'_>) {
        let cols = columns_of(node);
        self.column_nodes = column_nodes_of(node);
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

    /// Delivers `out`, writes back what the model changed itself (its props, and the widths of the
    /// columns the user resized), and applies its focus and scroll requests.
    fn deliver(&mut self, cx: &mut EventCx, out: Vec<Emit>) {
        let changed = self.table.take_changed();
        // a selection, cursor, sort or data change: the rows' a11y nodes follow (a hover does not)
        if !out.is_empty() || !changed.is_empty() {
            cx.request_a11y();
        }
        for name in changed {
            let Some(i) = TABLE_PROPS.iter().position(|p| p.name == name) else { continue };
            if let Some(v) = self.table.state.get_prop(name) {
                let _ = cx.set_native(TABLE.prop_id(i), to_core(&v, Some(&TABLE_PROPS[i])));
            }
        }
        for k in self.table.take_changed_columns() {
            let (Some(&c), Some(w)) = (self.column_nodes.get(k), self.table.columns.get(k).and_then(|c| c.width)) else { continue };
            // the column's prop now holds the user's width (read back as unchanged)
            if cx.set_native_on(c, column_width(), Value::F64(w as f64)).is_ok()
                && let Some(r) = self.read_widths.get_mut(k)
            {
                *r = Some(w);
            }
        }
        for e in &out {
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
        // the soonest of the model's timers, a held track press and the overlay bars' fade
        let wake = [self.table.next_wakeup(self.now), self.scroller.hold_wake(), self.scroller.bar_wake(self.now, &self.policy)].into_iter().flatten().min();
        if let Some(t) = wake {
            cx.wake_at(t);
        }
        cx.set_cursor(self.table.cursor_icon());
        cx.request_paint();
    }

    /// The offset moved (the bars or the wheel): the model follows, the bars show, and the common
    /// `scroll` event goes out.
    fn scrolled(&mut self, cx: &mut EventCx) {
        self.off = self.scroller.offset();
        self.clamp_off();
        self.table.set_viewport(self.size, self.off);
        self.scroller.touch(self.now);
        let rec: Vec<(Arc<str>, Value)> = vec![("scrollLeft".into(), Value::F64(self.off.x as f64)), ("scrollTop".into(), Value::F64(self.off.y as f64))];
        cx.emit(skelter_core::common::SCROLL, Some(Value::Record(rec.into())));
        cx.request_paint();
        // other rows are realised
        cx.request_a11y();
    }

    /// Pointer events over the bars (a thumb drag, a track press and its hold): `Some` when they
    /// took the event.
    fn bar_event(&mut self, cx: &mut EventCx, ev: &Event) -> Option<Handled> {
        let (phase, p) = match ev {
            Event::PointerDown(p) => (PointerPhase::Down, Some(p)),
            Event::PointerMove(p) => (PointerPhase::Move, Some(p)),
            Event::PointerUp(p) => (PointerPhase::Up, Some(p)),
            Event::PointerCancel => (PointerPhase::Cancel, None),
            Event::HoverLeave => (PointerPhase::Leave, None),
            _ => return None,
        };
        let pos = p.map_or(Point::new(-1.0, -1.0), |p| p.local);
        let primary = p.is_some_and(|p| p.button == Some(skelter_base::event::MouseButton::Primary) || (phase != PointerPhase::Down && p.buttons & 1 != 0));
        let shift = p.is_some_and(|p| p.modifiers.contains(Modifiers::SHIFT));
        let hovered = self.scroller.hovered();
        let (ours, moved) = self.scroller.pointer(self.now, &self.policy, phase, primary, pos, shift);
        if moved {
            self.scrolled(cx);
        }
        if hovered != self.scroller.hovered() || ours {
            cx.request_paint();
        }
        // the table sees the hover end and the cancel too
        if !ours || matches!(phase, PointerPhase::Leave | PointerPhase::Cancel) {
            return None;
        }
        match phase {
            PointerPhase::Down => cx.capture_pointer(),
            PointerPhase::Up => cx.release_pointer(),
            _ => {}
        }
        self.deliver(cx, Vec::new());
        Some(Handled::Yes)
    }

    /// Records the bars over the body.
    fn record_bars(&self, cx: &mut RecordCx) {
        let node = cx.node;
        let parts: Vec<(&'static str, &svelters_style::ComputedStyle)> = BAR_PARTS.iter().filter_map(|n| Some((*n, node.part(n)?))).collect();
        let styles = Styles { computed: node.css(), parts: &parts, children: &[], state: StateBits(node.state().0), dark: self.dark, theme: None };
        let classic = self.policy.scrollbars == Scrollbars::Classic;
        let alpha = self.scroller.bar_alpha(self.now, &self.policy);
        let mut list = DisplayList::new();
        let mut res = CollectRes::default();
        let mut none = NoShaper;
        {
            let mut pc = PaintCx::new(&mut list, &mut res, &mut none, node.scale());
            self.scroller.record_bars(&mut pc, &styles, alpha, classic);
        }
        let bounds = list.bounds;
        for item in list.items {
            cx.push(item, bounds);
        }
    }

    /// The cursor cell: its row position and data column.
    fn cursor_cell(&self) -> Option<(u64, usize)> {
        let pos = self.table.state.sel.cursor.and_then(|k| self.table.state.pos_of(k))?;
        let data = *self.table.shown().get(self.table.state.cur_col)?;
        Some((pos, data))
    }

    /// Starts editing cell (`pos`, `data`); `typed` replaces its value (empty: the value, selected).
    fn begin_edit(&mut self, cx: &mut EventCx, pos: u64, data: usize, typed: &str) {
        let value = self.table.state.cell(pos, data).unwrap_or("").to_owned();
        let policy = cx.service::<crate::interact::Policy>().copied().unwrap_or_else(crate::host::os_policy);
        let editor = CellEditor::begin(&value, typed, TextPolicy::from_policy(TextPlatform::current(), &policy));
        self.edit = Some(Edit { pos, data, rect: Rect::ZERO, editor });
        cx.request_paint();
    }

    /// Routes `ev` to the cell editor while one is open, or starts one (F2 or typing on an editable
    /// cursor cell). `Some` when the editor took the event; a press elsewhere and focus leaving end
    /// the edit and go on to the table.
    fn edit_event(&mut self, cx: &mut EventCx, ev: &Event) -> Option<Handled> {
        if self.edit.is_none() {
            let Event::Key(k) = ev else { return None };
            if k.state != KeyState::Pressed || [Modifiers::CTRL, Modifiers::SUPER, Modifiers::ALT].iter().any(|m| k.modifiers.contains(*m)) {
                return None;
            }
            let typed = match (&k.key, &k.text) {
                (Some(skelter_base::key::Key::F2), _) => String::new(),
                (_, Some(t)) if !t.is_empty() && !t.chars().any(char::is_control) => t.clone(),
                _ => return None,
            };
            let (pos, data) = self.cursor_cell().filter(|&(_, d)| self.table.columns.get(d).is_some_and(|c| c.editable))?;
            self.begin_edit(cx, pos, data, &typed);
            return Some(Handled::Yes);
        }
        let style = cx.view(cx.node()).map(|v| text_style(v.style()))?;
        let now = cx.time();
        let took = {
            let edit = self.edit.as_mut().expect("editing");
            let mut mono = MonoLayouter { advance: 8.0, line_height: 16.0 };
            let mut sys: SystemLayouter;
            let lay: &mut dyn Layouter = match cx.service::<skelter_text::TextSystem>() {
                Some(ts) => {
                    sys = SystemLayouter { system: ts, style };
                    &mut sys
                }
                None => &mut mono,
            };
            let field = |phase, p: &skelter_core::PointerInput| FieldPointer::new(phase, p.button, p.buttons, p.local);
            match ev {
                Event::Key(k) => {
                    edit.editor.key(k, now, lay);
                    true
                }
                Event::Ime(i) => {
                    edit.editor.field.ime(i, now, lay);
                    true
                }
                Event::PointerDown(p) if edit.rect.contains(p.local) => {
                    edit.editor.field.pointer(&field(PointerPhase::Down, p), now, lay);
                    true
                }
                Event::PointerMove(p) if p.buttons != 0 && edit.rect.contains(p.local) => edit.editor.field.pointer(&field(PointerPhase::Move, p), now, lay),
                Event::PointerUp(p) if edit.rect.contains(p.local) => {
                    edit.editor.field.pointer(&field(PointerPhase::Up, p), now, lay);
                    true
                }
                Event::PointerDown(_) | Event::FocusOut => {
                    edit.editor.focus_out();
                    false
                }
                _ => return None,
            }
        };
        self.finish_edit(cx);
        took.then_some(Handled::Yes)
    }

    /// After the editor took input: its caret and repaint, and the end of the edit when it asked
    /// (commit or revert, then the move of Tab/Shift+Tab).
    fn finish_edit(&mut self, cx: &mut EventCx) {
        let Some(edit) = self.edit.as_mut() else { return };
        if let Some(r) = edit.editor.field.caret_update() {
            cx.set_caret(r);
        }
        if edit.editor.field.take_repaint() {
            cx.request_paint();
        }
        let Some(end) = edit.editor.take_end() else { return };
        let (pos, data) = (edit.pos, edit.data);
        self.edit = None;
        cx.set_caret(None);
        cx.request_paint();
        if end.commit {
            let i = TABLE_SIGNALS.iter().position(|s| s.name == "commit").expect("the commit signal");
            let rec = vec![(Arc::from("row"), Value::I64(pos as i64)), (Arc::from("column"), Value::I64(data as i64)), (Arc::from("value"), Value::Str(Arc::from(end.text.as_str())))];
            cx.emit(TABLE.signal_id(i), Some(Value::Record(rec.into())));
            // shown until JS answers with `set-rows`
            let st = &mut self.table.state;
            if let Some(cell) = pos.checked_sub(st.first).and_then(|r| st.rows.get_mut(r as usize)).and_then(|row| row.get_mut(data)) {
                *cell = end.text;
            }
        }
        if end.step != 0 {
            let shown = self.table.shown();
            let from = self.table.state.cur_col as i64;
            let next = std::iter::successors(Some(from + end.step as i64), |k| Some(k + end.step as i64))
                .take_while(|k| *k >= 0 && (*k as usize) < shown.len())
                .find(|k| self.table.columns[shown[*k as usize]].editable);
            if let Some(k) = next {
                self.table.state.cur_col = k as usize;
                self.begin_edit(cx, pos, shown[k as usize], "");
            }
        }
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
        cx.request_a11y();
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
            self.dark = t.is_dark();
        }
        if let Some(p) = cx.service::<Policy>() {
            self.policy = *p;
        }
        self.size = node.size();
        self.table.set_viewport(self.size, self.off);
        self.clamp_off();
        self.table.set_viewport(self.size, self.off);
        let mut out = Vec::new();
        let (table, look) = (&mut self.table, &self.look);
        with_text(cx.service::<skelter_text::TextSystem>(), |p| table.prepare(look, p, &mut out));
        for e in &out {
            if let Some(i) = TABLE_SIGNALS.iter().position(|s| s.name == e.signal) {
                cx.emit(TABLE.signal_id(i), emit_arg(&TABLE_SIGNALS[i], &e.args));
            }
        }
        if let Some(off) = self.table.take_scroll_request() {
            self.off = off;
            self.clamp_off();
            self.table.set_viewport(self.size, self.off);
        }
        // the bars over the body, at the offset
        let g = self.table.geom();
        self.scroller.set_geometry(self.table.content_size(), g.body.size());
        self.scroller.set_offset(self.off);
        let show = [self.scroller.overflows(0), self.scroller.overflows(1)];
        self.scroller.layout_bars(g.body, BAR_WIDTH, MIN_THUMB, show);
        // the cell editor over its cell
        let shown = self.table.shown();
        if let Some(edit) = self.edit.as_mut()
            && let Some(k) = shown.iter().position(|&d| d == edit.data)
        {
            let (cr, _) = g.cell_rect(edit.pos, k);
            edit.rect = cr;
            let pad = self.look.cell_padding;
            let content = Rect::new(cr.min.x + pad, cr.min.y, (cr.max.x - pad).max(cr.min.x + pad), cr.max.y);
            let style = text_style(node.style());
            let to_window = node.window_transform();
            match cx.service::<skelter_text::TextSystem>() {
                Some(ts) => edit.editor.field.prepare(&mut SystemLayouter { system: ts, style }, content, to_window),
                None => edit.editor.field.prepare(&mut MonoLayouter { advance: 8.0, line_height: 16.0 }, content, to_window),
            }
            edit.editor.field.take_repaint();
        }
    }

    fn record(&self, cx: &mut RecordCx) {
        record_core(cx, |p, list| self.table.record(&self.look, p, list, self.now));
        if let Some(edit) = &self.edit {
            // the field over the cell's own text
            let q = skelter_base::display::Quad {
                rect: edit.rect,
                radii: Default::default(),
                fill: skelter_base::color::Fill::Solid(self.look.background),
                border_widths: skelter_base::geom::Insets::all(1.0),
                border_color: self.look.focus_ring,
                border_style: Default::default(),
            };
            cx.push(skelter_base::display::Prim::Quad(q), edit.rect);
            let t = self.look.text.color;
            let colors = FieldColors { text: t, placeholder: t.with_alpha(t.a * 0.5), selection: self.look.selected, selection_inactive: self.look.selected_inactive, preedit_underline: t };
            edit.editor.field.record(cx, &colors);
        }
        self.record_bars(cx);
    }

    fn event(&mut self, cx: &mut EventCx, ev: &Event) -> Handled {
        self.now = cx.time();
        if let Some(h) = self.edit_event(cx, ev) {
            return h;
        }
        if let Some(h) = self.bar_event(cx, ev) {
            return h;
        }
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
                self.scroller.set_offset(self.off);
                self.scroller.scroll_by([true, true], Vec2::new(-d.x, -d.y));
                if self.scroller.offset() != before {
                    self.scrolled(cx);
                }
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
            // FQ-66: AccessKit's `Click` on a row, a cell or a column header, as a pointer's click
            Event::A11yChild(req, i) if req.action == skelter_base::accesskit::Action::Click => {
                let target = self.a11y_targets.borrow().get(*i as usize).copied();
                match target {
                    Some(A11yTarget::Header(d)) => self.table.click_header(d, &mut out),
                    Some(A11yTarget::Row(pos)) => self.table.click_row(pos, None, Default::default(), &mut out),
                    Some(A11yTarget::Cell(pos, k)) => self.table.click_row(pos, Some(k), Default::default(), &mut out),
                    Some(A11yTarget::HeaderRow) | None => {}
                }
                target.is_some_and(|t| t != A11yTarget::HeaderRow)
            }
            Event::FocusIn(reason) => {
                self.table.set_focus(true, *reason == FocusReason::Keyboard);
                true
            }
            Event::FocusOut => {
                self.table.set_focus(false, false);
                true
            }
            Event::Timer => {
                let (moved, _) = self.scroller.repeat(self.now);
                if moved {
                    self.scrolled(cx);
                }
                true
            }
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
        cx.request_a11y();
        Ok(None)
    }

    /// The table, its header row of column headers and its realised rows with their cells, as
    /// virtual children (FQ-66): rows and cells named from the cell text, with their row and column
    /// index and selected state, and `Click` (a row's, a cell's: select as a click; a header's:
    /// `header-clicked` and the sort). Only the rows and columns in view, as drawn.
    fn a11y(&self, cx: &mut A11yCx) {
        use skelter_base::accesskit::{Action, Node, SortDirection};
        cx.default_a11y();
        cx.out.set_role(Role::Table);
        let t = &self.table;
        let len = t.state.len();
        let shown = t.shown();
        cx.out.set_row_count(len as usize);
        cx.out.set_column_count(shown.len());
        let g = t.geom();
        let cols: Vec<usize> = g.visible_cols().collect();
        let mut targets = Vec::new();
        let mut head = Node::new(Role::Row);
        head.set_bounds(ax_rect(g.head));
        if let Some(hi) = cx.add_virtual_child(head) {
            targets.push(A11yTarget::HeaderRow);
            for &k in &cols {
                let d = shown[k];
                let col = &t.columns[d];
                let (r, clip) = g.head_rect(k);
                let mut n = Node::new(Role::ColumnHeader);
                n.set_label(col.title.as_str());
                n.set_column_index(k);
                n.set_bounds(ax_rect(r.intersect(&clip)));
                match t.state.shown_sort(d, col) {
                    "ascending" => n.set_sort_direction(SortDirection::Ascending),
                    "descending" => n.set_sort_direction(SortDirection::Descending),
                    _ => {}
                }
                n.add_action(Action::Click);
                if cx.add_virtual_child_of(hi, n).is_none() {
                    break;
                }
                targets.push(A11yTarget::Header(d));
            }
        }
        let (first, end) = g.visible_rows(len);
        'rows: for pos in first..end {
            let selected = t.state.sel.is_selected(t.state.key_of(pos));
            let texts: Vec<&str> = shown.iter().filter_map(|&d| t.state.cell(pos, d)).filter(|s| !s.is_empty()).collect();
            let mut row = Node::new(Role::Row);
            row.set_row_index(pos as usize);
            row.set_bounds(ax_rect(g.row_rect(pos).intersect(&g.body)));
            match texts.is_empty() {
                false => row.set_label(texts.join(" ")),
                true => {
                    if let Some(l) = t.state.label(pos as usize) {
                        row.set_label(l);
                    }
                }
            }
            row.set_selected(selected);
            row.add_action(Action::Click);
            let Some(ri) = cx.add_virtual_child(row) else { break };
            targets.push(A11yTarget::Row(pos));
            for &k in &cols {
                let (r, clip) = g.cell_rect(pos, k);
                let mut n = Node::new(Role::Cell);
                if let Some(text) = t.state.cell(pos, shown[k]) {
                    n.set_label(text);
                }
                n.set_row_index(pos as usize);
                n.set_column_index(k);
                n.set_bounds(ax_rect(r.intersect(&clip)));
                n.set_selected(selected);
                n.add_action(Action::Click);
                if cx.add_virtual_child_of(ri, n).is_none() {
                    break 'rows;
                }
                targets.push(A11yTarget::Cell(pos, k));
            }
        }
        *self.a11y_targets.borrow_mut() = targets;
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

    /// A table with two columns (`width` 100 and 80) and `rows` rows in a document.
    fn table_doc(rows: i64) -> (skelter_core::Document, skelter_base::ids::NodeId, Vec<skelter_base::ids::NodeId>) {
        use crate::testdoc::{child, doc};
        let (mut d, root) = doc(Size::new(400.0, 300.0), |r| register(r).unwrap());
        let t = child(&mut d, root, TABLE.id(), "width: 300px; height: 224px");
        d.set_prop(t, TABLE.prop_named("row-count").unwrap(), Value::I64(rows)).unwrap();
        d.set_prop(t, TABLE.prop_named("stretch-last").unwrap(), Value::Bool(false)).unwrap();
        let cols = [100.0, 80.0]
            .iter()
            .map(|w| {
                let c = child(&mut d, t, TABLE_COLUMN.id(), "");
                d.set_prop(c, column_width(), Value::F64(*w)).unwrap();
                c
            })
            .collect();
        (d, t, cols)
    }

    #[test]
    fn range_is_emitted_from_prepare_without_an_event() {
        let (mut d, t, _) = table_doc(1000);
        let range = TABLE.signal_named("range").unwrap();
        d.listen(t, range, skelter_core::ListenFlags::EMPTY);
        crate::testdoc::frame(&mut d);
        let ev = d.take_events();
        let r = ev.iter().find(|e| e.signal == range && e.target == t).expect("range after the frame, no event needed");
        let arg = r.arg.as_ref().unwrap();
        assert_eq!(arg.field("first"), Some(&Value::I64(0)));
        assert!(arg.field("last").and_then(Value::as_i64).is_some_and(|l| l >= 9), "the visible rows at least: {arg:?}");
    }

    #[test]
    fn a_user_resize_writes_the_column_width_back() {
        use crate::testdoc::{frame, pointer};
        use skelter_base::event::PointerPhase as Ph;
        let (mut d, t, cols) = table_doc(10);
        let resized = TABLE.signal_named("column-resized").unwrap();
        d.listen(t, resized, skelter_core::ListenFlags::EMPTY);
        frame(&mut d);
        d.take_events();
        // the first column's right edge, in the header
        d.dispatch(&pointer(Ph::Move, 100.0, 10.0, 0, false));
        d.dispatch(&pointer(Ph::Down, 100.0, 10.0, 10, false));
        d.dispatch(&pointer(Ph::Move, 130.0, 10.0, 20, true));
        let up = d.dispatch(&pointer(Ph::Up, 130.0, 10.0, 30, false));
        let mut ev: Vec<_> = up.events.into_iter().filter(|e| e.signal == resized).collect();
        ev.extend(d.take_events().into_iter().filter(|e| e.signal == resized));
        assert_eq!(ev.len(), 1, "column-resized reports it");
        assert_eq!(d.view(cols[0]).unwrap().prop_f64(column_width()), Some(130.0), "written to the column");
        assert_eq!(d.view(cols[1]).unwrap().prop_f64(column_width()), Some(80.0));
        // read back next frame as the column's own width: the resize stays
        frame(&mut d);
        assert_eq!(d.view(cols[0]).unwrap().prop_f64(column_width()), Some(130.0));
    }

    #[test]
    fn typing_on_an_editable_cell_edits_through_the_text_field_and_commits() {
        use crate::testdoc::{click, frame, key};
        use skelter_base::key::Key;
        let (mut d, t, cols) = table_doc(3);
        d.set_prop(cols[1], TABLE_COLUMN.prop_named("editable").unwrap(), Value::Bool(true)).unwrap();
        let rows = Value::List(vec![Value::List(vec![Value::from("a"), Value::from("old")].into()), Value::List(vec![Value::from("b"), Value::from("2")].into())].into());
        d.call(t, TABLE.method_id(0), &[Value::I64(0), rows]).unwrap();
        let commit = TABLE.signal_named("commit").unwrap();
        d.listen(t, commit, skelter_core::ListenFlags::EMPTY);
        frame(&mut d);
        // the cursor on row 0's editable cell (the header is 24 high, rows 20; column 1 at 100..180)
        click(&mut d, 140.0, 34.0);
        assert_eq!(d.focused(crate::testdoc::W), Some(t));
        let mut commits: Vec<Value> = Vec::new();
        let send = |d: &mut skelter_core::Document, commits: &mut Vec<Value>, k: Key, text: Option<&str>| {
            let out = d.dispatch(&key(k, text));
            frame(d);
            commits.extend(out.events.into_iter().chain(d.take_events()).filter(|e| e.signal == commit).filter_map(|e| e.arg));
        };
        // typing replaces the value; the field edits, Enter commits
        send(&mut d, &mut commits, Key::A, Some("x"));
        send(&mut d, &mut commits, Key::A, Some("y"));
        send(&mut d, &mut commits, Key::Backspace, None);
        send(&mut d, &mut commits, Key::A, Some("z"));
        send(&mut d, &mut commits, Key::Enter, Some("\r"));
        assert_eq!(commits.len(), 1, "{commits:?}");
        assert_eq!(commits[0].field("row"), Some(&Value::I64(0)));
        assert_eq!(commits[0].field("column"), Some(&Value::I64(1)));
        assert_eq!(commits[0].field("value"), Some(&Value::from("xz")), "Backspace went through the field");
        // F2 selects the (now shown) value: typing replaces it; Escape reverts without a commit
        send(&mut d, &mut commits, Key::F2, None);
        send(&mut d, &mut commits, Key::A, Some("q"));
        send(&mut d, &mut commits, Key::Escape, None);
        assert_eq!(commits.len(), 1, "reverted");
        // Tab commits (no editable cell after it: the edit ends)
        send(&mut d, &mut commits, Key::F2, None);
        // Right collapses the selection to its end on every platform (End moves nothing on macOS)
        send(&mut d, &mut commits, Key::ArrowRight, None);
        send(&mut d, &mut commits, Key::A, Some("!"));
        send(&mut d, &mut commits, Key::Tab, None);
        assert_eq!(commits.last().and_then(|c| c.field("value")), Some(&Value::from("xz!")));
        // a plain key on a non-editable cell is the table's (type-ahead), not an edit
        click(&mut d, 40.0, 54.0);
        send(&mut d, &mut commits, Key::A, Some("b"));
        send(&mut d, &mut commits, Key::Enter, Some("\r"));
        assert_eq!(commits.len(), 2);
    }

    #[test]
    fn the_body_scrolls_by_its_bars_and_emits_scroll() {
        use crate::testdoc::{frame, pointer};
        use skelter_base::event::PointerPhase as Ph;
        let (mut d, t, _) = table_doc(1000);
        assert_eq!(TABLE.parts, BAR_PARTS, "the scroll-area's bar parts");
        let range = TABLE.signal_named("range").unwrap();
        d.listen(t, range, skelter_core::ListenFlags::EMPTY);
        d.listen(t, skelter_core::common::SCROLL, skelter_core::ListenFlags::EMPTY);
        let c = frame(&mut d).expect("a commit");
        // the vertical thumb is recorded at the body's right edge (the body is 300×200 under a 24-point header)
        let list = c.lists.iter().find(|(n, _)| *n == t).map(|(_, l)| (**l).clone()).expect("the table's list");
        let thumb = |l: &DisplayList| {
            l.items.iter().find_map(|i| match i {
                skelter_base::display::Prim::Quad(q) if q.rect.min.x >= 290.0 - 0.5 && q.rect.width() <= BAR_WIDTH + 0.5 && q.rect.height() >= MIN_THUMB - 0.5 => Some(q.rect),
                _ => None,
            })
        };
        let r = thumb(&list).expect("the thumb");
        assert_eq!((r.min.y, r.height()), (24.0, MIN_THUMB), "at the top, the shortest thumb");
        d.take_events();
        // drag the thumb halfway down the track
        let x = (r.min.x + r.max.x) / 2.0;
        d.dispatch(&pointer(Ph::Down, x, 30.0, 10, false));
        let mv = d.dispatch(&pointer(Ph::Move, x, 30.0 + 90.0, 20, true));
        d.dispatch(&pointer(Ph::Up, x, 120.0, 30, false));
        let scroll = mv.events.iter().find(|e| e.signal == skelter_core::common::SCROLL).expect("the common scroll event").arg.clone().unwrap();
        let top = scroll.field("scrollTop").and_then(Value::as_f64).unwrap();
        // 90 of the track's 180 free points: half of the 19 800-point range
        assert!((top - 9900.0).abs() < 1.0, "{top}");
        frame(&mut d);
        let r = d.take_events().into_iter().find(|e| e.signal == range).expect("the rows now in view").arg.unwrap();
        assert!(r.field("first").and_then(Value::as_i64).is_some_and(|f| f > 400 && f <= 495), "{r:?}");
        // a press on the track below the thumb pages down
        let page = d.dispatch(&pointer(Ph::Down, x, 210.0, 40, false));
        d.dispatch(&pointer(Ph::Up, x, 210.0, 50, false));
        let top2 = page.events.iter().find(|e| e.signal == skelter_core::common::SCROLL).and_then(|e| e.arg.as_ref()?.field("scrollTop")?.as_f64()).expect("paged");
        assert!(top2 > top, "{top2} after {top}");
    }
}
