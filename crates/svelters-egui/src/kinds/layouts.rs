//! WG1 layout containers: `grid` (a taffy CSS grid), `collapsing-header` and `scroll-area`
//! (flex boxes with chrome: their `Box` props come from the base). Their entries are appended to
//! [`KINDS`](super::KINDS); layout is `layout.rs`, chrome `leaf::container`.
//!
//! Layout behaviour (SG1's boundary rule applies unchanged to all three): `grid` is a plain
//! container; `collapsing-header`'s `open` changes its own size (a style change: restyle and
//! relayout from its parent); `scroll-area`'s offsets are paint-only (no relayout).

use super::{ALIGN, Def, KindDef, P, PropDef, SignalDef, Ty, input, layout, length, prop, signal};

pub const JUSTIFY_ITEMS: &[&str] = &["start", "end", "center", "stretch"];
pub const SCROLL_DIRECTION: &[&str] = &["vertical", "horizontal", "both"];

static GRID_PROPS: &[PropDef] = &[
    layout(
        "grid-template-columns",
        P::GridTemplateColumns,
        Ty::Str,
        Def::Str("auto 1fr"),
        "The column tracks, CSS syntax: `auto`, `min-content`, `max-content`, `<n>fr`, `<n>px` (or a bare number: points), `<n>%`, `minmax(a, b)`, `fit-content(<len>)`, `repeat(<n>, tracks)`. Rows are implicit (`auto`). The default is a form: a label column and a field column.",
    ),
    layout("gap", P::Gap, Ty::F64, Def::F64(0.0), "Space between rows and between columns, in points (row-gap and column-gap override it)."),
    length("row-gap", P::RowGap, "Space between rows in points; null to use `gap`."),
    length("column-gap", P::ColumnGap, "Space between columns in points; null to use `gap`."),
    layout("padding", P::Padding, Ty::F64, Def::F64(0.0), "Inner spacing on all four sides, in points."),
    layout("align-items", P::AlignItems, Ty::Enum(ALIGN), Def::Str("stretch"), "Vertical alignment of each child in its cell."),
    layout("justify-items", P::JustifyItems, Ty::Enum(JUSTIFY_ITEMS), Def::Str("stretch"), "Horizontal alignment of each child in its cell."),
];

static COLLAPSING_HEADER_PROPS: &[PropDef] = &[
    prop("text", P::Text, Ty::Str, Def::Str(""), "The header's caption (also its accessible name)."),
    input("open", P::Open, Ty::Bool, Def::Bool(false), "Shows the children; a click on the header (or Enter/Space on it) toggles it."),
];

static COLLAPSING_HEADER_SIGNALS: &[SignalDef] = &[signal("changed", &[("open", Ty::Bool)], "The user opened or closed it; carries the new state.")];

static SCROLL_AREA_PROPS: &[PropDef] = &[
    prop("direction", P::ScrollDirection, Ty::Enum(SCROLL_DIRECTION), Def::Str("vertical"), "The axes that scroll (taffy `overflow: scroll` on them)."),
    input("scroll-x", P::ScrollX, Ty::F64, Def::F64(0.0), "Horizontal offset in points; scrolling changes it, and a write scrolls (clamped to the content)."),
    input("scroll-y", P::ScrollY, Ty::F64, Def::F64(0.0), "Vertical offset in points; scrolling changes it, and a write scrolls (clamped to the content)."),
];

static SCROLL_AREA_SIGNALS: &[SignalDef] =
    &[signal("changed", &[("scroll-x", Ty::F64), ("scroll-y", Ty::F64)], "The user scrolled (or a write was clamped); carries the new offsets.")];

pub(super) const GRID: KindDef = KindDef {
        id: "Grid",
        tag: Some("grid"),
        native: "taffy::Style (CSS grid)",
        base: Some("Widget"),
        toplevel: false,
        slot: Some("children"),
        text_content: None,
        present: None,
        props: GRID_PROPS,
        signals: &[],
        methods: &[],
        doc: "A CSS grid container laid out by taffy: children fill the cells row by row (auto-placement); draws nothing itself.",
    };

pub(super) const COLLAPSING_HEADER: KindDef = KindDef {
        id: "CollapsingHeader",
        tag: Some("collapsing-header"),
        native: "egui::CollapsingHeader (header) + taffy flexbox (body)",
        base: Some("Box"),
        toplevel: false,
        slot: Some("children"),
        text_content: Some("text"),
        present: None,
        props: COLLAPSING_HEADER_PROPS,
        signals: COLLAPSING_HEADER_SIGNALS,
        methods: &[],
        doc: "A clickable header with a caret; its children (a flex box, indented) are shown and laid out only while `open`.",
    };

pub(super) const SCROLL_AREA: KindDef = KindDef {
        id: "ScrollArea",
        tag: Some("scroll-area"),
        native: "egui::ScrollArea + taffy flexbox (content)",
        base: Some("Box"),
        toplevel: false,
        slot: Some("children"),
        text_content: None,
        present: None,
        props: SCROLL_AREA_PROPS,
        signals: SCROLL_AREA_SIGNALS,
        methods: &[],
        doc: "A flex box whose content scrolls when it overflows the area's size (taffy `overflow: scroll`); give it a size or let flex-grow size it.",
    };
