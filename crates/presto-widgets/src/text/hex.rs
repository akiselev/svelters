//! The colour editor's hex field (`color-editor`'s `::part(fields)` hex box): a [`TextField`] that
//! accepts hex digits and `#`, shows the colour as `#rrggbb` (or `#rrggbbaa` with alpha), and
//! applies a valid hex at Enter or when focus leaves; an invalid one keeps the colour and shows
//! `:invalid` until the next edit; Escape drops the edit. Ported from svelters-egui's
//! `widgets/color_editor.rs` (`parse_color`, `color_hex` and the hex field's seam).

use super::editor::{EditorConfig, FocusReason};
use super::field::{FieldSignal, TextField};
use super::keys::KeyAction;
use super::policy::TextPolicy;
use super::shim::Layouter;
use presto_base::color::Color;
use presto_base::event::KeyEvent;
use presto_base::time::Time;
use std::borrow::Cow;

/// Parses CSS hex (`#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`; the `#` optional) into unmultiplied
/// sRGBA bytes.
pub fn parse_hex(s: &str) -> Option<[u8; 4]> {
    let h = s.trim();
    let h = h.strip_prefix('#').unwrap_or(h);
    if !h.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let nib = |i: usize| u8::from_str_radix(&h[i..i + 1], 16).ok().map(|v| v * 17);
    let byte = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
    match h.len() {
        3 => Some([nib(0)?, nib(1)?, nib(2)?, 255]),
        4 => Some([nib(0)?, nib(1)?, nib(2)?, nib(3)?]),
        6 => Some([byte(0)?, byte(2)?, byte(4)?, 255]),
        8 => Some([byte(0)?, byte(2)?, byte(4)?, byte(6)?]),
        _ => None,
    }
}

/// Unmultiplied sRGBA as CSS hex: `#rrggbb` when opaque, else `#rrggbbaa`.
pub fn color_hex([r, g, b, a]: [u8; 4]) -> String {
    if a == 255 { format!("#{r:02x}{g:02x}{b:02x}") } else { format!("#{r:02x}{g:02x}{b:02x}{a:02x}") }
}

/// A presto colour's bytes (rounded).
pub fn color_bytes(c: Color) -> [u8; 4] {
    let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    [q(c.r), q(c.g), q(c.b), q(c.a)]
}

/// Keeps hex digits and `#` (typing, paste, IME commit).
fn hex_filter(s: &str) -> Cow<'_, str> {
    if s.chars().all(|c| c.is_ascii_hexdigit() || c == '#') {
        Cow::Borrowed(s)
    } else {
        Cow::Owned(s.chars().filter(|c| c.is_ascii_hexdigit() || *c == '#').collect())
    }
}

/// What the hex field tells the colour editor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HexSignal {
    /// A valid hex was applied (alpha forced opaque when the editor has no alpha).
    Commit(Color),
}

pub struct HexField {
    pub field: TextField,
    /// The editor has an alpha channel (else `#rrggbb` and alpha forced to 1).
    pub alpha: bool,
    color: Color,
    invalid: bool,
    signals: Vec<HexSignal>,
}

impl HexField {
    pub fn new(color: Color, alpha: bool, policy: TextPolicy) -> Self {
        // `#` and eight digits
        let cfg = EditorConfig { max_chars: Some(9), ..EditorConfig::default() };
        let mut field = TextField::new("", cfg, policy);
        field.editor.set_filter(Some(hex_filter));
        let mut h = HexField { field, alpha, color, invalid: false, signals: Vec::new() };
        h.show();
        h
    }

    fn shown_hex(&self) -> String {
        let mut b = color_bytes(self.color);
        if !self.alpha {
            b[3] = 255;
        }
        color_hex(b)
    }

    fn show(&mut self) {
        let t = self.shown_hex();
        self.field.set_text(&t);
        self.field.take_signals();
    }

    pub fn color(&self) -> Color {
        self.color
    }

    /// The last applied text was not a colour (`:invalid`, AccessKit `invalid`).
    pub fn invalid(&self) -> bool {
        self.invalid
    }

    pub fn take_signals(&mut self) -> Vec<HexSignal> {
        std::mem::take(&mut self.signals)
    }

    /// The colour prop (the editor's other parts changed it). An edit in progress is kept.
    pub fn set_color(&mut self, c: Color) {
        self.color = c;
        if !self.field.editor.focused() {
            self.show();
        }
    }

    fn apply(&mut self) {
        let text = self.field.text();
        if text == self.shown_hex() {
            return;
        }
        match parse_hex(&text) {
            Some(mut b) => {
                if !self.alpha {
                    b[3] = 255;
                }
                self.color = Color::from_rgba8(b[0], b[1], b[2], b[3]);
                self.invalid = false;
                self.signals.push(HexSignal::Commit(self.color));
                self.show();
            }
            None => self.invalid = true,
        }
    }

    pub fn focus_in(&mut self, reason: FocusReason) {
        self.field.focus_in(reason);
        self.field.take_signals();
    }

    pub fn focus_out(&mut self) {
        self.apply();
        self.field.focus_out();
        self.field.take_signals();
        if !self.invalid {
            self.show();
        }
    }

    pub fn key(&mut self, ev: &KeyEvent, now: Time, lay: &mut dyn Layouter) -> KeyAction {
        let action = self.field.key(ev, now, lay);
        let edited = self.field.take_signals().contains(&FieldSignal::Changed);
        match action {
            KeyAction::Submit => self.apply(),
            KeyAction::Cancel => {
                self.invalid = false;
                self.show();
                let len = self.field.text().len();
                self.field.editor.set_selection(0, len);
            }
            _ if edited => self.invalid = false,
            _ => {}
        }
        action
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::policy::Platform;
    use crate::text::shim::MonoLayouter;
    use presto_base::event::{KeyLocation, KeyState, Modifiers};
    use presto_base::key::Key;

    fn ev(k: Key, text: Option<&str>) -> KeyEvent {
        KeyEvent { key: Some(k), physical: 0, text: text.map(str::to_string), state: KeyState::Pressed, repeat: false, location: KeyLocation::Standard, modifiers: Modifiers::default() }
    }

    #[test]
    fn hex_parse_and_print() {
        assert_eq!(parse_hex("#f80"), Some([255, 136, 0, 255]));
        assert_eq!(parse_hex("11223344"), Some([0x11, 0x22, 0x33, 0x44]));
        assert_eq!(parse_hex("#12345"), None);
        assert_eq!(parse_hex("#zzz"), None);
        assert_eq!(color_hex([1, 2, 3, 255]), "#010203");
        assert_eq!(color_hex([1, 2, 3, 4]), "#01020304");
    }

    #[test]
    fn typing_applies_on_enter_and_filters() {
        let mut lay = MonoLayouter { advance: 8.0, line_height: 16.0 };
        let mut h = HexField::new(Color::from_rgba8(255, 0, 0, 255), false, TextPolicy::of(Platform::Windows));
        assert_eq!(h.field.text(), "#ff0000");
        h.focus_in(FocusReason::Keyboard);
        // all selected: typing replaces; `x` and `!` are filtered out
        for c in ["#", "0", "x", "f", "!", "0"] {
            h.key(&ev(Key::A, Some(c)), Time(0), &mut lay);
        }
        assert_eq!(h.field.text(), "#0f0");
        h.key(&ev(Key::Enter, Some("\r")), Time(0), &mut lay);
        assert_eq!(h.take_signals(), vec![HexSignal::Commit(Color::from_rgba8(0, 255, 0, 255))]);
        assert_eq!(h.field.text(), "#00ff00");
        // an invalid hex keeps the colour and shows :invalid; Escape restores
        h.field.editor.select_all();
        for c in ["1", "2"] {
            h.key(&ev(Key::A, Some(c)), Time(0), &mut lay);
        }
        h.key(&ev(Key::Enter, Some("\r")), Time(0), &mut lay);
        assert!(h.invalid() && h.take_signals().is_empty());
        h.key(&ev(Key::Escape, None), Time(0), &mut lay);
        assert!(!h.invalid());
        assert_eq!(h.field.text(), "#00ff00");
    }
}
