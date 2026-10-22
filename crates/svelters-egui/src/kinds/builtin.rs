//! The built-in kind table [`KINDS`] (its order is the kind ids: index + 1), with the tables of the
//! `Widget` base and the leaves. The `window` and `box` tables are in `containers.rs`.
//!
//! WG1 (plans/milestones/WG1-sc1-widgets.md) appended its kinds at the end of [`KINDS`] (their
//! tables are one file per family: `controls.rs`, `decor.rs`, `layouts.rs`), and `invalid` and
//! `font-weight` at the end of the tables here, so every earlier kind and member id is unchanged.

use super::containers::{BOX_PROPS, WINDOW_METHODS, WINDOW_PROPS, WINDOW_SIGNALS};
use super::{Def, KindDef, MethodDef, P, PropDef, SignalDef, Ty, input, layout, length, prop, signal};

pub const ALIGN: &[&str] = &["start", "end", "center", "stretch", "baseline"];
pub const ALIGN_SELF: &[&str] = &["auto", "start", "end", "center", "stretch", "baseline"];
/// SG4: `position` (`absolute`: out of flow, placed by `left`/`top`/`right`/`bottom` in the nearest
/// positioned ancestor, else the root).
pub const POSITION: &[&str] = &["static", "relative", "absolute"];
pub const JUSTIFY: &[&str] = &["start", "end", "center", "space-between", "space-around", "space-evenly"];
pub const DIRECTION: &[&str] = &["column", "row", "column-reverse", "row-reverse"];
pub const WRAP: &[&str] = &["nowrap", "wrap", "wrap-reverse"];
/// EV3 (`crate::focus`): `contain`: Tab cycles inside while focus is in it; `dialog`: also takes
/// focus when it appears and gives it back when it goes away.
pub const FOCUS_SCOPE: &[&str] = &["none", "contain", "dialog"];
pub const WRAP_MODE: &[&str] = &["wrap", "truncate", "extend"];

pub(super) const FONT_WEIGHT_DOC: &str = "CSS font weight (100-900). Drawn with the font family the embedder registered for the nearest weight (`svelters_egui::fonts::register_weight`); 400, or no registered family, draws the regular face.";

static WIDGET_PROPS: &[PropDef] = &[
    prop("enabled", P::Enabled, Ty::Bool, Def::Bool(true), "Takes input when true; drawn greyed out otherwise (`Ui::add_enabled`)."),
    layout("visible", P::Visible, Ty::Bool, Def::Bool(true), "Shown and laid out when true (`display: none` otherwise)."),
    prop("autofocus", P::Autofocus, Ty::Bool, Def::Bool(false), "Takes keyboard focus when it first appears (in a `dialog` scope: when the dialog opens)."),
    prop("tooltip", P::Tooltip, Ty::Str, Def::Str(""), "Hover text (`Response::on_hover_text`); empty for none."),
    prop(
        "accessible-name",
        P::AccessibleName,
        Ty::Str,
        Def::Str(""),
        "AccessKit name (`Response::accessible_name`); inputs default to their hint or text.",
    ),
    length("width", P::Width, "Width in points; null for auto."),
    length("height", P::Height, "Height in points; null for auto."),
    length("min-width", P::MinWidth, "Minimum width in points; null for auto."),
    length("min-height", P::MinHeight, "Minimum height in points; null for auto."),
    length("max-width", P::MaxWidth, "Maximum width in points; null for none."),
    length("max-height", P::MaxHeight, "Maximum height in points; null for none."),
    layout("flex-grow", P::FlexGrow, Ty::F64, Def::F64(0.0), "Share of the parent's free main-axis space."),
    layout("flex-shrink", P::FlexShrink, Ty::F64, Def::F64(1.0), "Share of the parent's main-axis overflow to give up."),
    length("flex-basis", P::FlexBasis, "Main-axis size before growing or shrinking; null for auto."),
    layout("align-self", P::AlignSelf, Ty::Enum(ALIGN_SELF), Def::Str("auto"), "Cross-axis alignment in the parent box, overriding its `align-items`."),
    layout("margin", P::Margin, Ty::F64, Def::F64(0.0), "Outer spacing on all four sides, in points."),
    prop(
        "anchor",
        P::Anchor,
        Ty::Str,
        Def::Str(""),
        "A name unique in its embedded root: the node's egui id derives from it (stable across runs, not only across reorders), it is the AccessKit author id, and `Embed::anchor_rect` finds it. Changing it drops the node's egui memory.",
    ),
    prop(
        "invalid",
        P::Invalid,
        Ty::Bool,
        Def::Bool(false),
        "Marks the value as invalid: an outline in the error colour (`Visuals::error_fg_color`) and AccessKit `invalid`. The app sets it; the widget never does.",
    ),
    // WG4b: drag and drop on any node (`dnd.rs`)
    prop("drag-type", P::DragType, Ty::Str, Def::Str(""), "Makes the node a drag source: the payload's type, a MIME-like string (`cad/part`); empty for none."),
    prop("drag-data", P::DragData, Ty::Str, Def::Str(""), "The drag payload's data (JSON for structured data); `drop` delivers it."),
    prop(
        "drop-types",
        P::DropTypes,
        Ty::Str,
        Def::Str(""),
        "Makes the node a drop target: the drag types it accepts, separated by spaces or commas (`*` any, `cad/*` a prefix); empty for none.",
    ),
    layout(
        "position",
        P::Position,
        Ty::Enum(POSITION),
        Def::Str("static"),
        "`absolute` takes the node out of the flow and places it by `left`/`top`/`right`/`bottom` inside its containing block: the nearest `relative` or `absolute` ancestor, else the root.",
    ),
    length("left", P::Left, "Inset from the containing block's left edge in points (`position: absolute` or `relative`); null for auto."),
    length("top", P::Top, "Inset from the containing block's top edge in points; null for auto."),
    length("right", P::Right, "Inset from the containing block's right edge in points; null for auto."),
    length("bottom", P::Bottom, "Inset from the containing block's bottom edge in points; null for auto."),
];

static LABEL_PROPS: &[PropDef] = &[
    prop("text", P::Text, Ty::Str, Def::Str(""), "The text."),
    prop("wrap-mode", P::WrapMode, Ty::Enum(WRAP_MODE), Def::Str("wrap"), "What a line longer than the width does (`egui::TextWrapMode`)."),
    prop("selectable", P::Selectable, Ty::Bool, Def::Bool(false), "The text can be selected and copied."),
    prop("font-weight", P::FontWeight, Ty::I64, Def::I64(400), FONT_WEIGHT_DOC),
];

static BUTTON_PROPS: &[PropDef] = &[
    prop("text", P::Text, Ty::Str, Def::Str(""), "The caption."),
    prop("selected", P::Selected, Ty::Bool, Def::Bool(false), "Drawn as selected (a toggle look); clicking does not change it."),
    prop("frame", P::Frame, Ty::Bool, Def::Bool(true), "Draws the button background and border."),
    prop("small", P::Small, Ty::Bool, Def::Bool(false), "No vertical padding (`Button::small`)."),
    prop("font-weight", P::FontWeight, Ty::I64, Def::I64(400), FONT_WEIGHT_DOC),
    prop("default", P::Default, Ty::Bool, Def::Bool(false), "Enter activates it while focus is in its focus scope (or root) and the focused node does not take Enter itself."),
    prop("cancel", P::Cancel, Ty::Bool, Def::Bool(false), "Escape activates it while focus is in its focus scope (or root)."),
];





static CHECKBOX_PROPS: &[PropDef] = &[
    prop("text", P::Text, Ty::Str, Def::Str(""), "The caption."),
    input("checked", P::Checked, Ty::Bool, Def::Bool(false), "Checked state; clicking toggles it."),
];

static TEXT_EDIT_PROPS: &[PropDef] = &[
    input("text", P::Text, Ty::Str, Def::Str(""), "The edited text; typing changes it."),
    prop("hint-text", P::HintText, Ty::Str, Def::Str(""), "Shown while the text is empty; also the default accessible name."),
    prop("multiline", P::Multiline, Ty::Bool, Def::Bool(false), "Several lines (`TextEdit::multiline`); Enter inserts a newline."),
    prop("password", P::Password, Ty::Bool, Def::Bool(false), "Hides the characters."),
    prop("char-limit", P::CharLimit, Ty::I64, Def::I64(0), "Maximum number of characters; 0 for none."),
];

static SLIDER_PROPS: &[PropDef] = &[
    input("value", P::Value, Ty::F64, Def::F64(0.0), "The value; dragging or the arrow keys change it."),
    prop("min", P::Min, Ty::F64, Def::F64(0.0), "Lower bound."),
    prop("max", P::Max, Ty::F64, Def::F64(100.0), "Upper bound."),
    prop("step", P::Step, Ty::F64, Def::F64(0.0), "Value granularity; 0 for continuous."),
    prop("text", P::Text, Ty::Str, Def::Str(""), "Caption after the slider."),
    prop("show-value", P::ShowValue, Ty::Bool, Def::Bool(true), "Shows the value box next to the slider."),
];

pub(super) const NONE: &[(&str, Ty)] = &[];

static WIDGET_SIGNALS: &[SignalDef] = dom_signals![
    signal("animationend", &[("id", Ty::I64)], "A native animation (TR2) finished; carries its id. The runtime listens to it itself."),
    // WG4b: drag and drop on any node (`kinds/dnd.rs`)
    signal("drag-begin", NONE, "Drag source: the user started dragging this node (`drag-type` set)."),
    signal("drag-end", &[("dropped", Ty::Bool)], "Drag source: the drag ended; `dropped` is true when a target took it."),
    signal("drag-enter", &[("type", Ty::Str)], "Drop target: a drag it accepts entered it (`drop-types` set); carries the drag's type."),
    signal("drag-leave", NONE, "Drop target: the drag left, was cancelled, or was dropped here (before `drop`)."),
    signal(
        "drop",
        &[("type", Ty::Str), ("data", Ty::Str), ("x", Ty::F64), ("y", Ty::F64)],
        "Drop target: a drag it accepts was released over it; carries the type, the data and the point in the node's own points.",
    )
];

static CLICK_SIGNALS: &[SignalDef] = &[signal("clicked", NONE, "Pressed and released (`Response::clicked`).")];

pub(super) static BUTTON_SIGNALS: &[SignalDef] = &[
    signal("clicked", NONE, "Pressed and released (`Response::clicked`)."),
    signal("double-clicked", NONE, "Double click (`Response::double_clicked`)."),
    signal("secondary-clicked", NONE, "Right click (`Response::secondary_clicked`)."),
];

static CHECKBOX_SIGNALS: &[SignalDef] = &[
    signal("changed", &[("checked", Ty::Bool)], "The user toggled it; carries the new state."),
    signal("clicked", NONE, "Pressed and released."),
];

static TEXT_EDIT_SIGNALS: &[SignalDef] = &[
    signal("changed", &[("text", Ty::Str)], "The user edited the text; carries the new text (once per frame)."),
    signal("gained-focus", NONE, "Took keyboard focus."),
    signal("lost-focus", NONE, "Lost keyboard focus."),
    // CX3: `bind:text|commit` updates its source on this event (egui.json `bindings.text.commit`)
    signal(
        "commit",
        &[("text", Ty::Str)],
        "The user committed the text: Enter in a one-line field (also when a default button takes it), or losing focus after an edit since the last commit; carries the text.",
    ),
    signal("submit", &[("text", Ty::Str)], "Enter in a one-line field (also when a default button takes it); carries the text. Fires after `commit`."),
];

static SLIDER_SIGNALS: &[SignalDef] = &[
    signal("changed", &[("value", Ty::F64)], "The user moved it; carries the new value."),
    signal("drag-started", NONE, "A drag on the slider began."),
    signal("drag-stopped", NONE, "A drag on the slider ended."),
];



/// OW2 (WG6 QA13): every node's methods (`widgets::scroll_area::scroll_into_view`).
static WIDGET_METHODS: &[MethodDef] = &[MethodDef {
    name: "scroll-into-view",
    doc: "`scroll-into-view(align?)`: scrolls the scroll areas around the node so it is visible on the next frame, innermost first; `align` is `nearest` (the default: the least scrolling, keeping the area's `scroll-margin`) or `center`. Qt's `ensureWidgetVisible`, the DOM's `scrollIntoView`.",
}];

static TEXT_EDIT_METHODS: &[MethodDef] = &[MethodDef { name: "request-focus", doc: "Moves keyboard focus here on the next frame." }];

pub static KINDS: &[KindDef] = &[
    KindDef {
        id: "Widget",
        tag: None,
        native: "egui::Widget",
        base: None,
        toplevel: false,
        slot: None,
        text_content: None,
        present: None,
        props: WIDGET_PROPS,
        signals: WIDGET_SIGNALS,
        methods: WIDGET_METHODS,
        doc: "Common properties of everything placed in a window: state, tooltip, accessibility and its taffy item style.",
    },
    KindDef {
        id: "Window",
        tag: Some("window"),
        native: "egui::ViewportBuilder",
        base: None,
        toplevel: true,
        slot: Some("content"),
        text_content: None,
        present: Some("present"),
        props: WINDOW_PROPS,
        signals: WINDOW_SIGNALS,
        methods: WINDOW_METHODS,
        doc: "A native window (the root viewport, or an immediate viewport); its children are laid out as a flex column.",
    },
    KindDef {
        id: "Box",
        tag: Some("box"),
        native: "taffy::Style (flexbox)",
        base: Some("Widget"),
        toplevel: false,
        slot: Some("children"),
        text_content: None,
        present: None,
        props: BOX_PROPS,
        signals: &[],
        methods: &[],
        doc: "A flexbox container laid out by taffy; draws nothing itself.",
    },
    KindDef {
        id: "Label",
        tag: Some("label"),
        native: "egui::Label",
        base: Some("Widget"),
        toplevel: false,
        slot: None,
        text_content: Some("text"),
        present: None,
        props: LABEL_PROPS,
        signals: CLICK_SIGNALS,
        methods: &[],
        doc: "Static text.",
    },
    KindDef {
        id: "Button",
        tag: Some("button"),
        native: "egui::Button",
        base: Some("Widget"),
        toplevel: false,
        slot: None,
        text_content: Some("text"),
        present: None,
        props: BUTTON_PROPS,
        signals: BUTTON_SIGNALS,
        methods: &[],
        doc: "A push button.",
    },
    KindDef {
        id: "Checkbox",
        tag: Some("checkbox"),
        native: "egui::Checkbox",
        base: Some("Widget"),
        toplevel: false,
        slot: None,
        text_content: Some("text"),
        present: None,
        props: CHECKBOX_PROPS,
        signals: CHECKBOX_SIGNALS,
        methods: &[],
        doc: "A check box with a caption.",
    },
    KindDef {
        id: "TextEdit",
        tag: Some("text-edit"),
        native: "egui::TextEdit",
        base: Some("Widget"),
        toplevel: false,
        slot: None,
        text_content: None,
        present: None,
        props: TEXT_EDIT_PROPS,
        signals: TEXT_EDIT_SIGNALS,
        methods: TEXT_EDIT_METHODS,
        doc: "A single- or multi-line text field.",
    },
    KindDef {
        id: "Slider",
        tag: Some("slider"),
        native: "egui::Slider",
        base: Some("Widget"),
        toplevel: false,
        slot: None,
        text_content: None,
        present: None,
        props: SLIDER_PROPS,
        signals: SLIDER_SIGNALS,
        methods: &[],
        doc: "A horizontal slider with an optional value box.",
    },
    // ---- WG1: appended, so the ids above stay (tables in `controls.rs`, `decor.rs`, `layouts.rs`) ----
    super::controls::TOGGLE_BUTTON,
    super::controls::COMBO_BOX,
    super::decor::SEPARATOR,
    super::decor::ICON,
    super::layouts::GRID,
    super::layouts::COLLAPSING_HEADER,
    super::layouts::SCROLL_AREA,
    // ---- WG2b: appended after WG1 (tables in `inputs.rs`, `panes.rs`, `media.rs`) ----
    super::inputs::HYPERLINK,
    super::inputs::RADIO_BUTTON,
    super::inputs::SWITCH,
    super::inputs::RANGE_SLIDER,
    super::inputs::DRAG_VALUE,
    super::panes::PANEL,
    super::panes::SIDES,
    super::panes::FRAME,
    super::panes::STACK,
    super::media::IMAGE,
    super::media::PROGRESS_BAR,
    super::media::SPINNER,
    super::media::CANVAS,
    // ---- WG2a: appended after WG2b (tables in `menus.rs`, `popups.rs`) ----
    super::menus::MENU_ENTRY,
    super::menus::MENU_BAR,
    super::menus::MENU,
    super::menus::ACTION,
    super::menus::MENU_SEPARATOR,
    super::menus::CONTEXT_MENU,
    super::menus::MENU_BUTTON,
    super::popups::POPUP,
    super::popups::MODAL,
    super::popups::INNER_WINDOW,
    // WG4a
    super::views::TABLE,
    super::views::TABLE_COLUMN,
    super::views::TAB_VIEW,
    super::views::TAB,
    super::views::SPLIT_VIEW,
    super::views::TOAST_HOST,
    // ---- WG4b: appended (tables in `pickers.rs`) ----
    super::pickers::DATE_PICKER,
    super::pickers::COLOR_PICKER,
    // ---- LS1: appended after INT1's kinds (`virtuallist.rs`) ----
    super::virtuallist::DEF,
    // ---- WG5b: appended (`treeview.rs`) ----
    super::treeview::DEF,
    // ---- OW2: appended (`views.rs`; svelters' own widget only, `widgets::scrollbar`) ----
    super::views::SCROLLBAR,
];
