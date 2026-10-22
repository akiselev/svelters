//! WG4a, the Tier 1 views of EGUI.md §5 (plans/milestones/WG4a-tier1-views.md): `table` with its
//! `table-column`s, `tab-view` with its `tab`s, `split-view` and `toast-host`. Their entries are
//! appended to [`KINDS`](super::KINDS); state, layout and paint are in `crate::views`.
//!
//! - `table` (`egui_table`: virtual rows and columns). Columns are `<table-column>` children (data,
//!   never laid out or drawn as nodes). Rows are JS data in a window: the table emits `range(first,
//!   last)` for the rows it shows (plus `overscan`), JS answers with `set-rows(first, rows)` (a list
//!   of rows, each a list of cell strings). Sending every row once (`set-rows(0, all)`) also works.
//! - `tab-view`: its children are pages (`<tab title>`, a flex box); the bar shows their titles,
//!   only the `selected` page is laid out and drawn.
//! - `split-view`: two panes (its first two children) with a draggable divider; `position` is the
//!   first pane's size in points (`bind:position`).
//! - `toast-host`: draws the toasts of its viewport in a corner; `show(text, level?, duration?)`
//!   from JS (`bind:this`) or `svelters_egui::toast::show` from Rust.

use super::{Def, KindDef, MethodDef, Native, ORIENTATION, P, PropDef, SignalDef, Ty, input, prop, signal};

pub const CELL_ALIGN: &[&str] = &["start", "center", "end"];
pub const SORT: &[&str] = &["none", "ascending", "descending"];
pub const TOAST_CORNER: &[&str] = &["bottom-right", "bottom-left", "top-right", "top-left"];
pub const TOAST_LEVEL: &[&str] = &["info", "success", "warning", "error"];

const NONE: &[(&str, Ty)] = &[];

/// What a WG4a container's slot accepts (the spec's `slots.<name>.accepts`): a `table` takes only
/// `table-column`s; `None` for every other kind ([`super::slot_accepts`] asks the other families).
pub fn slot_accepts(id: &str) -> Option<&'static [&'static str]> {
    (id == "Table").then_some(&["TableColumn"])
}

// ---- table ----

static TABLE_PROPS: &[PropDef] = &[
    prop("row-count", P::RowCount, Ty::I64, Def::I64(0), "Number of rows (the scroll extent). Rows without data yet are drawn empty."),
    prop("row-height", P::RowHeight, Ty::F64, Def::F64(20.0), "Height of every row (and of the header row), in points."),
    prop("sticky-columns", P::StickyColumns, Ty::I64, Def::I64(0), "Number of leading columns that do not scroll horizontally."),
    prop("striped", P::Striped, Ty::Bool, Def::Bool(true), "Alternate rows get a faint background."),
    prop(
        "overscan",
        P::Overscan,
        Ty::I64,
        Def::I64(20),
        "Rows beyond the visible ones that `range` asks for on each side (data ready before it scrolls into view).",
    ),
    input("selected", P::SelectedIndex, Ty::I64, Def::I64(-1), "The selected row, -1 for none; a click on a row (or the arrow keys) changes it."),
];

static TABLE_SIGNALS: &[SignalDef] = &[
    signal(
        "range",
        &[("first", Ty::I64), ("last", Ty::I64)],
        "The rows the table needs data for changed: `first..=last` (the visible rows plus `overscan`); answer with `set-rows`.",
    ),
    signal("changed", &[("selected", Ty::I64)], "The user selected a row; carries its index."),
    signal("activated", &[("row", Ty::I64)], "A row was double-clicked (or Enter pressed on the selection)."),
    signal("header-clicked", &[("column", Ty::I64)], "A column header was clicked (sort on it in JS and set the column's `sort`)."),
];

static TABLE_METHODS: &[MethodDef] = &[
    MethodDef {
        name: "set-rows",
        doc: "`set-rows(first, rows)`: the data window, rows `first..first + rows.length`, each row a list of cell values (strings, numbers, booleans; null for empty). Replaces the previous window.",
    },
    MethodDef { name: "clear-rows", doc: "Drops the data window (every row draws empty until `set-rows`)." },
    MethodDef { name: "scroll-to-row", doc: "`scroll-to-row(row)`: scrolls the row into view on the next frame." },
];

static TABLE_COLUMN_PROPS: &[PropDef] = &[
    prop("title", P::Title, Ty::Str, Def::Str(""), "The header text."),
    prop("resizable", P::Resizable, Ty::Bool, Def::Bool(true), "Its right edge can be dragged (double-click: fit the content)."),
    prop("align", P::CellAlign, Ty::Enum(CELL_ALIGN), Def::Str("start"), "Horizontal alignment of its cells."),
    prop("sort", P::Sort, Ty::Enum(SORT), Def::Str("none"), "The sort arrow its header shows (the data is sorted by JS)."),
];

// ---- tab-view ----

static TAB_VIEW_PROPS: &[PropDef] = &[input("selected", P::SelectedIndex, Ty::I64, Def::I64(0), "Index of the shown page; a click on a tab changes it.")];

static TAB_VIEW_SIGNALS: &[SignalDef] = &[signal("changed", &[("selected", Ty::I64)], "The user picked a tab; carries its index.")];

static TAB_PROPS: &[PropDef] = &[
    prop("title", P::Title, Ty::Str, Def::Str(""), "The tab's caption (also its accessible name)."),
    prop("closable", P::Closable, Ty::Bool, Def::Bool(false), "The tab shows a close button (`close-requested`)."),
];

static TAB_SIGNALS: &[SignalDef] =
    &[signal("close-requested", NONE, "The tab's close button was clicked (or middle click on the tab); remove the tab to close it.")];

// ---- split-view ----

static SPLIT_VIEW_PROPS: &[PropDef] = &[
    prop(
        "orientation",
        P::Orientation,
        Ty::Enum(ORIENTATION),
        Def::Str("horizontal"),
        "`horizontal`: the panes side by side (a vertical divider); `vertical`: one above the other.",
    ),
    PropDef {
        name: "position",
        key: P::Position,
        ty: Ty::F64,
        default: Def::Null,
        nullable: true,
        native: Native::Widget,
        layout: false,
        doc: "Size of the first pane in points along the split; null for half. Dragging the divider changes it (`bind:position`); it is clamped by `min-size` when laid out.",
    },
    prop("min-size", P::MinSize, Ty::F64, Def::F64(32.0), "Smallest size of either pane, in points."),
    prop("resizable", P::Resizable, Ty::Bool, Def::Bool(true), "The divider can be dragged."),
];

static SPLIT_VIEW_SIGNALS: &[SignalDef] = &[
    signal("changed", &[("position", Ty::F64)], "The user dragged the divider; carries the first pane's new size."),
    signal("drag-started", NONE, "A divider drag began."),
    signal("drag-stopped", NONE, "A divider drag ended."),
];

// ---- toast-host ----

static TOAST_HOST_PROPS: &[PropDef] = &[
    prop("corner", P::Corner, Ty::Enum(TOAST_CORNER), Def::Str("bottom-right"), "The viewport corner the toasts stack in."),
    prop("duration", P::Duration, Ty::F64, Def::F64(4.0), "Seconds a toast stays when `show` gives none; 0 keeps it until closed."),
];

static TOAST_HOST_SIGNALS: &[SignalDef] = &[
    signal(
        "closed",
        &[("id", Ty::I64), ("reason", Ty::Str)],
        "A toast went away: `reason` is `timeout`, `dismissed` (its close button), `method` (`dismiss`/`clear`).",
    ),
    signal("clicked", &[("id", Ty::I64)], "A toast's body was clicked."),
];

static TOAST_HOST_METHODS: &[MethodDef] = &[
    MethodDef {
        name: "show",
        doc: "`show(text, level = 'info', duration = the host's)`: shows a toast (level: info, success, warning, error; duration in seconds, 0 = until closed) and returns its id.",
    },
    MethodDef { name: "dismiss", doc: "`dismiss(id)`: closes that toast (`closed(id, 'method')`)." },
    MethodDef { name: "clear", doc: "Closes every toast." },
];

pub(super) const TABLE: KindDef = KindDef {
    id: "Table",
    tag: Some("table"),
    native: "egui_table::Table",
    base: Some("Widget"),
    toplevel: false,
    slot: Some("columns"),
    text_content: None,
    present: None,
    props: TABLE_PROPS,
    signals: TABLE_SIGNALS,
    methods: TABLE_METHODS,
    doc: "A virtual table (`egui_table`): only the visible rows and columns are drawn. Its children are `table-column`s; its rows are a JS data window (`range` asks, `set-rows` answers). Give it a size or let flex-grow size it.",
};

pub(super) const TABLE_COLUMN: KindDef = KindDef {
    id: "TableColumn",
    tag: Some("table-column"),
    native: "egui_table::Column",
    base: Some("Widget"),
    toplevel: false,
    slot: None,
    text_content: None,
    present: None,
    props: TABLE_COLUMN_PROPS,
    signals: &[],
    methods: &[],
    doc: "A column of its parent `table` (data only: not laid out or drawn as a node). Its `width` (Widget's, in points; null for 100) is the column's width; a user resize keeps its width until the prop changes.",
};

pub(super) const TAB_VIEW: KindDef = KindDef {
    id: "TabView",
    tag: Some("tab-view"),
    native: "tab bar (egui selectable labels) + the selected page",
    base: Some("Widget"),
    toplevel: false,
    slot: Some("pages"),
    text_content: None,
    present: None,
    props: TAB_VIEW_PROPS,
    signals: TAB_VIEW_SIGNALS,
    methods: &[],
    doc: "Pages under a tab bar: each child (a `tab`) is a page titled by its `title`; only the `selected` one is laid out and drawn.",
};

pub(super) const TAB: KindDef = KindDef {
    id: "Tab",
    tag: Some("tab"),
    native: "taffy flexbox (a tab-view page)",
    base: Some("Box"),
    toplevel: false,
    slot: Some("children"),
    text_content: None,
    present: None,
    props: TAB_PROPS,
    signals: TAB_SIGNALS,
    methods: &[],
    doc: "A page of a `tab-view`: a flex box whose `title` is its tab's caption.",
};

pub(super) const SPLIT_VIEW: KindDef = KindDef {
    id: "SplitView",
    tag: Some("split-view"),
    native: "two panes + a draggable divider (egui drag sense)",
    base: Some("Widget"),
    toplevel: false,
    slot: Some("panes"),
    text_content: None,
    present: None,
    props: SPLIT_VIEW_PROPS,
    signals: SPLIT_VIEW_SIGNALS,
    methods: &[],
    doc: "Two panes (its first two children; more are not shown) split by a divider the user drags. `position` is the first pane's size.",
};

pub(super) const TOAST_HOST: KindDef = KindDef {
    id: "ToastHost",
    tag: Some("toast-host"),
    native: "egui::Area (foreground) of egui::Frame toasts",
    base: Some("Widget"),
    toplevel: false,
    slot: None,
    text_content: None,
    present: None,
    props: TOAST_HOST_PROPS,
    signals: TOAST_HOST_SIGNALS,
    methods: TOAST_HOST_METHODS,
    doc: "Shows transient notifications stacked in a corner of its viewport, over everything; takes no space itself. `show` from JS (`bind:this`), `svelters_egui::toast::show` from Rust.",
};
