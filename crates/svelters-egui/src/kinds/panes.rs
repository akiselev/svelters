//! WG2b containers (plans/milestones/WG2b-tier0-controls.md): `panel`, `sides`, `frame` and
//! `stack`. Their entries are appended to [`KINDS`](super::KINDS); layout hooks and chrome are
//! `crate::elements::panes`.
//!
//! Layout behaviour (SG1's boundary rule applies unchanged): `panel`'s `size` is its own width or
//! height (a style change: restyle, relayout from its parent); `frame`'s chrome is padding in its
//! taffy style; `stack` lays out its visible child (and, `homogeneous`, sizes to the largest
//! child), so `visible-*` writes are style changes of the stack. Those props are not CSS setters
//! (`layout: false`): their writes return `Dirty::LAYOUT` themselves (`crate::elements`). Only the
//! `Box` props (`gap`, `padding`, `align-items`) are setters.

use super::builtin::ALIGN;
use super::{Def, KindDef, Native, P, PropDef, SignalDef, Ty, layout, prop, signal};

pub const SIDE: &[&str] = &["left", "right", "top", "bottom"];
pub const FRAME_STYLE: &[&str] = &["group", "canvas", "window", "menu", "popup", "panel", "none"];

static PANEL_PROPS: &[PropDef] = &[
    prop(
        "side",
        P::Side,
        Ty::Enum(SIDE),
        Def::Str("left"),
        "The edge of the parent it belongs to: the separator and the resize grip are on the opposite edge, and `size` is its width (left, right) or height (top, bottom).",
    ),
    PropDef {
        name: "size",
        key: P::Size,
        ty: Ty::F64,
        default: Def::Null,
        nullable: true,
        native: Native::Widget,
        layout: false,
        doc: "Width (left, right) or height (top, bottom) in points; null for its content's. Dragging the grip of a `resizable` panel changes it, within `min-*`/`max-*`.",
    },
    prop("resizable", P::Resizable, Ty::Bool, Def::Bool(false), "The inner edge can be dragged to resize it."),
    prop("separator", P::ShowSeparator, Ty::Bool, Def::Bool(true), "Draws a line on the inner edge."),
];

static PANEL_SIGNALS: &[SignalDef] = &[signal("changed", &[("size", Ty::F64)], "The user resized it; carries the new size.")];

static SIDES_PROPS: &[PropDef] = &[
    layout("gap", P::Gap, Ty::F64, Def::F64(0.0), "Minimum space between the two sides, in points."),
    layout("padding", P::Padding, Ty::F64, Def::F64(0.0), "Inner spacing on all four sides, in points."),
    layout("align-items", P::AlignItems, Ty::Enum(ALIGN), Def::Str("stretch"), "Cross-axis alignment of the two sides (`center` is egui's look)."),
];

static FRAME_PROPS: &[PropDef] = &[
    prop("label", P::Text, Ty::Str, Def::Str(""), "A caption above the content (GTK's `Gtk.Frame:label`); empty for none."),
    prop(
        "frame-style",
        P::FrameStyle,
        Ty::Enum(FRAME_STYLE),
        Def::Str("group"),
        "The egui frame drawn around the content: `egui::Frame::group`, `canvas`, `window`, `menu`, `popup`, `side_top_panel` (`panel`), or no chrome (`none`). Its inner margin adds to `padding`.",
    ),
];

static STACK_PROPS: &[PropDef] = &[
    prop(
        "visible-child-name",
        P::VisibleChildName,
        Ty::Str,
        Def::Str(""),
        "Shows the child whose `anchor` is this name (GTK's `visible-child-name`; the child names itself with `anchor`). Empty: `visible-index` decides.",
    ),
    prop("visible-index", P::VisibleIndex, Ty::I64, Def::I64(0), "Shows the child at this index when `visible-child-name` is empty or names no child (Qt's `current-index`)."),
    prop("homogeneous", P::Homogeneous, Ty::Bool, Def::Bool(true), "Sized to the largest child (GTK's `hhomogeneous`/`vhomogeneous`); false: to the visible child."),
    layout("padding", P::Padding, Ty::F64, Def::F64(0.0), "Inner spacing on all four sides, in points."),
];

pub(super) const PANEL: KindDef = KindDef {
    id: "Panel",
    tag: Some("panel"),
    native: "egui panel look (`Visuals::panel_fill`, separator, resize grip) + taffy flexbox",
    base: Some("Box"),
    toplevel: false,
    slot: Some("children"),
    text_content: None,
    present: None,
    props: PANEL_PROPS,
    signals: PANEL_SIGNALS,
    methods: &[],
    doc: "A side or top/bottom panel: a flex box with the panel background and a separator on its inner edge, optionally resizable. Place it at the edge of a row (left/right) or column (top/bottom).",
};

pub(super) const SIDES: KindDef = KindDef {
    id: "Sides",
    tag: Some("sides"),
    native: "egui::Sides (taffy flex row, space-between)",
    base: Some("Widget"),
    toplevel: false,
    slot: Some("children"),
    text_content: None,
    present: None,
    props: SIDES_PROPS,
    signals: &[],
    methods: &[],
    doc: "A row with its first child at the start and its second at the end (wrap several items in a `box`); with more children, the space is shared between them.",
};

pub(super) const FRAME: KindDef = KindDef {
    id: "Frame",
    tag: Some("frame"),
    native: "egui::Frame + taffy flexbox",
    base: Some("Box"),
    toplevel: false,
    slot: Some("children"),
    text_content: None,
    present: None,
    props: FRAME_PROPS,
    signals: &[],
    methods: &[],
    doc: "A flex box drawn inside an egui frame (background, border, rounding, inner margin), with an optional caption.",
};

pub(super) const STACK: KindDef = KindDef {
    id: "Stack",
    tag: Some("stack"),
    native: "synthetic (one child shown at a time)",
    base: Some("Widget"),
    toplevel: false,
    slot: Some("children"),
    text_content: None,
    present: None,
    props: STACK_PROPS,
    signals: &[],
    methods: &[],
    doc: "Shows one of its children (GTK's `stack`, Qt's `stacked-widget`): the one anchored `visible-child-name`, else the one at `visible-index`. The others are neither drawn nor hit.",
};
