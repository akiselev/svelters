//! Key mapping without AppKit: macOS virtual key codes (`kVK_*`, HIToolbox `Events.h`) to egui-named
//! [`Key`]s, USB HID usages and key locations; `NSEventModifierFlags` to skelter [`Modifiers`]; the
//! modifier-key transitions `flagsChanged:` implies; the Option-as-Alt policy; and the choice of which
//! layout characters name a key (the layouts that switch on Cmd, non-Latin layouts).
//!
//! [`key_chars`] is adapted from GPUI's `parse_keystroke`, `crates/gpui_macos/src/events.rs` of
//! zed-industries/zed at a5c4ca8 (Apache-2.0). Modifications by the svelters authors.

use skelter_base::event::{KeyLocation, KeyState, Modifiers};
use skelter_base::key::Key;

/// USB HID usage page 7 (keyboard), as the high half of [`physical_code`]'s value.
pub const HID_KEYBOARD_PAGE: u32 = 0x0007_0000;

/// `NSEventModifierFlags` bits (device-independent).
pub mod flags {
    pub const CAPS_LOCK: u64 = 1 << 16;
    pub const SHIFT: u64 = 1 << 17;
    pub const CONTROL: u64 = 1 << 18;
    pub const OPTION: u64 = 1 << 19;
    pub const COMMAND: u64 = 1 << 20;
    pub const NUMERIC_PAD: u64 = 1 << 21;
    pub const FUNCTION: u64 = 1 << 23;
    /// The device-dependent bits AppKit keeps in the low word (IOKit `NX_DEVICE*KEYMASK`): which side's
    /// modifier is down.
    pub const LEFT_CONTROL: u64 = 0x0000_0001;
    pub const LEFT_SHIFT: u64 = 0x0000_0002;
    pub const RIGHT_SHIFT: u64 = 0x0000_0004;
    pub const LEFT_COMMAND: u64 = 0x0000_0008;
    pub const RIGHT_COMMAND: u64 = 0x0000_0010;
    pub const LEFT_OPTION: u64 = 0x0000_0020;
    pub const RIGHT_OPTION: u64 = 0x0000_0040;
    pub const RIGHT_CONTROL: u64 = 0x0000_2000;
}

/// Virtual key codes (`kVK_*`) this module names.
pub mod vk {
    pub const ANSI_A: u16 = 0x00;
    pub const RETURN: u16 = 0x24;
    pub const TAB: u16 = 0x30;
    pub const SPACE: u16 = 0x31;
    pub const DELETE: u16 = 0x33;
    pub const ESCAPE: u16 = 0x35;
    pub const RIGHT_COMMAND: u16 = 0x36;
    pub const COMMAND: u16 = 0x37;
    pub const SHIFT: u16 = 0x38;
    pub const CAPS_LOCK: u16 = 0x39;
    pub const OPTION: u16 = 0x3A;
    pub const CONTROL: u16 = 0x3B;
    pub const RIGHT_SHIFT: u16 = 0x3C;
    pub const RIGHT_OPTION: u16 = 0x3D;
    pub const RIGHT_CONTROL: u16 = 0x3E;
    pub const FUNCTION: u16 = 0x3F;
    pub const KEYPAD_ENTER: u16 = 0x4C;
}

/// Which held Option key acts as Alt (no text, no dead keys) instead of typing the layout's Option
/// characters (Option+A = "å" on US). Default: neither, as every Mac text field behaves.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OptionAsAlt {
    #[default]
    None,
    OnlyLeft,
    OnlyRight,
    Both,
}

/// Keys that mean the same on every layout, by virtual key code.
pub fn named_key(code: u16) -> Option<Key> {
    Some(match code {
        0x24 | 0x34 | 0x4C => Key::Enter,
        0x30 => Key::Tab,
        0x31 => Key::Space,
        0x33 => Key::Backspace,
        0x35 => Key::Escape,
        0x36 => Key::SuperRight,
        0x37 => Key::SuperLeft,
        0x38 => Key::ShiftLeft,
        0x39 => Key::CapsLock,
        0x3A => Key::AltLeft,
        0x3B => Key::ControlLeft,
        0x3C => Key::ShiftRight,
        0x3D => Key::AltRight,
        0x3E => Key::ControlRight,
        0x3F => Key::Fn,
        0x40 => Key::F17,
        0x41 => Key::Period,
        0x45 => Key::Plus,
        0x47 => Key::Clear,
        0x48 => Key::AudioVolumeUp,
        0x49 => Key::AudioVolumeDown,
        0x4A => Key::AudioVolumeMute,
        0x4B => Key::Slash,
        0x4E => Key::Minus,
        0x4F => Key::F18,
        0x50 => Key::F19,
        0x51 => Key::Equals,
        0x52 => Key::Num0,
        0x53 => Key::Num1,
        0x54 => Key::Num2,
        0x55 => Key::Num3,
        0x56 => Key::Num4,
        0x57 => Key::Num5,
        0x58 => Key::Num6,
        0x59 => Key::Num7,
        0x5A => Key::F20,
        0x5B => Key::Num8,
        0x5C => Key::Num9,
        0x5F => Key::Comma,
        0x60 => Key::F5,
        0x61 => Key::F6,
        0x62 => Key::F7,
        0x63 => Key::F3,
        0x64 => Key::F8,
        0x65 => Key::F9,
        0x67 => Key::F11,
        0x69 => Key::F13,
        0x6A => Key::F16,
        0x6B => Key::F14,
        0x6D => Key::F10,
        0x6E => Key::Menu,
        0x6F => Key::F12,
        0x71 => Key::F15,
        // PC keyboards' Insert arrives as kVK_Help.
        0x72 => Key::Insert,
        0x73 => Key::Home,
        0x74 => Key::PageUp,
        0x75 => Key::Delete,
        0x76 => Key::F4,
        0x77 => Key::End,
        0x78 => Key::F2,
        0x79 => Key::PageDown,
        0x7A => Key::F1,
        0x7B => Key::ArrowLeft,
        0x7C => Key::ArrowRight,
        0x7D => Key::ArrowDown,
        0x7E => Key::ArrowUp,
        _ => return None,
    })
}

/// The key a character key's position has on US QWERTY: the fallback name when the layout's
/// character has no egui name (Cyrillic, Greek, "ö"), so Cmd+С on a Russian layout is still Cmd+C.
pub fn qwerty_key(code: u16) -> Option<Key> {
    Some(match code {
        0x00 => Key::A,
        0x01 => Key::S,
        0x02 => Key::D,
        0x03 => Key::F,
        0x04 => Key::H,
        0x05 => Key::G,
        0x06 => Key::Z,
        0x07 => Key::X,
        0x08 => Key::C,
        0x09 => Key::V,
        0x0A => Key::IntlBackslash,
        0x0B => Key::B,
        0x0C => Key::Q,
        0x0D => Key::W,
        0x0E => Key::E,
        0x0F => Key::R,
        0x10 => Key::Y,
        0x11 => Key::T,
        0x12 => Key::Num1,
        0x13 => Key::Num2,
        0x14 => Key::Num3,
        0x15 => Key::Num4,
        0x16 => Key::Num6,
        0x17 => Key::Num5,
        0x18 => Key::Equals,
        0x19 => Key::Num9,
        0x1A => Key::Num7,
        0x1B => Key::Minus,
        0x1C => Key::Num8,
        0x1D => Key::Num0,
        0x1E => Key::CloseBracket,
        0x1F => Key::O,
        0x20 => Key::U,
        0x21 => Key::OpenBracket,
        0x22 => Key::I,
        0x23 => Key::P,
        0x25 => Key::L,
        0x26 => Key::J,
        0x27 => Key::Quote,
        0x28 => Key::K,
        0x29 => Key::Semicolon,
        0x2A => Key::Backslash,
        0x2B => Key::Comma,
        0x2C => Key::Slash,
        0x2D => Key::N,
        0x2E => Key::M,
        0x2F => Key::Period,
        0x32 => Key::Backtick,
        _ => return None,
    })
}

/// The physical key as a USB HID usage (`page << 16 | id`, page 7), 0 when the key has none.
pub fn physical_code(code: u16) -> u32 {
    let id: u32 = match code {
        0x00 => 0x04, // A
        0x01 => 0x16, // S
        0x02 => 0x07, // D
        0x03 => 0x09, // F
        0x04 => 0x0B, // H
        0x05 => 0x0A, // G
        0x06 => 0x1D, // Z
        0x07 => 0x1B, // X
        0x08 => 0x06, // C
        0x09 => 0x19, // V
        0x0A => 0x64, // ISO section: IntlBackslash
        0x0B => 0x05, // B
        0x0C => 0x14, // Q
        0x0D => 0x1A, // W
        0x0E => 0x08, // E
        0x0F => 0x15, // R
        0x10 => 0x1C, // Y
        0x11 => 0x17, // T
        0x12 => 0x1E, // 1
        0x13 => 0x1F, // 2
        0x14 => 0x20, // 3
        0x15 => 0x21, // 4
        0x16 => 0x23, // 6
        0x17 => 0x22, // 5
        0x18 => 0x2E, // =
        0x19 => 0x26, // 9
        0x1A => 0x24, // 7
        0x1B => 0x2D, // -
        0x1C => 0x25, // 8
        0x1D => 0x27, // 0
        0x1E => 0x30, // ]
        0x1F => 0x12, // O
        0x20 => 0x18, // U
        0x21 => 0x2F, // [
        0x22 => 0x0C, // I
        0x23 => 0x13, // P
        0x24 => 0x28, // Return
        0x25 => 0x0F, // L
        0x26 => 0x0D, // J
        0x27 => 0x34, // '
        0x28 => 0x0E, // K
        0x29 => 0x33, // ;
        0x2A => 0x31, // \
        0x2B => 0x36, // ,
        0x2C => 0x38, // /
        0x2D => 0x11, // N
        0x2E => 0x10, // M
        0x2F => 0x37, // .
        0x30 => 0x2B, // Tab
        0x31 => 0x2C, // Space
        0x32 => 0x35, // `
        0x33 => 0x2A, // Delete: Backspace
        0x34 => 0x58, // PowerBook Enter: keypad Enter
        0x35 => 0x29, // Escape
        0x36 => 0xE7, // Right Command: right GUI
        0x37 => 0xE3, // Command: left GUI
        0x38 => 0xE1, // Shift
        0x39 => 0x39, // Caps Lock
        0x3A => 0xE2, // Option: left Alt
        0x3B => 0xE0, // Control
        0x3C => 0xE5, // Right Shift
        0x3D => 0xE6, // Right Option
        0x3E => 0xE4, // Right Control
        0x40 => 0x6C, // F17
        0x41 => 0x63, // keypad .
        0x43 => 0x55, // keypad *
        0x45 => 0x57, // keypad +
        0x47 => 0x53, // keypad Clear: Num Lock
        0x48 => 0x80, // Volume Up
        0x49 => 0x81, // Volume Down
        0x4A => 0x7F, // Mute
        0x4B => 0x54, // keypad /
        0x4C => 0x58, // keypad Enter
        0x4E => 0x56, // keypad -
        0x4F => 0x6D, // F18
        0x50 => 0x6E, // F19
        0x51 => 0x67, // keypad =
        0x52 => 0x62, // keypad 0
        0x53 => 0x59, // keypad 1
        0x54 => 0x5A, // keypad 2
        0x55 => 0x5B, // keypad 3
        0x56 => 0x5C, // keypad 4
        0x57 => 0x5D, // keypad 5
        0x58 => 0x5E, // keypad 6
        0x59 => 0x5F, // keypad 7
        0x5A => 0x6F, // F20
        0x5B => 0x60, // keypad 8
        0x5C => 0x61, // keypad 9
        0x5D => 0x89, // JIS Yen: International3
        0x5E => 0x87, // JIS Underscore: International1
        0x5F => 0x85, // JIS keypad comma
        0x60 => 0x3E, // F5
        0x61 => 0x3F, // F6
        0x62 => 0x40, // F7
        0x63 => 0x3C, // F3
        0x64 => 0x41, // F8
        0x65 => 0x42, // F9
        0x66 => 0x91, // JIS Eisu: Lang2
        0x67 => 0x44, // F11
        0x68 => 0x90, // JIS Kana: Lang1
        0x69 => 0x68, // F13
        0x6A => 0x6B, // F16
        0x6B => 0x69, // F14
        0x6D => 0x43, // F10
        0x6E => 0x65, // Context menu: Application
        0x6F => 0x45, // F12
        0x71 => 0x6A, // F15
        0x72 => 0x49, // Help: Insert
        0x73 => 0x4A, // Home
        0x74 => 0x4B, // Page Up
        0x75 => 0x4C, // Forward Delete
        0x76 => 0x3D, // F4
        0x77 => 0x4D, // End
        0x78 => 0x3B, // F2
        0x79 => 0x4E, // Page Down
        0x7A => 0x3A, // F1
        0x7B => 0x50, // Left
        0x7C => 0x4F, // Right
        0x7D => 0x51, // Down
        0x7E => 0x52, // Up
        _ => return 0,
    };
    HID_KEYBOARD_PAGE | id
}

pub fn key_location(code: u16) -> KeyLocation {
    match code {
        0x37 | 0x38 | 0x3A | 0x3B => KeyLocation::Left,
        0x36 | 0x3C | 0x3D | 0x3E => KeyLocation::Right,
        0x41 | 0x43 | 0x45 | 0x47 | 0x4B | 0x4C | 0x4E | 0x51..=0x59 | 0x5B | 0x5C | 0x5F => {
            KeyLocation::Numpad
        }
        _ => KeyLocation::Standard,
    }
}

/// skelter's modifiers from `NSEventModifierFlags`. Option is Alt (macOS has no AltGr key; Option's
/// text role is the Option-as-Alt policy's business).
pub fn modifiers_from_flags(raw: u64) -> Modifiers {
    let mut m = Modifiers::default();
    if raw & flags::SHIFT != 0 {
        m = m.union(Modifiers::SHIFT);
    }
    if raw & flags::CONTROL != 0 {
        m = m.union(Modifiers::CTRL);
    }
    if raw & flags::OPTION != 0 {
        m = m.union(Modifiers::ALT);
    }
    if raw & flags::COMMAND != 0 {
        m = m.union(Modifiers::SUPER);
    }
    m
}

/// What a `flagsChanged:` event did to the key it names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Transition {
    Pressed,
    Released,
    /// Caps Lock: AppKit reports each press as a toggle and no release.
    Toggled,
}

/// The modifier key a `flagsChanged:` event names and whether it went down or up. The side comes from
/// the device-dependent bits; events without them (synthesised ones) fall back to the generic flag.
pub fn modifier_transition(code: u16, raw: u64) -> Option<(Key, Transition)> {
    let (key, side, other, generic) = match code {
        vk::SHIFT => (
            Key::ShiftLeft,
            flags::LEFT_SHIFT,
            flags::RIGHT_SHIFT,
            flags::SHIFT,
        ),
        vk::RIGHT_SHIFT => (
            Key::ShiftRight,
            flags::RIGHT_SHIFT,
            flags::LEFT_SHIFT,
            flags::SHIFT,
        ),
        vk::CONTROL => (
            Key::ControlLeft,
            flags::LEFT_CONTROL,
            flags::RIGHT_CONTROL,
            flags::CONTROL,
        ),
        vk::RIGHT_CONTROL => (
            Key::ControlRight,
            flags::RIGHT_CONTROL,
            flags::LEFT_CONTROL,
            flags::CONTROL,
        ),
        vk::OPTION => (
            Key::AltLeft,
            flags::LEFT_OPTION,
            flags::RIGHT_OPTION,
            flags::OPTION,
        ),
        vk::RIGHT_OPTION => (
            Key::AltRight,
            flags::RIGHT_OPTION,
            flags::LEFT_OPTION,
            flags::OPTION,
        ),
        vk::COMMAND => (
            Key::SuperLeft,
            flags::LEFT_COMMAND,
            flags::RIGHT_COMMAND,
            flags::COMMAND,
        ),
        vk::RIGHT_COMMAND => (
            Key::SuperRight,
            flags::RIGHT_COMMAND,
            flags::LEFT_COMMAND,
            flags::COMMAND,
        ),
        vk::CAPS_LOCK => return Some((Key::CapsLock, Transition::Toggled)),
        vk::FUNCTION => {
            let t = if raw & flags::FUNCTION != 0 {
                Transition::Pressed
            } else {
                Transition::Released
            };
            return Some((Key::Fn, t));
        }
        _ => return None,
    };
    let device_bits = raw & (side | other) != 0;
    let down = if device_bits {
        raw & side != 0
    } else {
        raw & generic != 0
    };
    Some((
        key,
        if down {
            Transition::Pressed
        } else {
            Transition::Released
        },
    ))
}

impl Transition {
    /// The key events this transition produces, in order.
    pub fn states(self) -> &'static [KeyState] {
        match self {
            Transition::Pressed => &[KeyState::Pressed],
            Transition::Released => &[KeyState::Released],
            Transition::Toggled => &[KeyState::Pressed, KeyState::Released],
        }
    }
}

/// Whether the held Option key acts as Alt under `policy` (the key then types no text).
pub fn option_is_alt(policy: OptionAsAlt, raw: u64) -> bool {
    if raw & flags::OPTION == 0 {
        return false;
    }
    let left = raw & flags::LEFT_OPTION != 0;
    let right = raw & flags::RIGHT_OPTION != 0;
    match policy {
        OptionAsAlt::None => false,
        OptionAsAlt::Both => true,
        // Without device bits the side is unknown: only `Both` applies then.
        OptionAsAlt::OnlyLeft => left,
        OptionAsAlt::OnlyRight => right,
    }
}

/// What the current layout types for one key under the modifier combinations [`key_chars`] needs
/// (`UCKeyTranslate` results, dead keys resolved to their spacing character).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LayoutChars {
    pub plain: String,
    pub shift: String,
    pub cmd: String,
    pub cmd_shift: String,
}

/// The characters that name a key press for key bindings (the egui key name comes from them).
///
/// - Layouts that switch to QWERTY while Cmd is down (Dvorak-QWERTY) and non-ASCII layouts
///   (`always_cmd_layout`: Armenian, Russian) use the Cmd layer, so Cmd+S is Cmd+S.
/// - Shifted letters stay letters (Shift is a modifier of the letter); other shifted keys name the
///   shifted character (Shift+1 is "!"), except where Shift+Cmd types the same as Cmd (Norwegian), or
///   reports the unshifted letter (Ukrainian).
///
/// Cases (QWERTY key | none | Cmd | Cmd-Shift): Armenian s | ս | cmd-s | cmd-shift-s; Dvorak-QWERTY
/// s | o | cmd-s | cmd-shift-s; Ukrainian-QWERTY s | с | cmd-s | cmd-shift-s; Czech 7 | ý | cmd-ý |
/// cmd-7; Norwegian 7 | 7 | cmd-7 | cmd-/; Russian 7 | 7 | cmd-7 | cmd-&; German ; | ö | cmd-ö | cmd-Ö.
pub fn key_chars(l: &LayoutChars, command: bool, shift: bool, always_cmd_layout: bool) -> String {
    let mut ignoring = l.plain.clone();
    let mut with_shift = l.shift.clone();
    if command || always_cmd_layout {
        if l.cmd_shift != l.cmd {
            with_shift = l.cmd_shift.clone();
        } else if l.cmd.to_ascii_uppercase() != l.cmd {
            with_shift = l.cmd.to_ascii_uppercase();
        }
        ignoring = l.cmd.clone();
    }
    if shift && !ignoring.is_empty() && ignoring.chars().all(|c| c.is_ascii_lowercase()) {
        ignoring
    } else if shift {
        with_shift
    } else {
        ignoring
    }
}

/// Whether the layout types non-ASCII on the A key but ASCII with Cmd (Russian, Armenian): such
/// layouts name keys from their Cmd layer even without Cmd.
pub fn always_cmd_layout(a_plain: &str, a_cmd: &str) -> bool {
    !a_plain.is_ascii() && a_cmd.is_ascii()
}

/// The egui key for a press: layout-independent keys by code, character keys by the characters
/// [`key_chars`] chose, falling back to the key's US QWERTY name.
pub fn key_for(code: u16, chars: &str) -> Option<Key> {
    if let Some(k) = named_key(code) {
        return Some(k);
    }
    let mut it = chars.chars();
    if let (Some(c), None) = (it.next(), it.next()) {
        let mut buf = [0u8; 4];
        if let Some(k) = Key::from_name(c.encode_utf8(&mut buf)) {
            return Some(k);
        }
    }
    qwerty_key(code)
}

/// The text a key press types, from `NSEvent.characters`: `None` for control characters and AppKit's
/// private-use function-key characters (U+F700–U+F8FF), and when Cmd or Ctrl is down.
pub fn typed_text(characters: &str, raw: u64) -> Option<String> {
    if raw & (flags::COMMAND | flags::CONTROL) != 0 || characters.is_empty() {
        return None;
    }
    let printable = characters
        .chars()
        .all(|c| !c.is_control() && !('\u{F700}'..='\u{F8FF}').contains(&c));
    printable.then(|| characters.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chars(plain: &str, shift: &str, cmd: &str, cmd_shift: &str) -> LayoutChars {
        LayoutChars {
            plain: plain.into(),
            shift: shift.into(),
            cmd: cmd.into(),
            cmd_shift: cmd_shift.into(),
        }
    }

    #[test]
    fn named_keys_are_layout_independent() {
        assert_eq!(named_key(0x24), Some(Key::Enter));
        assert_eq!(named_key(vk::KEYPAD_ENTER), Some(Key::Enter));
        assert_eq!(named_key(0x7B), Some(Key::ArrowLeft));
        assert_eq!(named_key(0x75), Some(Key::Delete));
        assert_eq!(named_key(0x33), Some(Key::Backspace));
        assert_eq!(named_key(0x72), Some(Key::Insert));
        assert_eq!(named_key(0x6F), Some(Key::F12));
        assert_eq!(named_key(0x5A), Some(Key::F20));
        assert_eq!(named_key(0x52), Some(Key::Num0));
        assert_eq!(named_key(0x00), None, "A is a character key");
    }

    #[test]
    fn every_named_key_has_an_egui_name_that_round_trips() {
        for code in 0u16..0x80 {
            for k in [named_key(code), qwerty_key(code)].into_iter().flatten() {
                assert_eq!(Key::from_name(k.name()), Some(k), "code {code:#x}");
            }
        }
    }

    #[test]
    fn physical_codes_are_hid_usages() {
        assert_eq!(physical_code(0x00), 0x0007_0004);
        assert_eq!(physical_code(0x06), 0x0007_001D);
        assert_eq!(physical_code(0x24), 0x0007_0028);
        assert_eq!(physical_code(0x37), 0x0007_00E3);
        assert_eq!(physical_code(0x36), 0x0007_00E7);
        assert_eq!(physical_code(0x3F), 0, "Fn has no keyboard-page usage");
        // Every key code with a name has a usage.
        for code in 0u16..0x80 {
            if named_key(code).is_some_and(|k| k != Key::Fn) || qwerty_key(code).is_some() {
                assert_ne!(physical_code(code), 0, "code {code:#x}");
            }
        }
    }

    #[test]
    fn locations() {
        assert_eq!(key_location(vk::SHIFT), KeyLocation::Left);
        assert_eq!(key_location(vk::RIGHT_OPTION), KeyLocation::Right);
        assert_eq!(key_location(0x53), KeyLocation::Numpad);
        assert_eq!(key_location(vk::KEYPAD_ENTER), KeyLocation::Numpad);
        assert_eq!(key_location(vk::RETURN), KeyLocation::Standard);
    }

    #[test]
    fn modifiers() {
        let m = modifiers_from_flags(flags::SHIFT | flags::COMMAND | flags::LEFT_COMMAND);
        assert!(m.contains(Modifiers::SHIFT) && m.contains(Modifiers::SUPER));
        assert!(!m.contains(Modifiers::CTRL) && !m.contains(Modifiers::ALT));
        assert!(m.primary(true));
        assert_eq!(
            modifiers_from_flags(flags::CAPS_LOCK | flags::NUMERIC_PAD),
            Modifiers::default()
        );
    }

    #[test]
    fn modifier_transitions_use_the_device_side() {
        // Left shift down.
        assert_eq!(
            modifier_transition(vk::SHIFT, flags::SHIFT | flags::LEFT_SHIFT),
            Some((Key::ShiftLeft, Transition::Pressed))
        );
        // Right shift goes down while left is held: the event names the right key.
        assert_eq!(
            modifier_transition(
                vk::RIGHT_SHIFT,
                flags::SHIFT | flags::LEFT_SHIFT | flags::RIGHT_SHIFT
            ),
            Some((Key::ShiftRight, Transition::Pressed))
        );
        // Left released while right is still held: generic Shift stays set, the left bit clears.
        assert_eq!(
            modifier_transition(vk::SHIFT, flags::SHIFT | flags::RIGHT_SHIFT),
            Some((Key::ShiftLeft, Transition::Released))
        );
        assert_eq!(
            modifier_transition(vk::RIGHT_COMMAND, 0),
            Some((Key::SuperRight, Transition::Released))
        );
        assert_eq!(
            modifier_transition(vk::RIGHT_CONTROL, flags::CONTROL | flags::RIGHT_CONTROL),
            Some((Key::ControlRight, Transition::Pressed))
        );
        // A synthesised event without device bits.
        assert_eq!(
            modifier_transition(vk::OPTION, flags::OPTION),
            Some((Key::AltLeft, Transition::Pressed))
        );
        assert_eq!(
            modifier_transition(vk::CAPS_LOCK, flags::CAPS_LOCK),
            Some((Key::CapsLock, Transition::Toggled))
        );
        assert_eq!(
            Transition::Toggled.states(),
            &[KeyState::Pressed, KeyState::Released]
        );
        assert_eq!(
            modifier_transition(vk::FUNCTION, flags::FUNCTION),
            Some((Key::Fn, Transition::Pressed))
        );
        assert_eq!(modifier_transition(vk::ANSI_A, flags::SHIFT), None);
    }

    #[test]
    fn option_as_alt_policy() {
        let left = flags::OPTION | flags::LEFT_OPTION;
        let right = flags::OPTION | flags::RIGHT_OPTION;
        assert!(!option_is_alt(OptionAsAlt::None, left));
        assert!(option_is_alt(OptionAsAlt::Both, right));
        assert!(option_is_alt(OptionAsAlt::OnlyLeft, left));
        assert!(!option_is_alt(OptionAsAlt::OnlyLeft, right));
        assert!(option_is_alt(OptionAsAlt::OnlyRight, right));
        assert!(!option_is_alt(OptionAsAlt::OnlyRight, left));
        assert!(!option_is_alt(OptionAsAlt::Both, flags::SHIFT));
        assert!(
            !option_is_alt(OptionAsAlt::OnlyLeft, flags::OPTION),
            "side unknown"
        );
    }

    #[test]
    fn key_chars_cases() {
        // US: Shift+A stays the letter; Shift+1 names "!".
        let a = chars("a", "A", "a", "A");
        assert_eq!(key_chars(&a, false, true, false), "a");
        let one = chars("1", "!", "1", "!");
        assert_eq!(key_chars(&one, false, true, false), "!");
        assert_eq!(key_chars(&one, true, false, false), "1");
        // Armenian: non-ASCII plain layer, ASCII Cmd layer.
        assert!(always_cmd_layout("ա", "a"));
        let arm = chars("ս", "Ս", "s", "S");
        assert_eq!(key_chars(&arm, false, false, true), "s");
        assert_eq!(key_chars(&arm, true, true, true), "s");
        // Dvorak-QWERTY: o without Cmd, s with Cmd.
        let dq = chars("o", "O", "s", "S");
        assert_eq!(key_chars(&dq, false, false, false), "o");
        assert_eq!(key_chars(&dq, true, false, false), "s");
        // Ukrainian-QWERTY: Cmd-Shift-S reports cmd-s; the shifted name is the uppercase letter, and
        // Shift on a letter keeps the letter.
        let ua = chars("с", "С", "s", "s");
        assert_eq!(key_chars(&ua, true, true, false), "s");
        // Czech: 7 types ý; Cmd-Shift-7 is cmd-7.
        let cz = chars("ý", "7", "ý", "7");
        assert_eq!(key_chars(&cz, true, false, false), "ý");
        assert_eq!(key_chars(&cz, true, true, false), "7");
        // Norwegian: Shift+Cmd types the same as Cmd, so the shifted name stays the Shift layer's "/".
        let no = chars("7", "/", "7", "7");
        assert_eq!(key_chars(&no, true, true, false), "/");
        // Russian: Cmd-Shift-7 is cmd-& (the Cmd layer's shifted character).
        let ru = chars("7", ".", "7", "&");
        assert_eq!(key_chars(&ru, true, true, true), "&");
        // German: Shift on a non-ASCII letter names the shifted character.
        let de = chars("ö", "Ö", "ö", "Ö");
        assert_eq!(key_chars(&de, true, true, false), "Ö");
    }

    #[test]
    fn keys_from_codes_and_characters() {
        assert_eq!(key_for(0x00, "a"), Some(Key::A));
        assert_eq!(
            key_for(0x01, "o"),
            Some(Key::O),
            "Dvorak: the layout names the key"
        );
        assert_eq!(key_for(0x12, "!"), Some(Key::Exclamationmark));
        assert_eq!(key_for(0x18, "+"), Some(Key::Plus));
        assert_eq!(
            key_for(0x08, "с"),
            Some(Key::C),
            "Cyrillic falls back to the QWERTY position"
        );
        assert_eq!(key_for(0x29, "ö"), Some(Key::Semicolon));
        assert_eq!(key_for(0x7E, ""), Some(Key::ArrowUp));
        assert_eq!(key_for(0x24, "\r"), Some(Key::Enter));
        assert_eq!(key_for(0x5D, "¥"), None);
    }

    #[test]
    fn typed_text_filters() {
        assert_eq!(typed_text("a", 0), Some("a".into()));
        assert_eq!(typed_text("å", flags::OPTION), Some("å".into()));
        assert_eq!(typed_text("a", flags::COMMAND), None);
        assert_eq!(typed_text("\u{1}", flags::CONTROL), None);
        assert_eq!(typed_text("\r", 0), None);
        assert_eq!(typed_text("\u{F700}", flags::FUNCTION), None, "arrow keys");
        assert_eq!(typed_text("", 0), None, "dead keys type nothing");
    }
}
