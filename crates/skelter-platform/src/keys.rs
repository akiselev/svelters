//! winit keys → skelter keys. The logical key gets egui's name ([`skelter_base::key::Key`]); the
//! physical key becomes its code ([`physical_code`]).
//!
//! **Physical key codes** (`KeyEvent.physical`), the same in every backend (winit here, and
//! skelter-platform-win32, -macos and -wayland):
//! - a key with a USB HID usage: `page << 16 | usage` (HID Usage Tables 1.4; keyboard page 0x07, so A
//!   is `0x0007_0004`, consumer page 0x0C, generic desktop 0x01), Chromium's form;
//! - a key without one: `0xFFFF_0000 | native`, the OS's own code in the low 16 bits (Windows set-1
//!   scancode with `0xE0xx` for extended keys, macOS `kVK_*`, Linux evdev, winit's native code);
//! - 0 only when the OS named no key at all.
//!
//! `key_from_named` and `key_from_code` follow egui-winit 0.37.0 `src/lib.rs` (`key_from_named_key`,
//! `key_from_key_code`; MIT OR Apache-2.0, Copyright (c) 2018-2021 Emil Ernerfeldt), so keys reach
//! cadatomic's keymaps exactly as egui delivered them.

use skelter_base::event::KeyLocation;
use skelter_base::key::Key;
use winit::keyboard::{Key as WKey, KeyCode, KeyLocation as WLocation, NamedKey, PhysicalKey};

/// The key to deliver: the logical key, else the physical key's US-layout key, so layouts without
/// Latin letters still reach Ctrl+C (egui-winit's rule, egui #3653).
pub fn key_from_winit(logical: &WKey, physical: PhysicalKey) -> Option<Key> {
    let code = match physical {
        PhysicalKey::Code(c) => key_from_code(c),
        PhysicalKey::Unidentified(_) => None,
    };
    key_from_logical(logical).or(code)
}

pub fn key_from_logical(key: &WKey) -> Option<Key> {
    match key {
        WKey::Named(n) => key_from_named(*n),
        WKey::Character(s) => Key::from_name(s.as_str()),
        WKey::Unidentified(_) | WKey::Dead(_) => None,
    }
}

pub fn location(l: WLocation) -> KeyLocation {
    match l {
        WLocation::Standard => KeyLocation::Standard,
        WLocation::Left => KeyLocation::Left,
        WLocation::Right => KeyLocation::Right,
        WLocation::Numpad => KeyLocation::Numpad,
    }
}

pub fn key_from_named(named: NamedKey) -> Option<Key> {
    Some(match named {
        NamedKey::Enter => Key::Enter,
        NamedKey::Tab => Key::Tab,
        NamedKey::ArrowDown => Key::ArrowDown,
        NamedKey::ArrowLeft => Key::ArrowLeft,
        NamedKey::ArrowRight => Key::ArrowRight,
        NamedKey::ArrowUp => Key::ArrowUp,
        NamedKey::End => Key::End,
        NamedKey::Home => Key::Home,
        NamedKey::PageDown => Key::PageDown,
        NamedKey::PageUp => Key::PageUp,
        NamedKey::Backspace => Key::Backspace,
        NamedKey::Delete => Key::Delete,
        NamedKey::Insert => Key::Insert,
        NamedKey::Escape => Key::Escape,
        NamedKey::Cut => Key::Cut,
        NamedKey::Copy => Key::Copy,
        NamedKey::Paste => Key::Paste,
        NamedKey::Space => Key::Space,
        NamedKey::F1 => Key::F1,
        NamedKey::F2 => Key::F2,
        NamedKey::F3 => Key::F3,
        NamedKey::F4 => Key::F4,
        NamedKey::F5 => Key::F5,
        NamedKey::F6 => Key::F6,
        NamedKey::F7 => Key::F7,
        NamedKey::F8 => Key::F8,
        NamedKey::F9 => Key::F9,
        NamedKey::F10 => Key::F10,
        NamedKey::F11 => Key::F11,
        NamedKey::F12 => Key::F12,
        NamedKey::F13 => Key::F13,
        NamedKey::F14 => Key::F14,
        NamedKey::F15 => Key::F15,
        NamedKey::F16 => Key::F16,
        NamedKey::F17 => Key::F17,
        NamedKey::F18 => Key::F18,
        NamedKey::F19 => Key::F19,
        NamedKey::F20 => Key::F20,
        NamedKey::F21 => Key::F21,
        NamedKey::F22 => Key::F22,
        NamedKey::F23 => Key::F23,
        NamedKey::F24 => Key::F24,
        NamedKey::F25 => Key::F25,
        NamedKey::F26 => Key::F26,
        NamedKey::F27 => Key::F27,
        NamedKey::F28 => Key::F28,
        NamedKey::F29 => Key::F29,
        NamedKey::F30 => Key::F30,
        NamedKey::F31 => Key::F31,
        NamedKey::F32 => Key::F32,
        NamedKey::F33 => Key::F33,
        NamedKey::F34 => Key::F34,
        NamedKey::F35 => Key::F35,
        NamedKey::BrowserBack => Key::BrowserBack,
        NamedKey::BrowserForward => Key::BrowserForward,
        NamedKey::BrowserRefresh => Key::BrowserRefresh,
        NamedKey::BrowserSearch => Key::BrowserSearch,
        NamedKey::BrowserHome => Key::BrowserHome,
        NamedKey::BrowserFavorites => Key::BrowserFavorites,
        NamedKey::BrowserStop => Key::BrowserStop,
        NamedKey::MediaPlayPause => Key::MediaPlayPause,
        NamedKey::MediaTrackNext => Key::MediaTrackNext,
        NamedKey::MediaTrackPrevious => Key::MediaTrackPrevious,
        NamedKey::MediaStop => Key::MediaStop,
        NamedKey::AudioVolumeMute => Key::AudioVolumeMute,
        NamedKey::AudioVolumeDown => Key::AudioVolumeDown,
        NamedKey::AudioVolumeUp => Key::AudioVolumeUp,
        NamedKey::LaunchMail => Key::LaunchMail,
        NamedKey::LaunchApplication1 => Key::LaunchApp1,
        NamedKey::LaunchApplication2 => Key::LaunchApp2,
        NamedKey::CapsLock => Key::CapsLock,
        NamedKey::NumLock => Key::NumLock,
        NamedKey::ScrollLock => Key::ScrollLock,
        NamedKey::PrintScreen => Key::PrintScreen,
        NamedKey::Pause => Key::Pause,
        NamedKey::ContextMenu => Key::Menu,
        NamedKey::Fn => Key::Fn,
        NamedKey::Eject => Key::Eject,
        NamedKey::Help => Key::Help,
        NamedKey::Power => Key::Power,
        NamedKey::Standby => Key::Sleep,
        NamedKey::Clear => Key::Clear,
        _ => return None,
    })
}

pub fn key_from_code(code: KeyCode) -> Option<Key> {
    Some(match code {
        KeyCode::ArrowDown => Key::ArrowDown,
        KeyCode::ArrowLeft => Key::ArrowLeft,
        KeyCode::ArrowRight => Key::ArrowRight,
        KeyCode::ArrowUp => Key::ArrowUp,
        KeyCode::Escape => Key::Escape,
        KeyCode::Tab => Key::Tab,
        KeyCode::Backspace => Key::Backspace,
        KeyCode::Enter | KeyCode::NumpadEnter => Key::Enter,
        KeyCode::Insert => Key::Insert,
        KeyCode::Delete => Key::Delete,
        KeyCode::Home => Key::Home,
        KeyCode::End => Key::End,
        KeyCode::PageUp => Key::PageUp,
        KeyCode::PageDown => Key::PageDown,
        KeyCode::Space => Key::Space,
        KeyCode::Comma => Key::Comma,
        KeyCode::Period => Key::Period,
        KeyCode::Semicolon => Key::Semicolon,
        KeyCode::Backslash => Key::Backslash,
        KeyCode::Slash | KeyCode::NumpadDivide => Key::Slash,
        KeyCode::BracketLeft => Key::OpenBracket,
        KeyCode::BracketRight => Key::CloseBracket,
        KeyCode::Backquote => Key::Backtick,
        KeyCode::Quote => Key::Quote,
        KeyCode::Cut => Key::Cut,
        KeyCode::Copy => Key::Copy,
        KeyCode::Paste => Key::Paste,
        KeyCode::Minus | KeyCode::NumpadSubtract => Key::Minus,
        KeyCode::NumpadAdd => Key::Plus,
        KeyCode::Equal => Key::Equals,
        KeyCode::Digit0 | KeyCode::Numpad0 => Key::Num0,
        KeyCode::Digit1 | KeyCode::Numpad1 => Key::Num1,
        KeyCode::Digit2 | KeyCode::Numpad2 => Key::Num2,
        KeyCode::Digit3 | KeyCode::Numpad3 => Key::Num3,
        KeyCode::Digit4 | KeyCode::Numpad4 => Key::Num4,
        KeyCode::Digit5 | KeyCode::Numpad5 => Key::Num5,
        KeyCode::Digit6 | KeyCode::Numpad6 => Key::Num6,
        KeyCode::Digit7 | KeyCode::Numpad7 => Key::Num7,
        KeyCode::Digit8 | KeyCode::Numpad8 => Key::Num8,
        KeyCode::Digit9 | KeyCode::Numpad9 => Key::Num9,
        KeyCode::KeyA => Key::A,
        KeyCode::KeyB => Key::B,
        KeyCode::KeyC => Key::C,
        KeyCode::KeyD => Key::D,
        KeyCode::KeyE => Key::E,
        KeyCode::KeyF => Key::F,
        KeyCode::KeyG => Key::G,
        KeyCode::KeyH => Key::H,
        KeyCode::KeyI => Key::I,
        KeyCode::KeyJ => Key::J,
        KeyCode::KeyK => Key::K,
        KeyCode::KeyL => Key::L,
        KeyCode::KeyM => Key::M,
        KeyCode::KeyN => Key::N,
        KeyCode::KeyO => Key::O,
        KeyCode::KeyP => Key::P,
        KeyCode::KeyQ => Key::Q,
        KeyCode::KeyR => Key::R,
        KeyCode::KeyS => Key::S,
        KeyCode::KeyT => Key::T,
        KeyCode::KeyU => Key::U,
        KeyCode::KeyV => Key::V,
        KeyCode::KeyW => Key::W,
        KeyCode::KeyX => Key::X,
        KeyCode::KeyY => Key::Y,
        KeyCode::KeyZ => Key::Z,
        KeyCode::F1 => Key::F1,
        KeyCode::F2 => Key::F2,
        KeyCode::F3 => Key::F3,
        KeyCode::F4 => Key::F4,
        KeyCode::F5 => Key::F5,
        KeyCode::F6 => Key::F6,
        KeyCode::F7 => Key::F7,
        KeyCode::F8 => Key::F8,
        KeyCode::F9 => Key::F9,
        KeyCode::F10 => Key::F10,
        KeyCode::F11 => Key::F11,
        KeyCode::F12 => Key::F12,
        KeyCode::F13 => Key::F13,
        KeyCode::F14 => Key::F14,
        KeyCode::F15 => Key::F15,
        KeyCode::F16 => Key::F16,
        KeyCode::F17 => Key::F17,
        KeyCode::F18 => Key::F18,
        KeyCode::F19 => Key::F19,
        KeyCode::F20 => Key::F20,
        KeyCode::F21 => Key::F21,
        KeyCode::F22 => Key::F22,
        KeyCode::F23 => Key::F23,
        KeyCode::F24 => Key::F24,
        KeyCode::F25 => Key::F25,
        KeyCode::F26 => Key::F26,
        KeyCode::F27 => Key::F27,
        KeyCode::F28 => Key::F28,
        KeyCode::F29 => Key::F29,
        KeyCode::F30 => Key::F30,
        KeyCode::F31 => Key::F31,
        KeyCode::F32 => Key::F32,
        KeyCode::F33 => Key::F33,
        KeyCode::F34 => Key::F34,
        KeyCode::F35 => Key::F35,
        KeyCode::ShiftLeft => Key::ShiftLeft,
        KeyCode::ShiftRight => Key::ShiftRight,
        KeyCode::ControlLeft => Key::ControlLeft,
        KeyCode::ControlRight => Key::ControlRight,
        KeyCode::AltLeft => Key::AltLeft,
        KeyCode::AltRight => Key::AltRight,
        KeyCode::SuperLeft => Key::SuperLeft,
        KeyCode::SuperRight => Key::SuperRight,
        KeyCode::IntlBackslash => Key::IntlBackslash,
        KeyCode::CapsLock => Key::CapsLock,
        KeyCode::NumLock => Key::NumLock,
        KeyCode::ScrollLock => Key::ScrollLock,
        KeyCode::PrintScreen => Key::PrintScreen,
        KeyCode::Pause => Key::Pause,
        KeyCode::ContextMenu => Key::Menu,
        KeyCode::BrowserBack => Key::BrowserBack,
        KeyCode::BrowserForward => Key::BrowserForward,
        KeyCode::BrowserRefresh => Key::BrowserRefresh,
        KeyCode::BrowserSearch => Key::BrowserSearch,
        KeyCode::BrowserHome => Key::BrowserHome,
        KeyCode::BrowserFavorites => Key::BrowserFavorites,
        KeyCode::BrowserStop => Key::BrowserStop,
        KeyCode::MediaPlayPause => Key::MediaPlayPause,
        KeyCode::MediaTrackNext => Key::MediaTrackNext,
        KeyCode::MediaTrackPrevious => Key::MediaTrackPrevious,
        KeyCode::MediaStop => Key::MediaStop,
        KeyCode::AudioVolumeMute => Key::AudioVolumeMute,
        KeyCode::AudioVolumeDown => Key::AudioVolumeDown,
        KeyCode::AudioVolumeUp => Key::AudioVolumeUp,
        KeyCode::LaunchMail => Key::LaunchMail,
        KeyCode::LaunchApp1 => Key::LaunchApp1,
        KeyCode::LaunchApp2 => Key::LaunchApp2,
        KeyCode::Fn => Key::Fn,
        KeyCode::Eject => Key::Eject,
        KeyCode::Help => Key::Help,
        KeyCode::Power => Key::Power,
        KeyCode::Sleep => Key::Sleep,
        _ => return None,
    })
}

const KEYBOARD: u32 = 0x07 << 16;
const CONSUMER: u32 = 0x0C << 16;
const DESKTOP: u32 = 0x01 << 16;

/// `KeyEvent.physical` for a winit key (see the module docs): its HID usage, else `0xFFFF_0000 |`
/// the native code winit reports for an unidentified key, else 0.
pub fn physical_code(physical: PhysicalKey) -> u32 {
    use winit::keyboard::NativeKeyCode as N;
    match (hid_usage(physical), physical) {
        (0, PhysicalKey::Unidentified(native)) => match native {
            N::Windows(c) | N::MacOS(c) => 0xFFFF_0000 | c as u32,
            N::Xkb(c) | N::Android(c) => 0xFFFF_0000 | (c & 0xFFFF),
            N::Unidentified => 0,
        },
        (hid, _) => hid,
    }
}

/// The USB HID usage of a physical key (HID Usage Tables 1.4: §10 Keyboard/Keypad, §15 Consumer,
/// §4 Generic Desktop), `page << 16 | usage`; 0 when the key has none.
pub fn hid_usage(physical: PhysicalKey) -> u32 {
    let PhysicalKey::Code(code) = physical else { return 0 };
    use KeyCode as K;
    let kb = |u: u32| KEYBOARD | u;
    match code {
        K::KeyA => kb(0x04),
        K::KeyB => kb(0x05),
        K::KeyC => kb(0x06),
        K::KeyD => kb(0x07),
        K::KeyE => kb(0x08),
        K::KeyF => kb(0x09),
        K::KeyG => kb(0x0A),
        K::KeyH => kb(0x0B),
        K::KeyI => kb(0x0C),
        K::KeyJ => kb(0x0D),
        K::KeyK => kb(0x0E),
        K::KeyL => kb(0x0F),
        K::KeyM => kb(0x10),
        K::KeyN => kb(0x11),
        K::KeyO => kb(0x12),
        K::KeyP => kb(0x13),
        K::KeyQ => kb(0x14),
        K::KeyR => kb(0x15),
        K::KeyS => kb(0x16),
        K::KeyT => kb(0x17),
        K::KeyU => kb(0x18),
        K::KeyV => kb(0x19),
        K::KeyW => kb(0x1A),
        K::KeyX => kb(0x1B),
        K::KeyY => kb(0x1C),
        K::KeyZ => kb(0x1D),
        K::Digit1 => kb(0x1E),
        K::Digit2 => kb(0x1F),
        K::Digit3 => kb(0x20),
        K::Digit4 => kb(0x21),
        K::Digit5 => kb(0x22),
        K::Digit6 => kb(0x23),
        K::Digit7 => kb(0x24),
        K::Digit8 => kb(0x25),
        K::Digit9 => kb(0x26),
        K::Digit0 => kb(0x27),
        K::Enter => kb(0x28),
        K::Escape => kb(0x29),
        K::Backspace => kb(0x2A),
        K::Tab => kb(0x2B),
        K::Space => kb(0x2C),
        K::Minus => kb(0x2D),
        K::Equal => kb(0x2E),
        K::BracketLeft => kb(0x2F),
        K::BracketRight => kb(0x30),
        K::Backslash => kb(0x31),
        K::Semicolon => kb(0x33),
        K::Quote => kb(0x34),
        K::Backquote => kb(0x35),
        K::Comma => kb(0x36),
        K::Period => kb(0x37),
        K::Slash => kb(0x38),
        K::CapsLock => kb(0x39),
        K::F1 => kb(0x3A),
        K::F2 => kb(0x3B),
        K::F3 => kb(0x3C),
        K::F4 => kb(0x3D),
        K::F5 => kb(0x3E),
        K::F6 => kb(0x3F),
        K::F7 => kb(0x40),
        K::F8 => kb(0x41),
        K::F9 => kb(0x42),
        K::F10 => kb(0x43),
        K::F11 => kb(0x44),
        K::F12 => kb(0x45),
        K::PrintScreen => kb(0x46),
        K::ScrollLock => kb(0x47),
        K::Pause => kb(0x48),
        K::Insert => kb(0x49),
        K::Home => kb(0x4A),
        K::PageUp => kb(0x4B),
        K::Delete => kb(0x4C),
        K::End => kb(0x4D),
        K::PageDown => kb(0x4E),
        K::ArrowRight => kb(0x4F),
        K::ArrowLeft => kb(0x50),
        K::ArrowDown => kb(0x51),
        K::ArrowUp => kb(0x52),
        K::NumLock => kb(0x53),
        K::NumpadDivide => kb(0x54),
        K::NumpadMultiply => kb(0x55),
        K::NumpadSubtract => kb(0x56),
        K::NumpadAdd => kb(0x57),
        K::NumpadEnter => kb(0x58),
        K::Numpad1 => kb(0x59),
        K::Numpad2 => kb(0x5A),
        K::Numpad3 => kb(0x5B),
        K::Numpad4 => kb(0x5C),
        K::Numpad5 => kb(0x5D),
        K::Numpad6 => kb(0x5E),
        K::Numpad7 => kb(0x5F),
        K::Numpad8 => kb(0x60),
        K::Numpad9 => kb(0x61),
        K::Numpad0 => kb(0x62),
        K::NumpadDecimal => kb(0x63),
        K::IntlBackslash => kb(0x64),
        K::ContextMenu => kb(0x65),
        K::Power => kb(0x66),
        K::NumpadEqual => kb(0x67),
        K::F13 => kb(0x68),
        K::F14 => kb(0x69),
        K::F15 => kb(0x6A),
        K::F16 => kb(0x6B),
        K::F17 => kb(0x6C),
        K::F18 => kb(0x6D),
        K::F19 => kb(0x6E),
        K::F20 => kb(0x6F),
        K::F21 => kb(0x70),
        K::F22 => kb(0x71),
        K::F23 => kb(0x72),
        K::F24 => kb(0x73),
        K::Help => kb(0x75),
        K::Select => kb(0x77),
        K::Again => kb(0x79),
        K::Undo => kb(0x7A),
        K::Cut => kb(0x7B),
        K::Copy => kb(0x7C),
        K::Paste => kb(0x7D),
        K::Find => kb(0x7E),
        K::AudioVolumeMute => kb(0x7F),
        K::AudioVolumeUp => kb(0x80),
        K::AudioVolumeDown => kb(0x81),
        K::NumpadComma => kb(0x85),
        K::IntlRo => kb(0x87),
        K::KanaMode => kb(0x88),
        K::IntlYen => kb(0x89),
        K::Convert => kb(0x8A),
        K::NonConvert => kb(0x8B),
        K::Lang1 => kb(0x90),
        K::Lang2 => kb(0x91),
        K::Lang3 | K::Katakana => kb(0x92),
        K::Lang4 | K::Hiragana => kb(0x93),
        K::Lang5 => kb(0x94),
        K::NumpadParenLeft => kb(0xB6),
        K::NumpadParenRight => kb(0xB7),
        K::NumpadBackspace => kb(0xBB),
        K::NumpadHash => kb(0xCC),
        K::NumpadMemoryStore => kb(0xD0),
        K::NumpadMemoryRecall => kb(0xD1),
        K::NumpadMemoryClear => kb(0xD2),
        K::NumpadMemoryAdd => kb(0xD3),
        K::NumpadMemorySubtract => kb(0xD4),
        K::NumpadClear => kb(0xD8),
        K::NumpadClearEntry => kb(0xD9),
        K::ControlLeft => kb(0xE0),
        K::ShiftLeft => kb(0xE1),
        K::AltLeft => kb(0xE2),
        K::SuperLeft => kb(0xE3),
        K::ControlRight => kb(0xE4),
        K::ShiftRight => kb(0xE5),
        K::AltRight => kb(0xE6),
        K::SuperRight => kb(0xE7),
        K::MediaTrackNext => CONSUMER | 0xB5,
        K::MediaTrackPrevious => CONSUMER | 0xB6,
        K::MediaStop => CONSUMER | 0xB7,
        K::Eject => CONSUMER | 0xB8,
        K::MediaPlayPause => CONSUMER | 0xCD,
        K::MediaSelect => CONSUMER | 0x183,
        K::LaunchMail => CONSUMER | 0x18A,
        K::LaunchApp2 => CONSUMER | 0x192,
        K::LaunchApp1 => CONSUMER | 0x194,
        K::BrowserSearch => CONSUMER | 0x221,
        K::BrowserHome => CONSUMER | 0x223,
        K::BrowserBack => CONSUMER | 0x224,
        K::BrowserForward => CONSUMER | 0x225,
        K::BrowserStop => CONSUMER | 0x226,
        K::BrowserRefresh => CONSUMER | 0x227,
        K::BrowserFavorites => CONSUMER | 0x22A,
        K::Sleep => DESKTOP | 0x82,
        K::WakeUp => DESKTOP | 0x83,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use winit::keyboard::SmolStr;

    /// Every key skelter can deliver keeps its egui name through `name` → `from_name`, so keymaps
    /// written as egui names keep matching.
    #[test]
    fn delivered_keys_round_trip_through_egui_names() {
        let codes = [
            KeyCode::KeyA, KeyCode::KeyZ, KeyCode::Digit0, KeyCode::Digit9, KeyCode::F1, KeyCode::F35,
            KeyCode::ArrowUp, KeyCode::Enter, KeyCode::NumpadEnter, KeyCode::Backquote, KeyCode::Minus,
            KeyCode::Equal, KeyCode::BracketLeft, KeyCode::ShiftLeft, KeyCode::SuperRight,
            KeyCode::IntlBackslash, KeyCode::ContextMenu, KeyCode::MediaPlayPause, KeyCode::Sleep,
        ];
        for c in codes {
            let k = key_from_code(c).unwrap_or_else(|| panic!("{c:?} has no key"));
            assert_eq!(Key::from_name(k.name()), Some(k), "{c:?} → {k:?} → {:?}", k.name());
        }
        for n in [NamedKey::Escape, NamedKey::Tab, NamedKey::Space, NamedKey::PageDown, NamedKey::Standby] {
            let k = key_from_named(n).unwrap();
            assert_eq!(Key::from_name(k.name()), Some(k));
        }
    }

    #[test]
    fn logical_wins_and_physical_backs_up_non_latin_layouts() {
        // Dvorak: the physical KeyB position types "x".
        let k = key_from_winit(&WKey::Character(SmolStr::new("x")), PhysicalKey::Code(KeyCode::KeyB));
        assert_eq!(k, Some(Key::X));
        // Russian: "с" on the physical C key still reaches Key::C (Ctrl+C).
        let k = key_from_winit(&WKey::Character(SmolStr::new("с")), PhysicalKey::Code(KeyCode::KeyC));
        assert_eq!(k, Some(Key::C));
        let k = key_from_winit(&WKey::Named(NamedKey::Enter), PhysicalKey::Code(KeyCode::NumpadEnter));
        assert_eq!(k, Some(Key::Enter));
        assert_eq!(key_from_winit(&WKey::Dead(Some('´')), PhysicalKey::Code(KeyCode::Equal)), Some(Key::Equals));
    }

    #[test]
    fn physical_codes_fall_back_to_the_native_code() {
        use winit::keyboard::NativeKeyCode;
        assert_eq!(physical_code(PhysicalKey::Code(KeyCode::KeyA)), 0x0007_0004);
        assert_eq!(physical_code(PhysicalKey::Unidentified(NativeKeyCode::Windows(0xE022))), 0xFFFF_E022);
        assert_eq!(physical_code(PhysicalKey::Unidentified(NativeKeyCode::Xkb(464))), 0xFFFF_0000 | 464);
        assert_eq!(physical_code(PhysicalKey::Unidentified(NativeKeyCode::MacOS(0x3F))), 0xFFFF_003F);
        assert_eq!(physical_code(PhysicalKey::Unidentified(NativeKeyCode::Unidentified)), 0);
    }

    #[test]
    fn hid_usages_are_distinct_and_on_the_keyboard_page() {
        assert_eq!(hid_usage(PhysicalKey::Code(KeyCode::KeyA)), 0x0007_0004);
        assert_eq!(hid_usage(PhysicalKey::Code(KeyCode::Enter)), 0x0007_0028);
        assert_eq!(hid_usage(PhysicalKey::Code(KeyCode::SuperRight)), 0x0007_00E7);
        let mut seen: HashMap<u32, KeyCode> = HashMap::new();
        let all = [
            KeyCode::KeyA, KeyCode::KeyM, KeyCode::Digit1, KeyCode::Digit0, KeyCode::F12, KeyCode::F13,
            KeyCode::Numpad0, KeyCode::NumpadEnter, KeyCode::Enter, KeyCode::ControlLeft, KeyCode::ControlRight,
            KeyCode::BrowserBack, KeyCode::MediaStop, KeyCode::Sleep, KeyCode::AudioVolumeUp,
        ];
        for c in all {
            let u = hid_usage(PhysicalKey::Code(c));
            assert_ne!(u, 0, "{c:?}");
            if let Some(prev) = seen.insert(u, c) {
                panic!("{c:?} and {prev:?} share usage {u:#x}");
            }
        }
    }
}
