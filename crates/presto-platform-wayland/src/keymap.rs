//! Key mapping: xkb keysyms to presto's egui-named [`Key`], Linux evdev key codes to USB HID usages and
//! key locations.
//!
//! The logical key follows egui-winit's rule: the layout-mapped keysym names the key when egui has a name
//! for it, and otherwise the physical position does (so Ctrl+C still works on a Cyrillic layout, where the
//! keysym is `Cyrillic_es`). Physical codes are USB HID usages encoded as `page << 16 | usage` (keyboard
//! page 0x07, consumer page 0x0C); a code with no HID usage is `0xFFFF_0000 | evdev`.
//!
//! Sources: `linux/input-event-codes.h` (evdev codes), USB HID Usage Tables 1.5 §10 (keyboard page) and
//! §15 (consumer page), xkbcommon's `xkbcommon-keysyms.h` (through the `xkeysym` crate).

use presto_base::event::KeyLocation;
use presto_base::key::Key;
use xkeysym::key as ks;

/// The evdev code of a `wl_keyboard.key` event; xkb keycodes are evdev + 8.
pub const XKB_EVDEV_OFFSET: u32 = 8;

/// The logical key for a key press: the keysym's egui name, else the physical key's.
pub fn logical_key(keysym: u32, evdev: u32) -> Option<Key> {
    key_from_keysym(keysym).or_else(|| key_from_evdev(evdev))
}

/// The egui key a keysym names, if any.
pub fn key_from_keysym(sym: u32) -> Option<Key> {
    use Key::*;
    let k = match sym {
        ks::Escape => Escape,
        ks::Tab | ks::ISO_Left_Tab | ks::KP_Tab => Tab,
        ks::BackSpace => Backspace,
        ks::Return | ks::KP_Enter => Enter,
        ks::space | ks::KP_Space => Space,
        ks::Insert | ks::KP_Insert => Insert,
        ks::Delete | ks::KP_Delete => Delete,
        ks::Home | ks::KP_Home => Home,
        ks::End | ks::KP_End => End,
        ks::Prior | ks::KP_Prior => PageUp,
        ks::Next | ks::KP_Next => PageDown,
        ks::Left | ks::KP_Left => ArrowLeft,
        ks::Right | ks::KP_Right => ArrowRight,
        ks::Up | ks::KP_Up => ArrowUp,
        ks::Down | ks::KP_Down => ArrowDown,

        ks::XF86_Copy => Copy,
        ks::XF86_Cut => Cut,
        ks::XF86_Paste => Paste,

        ks::KP_0 => Num0,
        ks::KP_1 => Num1,
        ks::KP_2 => Num2,
        ks::KP_3 => Num3,
        ks::KP_4 => Num4,
        ks::KP_5 => Num5,
        ks::KP_6 => Num6,
        ks::KP_7 => Num7,
        ks::KP_8 => Num8,
        ks::KP_9 => Num9,
        ks::KP_Add => Plus,
        ks::KP_Subtract => Minus,
        ks::KP_Decimal => Period,
        ks::KP_Divide => Slash,
        ks::KP_Equal => Equals,
        ks::KP_Separator => Comma,

        ks::F1..=ks::F35 => return function_key(sym - ks::F1),
        ks::KP_F1..=ks::KP_F4 => return function_key(sym - ks::KP_F1),

        ks::XF86_Back => BrowserBack,
        ks::XF86_Forward => BrowserForward,
        ks::XF86_Reload | ks::XF86_Refresh => BrowserRefresh,
        ks::XF86_Search => BrowserSearch,
        ks::XF86_HomePage => BrowserHome,
        ks::XF86_Favorites => BrowserFavorites,
        ks::XF86_Stop => BrowserStop,

        ks::XF86_AudioPlay | ks::XF86_AudioPause => MediaPlayPause,
        ks::XF86_AudioNext => MediaTrackNext,
        ks::XF86_AudioPrev => MediaTrackPrevious,
        ks::XF86_AudioStop => MediaStop,
        ks::XF86_AudioMute => AudioVolumeMute,
        ks::XF86_AudioLowerVolume => AudioVolumeDown,
        ks::XF86_AudioRaiseVolume => AudioVolumeUp,

        ks::XF86_Mail => LaunchMail,
        ks::XF86_MyComputer => LaunchApp1,
        ks::XF86_Calculator => LaunchApp2,

        ks::Shift_L => ShiftLeft,
        ks::Shift_R => ShiftRight,
        ks::Control_L => ControlLeft,
        ks::Control_R => ControlRight,
        ks::Alt_L => AltLeft,
        ks::Alt_R => AltRight,
        ks::Super_L => SuperLeft,
        ks::Super_R => SuperRight,

        ks::Caps_Lock => CapsLock,
        ks::Num_Lock => NumLock,
        ks::Scroll_Lock => ScrollLock,
        ks::Print => PrintScreen,
        ks::Pause => Pause,
        ks::Menu => Menu,

        ks::XF86_Fn => Fn,
        ks::XF86_Eject => Eject,
        ks::Help => Help,
        ks::XF86_PowerOff => Power,
        ks::XF86_Sleep => Sleep,
        ks::Clear => Clear,

        _ => return xkeysym::Keysym::new(sym).key_char().and_then(key_from_char),
    };
    Some(k)
}

fn function_key(index: u32) -> Option<Key> {
    use Key::*;
    const FKEYS: [Key; 35] = [
        F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12, F13, F14, F15, F16, F17, F18, F19, F20, F21, F22, F23, F24,
        F25, F26, F27, F28, F29, F30, F31, F32, F33, F34, F35,
    ];
    FKEYS.get(index as usize).copied()
}

/// The egui key a character names (letters case-insensitively, digits, the punctuation egui knows).
pub fn key_from_char(c: char) -> Option<Key> {
    use Key::*;
    const LETTERS: [Key; 26] = [A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z];
    const DIGITS: [Key; 10] = [Num0, Num1, Num2, Num3, Num4, Num5, Num6, Num7, Num8, Num9];
    Some(match c {
        'a'..='z' => LETTERS[(c as u8 - b'a') as usize],
        'A'..='Z' => LETTERS[(c as u8 - b'A') as usize],
        '0'..='9' => DIGITS[(c as u8 - b'0') as usize],
        ':' => Colon,
        ',' => Comma,
        '\\' => Backslash,
        '/' => Slash,
        '|' => Pipe,
        '?' => Questionmark,
        '!' => Exclamationmark,
        '[' => OpenBracket,
        ']' => CloseBracket,
        '{' => OpenCurlyBracket,
        '}' => CloseCurlyBracket,
        '`' => Backtick,
        '-' => Minus,
        '.' => Period,
        '+' => Plus,
        '=' => Equals,
        ';' => Semicolon,
        '\'' => Quote,
        ' ' => Space,
        _ => return None,
    })
}

/// One row of the evdev table: code, HID usage (`page << 16 | id`), the egui key at that position on a
/// US layout, and the key's location.
struct Phys {
    evdev: u16,
    hid: u32,
    key: Option<Key>,
    loc: KeyLocation,
}

const fn kb(evdev: u16, usage: u32, key: Option<Key>, loc: KeyLocation) -> Phys {
    Phys { evdev, hid: 0x0007_0000 | usage, key, loc }
}

const fn cs(evdev: u16, usage: u32, key: Option<Key>) -> Phys {
    Phys { evdev, hid: 0x000C_0000 | usage, key, loc: KeyLocation::Standard }
}

use KeyLocation::{Left as L, Numpad as N, Right as R, Standard as S};

#[rustfmt::skip]
static PHYS: &[Phys] = &[
    kb(1, 0x29, Some(Key::Escape), S),
    kb(2, 0x1E, Some(Key::Num1), S), kb(3, 0x1F, Some(Key::Num2), S), kb(4, 0x20, Some(Key::Num3), S),
    kb(5, 0x21, Some(Key::Num4), S), kb(6, 0x22, Some(Key::Num5), S), kb(7, 0x23, Some(Key::Num6), S),
    kb(8, 0x24, Some(Key::Num7), S), kb(9, 0x25, Some(Key::Num8), S), kb(10, 0x26, Some(Key::Num9), S),
    kb(11, 0x27, Some(Key::Num0), S),
    kb(12, 0x2D, Some(Key::Minus), S), kb(13, 0x2E, Some(Key::Equals), S),
    kb(14, 0x2A, Some(Key::Backspace), S), kb(15, 0x2B, Some(Key::Tab), S),
    kb(16, 0x14, Some(Key::Q), S), kb(17, 0x1A, Some(Key::W), S), kb(18, 0x08, Some(Key::E), S),
    kb(19, 0x15, Some(Key::R), S), kb(20, 0x17, Some(Key::T), S), kb(21, 0x1C, Some(Key::Y), S),
    kb(22, 0x18, Some(Key::U), S), kb(23, 0x0C, Some(Key::I), S), kb(24, 0x12, Some(Key::O), S),
    kb(25, 0x13, Some(Key::P), S),
    kb(26, 0x2F, Some(Key::OpenBracket), S), kb(27, 0x30, Some(Key::CloseBracket), S),
    kb(28, 0x28, Some(Key::Enter), S), kb(29, 0xE0, Some(Key::ControlLeft), L),
    kb(30, 0x04, Some(Key::A), S), kb(31, 0x16, Some(Key::S), S), kb(32, 0x07, Some(Key::D), S),
    kb(33, 0x09, Some(Key::F), S), kb(34, 0x0A, Some(Key::G), S), kb(35, 0x0B, Some(Key::H), S),
    kb(36, 0x0D, Some(Key::J), S), kb(37, 0x0E, Some(Key::K), S), kb(38, 0x0F, Some(Key::L), S),
    kb(39, 0x33, Some(Key::Semicolon), S), kb(40, 0x34, Some(Key::Quote), S),
    kb(41, 0x35, Some(Key::Backtick), S), kb(42, 0xE1, Some(Key::ShiftLeft), L),
    kb(43, 0x31, Some(Key::Backslash), S),
    kb(44, 0x1D, Some(Key::Z), S), kb(45, 0x1B, Some(Key::X), S), kb(46, 0x06, Some(Key::C), S),
    kb(47, 0x19, Some(Key::V), S), kb(48, 0x05, Some(Key::B), S), kb(49, 0x11, Some(Key::N), S),
    kb(50, 0x10, Some(Key::M), S),
    kb(51, 0x36, Some(Key::Comma), S), kb(52, 0x37, Some(Key::Period), S), kb(53, 0x38, Some(Key::Slash), S),
    kb(54, 0xE5, Some(Key::ShiftRight), R), kb(55, 0x55, None, N),
    kb(56, 0xE2, Some(Key::AltLeft), L), kb(57, 0x2C, Some(Key::Space), S),
    kb(58, 0x39, Some(Key::CapsLock), S),
    kb(59, 0x3A, Some(Key::F1), S), kb(60, 0x3B, Some(Key::F2), S), kb(61, 0x3C, Some(Key::F3), S),
    kb(62, 0x3D, Some(Key::F4), S), kb(63, 0x3E, Some(Key::F5), S), kb(64, 0x3F, Some(Key::F6), S),
    kb(65, 0x40, Some(Key::F7), S), kb(66, 0x41, Some(Key::F8), S), kb(67, 0x42, Some(Key::F9), S),
    kb(68, 0x43, Some(Key::F10), S),
    kb(69, 0x53, Some(Key::NumLock), N), kb(70, 0x47, Some(Key::ScrollLock), S),
    kb(71, 0x5F, Some(Key::Num7), N), kb(72, 0x60, Some(Key::Num8), N), kb(73, 0x61, Some(Key::Num9), N),
    kb(74, 0x56, Some(Key::Minus), N),
    kb(75, 0x5C, Some(Key::Num4), N), kb(76, 0x5D, Some(Key::Num5), N), kb(77, 0x5E, Some(Key::Num6), N),
    kb(78, 0x57, Some(Key::Plus), N),
    kb(79, 0x59, Some(Key::Num1), N), kb(80, 0x5A, Some(Key::Num2), N), kb(81, 0x5B, Some(Key::Num3), N),
    kb(82, 0x62, Some(Key::Num0), N), kb(83, 0x63, Some(Key::Period), N),
    kb(85, 0x94, None, S),                              // KEY_ZENKAKUHANKAKU: Lang5
    kb(86, 0x64, Some(Key::IntlBackslash), S),          // KEY_102ND: Non-US \ and |
    kb(87, 0x44, Some(Key::F11), S), kb(88, 0x45, Some(Key::F12), S),
    kb(89, 0x87, None, S),                              // KEY_RO: International1
    kb(90, 0x92, None, S),                              // KEY_KATAKANA: Lang3
    kb(91, 0x93, None, S),                              // KEY_HIRAGANA: Lang4
    kb(92, 0x8A, None, S),                              // KEY_HENKAN: International4
    kb(93, 0x88, None, S),                              // KEY_KATAKANAHIRAGANA: International2
    kb(94, 0x8B, None, S),                              // KEY_MUHENKAN: International5
    kb(95, 0x8C, None, N),                              // KEY_KPJPCOMMA: International6
    kb(96, 0x58, Some(Key::Enter), N), kb(97, 0xE4, Some(Key::ControlRight), R),
    kb(98, 0x54, Some(Key::Slash), N), kb(99, 0x46, Some(Key::PrintScreen), S),
    kb(100, 0xE6, Some(Key::AltRight), R),
    kb(102, 0x4A, Some(Key::Home), S), kb(103, 0x52, Some(Key::ArrowUp), S),
    kb(104, 0x4B, Some(Key::PageUp), S), kb(105, 0x50, Some(Key::ArrowLeft), S),
    kb(106, 0x4F, Some(Key::ArrowRight), S), kb(107, 0x4D, Some(Key::End), S),
    kb(108, 0x51, Some(Key::ArrowDown), S), kb(109, 0x4E, Some(Key::PageDown), S),
    kb(110, 0x49, Some(Key::Insert), S), kb(111, 0x4C, Some(Key::Delete), S),
    kb(113, 0x7F, Some(Key::AudioVolumeMute), S), kb(114, 0x81, Some(Key::AudioVolumeDown), S),
    kb(115, 0x80, Some(Key::AudioVolumeUp), S), kb(116, 0x66, Some(Key::Power), S),
    kb(117, 0x67, Some(Key::Equals), N),
    kb(119, 0x48, Some(Key::Pause), S),
    kb(121, 0x85, Some(Key::Comma), N),                 // KEY_KPCOMMA
    kb(122, 0x90, None, S),                             // KEY_HANGEUL: Lang1
    kb(123, 0x91, None, S),                             // KEY_HANJA: Lang2
    kb(124, 0x89, None, S),                             // KEY_YEN: International3
    kb(125, 0xE3, Some(Key::SuperLeft), L), kb(126, 0xE7, Some(Key::SuperRight), R),
    kb(127, 0x65, Some(Key::Menu), S),                  // KEY_COMPOSE: Keyboard Application
    kb(128, 0x78, Some(Key::BrowserStop), S),           // KEY_STOP
    kb(129, 0x79, None, S),                             // KEY_AGAIN
    kb(131, 0x7A, None, S),                             // KEY_UNDO
    kb(133, 0x7C, Some(Key::Copy), S), kb(134, 0x74, None, S), // KEY_COPY, KEY_OPEN: Execute
    kb(135, 0x7D, Some(Key::Paste), S), kb(136, 0x7E, None, S), // KEY_PASTE, KEY_FIND
    kb(137, 0x7B, Some(Key::Cut), S), kb(138, 0x75, Some(Key::Help), S),
    kb(139, 0x76, Some(Key::Menu), S),                  // KEY_MENU: Keyboard Menu
    cs(140, 0x192, Some(Key::LaunchApp2)),              // KEY_CALC: AL Calculator
    cs(142, 0x032, Some(Key::Sleep)),                   // KEY_SLEEP: Sleep
    cs(155, 0x18A, Some(Key::LaunchMail)),              // KEY_MAIL: AL Email Reader
    cs(156, 0x22A, Some(Key::BrowserFavorites)),        // KEY_BOOKMARKS: AC Bookmarks
    cs(157, 0x194, Some(Key::LaunchApp1)),              // KEY_COMPUTER: AL Local Machine Browser
    cs(158, 0x224, Some(Key::BrowserBack)),             // KEY_BACK: AC Back
    cs(159, 0x225, Some(Key::BrowserForward)),          // KEY_FORWARD: AC Forward
    cs(161, 0x0B8, Some(Key::Eject)),                   // KEY_EJECTCD: Eject
    cs(163, 0x0B5, Some(Key::MediaTrackNext)),          // KEY_NEXTSONG
    cs(164, 0x0CD, Some(Key::MediaPlayPause)),          // KEY_PLAYPAUSE
    cs(165, 0x0B6, Some(Key::MediaTrackPrevious)),      // KEY_PREVIOUSSONG
    cs(166, 0x0B7, Some(Key::MediaStop)),               // KEY_STOPCD
    cs(172, 0x223, Some(Key::BrowserHome)),             // KEY_HOMEPAGE: AC Home
    cs(173, 0x227, Some(Key::BrowserRefresh)),          // KEY_REFRESH: AC Refresh
    kb(183, 0x68, Some(Key::F13), S), kb(184, 0x69, Some(Key::F14), S), kb(185, 0x6A, Some(Key::F15), S),
    kb(186, 0x6B, Some(Key::F16), S), kb(187, 0x6C, Some(Key::F17), S), kb(188, 0x6D, Some(Key::F18), S),
    kb(189, 0x6E, Some(Key::F19), S), kb(190, 0x6F, Some(Key::F20), S), kb(191, 0x70, Some(Key::F21), S),
    kb(192, 0x71, Some(Key::F22), S), kb(193, 0x72, Some(Key::F23), S), kb(194, 0x73, Some(Key::F24), S),
    cs(217, 0x221, Some(Key::BrowserSearch)),           // KEY_SEARCH: AC Search
    kb(464, 0x00, Some(Key::Fn), S),                    // KEY_FN: no HID usage on the keyboard page
];

fn phys(evdev: u32) -> Option<&'static Phys> {
    // Sorted by code: binary search.
    PHYS.binary_search_by_key(&evdev, |p| p.evdev as u32).ok().map(|i| &PHYS[i])
}

/// The key at this physical position on a US layout.
pub fn key_from_evdev(evdev: u32) -> Option<Key> {
    phys(evdev).and_then(|p| p.key)
}

/// The physical code presto reports: the HID usage, or `0xFFFF_0000 | evdev` when there is none.
pub fn physical_code(evdev: u32) -> u32 {
    match phys(evdev) {
        Some(p) if p.hid & 0xFFFF != 0 => p.hid,
        _ => 0xFFFF_0000 | (evdev & 0xFFFF),
    }
}

pub fn key_location(evdev: u32) -> KeyLocation {
    phys(evdev).map_or(KeyLocation::Standard, |p| p.loc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_is_sorted_and_unique() {
        for w in PHYS.windows(2) {
            assert!(w[0].evdev < w[1].evdev, "evdev {} before {}", w[0].evdev, w[1].evdev);
        }
    }

    #[test]
    fn letters_digits_and_punctuation_by_keysym() {
        assert_eq!(key_from_keysym(ks::a), Some(Key::A));
        assert_eq!(key_from_keysym(ks::A), Some(Key::A));
        assert_eq!(key_from_keysym(ks::z), Some(Key::Z));
        assert_eq!(key_from_keysym(ks::_0), Some(Key::Num0));
        assert_eq!(key_from_keysym(ks::_9), Some(Key::Num9));
        assert_eq!(key_from_keysym(ks::colon), Some(Key::Colon));
        assert_eq!(key_from_keysym(ks::bracketleft), Some(Key::OpenBracket));
        assert_eq!(key_from_keysym(ks::braceright), Some(Key::CloseCurlyBracket));
        assert_eq!(key_from_keysym(ks::apostrophe), Some(Key::Quote));
        assert_eq!(key_from_keysym(ks::grave), Some(Key::Backtick));
        assert_eq!(key_from_keysym(ks::question), Some(Key::Questionmark));
        assert_eq!(key_from_keysym(ks::exclam), Some(Key::Exclamationmark));
        assert_eq!(key_from_keysym(ks::bar), Some(Key::Pipe));
    }

    #[test]
    fn named_keys_by_keysym() {
        assert_eq!(key_from_keysym(ks::Return), Some(Key::Enter));
        assert_eq!(key_from_keysym(ks::KP_Enter), Some(Key::Enter));
        assert_eq!(key_from_keysym(ks::ISO_Left_Tab), Some(Key::Tab));
        assert_eq!(key_from_keysym(ks::Prior), Some(Key::PageUp));
        assert_eq!(key_from_keysym(ks::KP_Next), Some(Key::PageDown));
        assert_eq!(key_from_keysym(ks::F1), Some(Key::F1));
        assert_eq!(key_from_keysym(ks::F12), Some(Key::F12));
        assert_eq!(key_from_keysym(ks::F35), Some(Key::F35));
        assert_eq!(key_from_keysym(ks::KP_F4), Some(Key::F4));
        assert_eq!(key_from_keysym(ks::XF86_AudioPlay), Some(Key::MediaPlayPause));
        assert_eq!(key_from_keysym(ks::Super_R), Some(Key::SuperRight));
        assert_eq!(key_from_keysym(ks::Control_L), Some(Key::ControlLeft));
        assert_eq!(key_from_keysym(ks::Print), Some(Key::PrintScreen));
    }

    #[test]
    fn keys_without_egui_names() {
        assert_eq!(key_from_keysym(ks::dead_acute), None);
        assert_eq!(key_from_keysym(ks::ISO_Level3_Shift), None);
        assert_eq!(key_from_keysym(ks::KP_Multiply), None);
        assert_eq!(key_from_keysym(ks::Cyrillic_es), None);
        assert_eq!(key_from_keysym(ks::NoSymbol), None);
    }

    #[test]
    fn non_latin_layouts_fall_back_to_the_physical_key() {
        // KEY_C (46) produces Cyrillic_es on a Russian layout: Ctrl+C must still be C.
        assert_eq!(logical_key(ks::Cyrillic_es, 46), Some(Key::C));
        // A Latin layout with a different physical position wins over the position: AZERTY's A is KEY_Q.
        assert_eq!(logical_key(ks::a, 16), Some(Key::A));
    }

    #[test]
    fn physical_codes_are_hid_usages() {
        assert_eq!(physical_code(30), 0x0007_0004); // KEY_A
        assert_eq!(physical_code(1), 0x0007_0029); // KEY_ESC
        assert_eq!(physical_code(125), 0x0007_00E3); // KEY_LEFTMETA
        assert_eq!(physical_code(164), 0x000C_00CD); // KEY_PLAYPAUSE
        assert_eq!(physical_code(464), 0xFFFF_0000 | 464); // KEY_FN
        assert_eq!(physical_code(600), 0xFFFF_0000 | 600);
    }

    #[test]
    fn locations() {
        assert_eq!(key_location(42), KeyLocation::Left);
        assert_eq!(key_location(54), KeyLocation::Right);
        assert_eq!(key_location(96), KeyLocation::Numpad);
        assert_eq!(key_location(28), KeyLocation::Standard);
        assert_eq!(key_location(9999), KeyLocation::Standard);
    }

    #[test]
    fn every_mapped_key_round_trips_its_egui_name() {
        for p in PHYS {
            if let Some(k) = p.key {
                assert_eq!(Key::from_name(k.name()), Some(k), "{:?}", k);
            }
        }
    }
}
