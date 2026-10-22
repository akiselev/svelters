//! WG1 form controls: `toggle-button` (a segmented group with `radio`) and `combo-box`. Their
//! entries are appended to [`KINDS`](super::KINDS); painting is `leaf.rs`, measure `layout.rs`.

use super::builtin::FONT_WEIGHT_DOC;
use super::{Def, KindDef, P, PropDef, SignalDef, Ty, input, prop, signal};
use super::builtin::NONE;

static TOGGLE_BUTTON_PROPS: &[PropDef] = &[
    prop("text", P::Text, Ty::Str, Def::Str(""), "The caption."),
    input("checked", P::Checked, Ty::Bool, Def::Bool(false), "Pressed state; a click toggles it (with `radio`, a click only sets it)."),
    prop(
        "radio",
        P::Radio,
        Ty::Bool,
        Def::Bool(false),
        "One of a segmented group: a click checks it and never unchecks it (`Ui::selectable_value`); AccessKit role `RadioButton`. The app unchecks the others.",
    ),
    prop("frame", P::Frame, Ty::Bool, Def::Bool(true), "Draws the button background and border."),
    prop("font-weight", P::FontWeight, Ty::I64, Def::I64(400), FONT_WEIGHT_DOC),
];

static TOGGLE_BUTTON_SIGNALS: &[SignalDef] = &[
    signal("changed", &[("checked", Ty::Bool)], "The user toggled it; carries the new state."),
    signal("clicked", NONE, "Pressed and released (also when a `radio` click left it checked)."),
];

static COMBO_BOX_PROPS: &[PropDef] = &[
    prop("options", P::Options, Ty::StrList, Def::Null, "The choices, in order (the popup's rows)."),
    input("selected", P::SelectedIndex, Ty::I64, Def::I64(0), "Index of the chosen option; picking a row changes it. Out of range shows no text."),
    prop("hint-text", P::HintText, Ty::Str, Def::Str(""), "Shown while `selected` is out of range."),
];

static COMBO_BOX_SIGNALS: &[SignalDef] = &[signal("changed", &[("selected", Ty::I64)], "The user picked a row; carries its index.")];

pub(super) const TOGGLE_BUTTON: KindDef = KindDef {
        id: "ToggleButton",
        tag: Some("toggle-button"),
        native: "egui::Button (selected)",
        base: Some("Widget"),
        toplevel: false,
        slot: None,
        text_content: Some("text"),
        present: None,
        props: TOGGLE_BUTTON_PROPS,
        signals: TOGGLE_BUTTON_SIGNALS,
        methods: &[],
        doc: "A button that stays pressed (`checked`). With `radio`, a row of them is a segmented control: a click checks one, the app unchecks the rest.",
    };

pub(super) const COMBO_BOX: KindDef = KindDef {
        id: "ComboBox",
        tag: Some("combo-box"),
        native: "egui::ComboBox",
        base: Some("Widget"),
        toplevel: false,
        slot: None,
        text_content: None,
        present: None,
        props: COMBO_BOX_PROPS,
        signals: COMBO_BOX_SIGNALS,
        methods: &[],
        doc: "A drop-down list: shows the selected option, picks one from a popup. Name it with `accessible-name` (it has no caption).",
    };
