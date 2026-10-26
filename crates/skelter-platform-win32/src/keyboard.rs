//! Keyboard: `WM_KEYDOWN`/`WM_KEYUP` (and the `SYS` variants) to [`KeyEvent`]s.
//!
//! - **Logical keys** carry egui's names ([`Key`]): named keys come from the virtual key, printable
//!   keys from the active layout's unshifted character (`ToUnicodeEx` with flag 4, which leaves the
//!   dead-key state alone), so `Ctrl+Z` on AZERTY is `Z` wherever the layout puts it.
//! - **Physical keys** are USB HID usages (`0x07_00XX`, page 7) from the set-1 scancode.
//! - **Text** is what the OS's own translation produced: `TranslateMessage` posts `WM_CHAR`s after
//!   the key-down, which [`KeyTranslator::key`] collects from the queue. Dead keys produce
//!   `WM_DEADCHAR` (no text) and the next key's `WM_CHAR` carries the composed character.
//! - **AltGr** is kept apart from Ctrl+Alt: layouts with AltGr make Windows send a synthetic left
//!   Ctrl down (same message time, scancode 0x1D) right before the right Alt; the translator drops it
//!   and reports [`Modifiers::ALT_GR`] while that right Alt is down (winit #2945).

use skelter_base::event::{KeyEvent, KeyLocation, KeyState, Modifiers};
use skelter_base::key::Key;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::{
    MSG, PM_NOREMOVE, PM_REMOVE, PeekMessageW, WM_CHAR, WM_DEADCHAR, WM_KEYDOWN, WM_SYSCHAR,
    WM_SYSDEADCHAR, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

/// The parts of a key message's `LPARAM`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyLparam {
    /// Set-1 scancode with `0xE000` added for extended keys.
    pub scancode: u16,
    pub extended: bool,
    /// The key was already down (auto-repeat).
    pub repeat: bool,
    pub released: bool,
}

impl KeyLparam {
    pub fn parse(lparam: isize) -> Self {
        let v = lparam as u32;
        let extended = v & (1 << 24) != 0;
        let code = ((v >> 16) & 0xFF) as u16;
        KeyLparam {
            scancode: if extended { 0xE000 | code } else { code },
            extended,
            repeat: v & (1 << 30) != 0,
            released: v & (1 << 31) != 0,
        }
    }
}

/// Resolves the generic `VK_SHIFT`/`VK_CONTROL`/`VK_MENU` to their left or right variant.
pub fn sided_vk(vk: VIRTUAL_KEY, lp: KeyLparam) -> VIRTUAL_KEY {
    match vk {
        VK_SHIFT => {
            if lp.scancode == 0x36 {
                VK_RSHIFT
            } else {
                VK_LSHIFT
            }
        }
        VK_CONTROL => {
            if lp.extended {
                VK_RCONTROL
            } else {
                VK_LCONTROL
            }
        }
        VK_MENU => {
            if lp.extended {
                VK_RMENU
            } else {
                VK_LMENU
            }
        }
        other => other,
    }
}

/// Where the key sits on the keyboard.
pub fn key_location(vk: VIRTUAL_KEY, lp: KeyLparam) -> KeyLocation {
    match vk {
        VK_LSHIFT | VK_LCONTROL | VK_LMENU | VK_LWIN => KeyLocation::Left,
        VK_RSHIFT | VK_RCONTROL | VK_RMENU | VK_RWIN => KeyLocation::Right,
        VK_NUMPAD0 | VK_NUMPAD1 | VK_NUMPAD2 | VK_NUMPAD3 | VK_NUMPAD4 | VK_NUMPAD5 | VK_NUMPAD6
        | VK_NUMPAD7 | VK_NUMPAD8 | VK_NUMPAD9 | VK_MULTIPLY | VK_ADD | VK_SEPARATOR | VK_SUBTRACT
        | VK_DECIMAL | VK_DIVIDE | VK_NUMLOCK => KeyLocation::Numpad,
        VK_RETURN if lp.extended => KeyLocation::Numpad,
        // The navigation cluster's keys are extended; the same virtual keys without the extended
        // bit come from the numpad with Num Lock off.
        VK_INSERT | VK_DELETE | VK_HOME | VK_END | VK_PRIOR | VK_NEXT | VK_LEFT | VK_RIGHT | VK_UP
        | VK_DOWN | VK_CLEAR
            if !lp.extended =>
        {
            KeyLocation::Numpad
        }
        _ => KeyLocation::Standard,
    }
}

/// The egui key of a layout-independent virtual key (everything that is not a character on the
/// layout: navigation, editing, function, numpad, media and modifier keys). `None` for keys whose
/// meaning comes from the layout.
pub fn key_for_vk(vk: VIRTUAL_KEY) -> Option<Key> {
    Some(match vk {
        VK_LEFT => Key::ArrowLeft,
        VK_RIGHT => Key::ArrowRight,
        VK_UP => Key::ArrowUp,
        VK_DOWN => Key::ArrowDown,
        VK_ESCAPE => Key::Escape,
        VK_TAB => Key::Tab,
        VK_BACK => Key::Backspace,
        VK_RETURN => Key::Enter,
        VK_SPACE => Key::Space,
        VK_INSERT => Key::Insert,
        VK_DELETE => Key::Delete,
        VK_HOME => Key::Home,
        VK_END => Key::End,
        VK_PRIOR => Key::PageUp,
        VK_NEXT => Key::PageDown,
        VK_CLEAR => Key::Clear,
        VK_NUMPAD0 => Key::Num0,
        VK_NUMPAD1 => Key::Num1,
        VK_NUMPAD2 => Key::Num2,
        VK_NUMPAD3 => Key::Num3,
        VK_NUMPAD4 => Key::Num4,
        VK_NUMPAD5 => Key::Num5,
        VK_NUMPAD6 => Key::Num6,
        VK_NUMPAD7 => Key::Num7,
        VK_NUMPAD8 => Key::Num8,
        VK_NUMPAD9 => Key::Num9,
        VK_ADD => Key::Plus,
        VK_SUBTRACT => Key::Minus,
        VK_DECIMAL => Key::Period,
        VK_DIVIDE => Key::Slash,
        VK_SEPARATOR => Key::Comma,
        VK_F1 => Key::F1,
        VK_F2 => Key::F2,
        VK_F3 => Key::F3,
        VK_F4 => Key::F4,
        VK_F5 => Key::F5,
        VK_F6 => Key::F6,
        VK_F7 => Key::F7,
        VK_F8 => Key::F8,
        VK_F9 => Key::F9,
        VK_F10 => Key::F10,
        VK_F11 => Key::F11,
        VK_F12 => Key::F12,
        VK_F13 => Key::F13,
        VK_F14 => Key::F14,
        VK_F15 => Key::F15,
        VK_F16 => Key::F16,
        VK_F17 => Key::F17,
        VK_F18 => Key::F18,
        VK_F19 => Key::F19,
        VK_F20 => Key::F20,
        VK_F21 => Key::F21,
        VK_F22 => Key::F22,
        VK_F23 => Key::F23,
        VK_F24 => Key::F24,
        VK_BROWSER_BACK => Key::BrowserBack,
        VK_BROWSER_FORWARD => Key::BrowserForward,
        VK_BROWSER_REFRESH => Key::BrowserRefresh,
        VK_BROWSER_SEARCH => Key::BrowserSearch,
        VK_BROWSER_HOME => Key::BrowserHome,
        VK_BROWSER_FAVORITES => Key::BrowserFavorites,
        VK_BROWSER_STOP => Key::BrowserStop,
        VK_MEDIA_PLAY_PAUSE => Key::MediaPlayPause,
        VK_MEDIA_NEXT_TRACK => Key::MediaTrackNext,
        VK_MEDIA_PREV_TRACK => Key::MediaTrackPrevious,
        VK_MEDIA_STOP => Key::MediaStop,
        VK_VOLUME_MUTE => Key::AudioVolumeMute,
        VK_VOLUME_DOWN => Key::AudioVolumeDown,
        VK_VOLUME_UP => Key::AudioVolumeUp,
        VK_LAUNCH_MAIL => Key::LaunchMail,
        VK_LAUNCH_APP1 => Key::LaunchApp1,
        VK_LAUNCH_APP2 => Key::LaunchApp2,
        VK_LSHIFT => Key::ShiftLeft,
        VK_RSHIFT => Key::ShiftRight,
        VK_LCONTROL => Key::ControlLeft,
        VK_RCONTROL => Key::ControlRight,
        VK_LMENU => Key::AltLeft,
        VK_RMENU => Key::AltRight,
        VK_LWIN => Key::SuperLeft,
        VK_RWIN => Key::SuperRight,
        VK_CAPITAL => Key::CapsLock,
        VK_NUMLOCK => Key::NumLock,
        VK_SCROLL => Key::ScrollLock,
        VK_SNAPSHOT => Key::PrintScreen,
        VK_PAUSE => Key::Pause,
        VK_APPS => Key::Menu,
        VK_HELP => Key::Help,
        VK_SLEEP => Key::Sleep,
        _ => return None,
    })
}

/// The egui key named by a character the layout puts on a key: ASCII letters (either case), digits
/// and the punctuation egui names.
pub fn key_for_char(c: char) -> Option<Key> {
    if c.is_ascii_alphabetic() {
        let mut buf = [0u8; 4];
        return Key::from_name(c.to_ascii_uppercase().encode_utf8(&mut buf));
    }
    if c.is_ascii_graphic() {
        let mut buf = [0u8; 4];
        return Key::from_name(c.encode_utf8(&mut buf));
    }
    None
}

/// The logical key: named virtual keys first, then the layout's character, then the virtual key of
/// a letter or digit (layouts whose base character is not Latin, e.g. Russian or Greek, still give
/// `VK_A..VK_Z` for those positions, which is what shortcuts expect).
pub fn logical_key(vk: VIRTUAL_KEY, layout_char: Option<char>) -> Option<Key> {
    if let Some(k) = key_for_vk(vk) {
        return Some(k);
    }
    if let Some(k) = layout_char.and_then(key_for_char) {
        return Some(k);
    }
    match vk.0 {
        0x30..=0x39 | 0x41..=0x5A => key_for_char(vk.0 as u8 as char),
        _ => None,
    }
}

/// The `KeyEvent.physical` code of a set-1 scancode: its HID usage, else `0xFFFF_0000 | scancode` (the
/// encoding skelter-platform's key docs fix for every backend).
pub fn physical_code(scancode: u16) -> u32 {
    match hid_for_scancode(scancode) {
        0 => 0xFFFF_0000 | scancode as u32,
        hid => hid,
    }
}

/// The USB HID usage (page 7, encoded `0x0007_00XX`) of a set-1 scancode (`0xE0xx` for extended
/// keys), or 0 when the key has none on page 7 (media keys are on the consumer page).
///
/// Table: Microsoft's "Keyboard Scan Code Specification" (set 1 to HID usage), with Windows'
/// reporting of Num Lock as `0xE045` and Pause as `0x45` in key messages.
pub fn hid_for_scancode(scancode: u16) -> u32 {
    let usage: u32 = match scancode {
        0x01 => 0x29,
        0x02..=0x0B => 0x1E + (scancode as u32 - 0x02),
        0x0C => 0x2D,
        0x0D => 0x2E,
        0x0E => 0x2A,
        0x0F => 0x2B,
        0x10 => 0x14,
        0x11 => 0x1A,
        0x12 => 0x08,
        0x13 => 0x15,
        0x14 => 0x17,
        0x15 => 0x1C,
        0x16 => 0x18,
        0x17 => 0x0C,
        0x18 => 0x12,
        0x19 => 0x13,
        0x1A => 0x2F,
        0x1B => 0x30,
        0x1C => 0x28,
        0x1D => 0xE0,
        0x1E => 0x04,
        0x1F => 0x16,
        0x20 => 0x07,
        0x21 => 0x09,
        0x22 => 0x0A,
        0x23 => 0x0B,
        0x24 => 0x0D,
        0x25 => 0x0E,
        0x26 => 0x0F,
        0x27 => 0x33,
        0x28 => 0x34,
        0x29 => 0x35,
        0x2A => 0xE1,
        0x2B => 0x31,
        0x2C => 0x1D,
        0x2D => 0x1B,
        0x2E => 0x06,
        0x2F => 0x19,
        0x30 => 0x05,
        0x31 => 0x11,
        0x32 => 0x10,
        0x33 => 0x36,
        0x34 => 0x37,
        0x35 => 0x38,
        0x36 => 0xE5,
        0x37 => 0x55,
        0x38 => 0xE2,
        0x39 => 0x2C,
        0x3A => 0x39,
        0x3B..=0x44 => 0x3A + (scancode as u32 - 0x3B),
        0x45 => 0x48,
        0x46 => 0x47,
        0x47 => 0x5F,
        0x48 => 0x60,
        0x49 => 0x61,
        0x4A => 0x56,
        0x4B => 0x5C,
        0x4C => 0x5D,
        0x4D => 0x5E,
        0x4E => 0x57,
        0x4F => 0x59,
        0x50 => 0x5A,
        0x51 => 0x5B,
        0x52 => 0x62,
        0x53 => 0x63,
        0x56 => 0x64,
        0x57 => 0x44,
        0x58 => 0x45,
        0x59 => 0x67,
        0x64..=0x6E => 0x68 + (scancode as u32 - 0x64),
        0x70 => 0x88,
        0x73 => 0x87,
        0x76 => 0x73,
        0x79 => 0x8A,
        0x7B => 0x8B,
        0x7D => 0x89,
        0x7E => 0x85,
        0xE01C => 0x58,
        0xE01D => 0xE4,
        0xE035 => 0x54,
        0xE037 => 0x46,
        0xE038 => 0xE6,
        0xE045 => 0x53,
        0xE046 => 0x48,
        0xE047 => 0x4A,
        0xE048 => 0x52,
        0xE049 => 0x4B,
        0xE04B => 0x50,
        0xE04D => 0x4F,
        0xE04F => 0x4D,
        0xE050 => 0x51,
        0xE051 => 0x4E,
        0xE052 => 0x49,
        0xE053 => 0x4C,
        0xE05B => 0xE3,
        0xE05C => 0xE7,
        0xE05D => 0x65,
        0xE05E => 0x66,
        _ => return 0,
    };
    0x0007_0000 | usage
}

/// Keeps the text a key's `WM_CHAR`s produced: drops control characters (Ctrl+A gives U+0001) and
/// unpaired surrogates.
pub fn text_from_utf16(units: &[u16]) -> Option<String> {
    let s: String = char::decode_utf16(units.iter().copied())
        .filter_map(Result::ok)
        .filter(|c| !c.is_control())
        .collect();
    (!s.is_empty()).then_some(s)
}

/// Which modifier keys are down, tracked from key messages (`GetKeyState` cannot tell the
/// synthetic AltGr Ctrl from a real one).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ModifierKeys {
    pub lshift: bool,
    pub rshift: bool,
    pub lctrl: bool,
    pub rctrl: bool,
    pub lalt: bool,
    pub ralt: bool,
    pub lwin: bool,
    pub rwin: bool,
    /// The right Alt is down as AltGr (a synthetic left Ctrl came with it).
    pub altgr: bool,
}

impl ModifierKeys {
    pub fn modifiers(&self) -> Modifiers {
        let mut m = Modifiers::default();
        if self.lshift || self.rshift {
            m = m.union(Modifiers::SHIFT);
        }
        if self.lctrl || self.rctrl {
            m = m.union(Modifiers::CTRL);
        }
        if self.lalt || (self.ralt && !self.altgr) {
            m = m.union(Modifiers::ALT);
        }
        if self.ralt && self.altgr {
            m = m.union(Modifiers::ALT_GR);
        }
        if self.lwin || self.rwin {
            m = m.union(Modifiers::SUPER);
        }
        m
    }

    /// Records a sided modifier key going down or up; returns whether `vk` was a modifier.
    pub fn update(&mut self, vk: VIRTUAL_KEY, down: bool) -> bool {
        let slot = match vk {
            VK_LSHIFT => &mut self.lshift,
            VK_RSHIFT => &mut self.rshift,
            VK_LCONTROL => &mut self.lctrl,
            VK_RCONTROL => &mut self.rctrl,
            VK_LMENU => &mut self.lalt,
            VK_RMENU => &mut self.ralt,
            VK_LWIN => &mut self.lwin,
            VK_RWIN => &mut self.rwin,
            _ => return false,
        };
        *slot = down;
        if vk == VK_RMENU && !down {
            self.altgr = false;
        }
        true
    }
}

/// The per-thread key translation state.
#[derive(Debug, Default)]
pub struct KeyTranslator {
    pub mods: ModifierKeys,
    /// The synthetic left Ctrl of AltGr is down (dropped, so its key-up must be dropped too).
    fake_lctrl: bool,
}

impl KeyTranslator {
    /// Translates one key message. `None` when the message is not a key the app sees: the IME's
    /// `VK_PROCESSKEY`, AltGr's synthetic Ctrl.
    pub fn key(&mut self, hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM, msg_time: i32) -> Option<KeyEvent> {
        let down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
        let lp = KeyLparam::parse(lparam.0);
        let raw_vk = VIRTUAL_KEY(wparam.0 as u16);
        if raw_vk == VK_PROCESSKEY {
            return None;
        }
        if raw_vk == VK_CONTROL && !lp.extended {
            if down && !self.fake_lctrl && next_is_altgr_alt(hwnd, msg_time) {
                self.fake_lctrl = true;
                self.mods.altgr = true;
                return None;
            }
            if !down && self.fake_lctrl {
                self.fake_lctrl = false;
                return None;
            }
        }
        let vk = sided_vk(raw_vk, lp);
        let was_modifier = self.mods.update(vk, down);
        if vk == VK_RMENU && down && self.fake_lctrl {
            self.mods.altgr = true;
        }

        let text = if down && !was_modifier { collect_chars(hwnd) } else { None };
        let layout_char = layout_char(raw_vk, lp.scancode);
        Some(KeyEvent {
            key: logical_key(vk, layout_char),
            physical: physical_code(lp.scancode),
            text,
            state: if down { KeyState::Pressed } else { KeyState::Released },
            repeat: down && lp.repeat,
            location: key_location(vk, lp),
            modifiers: self.mods.modifiers(),
        })
    }

    /// Re-reads the modifier keys from the OS (focus changes lose the key-ups that went elsewhere).
    pub fn resync(&mut self) {
        let held = |vk: VIRTUAL_KEY| unsafe { GetKeyState(vk.0 as i32) } < 0;
        self.fake_lctrl = false;
        self.mods = ModifierKeys {
            lshift: held(VK_LSHIFT),
            rshift: held(VK_RSHIFT),
            lctrl: held(VK_LCONTROL),
            rctrl: held(VK_RCONTROL),
            lalt: held(VK_LMENU),
            ralt: held(VK_RMENU),
            lwin: held(VK_LWIN),
            rwin: held(VK_RWIN),
            altgr: false,
        };
    }
}

/// Whether the next queued message is the right Alt that a synthetic AltGr Ctrl announces.
fn next_is_altgr_alt(hwnd: HWND, msg_time: i32) -> bool {
    let mut next = MSG::default();
    let found = unsafe { PeekMessageW(&mut next, Some(hwnd), WM_KEYDOWN, WM_SYSKEYUP, PM_NOREMOVE) }.as_bool();
    found
        && (next.message == WM_KEYDOWN || next.message == WM_SYSKEYDOWN)
        && next.wParam.0 as u16 == VK_MENU.0
        && KeyLparam::parse(next.lParam.0).extended
        && next.time as i32 == msg_time
}

/// Removes the `WM_CHAR`/`WM_DEADCHAR` messages `TranslateMessage` posted for this key and returns
/// their text. `WM_SYSCHAR` (Alt+key) is removed but produces no text: it is a shortcut.
fn collect_chars(hwnd: HWND) -> Option<String> {
    let mut units: Vec<u16> = Vec::new();
    let mut msg = MSG::default();
    loop {
        let got = unsafe { PeekMessageW(&mut msg, Some(hwnd), WM_CHAR, WM_SYSDEADCHAR, PM_REMOVE) }.as_bool();
        if !got {
            break;
        }
        match msg.message {
            WM_CHAR => units.push(msg.wParam.0 as u16),
            WM_DEADCHAR | WM_SYSCHAR | WM_SYSDEADCHAR => {}
            _ => {}
        }
    }
    text_from_utf16(&units)
}

/// The layout's unshifted character for a key, without touching the dead-key state (flag 4,
/// Windows 10 1607+). Dead keys report their spacing character, which names no egui key.
fn layout_char(vk: VIRTUAL_KEY, scancode: u16) -> Option<char> {
    let state = [0u8; 256];
    let mut buf = [0u16; 8];
    let layout = unsafe { GetKeyboardLayout(0) };
    let n = unsafe { ToUnicodeEx(vk.0 as u32, (scancode & 0xFF) as u32, &state, &mut buf, 4, Some(layout)) };
    if n == 0 {
        return None;
    }
    let len = n.unsigned_abs() as usize;
    char::decode_utf16(buf[..len.min(buf.len())].iter().copied()).next()?.ok().filter(|c| !c.is_control())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lp(scancode: u32, extended: bool) -> KeyLparam {
        KeyLparam::parse(((scancode << 16) | if extended { 1 << 24 } else { 0 } | 1) as isize)
    }

    #[test]
    fn lparam_fields() {
        let p = KeyLparam::parse(((1u32 << 31) | (1 << 30) | (1 << 24) | (0x4B << 16) | 1) as i32 as isize);
        assert_eq!(p, KeyLparam { scancode: 0xE04B, extended: true, repeat: true, released: true });
    }

    #[test]
    fn named_keys_have_egui_names() {
        assert_eq!(key_for_vk(VK_LEFT).map(Key::name), Some("Left"));
        assert_eq!(key_for_vk(VK_PRIOR), Some(Key::PageUp));
        assert_eq!(key_for_vk(VK_F24), Some(Key::F24));
        assert_eq!(key_for_vk(VK_NUMPAD7), Some(Key::Num7));
        assert_eq!(key_for_vk(VK_APPS).map(Key::name), Some("Menu"));
        assert_eq!(key_for_vk(VIRTUAL_KEY(0x41)), None, "letters come from the layout");
    }

    #[test]
    fn layout_characters_name_keys() {
        assert_eq!(logical_key(VIRTUAL_KEY(0x41), Some('q')), Some(Key::Q), "AZERTY's A position types q");
        assert_eq!(logical_key(VK_OEM_MINUS, Some('-')), Some(Key::Minus));
        assert_eq!(logical_key(VK_OEM_1, Some(';')), Some(Key::Semicolon));
        assert_eq!(logical_key(VK_OEM_3, Some('`')), Some(Key::Backtick));
        assert_eq!(logical_key(VIRTUAL_KEY(0x46), Some('а')), Some(Key::F), "Cyrillic falls back to the VK");
        assert_eq!(logical_key(VK_OEM_7, Some('ä')), None);
        assert_eq!(logical_key(VK_SPACE, Some(' ')), Some(Key::Space));
        // Every egui name round-trips through the name table.
        for c in "abcxyz0189-=[];',./\\`".chars() {
            let k = key_for_char(c).unwrap();
            assert_eq!(Key::from_name(k.name()), Some(k));
        }
    }

    #[test]
    fn sides_and_locations() {
        assert_eq!(sided_vk(VK_SHIFT, lp(0x36, false)), VK_RSHIFT);
        assert_eq!(sided_vk(VK_SHIFT, lp(0x2A, false)), VK_LSHIFT);
        assert_eq!(sided_vk(VK_CONTROL, lp(0x1D, true)), VK_RCONTROL);
        assert_eq!(sided_vk(VK_MENU, lp(0x38, false)), VK_LMENU);
        assert_eq!(key_location(VK_RETURN, lp(0x1C, true)), KeyLocation::Numpad);
        assert_eq!(key_location(VK_RETURN, lp(0x1C, false)), KeyLocation::Standard);
        assert_eq!(key_location(VK_HOME, lp(0x47, true)), KeyLocation::Standard);
        assert_eq!(key_location(VK_HOME, lp(0x47, false)), KeyLocation::Numpad);
        assert_eq!(key_location(VK_RCONTROL, lp(0x1D, true)), KeyLocation::Right);
        assert_eq!(logical_key(sided_vk(VK_MENU, lp(0x38, false)), None), Some(Key::AltLeft));
        assert_eq!(logical_key(sided_vk(VK_MENU, lp(0x38, true)), None), Some(Key::AltRight));
    }

    #[test]
    fn scancodes_to_hid() {
        assert_eq!(hid_for_scancode(0x1E), 0x0007_0004, "A");
        assert_eq!(hid_for_scancode(0x2C), 0x0007_001D, "Z");
        assert_eq!(hid_for_scancode(0x0B), 0x0007_0027, "0");
        assert_eq!(hid_for_scancode(0x02), 0x0007_001E, "1");
        assert_eq!(hid_for_scancode(0x3B), 0x0007_003A, "F1");
        assert_eq!(hid_for_scancode(0x44), 0x0007_0043, "F10");
        assert_eq!(hid_for_scancode(0x58), 0x0007_0045, "F12");
        assert_eq!(hid_for_scancode(0xE048), 0x0007_0052, "Up");
        assert_eq!(hid_for_scancode(0xE01C), 0x0007_0058, "numpad Enter");
        assert_eq!(hid_for_scancode(0xE045), 0x0007_0053, "Num Lock");
        assert_eq!(hid_for_scancode(0x45), 0x0007_0048, "Pause");
        assert_eq!(hid_for_scancode(0xE038), 0x0007_00E6, "right Alt");
        assert_eq!(hid_for_scancode(0x56), 0x0007_0064, "ISO backslash");
        assert_eq!(hid_for_scancode(0xE022), 0, "media play has no page-7 usage");
        assert_eq!(physical_code(0x1E), 0x0007_0004, "A");
        assert_eq!(physical_code(0xE048), 0x0007_0052, "Up");
        assert_eq!(physical_code(0xE022), 0xFFFF_E022, "media play: its scancode");
    }

    #[test]
    fn altgr_is_not_ctrl_alt() {
        let mut m = ModifierKeys::default();
        m.update(VK_RMENU, true);
        m.altgr = true;
        assert_eq!(m.modifiers(), Modifiers::ALT_GR);
        m.update(VK_LCONTROL, true);
        assert_eq!(m.modifiers(), Modifiers::ALT_GR.union(Modifiers::CTRL), "a real Ctrl with AltGr");
        m.update(VK_RMENU, false);
        assert!(!m.altgr);
        let mut plain = ModifierKeys::default();
        plain.update(VK_LCONTROL, true);
        plain.update(VK_RMENU, true);
        assert_eq!(plain.modifiers(), Modifiers::CTRL.union(Modifiers::ALT));
    }

    #[test]
    fn char_text_drops_controls() {
        assert_eq!(text_from_utf16(&[0x01]), None);
        assert_eq!(text_from_utf16(&['é' as u16]), Some("é".into()));
        assert_eq!(text_from_utf16(&[0xD83D, 0xDE00]), Some("😀".into()));
        assert_eq!(text_from_utf16(&[0xD83D]), None, "a lone surrogate");
    }
}
