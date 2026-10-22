//! The container kinds' tables: `window` (a viewport whose content area is a flex column) and `box`
//! (a taffy flexbox). Their [`KindDef`](super::KindDef) entries are in [`KINDS`](super::KINDS), whose order
//! fixes the kind ids.

use super::builtin::NONE;
use super::{
    ALIGN, DIRECTION, Def, JUSTIFY, MethodDef, P, PropDef, SignalDef, Ty, WRAP, layout, length,
    prop,
};

pub(super) static BOX_PROPS: &[PropDef] = &[
    layout(
        "flex-direction",
        P::FlexDirection,
        Ty::Enum(DIRECTION),
        Def::Str("column"),
        "Main axis of the box.",
    ),
    layout(
        "flex-wrap",
        P::FlexWrap,
        Ty::Enum(WRAP),
        Def::Str("nowrap"),
        "Whether children wrap onto more lines.",
    ),
    layout(
        "gap",
        P::Gap,
        Ty::F64,
        Def::F64(0.0),
        "Space between children on both axes, in points (row-gap and column-gap override it).",
    ),
    length(
        "row-gap",
        P::RowGap,
        "Space between rows in points; null to use `gap`.",
    ),
    length(
        "column-gap",
        P::ColumnGap,
        "Space between columns in points; null to use `gap`.",
    ),
    layout(
        "padding",
        P::Padding,
        Ty::F64,
        Def::F64(0.0),
        "Inner spacing on all four sides, in points.",
    ),
    layout(
        "align-items",
        P::AlignItems,
        Ty::Enum(ALIGN),
        Def::Str("stretch"),
        "Cross-axis alignment of the children.",
    ),
    layout(
        "justify-content",
        P::JustifyContent,
        Ty::Enum(JUSTIFY),
        Def::Str("start"),
        "Main-axis distribution of the children.",
    ),
];

pub(super) static WINDOW_PROPS: &[PropDef] = &[
    prop(
        "title",
        P::Title,
        Ty::Str,
        Def::Str(""),
        "Window title (`ViewportCommand::Title`).",
    ),
    prop(
        "default-width",
        P::DefaultWidth,
        Ty::F64,
        Def::F64(480.0),
        "Initial inner width in points.",
    ),
    prop(
        "default-height",
        P::DefaultHeight,
        Ty::F64,
        Def::F64(360.0),
        "Initial inner height in points.",
    ),
    // The content area is a flex column (the window is the root of its taffy tree).
    layout(
        "padding",
        P::Padding,
        Ty::F64,
        Def::F64(8.0),
        "Inner spacing of the content area, in points.",
    ),
    layout(
        "gap",
        P::Gap,
        Ty::F64,
        Def::F64(0.0),
        "Space between the window's children, in points.",
    ),
];

pub(super) static WINDOW_SIGNALS: &[SignalDef] = &[SignalDef {
    name: "close-request",
    args: NONE,
    sync: true,
    doc: "The user asked to close the window; return true to keep it open. Runs before the frame's pass.",
}];

pub(super) static WINDOW_METHODS: &[MethodDef] = &[
    MethodDef {
        name: "present",
        doc: "Shows the window (the first one presented is the root viewport).",
    },
    MethodDef {
        name: "close",
        doc: "Closes the window as the user would (no close-request).",
    },
];
