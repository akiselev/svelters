//! WG4a, the Tier 1 views of EGUI.md §5 (plans/milestones/WG4a-tier1-views.md): `table` with its
//! `table-column`s, `tab-view` with its `tab`s, `split-view` and `toast-host`. Their entries are
//! appended to [`KINDS`](super::KINDS); state, layout and paint are in `crate::views`.
//!
//! - `table` (own, OW5b: virtual rows and columns). Columns are `<table-column>` children (data,
//!   never laid out or drawn as nodes). Rows are JS data in a window: the table emits `range(first,
//!   last)` for the rows it shows (plus `overscan`), JS answers with `set-rows(first, rows)` (a list
//!   of rows, each a list of cell strings). Sending every row once (`set-rows(0, all)`) also works.
//! - `tab-view`: its children are pages (`<tab title>`, a flex box); the bar shows their titles,
//!   only the `selected` page is laid out and drawn.
//! - `split-view`: two panes (its first two children) with a draggable divider; `divider-position` is the
//!   first pane's size in points (`bind:divider-position`).
//! - `toast-host`: draws the toasts of its viewport in a corner; `show(text, level?, duration?)`
//!   from JS (`bind:this`) or `svelters_egui::toast::show` from Rust.

use super::{Def, KindDef, MethodDef, Native, ORIENTATION, P, PropDef, SignalDef, Ty, input, optional, prop, signal};

pub const CELL_ALIGN: &[&str] = &["start", "center", "end"];
pub const SORT: &[&str] = &["none", "ascending", "descending"];
/// OW5b: `table-column`'s `header-align` (`auto`: the column's `align`).
pub const HEADER_ALIGN: &[&str] = &["auto", "start", "center", "end"];
/// OW5b: `table-column`'s `elide` (Qt `textElideMode`; WG6 QW13's modes).
pub const ELIDE: &[&str] = &["end", "middle", "start", "none"];
/// WG5a: `table`'s `selection` (the same modes as `virtual-list`'s).
pub const TABLE_SELECTION: &[&str] = &["none", "single", "multi"];
pub const TOAST_CORNER: &[&str] = &["bottom-right", "bottom-left", "top-right", "top-left"];
pub const TOAST_LEVEL: &[&str] = &["info", "success", "warning", "error"];

const NONE: &[(&str, Ty)] = &[];

/// What a WG4a container's slot accepts (the spec's `slots.<name>.accepts`): a `table` takes only
/// `table-column`s; `None` for every other kind ([`super::slot_accepts`] asks the other families).
pub fn slot_accepts(id: &str) -> Option<&'static [&'static str]> {
    match id {
        "Table" => Some(&["TableColumn"]),
        "ToastHost" => Some(&["Toast"]), // OW7b
        _ => None,
    }
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
    // WG5a: selection keyed by row id (a row's id is its position until `set-ids`)
    input(
        "selected",
        P::SelectedIds,
        Ty::IdList,
        Def::Null,
        "The selected row ids (`bind:selected`); clicks (Ctrl/Shift with `selection=\"multi\"`) and the keyboard change it. Kept by id across `set-ids` (sorts, filters).",
    ),
    prop("selection", P::ListSelection, Ty::Enum(TABLE_SELECTION), Def::Str("single"), "`single`, `multi` (Ctrl/Shift clicks and keys, ranges) or `none` (a cursor only)."),
    input("current", P::CurrentRow, Ty::I64, Def::I64(-1), "The cursor row's id (keyboard focus inside the table), -1 for none."),
    input("current-column", P::CurrentColumn, Ty::I64, Def::I64(0), "The cursor column (Left/Right move it; F2 and typing edit that cell)."),
    // OW5b (FM1's QW4, Qt's QHeaderView: sort indicator, hidden sections, saveState/restoreState)
    prop(
        "sortable",
        P::Sortable,
        Ty::Bool,
        Def::Bool(false),
        "A header click sorts on that column: `sort-column` becomes it, `sort-order` ascending, or flips when it already was (the rows are still sorted by JS, on `sort-changed`).",
    ),
    input("sort-column", P::SortColumn, Ty::I64, Def::I64(-1), "The column (child index) the header shows sorted, -1 for none: then each column's own `sort` shows."),
    input("sort-order", P::SortOrder, Ty::Enum(SORT), Def::Str("none"), "The order shown on `sort-column`."),
    prop("column-menu", P::ColumnMenu, Ty::Bool, Def::Bool(true), "A right click on the header opens a menu that hides and shows the `hideable` columns (after `header-context`, unless prevented)."),
    input(
        "hidden-columns",
        P::HiddenColumns,
        Ty::StrList,
        Def::Null,
        "The hidden columns, by `name` (a column without one by its child index, `\"2\"`). The column menu changes it.",
    ),
    input(
        "header-state",
        P::HeaderState,
        Ty::Str,
        Def::Str(""),
        "The header as JSON (Qt `saveState`): `{\"widths\":{name:points},\"hidden\":[name],\"sort\":{\"column\":name,\"order\":…}}`. The table writes it after the user resizes, hides or sorts; setting it restores them. Persist it with DX1's `uiState()` (`bind:header-state={ui.header}`): the table itself never writes files.",
    ),
    prop("stretch-last", P::StretchLast, Ty::Bool, Def::Bool(true), "The last shown column fills the width the columns leave (Qt `stretchLastSection`)."),
];

static TABLE_SIGNALS: &[SignalDef] = &[
    signal(
        "range",
        &[("first", Ty::I64), ("last", Ty::I64)],
        "The rows the table needs data for changed: `first..=last` (the visible rows plus `overscan`); answer with `set-rows`.",
    ),
    signal("changed", &[("selected", Ty::IdList)], "The user changed the selection; carries the selected row ids (`bind:selected`)."),
    signal("selectionchange", &[("ids", Ty::IdList)], "The user changed the selection; carries the selected row ids."),
    signal("activated", &[("row", Ty::I64)], "A row was double-clicked (a cell of a column that is not `editable`) or Enter pressed on the cursor row; carries its position."),
    SignalDef {
        name: "commit",
        args: &[("row", Ty::I64), ("column", Ty::I64), ("value", Ty::Provider)],
        sync: true,
        doc: "An edit is committed: row id, column index, the editor's value. Accept by returning nothing; reject with `e.result = 'reason'` (shown under the cell) or `e.preventDefault()`: the cell reverts.",
    },
    signal("header-clicked", &[("column", Ty::I64)], "A column header was clicked (sort on it in JS and set the column's `sort`)."),
    // OW5b
    signal("header-double-clicked", &[("column", Ty::I64)], "A column header was double-clicked (not its resize edge); carries the column (child index)."),
    signal(
        "header-context",
        &[("column", Ty::I64)],
        "A right click on the header (Menu key on the focused table); carries the column under the pointer. Unless prevented, `column-menu` opens.",
    ),
    signal("column-resized", &[("column", Ty::I64), ("width", Ty::F64)], "The user resized a column (drag released, a double-click fit, `fit-columns`); carries the column and its width. The column's `width` is bindable."),
    signal(
        "sort-changed",
        &[("sort-column", Ty::I64), ("sort-order", Ty::Enum(SORT))],
        "A header click on a `sortable` table changed the sort; carries the column and the order (sort the rows in JS).",
    ),
    signal("columns-changed", &[("hidden-columns", Ty::StrList)], "The column menu hid or showed a column; carries the hidden columns."),
    signal("header-state-changed", &[("header-state", Ty::Str)], "The user resized, hid or sorted; carries the new `header-state` (save it)."),
    signal("copy", &[("ids", Ty::IdList)], "Ctrl+C (Cmd+C) on the focused table; carries the selected row ids (the app copies the data it holds)."),
];

static TABLE_METHODS: &[MethodDef] = &[
    MethodDef {
        name: "set-rows",
        doc: "`set-rows(first, rows)`: the data window, rows `first..first + rows.length`, each row a list of cell values (strings, numbers, booleans; null for empty). Replaces the previous window.",
    },
    MethodDef { name: "clear-rows", doc: "Drops the data window (every row draws empty until `set-rows`)." },
    MethodDef { name: "scroll-to-row", doc: "`scroll-to-row(row)`: scrolls the row into view on the next frame." },
    MethodDef {
        name: "set-ids",
        doc: "`set-ids(ids, labels?)`: the id of every row by position (and the text type-ahead matches); the row count is then `ids.length`. Selection, cursor and edits follow ids across sorts and filters. null: ids are positions again.",
    },
    MethodDef { name: "edit", doc: "`edit(rowId, column)`: starts editing that cell (its column must be `editable`)." },
    MethodDef { name: "end-edit", doc: "`end-edit(commit)`: ends the open edit; true commits (`commit`), false reverts." },
    MethodDef {
        name: "fit-columns",
        doc: "`fit-columns(column?)`: sizes that column (child index), or every resizable shown column, to its header and the cells in the data window (Qt `resizeColumnsToContents`); `column-resized` for each.",
    },
];

static TABLE_COLUMN_PROPS: &[PropDef] = &[
    prop("title", P::Title, Ty::Str, Def::Str(""), "The header text."),
    prop("resizable", P::Resizable, Ty::Bool, Def::Bool(true), "Its right edge can be dragged (double-click: fit the content)."),
    prop("align", P::CellAlign, Ty::Enum(CELL_ALIGN), Def::Str("start"), "Horizontal alignment of its cells."),
    prop("sort", P::Sort, Ty::Enum(SORT), Def::Str("none"), "The sort arrow its header shows (the data is sorted by JS) while the table's `sort-column` is -1."),
    prop("editable", P::Editable, Ty::Bool, Def::Bool(false), "Its cells can be edited (F2, typing, double click); the editor is `editor`, else a text field."),
    prop(
        "editor",
        P::Editor,
        Ty::Snippet,
        Def::Bool(false),
        "The cell editor, a snippet taking the edit (`{ row, column, value (bindable), typed, commit(), cancel() }`); realised once and rebound per edit.",
    ),
    // OW5b (FM1's QW4 `name`, `hideable`; WIDGET-PARITY §2.18.5)
    prop("name", P::Name, Ty::Str, Def::Str(""), "Its name in `hidden-columns` and `header-state` (empty: its child index)."),
    prop("hideable", P::Hideable, Ty::Bool, Def::Bool(true), "The column menu lists it (it can be hidden)."),
    prop("header-align", P::HeaderAlign, Ty::Enum(HEADER_ALIGN), Def::Str("auto"), "Alignment of its header text (`auto`: its `align`)."),
    prop("elide", P::Elide, Ty::Enum(ELIDE), Def::Str("end"), "Where text too wide for a cell or the header is cut with `…` (`none`: clipped)."),
];

static TABLE_COLUMN_SIGNALS: &[SignalDef] = &[
    SignalDef { name: "editor-bind", args: &[("row", Ty::I64), ("value", Ty::Str), ("typed", Ty::Str)], sync: true, doc: "Runtime protocol: an edit of this column's cell starts (the editor snippet's outlet is `set-editor`)." },
    SignalDef { name: "editor-value", args: &[], sync: true, doc: "Runtime protocol: the editor's value to commit (the handler sets `e.result`)." },
    SignalDef { name: "editor-unbind", args: &[], sync: true, doc: "Runtime protocol: the edit ended; the editor stays realised for the next one." },
];

static TABLE_COLUMN_METHODS: &[MethodDef] = &[
    MethodDef { name: "set-editor", doc: "Runtime protocol: `set-editor(outlet)`, the realised editor snippet of this column (null drops it)." },
    MethodDef { name: "end-edit", doc: "`end-edit(commit)`: ends this column's open edit; true commits, false reverts." },
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
        name: "divider-position",
        key: P::DividerPosition,
        ty: Ty::F64,
        default: Def::Null,
        nullable: true,
        native: Native::Widget,
        layout: false,
        doc: "Size of the first pane in points along the split; null for half. Dragging the divider changes it (`bind:divider-position`); it is clamped by `min-size` when laid out.",
    },
    prop("min-size", P::MinSize, Ty::F64, Def::F64(32.0), "Smallest size of either pane, in points."),
    prop("resizable", P::Resizable, Ty::Bool, Def::Bool(true), "The divider can be dragged."),
    // ---- OW2: appended (N panes, WIDGET-PARITY §2.17) ----
    PropDef {
        name: "sizes",
        key: P::Sizes,
        ty: Ty::NumList,
        default: Def::Null,
        nullable: true,
        native: Native::Widget,
        layout: false,
        doc: "OW2: every pane's size in points along the split (`bind:sizes`, Qt's `sizes`); null: from `divider-position` (two panes) or equal shares. When the split view grows or shrinks, the difference goes to the panes by their `flex-grow` (Qt's stretch factors; the last pane when none has one). Each pane keeps `min-size` and its own `min-width`/`max-width` (or heights).",
    },
    prop(
        "collapsible",
        P::Collapsible,
        Ty::Bool,
        Def::Bool(false),
        "OW2: dragging a sash below half a pane's minimum collapses the pane to 0 (Qt's `childrenCollapsible`); Enter on a focused sash collapses or restores.",
    ),
    prop(
        "restore-on-double-click",
        P::RestoreOnDoubleClick,
        Ty::Bool,
        Def::Bool(true),
        "OW2: a double click on a sash restores the pane before it to its size before a collapse, or collapses it (`collapsible`).",
    ),
    prop(
        "commit-mode",
        P::CommitMode,
        Ty::Enum(COMMIT_MODE),
        Def::Str("live"),
        "OW2: `live` moves the panes while dragging; `release` draws a guide line and moves them once, on release (Qt's `opaqueResize = false`).",
    ),
];

/// OW2: when a drag writes its value (`split-view`'s `commit-mode`).
pub const COMMIT_MODE: &[&str] = &["live", "release"];

static SPLIT_VIEW_SIGNALS: &[SignalDef] = &[
    signal("changed", &[("divider-position", Ty::F64)], "The user dragged the divider; carries the first pane's new size."),
    signal("drag-started", NONE, "A divider drag began."),
    signal("drag-stopped", NONE, "A divider drag ended."),
    // ---- OW2: appended ----
    signal("commit", &[("sizes", Ty::NumList)], "OW2: the user finished a resize (drag released, a key step, a restore); carries every pane's size."),
];

// ---- scrollbar (OW2) ----

static SCROLLBAR_PROPS: &[PropDef] = &[
    input("value", P::Value, Ty::F64, Def::F64(0.0), "The position, from `min` to `max` (`bind:value`): the start of the visible part of what it scrolls."),
    prop("min", P::Min, Ty::F64, Def::F64(0.0), "The smallest value."),
    prop("max", P::Max, Ty::F64, Def::F64(100.0), "The largest value (the content's size less `page`)."),
    prop("page", P::PageSize, Ty::F64, Def::F64(10.0), "The visible part's size: the thumb's length in proportion, and what a press on the track scrolls."),
    prop("step", P::Step, Ty::F64, Def::F64(1.0), "What AccessKit's Increment and Decrement move."),
    prop("orientation", P::Orientation, Ty::Enum(ORIENTATION), Def::Str("vertical"), "`vertical` or `horizontal`."),
];

static SCROLLBAR_SIGNALS: &[SignalDef] = &[signal("changed", &[("value", Ty::F64)], "The user moved it (thumb, track, wheel, AccessKit); carries the new value.")];

pub(super) const SCROLLBAR: KindDef = KindDef {
    id: "Scrollbar",
    tag: Some("scrollbar"),
    native: "svelters' own (Scrollable; OW2)",
    base: Some("Widget"),
    toplevel: false,
    slot: None,
    text_content: None,
    present: None,
    props: SCROLLBAR_PROPS,
    signals: SCROLLBAR_SIGNALS,
    methods: &[],
    doc: "A standalone scroll bar (Qt's `QScrollBar`) for a view that scrolls itself (a canvas, a plot, cadatomic's viewport): `value` from `min` to `max`, the thumb sized by `page`. It looks like a scroll area's bar (overlay, VS Code's `scrollbarSlider.*`), always shown.",
};

// ---- toast-host ----

static TOAST_HOST_PROPS: &[PropDef] = &[
    prop("corner", P::Corner, Ty::Enum(TOAST_CORNER), Def::Str("bottom-right"), "The viewport corner the toasts stack in."),
    prop("duration", P::Duration, Ty::F64, Def::F64(4.0), "Seconds a toast stays when `show` gives none; 0 keeps it until closed."),
];

static TOAST_HOST_SIGNALS: &[SignalDef] = &[
    signal(
        "closed",
        &[("id", Ty::I64), ("reason", Ty::Str)],
        "A toast went away: `reason` is `timeout`, `dismissed` (its close button), `action` (one of its actions), `method` (`dismiss`/`clear`).",
    ),
    signal("clicked", &[("id", Ty::I64)], "A toast's body was clicked."),
    signal("action", &[("id", Ty::I64), ("index", Ty::I64)], "One of a toast's action buttons was pressed (`index` into `show`'s `actions`); the toast then closes (`closed(id, 'action')`)."),
];

static TOAST_HOST_METHODS: &[MethodDef] = &[
    MethodDef {
        name: "show",
        doc: "`show(text, level = 'info', duration = the host's, actions = [])`: shows a toast (level: info, success, warning, error; duration in seconds, 0 = until closed; `actions`: button labels, see `action`) and returns its id. `text` may be a run list (RT2): strings and `{text, b, i, code, href}` objects, drawn as one paragraph with the theme's run defaults (per-run CSS needs a declarative `<toast>` child, whose runs are nodes); a click on a run with an `href` opens it.",
    },
    MethodDef { name: "dismiss", doc: "`dismiss(id)`: closes that toast (`closed(id, 'method')`)." },
    MethodDef { name: "clear", doc: "Closes every toast." },
];

pub(super) const TABLE: KindDef = KindDef {
    id: "Table",
    tag: Some("table"),
    native: "own (OW5b)",
    base: Some("Widget"),
    toplevel: false,
    slot: Some("columns"),
    text_content: None,
    present: None,
    props: TABLE_PROPS,
    signals: TABLE_SIGNALS,
    methods: TABLE_METHODS,
    doc: "A virtual table, svelters' own (OW5b): only the visible rows and columns are drawn, under a frozen header; rows, cells, header cells, sort arrows and resize edges are CSS parts. Its children are `table-column`s; its rows are a JS data window (`range` asks, `set-rows` answers), keyed by `set-ids`. Keyboard navigation, type-ahead, keyed selection and cell editing (WG5a). Give it a size or let flex-grow size it.",
};

pub(super) const TABLE_COLUMN: KindDef = KindDef {
    id: "TableColumn",
    tag: Some("table-column"),
    native: "own (OW5b)",
    base: Some("Widget"),
    toplevel: false,
    slot: None,
    text_content: None,
    present: None,
    props: TABLE_COLUMN_PROPS,
    signals: TABLE_COLUMN_SIGNALS,
    methods: TABLE_COLUMN_METHODS,
    doc: "A column of its parent `table` (data only: not laid out or drawn as a node). Its `width` (Widget's, in points; null for 100; `min-width`/`max-width` clamp it) is the column's width; a user resize writes it (`bind:width`). `visible` false hides it; `tooltip` shows on its header; `table-column::part(header)` styles its header cell.",
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
    doc: "Panes (its children) side by side or stacked, split by sashes the user drags or moves with the keys (R11: a 1 px line, a 4 px hit area, a focus-coloured bar and grip after 300 ms of hover). `divider-position` is the first pane's size; `sizes` every pane's.",
};

// ---- toast (OW7b) ----

static TOAST_PROPS: &[PropDef] = &[
    prop("level", P::Level, Ty::Enum(TOAST_LEVEL), Def::Str("info"), "Its severity: the accent bar's colour (`::part(accent)`), the AccessKit politeness (`assertive` for warning and error) and the `[level=…]` selector."),
    optional("duration", P::Duration, Ty::F64, "Seconds it stays open after it shows (null: the host's `duration`; 0: until closed). Hovering holds it open."),
    input("open", P::Open, Ty::Bool, Def::Bool(true), "Shown while true. The host sets it false when the toast closes (`bind:open`); setting it true again shows it anew, with a fresh timeout."),
];

static TOAST_SIGNALS: &[SignalDef] = &[
    signal("changed", &[("open", Ty::Bool)], "It closed (or showed again); carries `open`."),
    signal(
        "closed",
        &[("reason", Ty::Str)],
        "It closed (`open` is now false): `reason` is `timeout`, `dismissed` (its close button), `action` (a click on one of its child buttons) or `\"\"` (`open` set false).",
    ),
    signal("clicked", &[], "Its body was clicked (not a child button or link)."),
];

pub(super) const TOAST: KindDef = KindDef {
    id: "Toast",
    tag: Some("toast"),
    native: "svelters' own toast surface (a taffy flex box) in its host's foreground Area",
    base: Some("Box"),
    toplevel: false,
    slot: Some("children"),
    text_content: None,
    present: None,
    props: TOAST_PROPS,
    signals: TOAST_SIGNALS,
    methods: &[],
    doc: "A declarative toast, a child of `toast-host`: a flex box (column) laid out by taffy, stacked by its host with the `show()` toasts. Its labels are ordinary nodes, so their RT2 runs (`<b>`, `<a href>`, `<code>`) take per-run CSS; its child buttons are its actions (a click closes it with `action`). Styled by `toast` and `toast[level=…]`, `toast::part(accent)`, `toast::part(close)`, with `:hover` and `:exiting` (while its `transition` plays out after it closed).",
};

pub(super) const TOAST_HOST: KindDef = KindDef {
    id: "ToastHost",
    tag: Some("toast-host"),
    native: "svelters' own toasts in an egui::Area (foreground)",
    base: Some("Widget"),
    toplevel: false,
    slot: Some("toasts"),
    text_content: None,
    present: None,
    props: TOAST_HOST_PROPS,
    signals: TOAST_HOST_SIGNALS,
    methods: TOAST_HOST_METHODS,
    doc: "Shows transient notifications stacked in a corner of its viewport, over everything; takes no space itself. Declarative `<toast>` children (OW7b), `show` from JS (`bind:this`) and `svelters_egui::toast::show` from Rust stack together: children first, in order, then the `show` toasts.",
};
