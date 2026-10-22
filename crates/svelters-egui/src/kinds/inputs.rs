//! WG2b input controls (plans/milestones/WG2b-tier0-controls.md): `hyperlink`, `radio-button`,
//! `switch`, `range-slider` and `drag-value`. Their entries are appended to [`KINDS`](super::KINDS);
//! painting and measuring are `crate::elements`.
//!
//! Vocabulary across backends: `radio-button` binds `checked` (Qt, WinUI) and `group` (Svelte's
//! `bind:group` idiom with a per-button `value`); `switch` binds `active` (GTK `Gtk.Switch`);
//! `drag-value` binds `value` like `slider`.

use super::builtin::NONE;
use super::{Def, KindDef, Native, P, PropDef, SignalDef, Ty, input, prop, signal};

static HYPERLINK_PROPS: &[PropDef] = &[
    prop("text", P::Text, Ty::Str, Def::Str(""), "The link text; the url when empty."),
    prop("url", P::Url, Ty::Str, Def::Str(""), "Opened on a click (`Context::open_url`: eframe opens the browser, an embedder reads `PlatformOutput::commands`); empty for none."),
    prop("new-tab", P::NewTab, Ty::Bool, Def::Bool(false), "Asks for a new browser tab (`OpenUrl::new_tab`). A middle click always does."),
];

static HYPERLINK_SIGNALS: &[SignalDef] = &[signal("clicked", NONE, "Pressed and released (before the url opens)."), signal("secondary-clicked", NONE, "Right click.")];

static RADIO_BUTTON_PROPS: &[PropDef] = &[
    prop("text", P::Text, Ty::Str, Def::Str(""), "The caption."),
    input(
        "checked",
        P::Checked,
        Ty::Bool,
        Def::Bool(false),
        "Chosen. A click checks it and unchecks its sibling radio buttons (Qt's auto-exclusive siblings); it never unchecks itself. With `value`, it follows `group == value`.",
    ),
    prop("value", P::Choice, Ty::Str, Def::Str(""), "What this button stands for in its group: a click writes it into `group`."),
    input(
        "group",
        P::Group,
        Ty::Str,
        Def::Str(""),
        "The group's chosen value (`bind:group={choice}` on every button of the group, each with its own `value`). A click sets it on this button and its siblings.",
    ),
];

static RADIO_BUTTON_SIGNALS: &[SignalDef] = &[
    signal("changed", &[("checked", Ty::Bool), ("group", Ty::Str)], "The user's click changed it (or a sibling's click unchecked it); carries the new state and group value."),
    signal("clicked", NONE, "Pressed and released."),
];

static SWITCH_PROPS: &[PropDef] = &[
    input("active", P::Active, Ty::Bool, Def::Bool(false), "On; a click or Space toggles it."),
    prop("text", P::Text, Ty::Str, Def::Str(""), "Caption after the switch (also its accessible name)."),
];

static SWITCH_SIGNALS: &[SignalDef] =
    &[signal("changed", &[("active", Ty::Bool)], "The user toggled it; carries the new state."), signal("clicked", NONE, "Pressed and released.")];

static RANGE_SLIDER_PROPS: &[PropDef] = &[
    input("low", P::Low, Ty::F64, Def::F64(0.0), "The lower end; dragging its handle (or the arrow keys on it) changes it. Kept within `min` and `high`."),
    input("high", P::High, Ty::F64, Def::F64(100.0), "The upper end; dragging its handle changes it. Kept within `low` and `max`."),
    prop("min", P::Min, Ty::F64, Def::F64(0.0), "Lower bound."),
    prop("max", P::Max, Ty::F64, Def::F64(100.0), "Upper bound."),
    prop("step", P::Step, Ty::F64, Def::F64(0.0), "Value granularity; 0 for continuous."),
    prop("text", P::Text, Ty::Str, Def::Str(""), "Caption after the slider (each handle is named `<text> low`/`<text> high`)."),
    prop("show-value", P::ShowValue, Ty::Bool, Def::Bool(true), "Shows an editable value box at each end."),
];

static RANGE_SLIDER_SIGNALS: &[SignalDef] = &[
    signal("changed", &[("low", Ty::F64), ("high", Ty::F64)], "The user moved a handle (or the widget clamped a written value); carries both ends."),
    signal("drag-started", NONE, "A drag on the rail began."),
    signal("drag-stopped", NONE, "A drag on the rail ended."),
];

const fn bound(name: &'static str, key: P, doc: &'static str) -> PropDef {
    PropDef { name, key, ty: Ty::F64, default: Def::Null, nullable: true, native: Native::Never, layout: false, doc }
}

static DRAG_VALUE_PROPS: &[PropDef] = &[
    input("value", P::Value, Ty::F64, Def::F64(0.0), "The value; dragging, the arrow keys or typing change it. Kept within `min` and `max`."),
    bound("min", P::Min, "Lower bound; null for none."),
    bound("max", P::Max, "Upper bound; null for none."),
    prop("speed", P::Speed, Ty::F64, Def::F64(1.0), "Value change per point dragged."),
    PropDef {
        name: "decimals",
        key: P::Decimals,
        ty: Ty::I64,
        default: Def::Null,
        nullable: true,
        native: Native::Never,
        layout: false,
        doc: "Decimals shown (`DragValue::fixed_decimals`); null for egui's automatic precision.",
    },
    prop("prefix", P::Prefix, Ty::Str, Def::Str(""), "Text before the number (`x: `)."),
    prop("suffix", P::Suffix, Ty::Str, Def::Str(""), "Text after the number (` mm`)."),
];

static DRAG_VALUE_SIGNALS: &[SignalDef] = &[
    signal("changed", &[("value", Ty::F64)], "The user changed it; carries the new value."),
    signal("drag-started", NONE, "A drag on it began."),
    signal("drag-stopped", NONE, "A drag on it ended."),
    // CX3: `bind:value|commit` updates its source on this event (egui.json `bindings.value.commit`)
    signal("commit", &[("value", Ty::F64)], "The user committed a value: a drag ended, or a change made otherwise (arrow keys, a typed value applied on Enter or blur); carries the value."),
];

pub(super) const HYPERLINK: KindDef = KindDef {
    id: "Hyperlink",
    tag: Some("hyperlink"),
    native: "egui::Hyperlink",
    base: Some("Widget"),
    toplevel: false,
    slot: None,
    text_content: Some("text"),
    present: None,
    props: HYPERLINK_PROPS,
    signals: HYPERLINK_SIGNALS,
    methods: &[],
    doc: "A link: text that opens `url` when clicked (AccessKit role `Link`).",
};

pub(super) const RADIO_BUTTON: KindDef = KindDef {
    id: "RadioButton",
    tag: Some("radio-button"),
    native: "egui::RadioButton",
    base: Some("Widget"),
    toplevel: false,
    slot: None,
    text_content: Some("text"),
    present: None,
    props: RADIO_BUTTON_PROPS,
    signals: RADIO_BUTTON_SIGNALS,
    methods: &[],
    doc: "One choice of several. Siblings are exclusive: a click checks it and unchecks the other radio buttons of the same parent. Bind `checked` per button, or `group` on every button with a `value` each.",
};

pub(super) const SWITCH: KindDef = KindDef {
    id: "Switch",
    tag: Some("switch"),
    native: "synthetic (egui toggle switch, AccessKit role `Switch`)",
    base: Some("Widget"),
    toplevel: false,
    slot: None,
    text_content: Some("text"),
    present: None,
    props: SWITCH_PROPS,
    signals: SWITCH_SIGNALS,
    methods: &[],
    doc: "An on/off switch with a sliding knob (GTK's `switch`; WinUI's `toggle-switch`).",
};

pub(super) const RANGE_SLIDER: KindDef = KindDef {
    id: "RangeSlider",
    tag: Some("range-slider"),
    native: "egui::RangeSlider",
    base: Some("Widget"),
    toplevel: false,
    slot: None,
    text_content: None,
    present: None,
    props: RANGE_SLIDER_PROPS,
    signals: RANGE_SLIDER_SIGNALS,
    methods: &[],
    doc: "A horizontal slider with two handles selecting `low`..`high`; the handles meet but never cross.",
};

pub(super) const DRAG_VALUE: KindDef = KindDef {
    id: "DragValue",
    tag: Some("drag-value"),
    native: "egui::DragValue",
    base: Some("Widget"),
    toplevel: false,
    slot: None,
    text_content: None,
    present: None,
    props: DRAG_VALUE_PROPS,
    signals: DRAG_VALUE_SIGNALS,
    methods: &[],
    doc: "A number edited by dragging horizontally, the arrow keys, or typing after a click. Name it with `accessible-name` or `tooltip`.",
};
