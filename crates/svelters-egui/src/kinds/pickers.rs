//! WG4b: `date-picker` (`egui_extras::DatePickerButton`) and `color-picker` (egui's
//! `color_edit_button_hsva`), both with `bind:value` (plans/milestones/WG4b-dialogs-pickers-dnd.md).
//!
//! The tables are `const`s spliced into [`super::KINDS`]; the node payload is [`W::Picker`] holding a
//! [`Picker`], whose props, painting and measuring live here. `leaf.rs`, `layout.rs`,
//! `scene/props.rs` and `scene/tree.rs` have one dispatch arm each.
//!
//! Values are strings, the same on every backend:
//! - a date is ISO 8601 (`2026-10-22`), `""` for none (the button then shows today);
//! - a colour is CSS hex, `#rrggbb` when opaque and `#rrggbbaa` otherwise (`#rgb`/`#rgba` are
//!   accepted on write and stored normalized).
//!
//! Writer lane: WIDGETS (WG4b).
//!
//! [`W::Picker`]: crate::scene::W::Picker

use super::{Def, KindDef, P, PropDef, SignalDef, Ty, input, prop, signal};
use egui::ecolor::Hsva;
use svelters_abi::Value;

/// `color-picker`'s `alpha` values (`egui::color_picker::Alpha`).
pub const ALPHA: &[&str] = &["opaque", "blend", "blend-or-additive"];

static DATE_PROPS: &[PropDef] = &[
    input("value", P::Value, Ty::Str, Def::Str(""), "The date, ISO 8601 (`2026-10-22`); `\"\"` for none (the button shows today). Picking a day and pressing Save changes it."),
    prop("format", P::Format, Ty::Str, Def::Str("%Y-%m-%d"), "How the button shows the date (`strftime`, jiff's `civil::Date::strftime`)."),
    prop("calendar-week", P::CalendarWeek, Ty::Bool, Def::Bool(true), "Shows ISO week numbers in the calendar."),
];

static COLOR_PROPS: &[PropDef] = &[
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
        "Whether the popup edits alpha (`egui::color_picker::Alpha`): `opaque` hides it, `blend` edits it, `blend-or-additive` also offers additive.",
    ),
];

static DATE_SIGNALS: &[SignalDef] = &[signal("changed", &[("value", Ty::Str)], "The user saved a date; carries it (ISO 8601).")];

static COLOR_SIGNALS: &[SignalDef] = &[signal("changed", &[("value", Ty::Str)], "The user edited the colour; carries it (CSS hex, once per frame while dragging).")];

pub const DATE_PICKER: KindDef = KindDef {
    id: "DatePicker",
    tag: Some("date-picker"),
    native: "egui_extras::DatePickerButton",
    base: Some("Widget"),
    toplevel: false,
    slot: None,
    text_content: None,
    present: None,
    props: DATE_PROPS,
    signals: DATE_SIGNALS,
    methods: &[],
    doc: "A button showing a date; clicking it opens a calendar popup (Save/Cancel).",
};

pub const COLOR_PICKER: KindDef = KindDef {
    id: "ColorPicker",
    tag: Some("color-picker"),
    native: "egui::color_picker::color_edit_button_hsva",
    base: Some("Widget"),
    toplevel: false,
    slot: None,
    text_content: None,
    present: None,
    props: COLOR_PROPS,
    signals: COLOR_SIGNALS,
    methods: &[],
    doc: "A colour swatch; clicking it opens egui's colour picker popup.",
};

/// WG6 (QW10): `key-sequence-edit` (Qt's `QKeySequenceEdit`).
static KEY_SEQ_PROPS: &[PropDef] = &[input(
    "value",
    P::Value,
    Ty::Str,
    Def::Str(""),
    "The shortcut, `Ctrl+Shift+K` (modifiers `Ctrl`, `Alt`, `Shift`, `Cmd` in that order, then egui's key name, `Key::name`); `\"\"` for none. Click (or Space/Enter) and press a shortcut to record it; Escape cancels, Backspace or Delete clears it.",
)];

static KEY_SEQ_SIGNALS: &[SignalDef] = &[signal("changed", &[("value", Ty::Str)], "The user recorded or cleared a shortcut; carries it.")];

pub const KEY_SEQUENCE_EDIT: KindDef = KindDef {
    id: "KeySequenceEdit",
    tag: Some("key-sequence-edit"),
    native: "egui::Button",
    base: Some("Widget"),
    toplevel: false,
    slot: None,
    text_content: None,
    present: None,
    props: KEY_SEQ_PROPS,
    signals: KEY_SEQ_SIGNALS,
    methods: &[],
    doc: "Records one keyboard shortcut (Qt's `QKeySequenceEdit`, one chord): a button showing the shortcut; while it records, it takes every key but Tab (EV3 `shortcut_override` answers true), so the app's own shortcuts do not fire.",
};

/// QW10: a shortcut as `key-sequence-edit` writes it.
pub fn shortcut_text(key: egui::Key, m: egui::Modifiers) -> String {
    let mut s = String::new();
    for (on, name) in [(m.ctrl || (m.command && !m.mac_cmd), "Ctrl"), (m.alt, "Alt"), (m.shift, "Shift"), (m.mac_cmd, "Cmd")] {
        if on {
            s.push_str(name);
            s.push('+');
        }
    }
    s.push_str(key.name());
    s
}

const KEY_SEQ_PROMPT: &str = "Press a shortcut…";
const KEY_SEQ_NONE: &str = "None";

/// The payload of a picker node.
#[derive(Clone, Debug, PartialEq)]
pub enum Picker {
    Date {
        /// ISO 8601 or empty.
        value: String,
        format: String,
        calendar_week: bool,
    },
    Color {
        /// Unmultiplied sRGBA.
        rgba: [u8; 4],
        alpha: &'static str,
        /// The popup's HSVA while the user edits (keeps the hue of greys); dropped on a host write.
        hsva: Option<Hsva>,
    },
    /// WG6 (QW10): `key-sequence-edit`.
    KeySeq {
        value: String,
        /// Waiting for a key (after a click, until a key, Escape or losing focus).
        recording: bool,
    },
}

/// Parses an ISO 8601 date (`""` is none).
pub fn parse_date(s: &str) -> Result<Option<jiff::civil::Date>, String> {
    if s.is_empty() {
        return Ok(None);
    }
    s.parse::<jiff::civil::Date>().map(Some).map_err(|e| format!("`{s}` is not an ISO 8601 date (YYYY-MM-DD): {e}"))
}

/// Parses a CSS hex colour into unmultiplied sRGBA.
pub fn parse_color(s: &str) -> Result<[u8; 4], String> {
    egui::Color32::from_hex(s.trim()).map(|c| c.to_srgba_unmultiplied()).map_err(|e| format!("`{s}` is not a hex colour (#rrggbb, #rrggbbaa, #rgb, #rgba): {e:?}"))
}

/// Unmultiplied sRGBA as CSS hex: `#rrggbb` when opaque, else `#rrggbbaa`.
pub fn color_hex([r, g, b, a]: [u8; 4]) -> String {
    if a == 255 { format!("#{r:02x}{g:02x}{b:02x}") } else { format!("#{r:02x}{g:02x}{b:02x}{a:02x}") }
}

impl Picker {
    /// The default payload of kind `id` (`DatePicker`, `ColorPicker`).
    pub fn new(id: &str) -> Picker {
        match id {
            "DatePicker" => Picker::Date { value: String::new(), format: "%Y-%m-%d".into(), calendar_week: true },
            "KeySequenceEdit" => Picker::KeySeq { value: String::new(), recording: false },
            _ => Picker::Color { rgba: [0, 0, 0, 255], alpha: "opaque", hsva: None },
        }
    }

    /// The `value` prop.
    pub fn value(&self) -> Value {
        match self {
            Picker::Date { value, .. } => Value::Str(value.clone()),
            Picker::Color { rgba, .. } => Value::Str(color_hex(*rgba)),
            Picker::KeySeq { value, .. } => Value::Str(value.clone()),
        }
    }

    /// What the node shows as text (queries, dumps): the value.
    pub fn text(&self) -> &str {
        match self {
            Picker::Date { value, .. } | Picker::KeySeq { value, .. } => value,
            Picker::Color { .. } => "",
        }
    }

    /// Writes prop `key` (already defaulted for `null`). A picker's props only need a repaint,
    /// except `format`, which changes the button's text.
    pub fn set(&mut self, key: P, v: &Value) -> Result<bool, String> {
        let s = || match v {
            Value::Str(s) => Ok(s.clone()),
            Value::Null => Ok(String::new()),
            other => Err(format!("expected a string, got {other:?}")),
        };
        match (key, self) {
            (P::Value, Picker::Date { value, .. }) => {
                let new = s()?;
                *value = parse_date(&new)?.map(|d| d.to_string()).unwrap_or_default();
                Ok(true)
            }
            (P::Format, Picker::Date { format, .. }) => {
                *format = s()?;
                Ok(true)
            }
            (P::CalendarWeek, Picker::Date { calendar_week, .. }) => {
                *calendar_week = match v {
                    Value::Bool(b) => *b,
                    other => return Err(format!("expected a boolean, got {other:?}")),
                };
                Ok(false)
            }
            (P::Value, Picker::Color { rgba, hsva, .. }) => {
                *rgba = parse_color(&s()?)?;
                *hsva = None;
                Ok(false)
            }
            (P::Value, Picker::KeySeq { value, .. }) => {
                *value = s()?;
                Ok(true)
            }
            (P::Alpha, Picker::Color { alpha, .. }) => {
                let new = s()?;
                *alpha = ALPHA.iter().copied().find(|a| *a == new).ok_or_else(|| format!("`{new}` is not one of {}", ALPHA.join(", ")))?;
                Ok(false)
            }
            (key, p) => Err(format!("{key:?} is not a prop of {p:?}")),
        }
    }

    /// Reads prop `key`.
    pub fn get(&self, key: P) -> Option<Value> {
        Some(match (key, self) {
            (P::Value, p) => p.value(),
            (P::Format, Picker::Date { format, .. }) => Value::Str(format.clone()),
            (P::CalendarWeek, Picker::Date { calendar_week, .. }) => Value::Bool(*calendar_week),
            (P::Alpha, Picker::Color { alpha, .. }) => Value::Str(alpha.to_string()),
            _ => return None,
        })
    }

    /// The date the button shows: the value, else today.
    fn shown_date(value: &str) -> jiff::civil::Date {
        parse_date(value).ok().flatten().unwrap_or_else(|| jiff::Zoned::now().date())
    }

    /// The button's text (as `DatePickerButton` draws it; no calendar icon: egui's default fonts have no emoji).
    fn date_text(value: &str, format: &str) -> String {
        Picker::shown_date(value).strftime(format).to_string()
    }

    /// Paints the widget in `ui` (the node's rect); a user edit updates the payload, which
    /// `leaf.rs` compares with the value before the widget ran.
    pub fn ui(&mut self, ui: &mut egui::Ui, size: egui::Vec2) -> egui::Response {
        match self {
            Picker::Date { value, format, calendar_week } => {
                let mut date = Picker::shown_date(value);
                let resp = ui.add(
                    egui_extras::DatePickerButton::new(&mut date).id_salt("date-picker").show_icon(false).combo_boxes(false).format(format.clone()).calendar_week(*calendar_week),
                );
                if resp.changed() {
                    *value = date.to_string();
                }
                resp
            }
            Picker::Color { rgba, alpha, hsva } => {
                let mode = match *alpha {
                    "blend" => egui::color_picker::Alpha::OnlyBlend,
                    "blend-or-additive" => egui::color_picker::Alpha::BlendOrAdditive,
                    _ => egui::color_picker::Alpha::Opaque,
                };
                let [r, g, b, a] = *rgba;
                let mut h = hsva.unwrap_or_else(|| Hsva::from_srgba_unmultiplied([r, g, b, a]));
                // the swatch fills the node's rect (egui sizes it by `interact_size`)
                ui.spacing_mut().interact_size = size.max(egui::vec2(8.0, 8.0));
                let resp = egui::color_picker::color_edit_button_hsva(ui, &mut h, mode);
                if resp.changed() {
                    *hsva = Some(h);
                    let mut new = h.to_srgba_unmultiplied();
                    if mode == egui::color_picker::Alpha::Opaque {
                        new[3] = 255;
                    }
                    *rgba = new;
                }
                resp
            }
            Picker::KeySeq { value, recording } => {
                let text = if *recording { KEY_SEQ_PROMPT } else if value.is_empty() { KEY_SEQ_NONE } else { value.as_str() };
                let resp = ui.add(egui::Button::new(text).selected(*recording).min_size(size));
                if resp.clicked() {
                    *recording = true;
                    resp.request_focus();
                }
                if *recording && !resp.has_focus() && !resp.clicked() {
                    *recording = false;
                } else if *recording {
                    let press = ui.input(|i| {
                        i.events.iter().find_map(|e| match e {
                            egui::Event::Key { key, pressed: true, repeat: false, modifiers, .. } => Some((*key, *modifiers)),
                            _ => None,
                        })
                    });
                    if let Some((key, m)) = press {
                        let plain = !m.ctrl && !m.alt && !m.shift && !m.command && !m.mac_cmd;
                        match key {
                            egui::Key::Escape | egui::Key::Tab if plain => {}
                            egui::Key::Backspace | egui::Key::Delete if plain => value.clear(),
                            k => *value = shortcut_text(k, m),
                        }
                        *recording = false;
                    }
                }
                resp
            }
        }
    }

    /// The size the widget asks for.
    pub fn measure(&self, m: &crate::layout::Metrics) -> egui::Vec2 {
        let sp = &m.style.spacing;
        match self {
            Picker::Date { value, format, .. } => {
                let font = egui::TextStyle::Button.resolve(&m.style);
                let t = m.ctx.fonts_mut(|f| f.layout_no_wrap(Picker::date_text(value, format), font, egui::Color32::WHITE).size());
                egui::vec2(t.x + 2.0 * sp.button_padding.x, (t.y + 2.0 * sp.button_padding.y).max(sp.interact_size.y))
            }
            Picker::Color { .. } => sp.interact_size,
            Picker::KeySeq { value, .. } => {
                let font = egui::TextStyle::Button.resolve(&m.style);
                let w = [KEY_SEQ_PROMPT, KEY_SEQ_NONE, value.as_str()]
                    .iter()
                    .map(|t| m.ctx.fonts_mut(|f| f.layout_no_wrap((*t).to_owned(), font.clone(), egui::Color32::WHITE).size()))
                    .fold(egui::Vec2::ZERO, |a, b| a.max(b));
                egui::vec2(w.x + 2.0 * sp.button_padding.x, (w.y + 2.0 * sp.button_padding.y).max(sp.interact_size.y))
            }
        }
    }
}

/// The kind ids of the pickers (for `scene/tree.rs`).
pub fn is_picker(id: &str) -> bool {
    matches!(id, "DatePicker" | "ColorPicker" | "KeySequenceEdit")
}
