//! The pickers' kind rows: WG4b's `date-picker` and `color-picker` (both with `bind:value`,
//! plans/milestones/WG4b-dialogs-pickers-dnd.md), and OW6's new `calendar` and `color-editor`
//! (plans/milestones/OW6-pickers.md). All four are svelters' own widgets only
//! (`crate::widgets::{date_picker, color_picker, calendar, color_editor}`; `widgets::OWN_ONLY`):
//! OW6 deleted the egui wrappers (`egui_extras::DatePickerButton`, `color_edit_button_hsva`).
//!
//! Values are strings, the same on every backend:
//! - a date is ISO 8601 (`2026-10-22`), `""` for none (the button then shows today);
//! - a colour is CSS hex, `#rrggbb` when opaque and `#rrggbbaa` otherwise (`#rgb`/`#rgba` are
//!   accepted on write and stored normalized).
//!
//! Writer lanes: WIDGETS (WG4b), OW6.

use super::{Def, KindDef, P, PropDef, SignalDef, Ty, input, prop, signal};

/// `color-picker`'s `alpha` values (`egui::color_picker::Alpha`).
pub const ALPHA: &[&str] = &["opaque", "blend", "blend-or-additive"];

/// `calendar`'s and `date-picker`'s calendar props (WIDGET-PARITY §2.19).
macro_rules! calendar_props {
    () => {
        [
            prop("min", P::Min, Ty::Str, Def::Str(""), "The first day that can be picked (ISO 8601); days before it are `:disabled`. `\"\"`: no limit."),
            prop("max", P::Max, Ty::Str, Def::Str(""), "The last day that can be picked (ISO 8601); days after it are `:disabled`. `\"\"`: no limit."),
            prop("first-day", P::FirstDay, Ty::Enum(crate::widgets::calendar::FIRST_DAY), Def::Str("auto"), "The weekday the grid starts on; `auto` is the locale's (Monday until IN1)."),
            prop("weekday-format", P::WeekdayFormat, Ty::Enum(crate::widgets::calendar::WEEKDAY_FORMAT), Def::Str("short"), "The weekday header: `narrow` (M), `short` (Mo), `long` (Monday) or `none` (no header row)."),
            prop("navigation", P::Navigation, Ty::Bool, Def::Bool(true), "Shows the header with the previous/next month buttons and the month and year."),
            prop("read-only", P::ReadOnly, Ty::Bool, Def::Bool(false), "Shows and roves the calendar but never picks (Qt's `NoSelection`)."),
            prop("day-parts", P::DayParts, Ty::StrList, Def::Null, "Extra part names per date, `\"YYYY-MM-DD name…\"` each: `[\"2026-10-22 milestone\"]` makes that day match `::part(day milestone)`."),
        ]
    };
}

/// `color-editor`'s and `color-picker`'s editor props (WIDGET-PARITY §2.20).
macro_rules! color_editor_props {
    () => {
        [
    prop("fields", P::Fields, Ty::Str, Def::Str("hex rgb"), "Which fields show under the strips, space-separated: `hex` (a hex field), `rgb` (R, G, B boxes, 0–255), `hsv` (H 0–360, S and V 0–100)."),
    prop("swatches", P::Swatches, Ty::StrList, Def::Null, "A palette of hex colours shown as cells (`::part(swatch-cell)`); a click picks one."),
    input("custom-colors", P::CustomColors, Ty::StrList, Def::Null, "The user's colours (`bind:custom-colors`, up to 16): the add cell appends the current colour. Persist it with `uiState()`."),
        ]
    };
}


static DATE_PROPS: &[PropDef] = &{
    let [a, b, c, d, e, f, g] = calendar_props!();
    [
        input("value", P::Value, Ty::Str, Def::Str(""), "The date, ISO 8601 (`2026-10-22`); `\"\"` for none (the button shows today). Picking a day (and, with `commit-mode=\"confirm\"`, pressing Save) changes it."),
        prop("format", P::Format, Ty::Str, Def::Str("%Y-%m-%d"), "How the button shows the date (`strftime`, jiff's `civil::Date::strftime`)."),
        prop("calendar-week", P::CalendarWeek, Ty::Bool, Def::Bool(true), "Shows ISO week numbers in the calendar."),
        // ---- OW6: appended ----
        a,
        b,
        c,
        d,
        e,
        f,
        g,
        prop("commit-mode", P::CommitMode, Ty::Enum(crate::widgets::date_picker::DATE_COMMIT_MODE), Def::Str("confirm"), "`confirm`: a picked day is pending until Save (Enter saves, Cancel or Escape drops it); `immediate`: a pick commits and closes the popup (Qt)."),
    ]
};

static COLOR_PROPS: &[PropDef] = &{
    let [x, y, z] = color_editor_props!();
    [
    input(
        "value",
        P::Value,
        Ty::Str,
        Def::Str("#000000"),
        "The colour, CSS hex: `#rrggbb` when opaque, `#rrggbbaa` otherwise (`#rgb`, `#rgba` accepted). Editing it in the popup changes it.",
    ),
    prop(
        "alpha",
        P::Alpha,
        Ty::Enum(ALPHA),
        Def::Str("opaque"),
        "Whether the popup edits alpha: `opaque` hides the alpha strip, `blend` and `blend-or-additive` show it (additive colours are not offered by the own editor).",
    ),
    // ---- OW6: appended ----
    x,
    y,
    z,
    prop("commit-mode", P::CommitMode, Ty::Enum(crate::widgets::color_editor::COLOR_COMMIT_MODE), Def::Str("live"), "`live`: each edit changes `value` (closing the popup commits); `confirm`: edits are pending until Save, Cancel or Escape drops them (Qt's dialog)."),
    ]
};

static DATE_SIGNALS: &[SignalDef] = &[
    signal("changed", &[("value", Ty::Str)], "The user saved a date; carries it (ISO 8601)."),
    // ---- OW6: appended ----
    signal("commit", &[("value", Ty::Str)], "A pick was applied (§2.4): Save, or the pick itself with `commit-mode=\"immediate\"`."),
    signal("page-changed", &[("year", Ty::I64), ("month", Ty::I64)], "The user moved the calendar to another month."),
];

static COLOR_SIGNALS: &[SignalDef] = &[
    signal("changed", &[("value", Ty::Str)], "The user edited the colour; carries it (CSS hex, once per frame while dragging)."),
    // ---- OW6: appended ----
    signal("commit", &[("value", Ty::Str)], "A commit point (§2.4): `live`, a drag released, a step, a field or a swatch, and the popup closing after edits; `confirm`, Save."),
];

pub const DATE_PICKER: KindDef = KindDef {
    id: "DatePicker",
    tag: Some("date-picker"),
    native: "svelters' own (OW6; was egui_extras::DatePickerButton)",
    base: Some("Widget"),
    toplevel: false,
    slot: None,
    text_content: None,
    present: None,
    props: DATE_PROPS,
    signals: DATE_SIGNALS,
    methods: &[],
    doc: "A drop button showing a date; clicking it opens the calendar (the `calendar` kind's) in a popup, with Save/Cancel unless `commit-mode=\"immediate\"`.",
};

pub const COLOR_PICKER: KindDef = KindDef {
    id: "ColorPicker",
    tag: Some("color-picker"),
    native: "svelters' own (OW6; was egui::color_picker::color_edit_button_hsva)",
    base: Some("Widget"),
    toplevel: false,
    slot: None,
    text_content: None,
    present: None,
    props: COLOR_PROPS,
    signals: COLOR_SIGNALS,
    methods: &[],
    doc: "A colour swatch drop button; clicking it opens the colour editor (the `color-editor` kind's) in a popup.",
};

// ---- OW6: calendar (new kind; svelters' own widget only, `widgets::calendar`) ----

static CALENDAR_PROPS: &[PropDef] = &{
    let [a, b, c, d, e, f, g] = calendar_props!();
    [
        input("value", P::Value, Ty::Str, Def::Str(""), "The picked date, ISO 8601 (`2026-10-22`); `\"\"` for none."),
        prop("calendar-week", P::CalendarWeek, Ty::Bool, Def::Bool(true), "Shows ISO week numbers in front of each week (`::part(week-number)`)."),
        a,
        b,
        c,
        d,
        e,
        f,
        g,
    ]
};

static CALENDAR_SIGNALS: &[SignalDef] = &[
    signal("changed", &[("value", Ty::Str)], "The user picked a day; carries it (ISO 8601)."),
    signal("commit", &[("value", Ty::Str)], "A pick is a commit (§2.4); carries the date."),
    signal("page-changed", &[("year", Ty::I64), ("month", Ty::I64)], "The user moved to another month (prev/next, PageUp/PageDown, arrows across a month)."),
];

pub const CALENDAR: KindDef = KindDef {
    id: "Calendar",
    tag: Some("calendar"),
    native: "svelters' own (OW6)",
    base: Some("Widget"),
    toplevel: false,
    slot: None,
    text_content: None,
    present: None,
    props: CALENDAR_PROPS,
    signals: CALENDAR_SIGNALS,
    methods: &[],
    doc: "An inline month calendar (Qt's `QCalendarWidget`): a header with previous/next, a weekday row, ISO week numbers and a 6×7 day grid. `date-picker` shows the same calendar in its popup.",
};

// ---- OW6: color-editor (new kind; svelters' own widget only, `widgets::color_editor`) ----

static COLOR_EDITOR_PROPS: &[PropDef] = &{
    let [a, b, c] = color_editor_props!();
    [
        input("value", P::Value, Ty::Str, Def::Str("#000000"), "The colour, CSS hex: `#rrggbb` when opaque, `#rrggbbaa` otherwise (`#rgb`, `#rgba` accepted)."),
        prop("alpha", P::Alpha, Ty::Enum(ALPHA), Def::Str("opaque"), "`opaque` hides the alpha strip; `blend` and `blend-or-additive` show it."),
        a,
        b,
        c,
    ]
};

static COLOR_EDITOR_SIGNALS: &[SignalDef] = &[
    signal("changed", &[("value", Ty::Str)], "The user edited the colour (each drag frame, step, field or swatch); carries it (CSS hex)."),
    signal("commit", &[("value", Ty::Str)], "A commit point: a drag released, a key step, a field set, a swatch picked (§2.4)."),
];

pub const COLOR_EDITOR: KindDef = KindDef {
    id: "ColorEditor",
    tag: Some("color-editor"),
    native: "svelters' own (OW6)",
    base: Some("Widget"),
    toplevel: false,
    slot: None,
    text_content: None,
    present: None,
    props: COLOR_EDITOR_PROPS,
    signals: COLOR_EDITOR_SIGNALS,
    methods: &[],
    doc: "An inline colour editor (the body of Qt's `QColorDialog`, GTK's `ColorChooserWidget`): a saturation/value square, hue and alpha strips, hex and channel fields, a palette and custom colours. `color-picker` shows the same editor in its popup.",
};
