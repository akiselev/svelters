//! The kind table: every element, property, signal and method the egui backend knows.
//!
//! This is the single source of the egui vocabulary (plans/research/EGUI.md §6, "Rust kind table as
//! the single source"). `resolve`/`describe` answer from it, the scene stores one typed field per
//! [`P`], and [`crate::spec`] turns it into `metadata/semantics/egui.json` for the compiler, the
//! TestHost and the docs (checked by `tests/spec.rs`).
//!
//! Names follow N1 where egui has a type (`label`, `button`, `checkbox`, `text-edit`, `slider`:
//! kebab-case of `egui::Label` etc.) and GTK-familiar names for the kinds we build (`window`, `box`).
//! Layout properties are CSS names because taffy's are (D2, D3): they are setters in the style
//! table, so `<style>` can use them on egui.

/// A property's value type (also its ABI tag).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Ty {
    Bool,
    F64,
    I64,
    Str,
    /// A string from a fixed list.
    Enum(&'static [&'static str]),
}

/// A property's default (JSON in the spec).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Def {
    Null,
    Bool(bool),
    F64(f64),
    I64(i64),
    Str(&'static str),
}

/// Every property the scene stores, by identity (one typed field each, see `scene.rs`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum P {
    // Widget (every element inside a window)
    Enabled,
    Visible,
    Tooltip,
    AccessibleName,
    Width,
    Height,
    MinWidth,
    MinHeight,
    MaxWidth,
    MaxHeight,
    FlexGrow,
    FlexShrink,
    FlexBasis,
    AlignSelf,
    Margin,
    // Box (and the window's content area)
    FlexDirection,
    FlexWrap,
    Gap,
    RowGap,
    ColumnGap,
    Padding,
    AlignItems,
    JustifyContent,
    // Window
    Title,
    DefaultWidth,
    DefaultHeight,
    // Leaves
    Text,
    WrapMode,
    Selectable,
    Selected,
    Frame,
    Small,
    Checked,
    HintText,
    Multiline,
    Password,
    CharLimit,
    Value,
    Min,
    Max,
    Step,
    ShowValue,
}

/// How a property can change without the host setting it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Native {
    /// Only `set_prop` changes it. Observing it is still exact: the scene never changes it alone.
    Never,
    /// The widget changes it from user input during the pass (`prop_changed` when observed).
    Widget,
}

pub struct PropDef {
    pub name: &'static str,
    pub key: P,
    pub ty: Ty,
    pub default: Def,
    pub nullable: bool,
    pub native: Native,
    /// A layout property: a setter in the style table (D3), taffy reads it.
    pub layout: bool,
    pub doc: &'static str,
}

pub struct SignalDef {
    pub name: &'static str,
    pub args: &'static [(&'static str, Ty)],
    /// Delivered through `call_sync` before the pass; the handler's `true` stops the default.
    pub sync: bool,
    pub doc: &'static str,
}

pub struct MethodDef {
    pub name: &'static str,
    pub doc: &'static str,
}

pub struct KindDef {
    /// The kind's id in the spec (`kinds` keys) and in `resolve(SVL_KIND)`.
    pub id: &'static str,
    /// The element tag; `None` for abstract kinds.
    pub tag: Option<&'static str>,
    /// What it renders with.
    pub native: &'static str,
    pub base: Option<&'static str>,
    pub toplevel: bool,
    /// Accepts children (default slot `slot`).
    pub slot: Option<&'static str>,
    pub text_content: Option<&'static str>,
    pub present: Option<&'static str>,
    pub props: &'static [PropDef],
    pub signals: &'static [SignalDef],
    pub methods: &'static [MethodDef],
    pub doc: &'static str,
}

const fn prop(name: &'static str, key: P, ty: Ty, default: Def, doc: &'static str) -> PropDef {
    PropDef { name, key, ty, default, nullable: false, native: Native::Never, layout: false, doc }
}

const fn length(name: &'static str, key: P, doc: &'static str) -> PropDef {
    PropDef { name, key, ty: Ty::F64, default: Def::Null, nullable: true, native: Native::Never, layout: true, doc }
}

const fn layout(name: &'static str, key: P, ty: Ty, default: Def, doc: &'static str) -> PropDef {
    PropDef { name, key, ty, default, nullable: false, native: Native::Never, layout: true, doc }
}

const fn input(name: &'static str, key: P, ty: Ty, default: Def, doc: &'static str) -> PropDef {
    PropDef { name, key, ty, default, nullable: false, native: Native::Widget, layout: false, doc }
}

const fn signal(name: &'static str, args: &'static [(&'static str, Ty)], doc: &'static str) -> SignalDef {
    SignalDef { name, args, sync: false, doc }
}

pub const ALIGN: &[&str] = &["start", "end", "center", "stretch", "baseline"];
pub const ALIGN_SELF: &[&str] = &["auto", "start", "end", "center", "stretch", "baseline"];
pub const JUSTIFY: &[&str] = &["start", "end", "center", "space-between", "space-around", "space-evenly"];
pub const DIRECTION: &[&str] = &["column", "row", "column-reverse", "row-reverse"];
pub const WRAP: &[&str] = &["nowrap", "wrap", "wrap-reverse"];
pub const WRAP_MODE: &[&str] = &["wrap", "truncate", "extend"];

static WIDGET_PROPS: &[PropDef] = &[
    prop("enabled", P::Enabled, Ty::Bool, Def::Bool(true), "Takes input when true; drawn greyed out otherwise (`Ui::add_enabled`)."),
    layout("visible", P::Visible, Ty::Bool, Def::Bool(true), "Shown and laid out when true (`display: none` otherwise)."),
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
];

static BOX_PROPS: &[PropDef] = &[
    layout("flex-direction", P::FlexDirection, Ty::Enum(DIRECTION), Def::Str("column"), "Main axis of the box."),
    layout("flex-wrap", P::FlexWrap, Ty::Enum(WRAP), Def::Str("nowrap"), "Whether children wrap onto more lines."),
    layout("gap", P::Gap, Ty::F64, Def::F64(0.0), "Space between children on both axes, in points (row-gap and column-gap override it)."),
    length("row-gap", P::RowGap, "Space between rows in points; null to use `gap`."),
    length("column-gap", P::ColumnGap, "Space between columns in points; null to use `gap`."),
    layout("padding", P::Padding, Ty::F64, Def::F64(0.0), "Inner spacing on all four sides, in points."),
    layout("align-items", P::AlignItems, Ty::Enum(ALIGN), Def::Str("stretch"), "Cross-axis alignment of the children."),
    layout("justify-content", P::JustifyContent, Ty::Enum(JUSTIFY), Def::Str("start"), "Main-axis distribution of the children."),
];

static WINDOW_PROPS: &[PropDef] = &[
    prop("title", P::Title, Ty::Str, Def::Str(""), "Window title (`ViewportCommand::Title`)."),
    prop("default-width", P::DefaultWidth, Ty::F64, Def::F64(480.0), "Initial inner width in points."),
    prop("default-height", P::DefaultHeight, Ty::F64, Def::F64(360.0), "Initial inner height in points."),
    // The content area is a flex column (the window is the root of its taffy tree).
    layout("padding", P::Padding, Ty::F64, Def::F64(8.0), "Inner spacing of the content area, in points."),
    layout("gap", P::Gap, Ty::F64, Def::F64(0.0), "Space between the window's children, in points."),
];

static LABEL_PROPS: &[PropDef] = &[
    prop("text", P::Text, Ty::Str, Def::Str(""), "The text."),
    prop("wrap-mode", P::WrapMode, Ty::Enum(WRAP_MODE), Def::Str("wrap"), "What a line longer than the width does (`egui::TextWrapMode`)."),
    prop("selectable", P::Selectable, Ty::Bool, Def::Bool(false), "The text can be selected and copied."),
];

static BUTTON_PROPS: &[PropDef] = &[
    prop("text", P::Text, Ty::Str, Def::Str(""), "The caption."),
    prop("selected", P::Selected, Ty::Bool, Def::Bool(false), "Drawn as selected (a toggle look); clicking does not change it."),
    prop("frame", P::Frame, Ty::Bool, Def::Bool(true), "Draws the button background and border."),
    prop("small", P::Small, Ty::Bool, Def::Bool(false), "No vertical padding (`Button::small`)."),
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

const NONE: &[(&str, Ty)] = &[];

static CLICK_SIGNALS: &[SignalDef] = &[signal("clicked", NONE, "Pressed and released (`Response::clicked`).")];

static BUTTON_SIGNALS: &[SignalDef] = &[
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
];

static SLIDER_SIGNALS: &[SignalDef] = &[
    signal("changed", &[("value", Ty::F64)], "The user moved it; carries the new value."),
    signal("drag-started", NONE, "A drag on the slider began."),
    signal("drag-stopped", NONE, "A drag on the slider ended."),
];

static WINDOW_SIGNALS: &[SignalDef] = &[SignalDef {
    name: "close-request",
    args: NONE,
    sync: true,
    doc: "The user asked to close the window; return true to keep it open. Runs before the frame's pass.",
}];

static WINDOW_METHODS: &[MethodDef] = &[
    MethodDef { name: "present", doc: "Shows the window (the first one presented is the root viewport)." },
    MethodDef { name: "close", doc: "Closes the window as the user would (no close-request)." },
];

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
        signals: &[],
        methods: &[],
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
];

/// Index of a kind in [`KINDS`] by id.
pub fn kind_index(id: &str) -> Option<usize> {
    KINDS.iter().position(|k| k.id == id)
}

/// Index of a kind in [`KINDS`] by tag.
pub fn tag_index(tag: &str) -> Option<usize> {
    KINDS.iter().position(|k| k.tag == Some(tag))
}

/// The kind and its bases, most derived first.
pub fn chain(kind: usize) -> Vec<usize> {
    let mut out = vec![kind];
    let mut k = &KINDS[kind];
    while let Some(b) = k.base {
        let i = kind_index(b).expect("base kind");
        out.push(i);
        k = &KINDS[i];
    }
    out
}

// ---- ids: (defining kind + 1) << 16 | (index + 1); kinds are index + 1 ----

pub fn kind_id(kind: usize) -> u32 {
    kind as u32 + 1
}

pub fn kind_of_id(id: u32) -> Option<usize> {
    let i = (id as usize).checked_sub(1)?;
    (i < KINDS.len()).then_some(i)
}

fn member_id(owner: usize, i: usize) -> u32 {
    ((owner as u32 + 1) << 16) | (i as u32 + 1)
}

fn split(id: u32) -> Option<(usize, usize)> {
    let owner = ((id >> 16) as usize).checked_sub(1)?;
    let i = ((id & 0xffff) as usize).checked_sub(1)?;
    (owner < KINDS.len()).then_some((owner, i))
}

/// A property of `kind` (or a base) by name: its id and definition.
pub fn find_prop(kind: usize, name: &str) -> Option<(u32, &'static PropDef)> {
    chain(kind).into_iter().find_map(|k| {
        KINDS[k].props.iter().position(|p| p.name == name).map(|i| (member_id(k, i), &KINDS[k].props[i]))
    })
}

/// The property definition behind an id, if `kind` has it.
pub fn prop_def(kind: usize, id: u32) -> Option<&'static PropDef> {
    let (owner, i) = split(id)?;
    chain(kind).contains(&owner).then(|| KINDS[owner].props.get(i)).flatten()
}

pub fn find_signal(kind: usize, name: &str) -> Option<(u32, &'static SignalDef)> {
    chain(kind).into_iter().find_map(|k| {
        KINDS[k].signals.iter().position(|s| s.name == name).map(|i| (member_id(k, i), &KINDS[k].signals[i]))
    })
}

pub fn signal_def(kind: usize, id: u32) -> Option<&'static SignalDef> {
    let (owner, i) = split(id)?;
    chain(kind).contains(&owner).then(|| KINDS[owner].signals.get(i)).flatten()
}

pub fn find_method(kind: usize, name: &str) -> Option<(u32, &'static MethodDef)> {
    chain(kind).into_iter().find_map(|k| {
        KINDS[k].methods.iter().position(|m| m.name == name).map(|i| (member_id(k, i), &KINDS[k].methods[i]))
    })
}

pub fn method_def(kind: usize, id: u32) -> Option<&'static MethodDef> {
    let (owner, i) = split(id)?;
    chain(kind).contains(&owner).then(|| KINDS[owner].methods.get(i)).flatten()
}

/// The default slot of a container kind (slot id 1; 0 is "default" on the ABI).
pub fn slot_of(kind: usize) -> Option<&'static str> {
    chain(kind).into_iter().find_map(|k| KINDS[k].slot)
}

/// The signal id of `name` on `kind`, or 0.
pub fn signal_id(kind: usize, name: &str) -> u32 {
    find_signal(kind, name).map(|(id, _)| id).unwrap_or(0)
}

/// The property id of `key` on `kind`, or 0.
pub fn prop_id(kind: usize, key: P) -> u32 {
    for k in chain(kind) {
        if let Some(i) = KINDS[k].props.iter().position(|p| p.key == key) {
            return member_id(k, i);
        }
    }
    0
}
