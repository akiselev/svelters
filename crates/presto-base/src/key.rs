//! Logical keys, named exactly as egui 0.37's `egui::Key` so cadatomic's keymaps and layouts
//! (`keymap.rs`, `keylayout.rs`), which store egui key names, round-trip unchanged.
//!
//! The variant list and the `name`/`from_name` tables are taken from egui 0.37.0
//! `crates/egui/src/data/key.rs` (MIT OR Apache-2.0, Copyright (c) 2018-2021 Emil Ernerfeldt).

/// A logical key (layout-mapped), with egui's names.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Key {
    // ----------------------------------------------
    // Commands:
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    ArrowUp,

    Escape,
    Tab,
    Backspace,
    Enter,
    Space,

    Insert,
    Delete,
    Home,
    End,
    PageUp,
    PageDown,

    Copy,
    Cut,
    Paste,

    // ----------------------------------------------
    // Punctuation:
    Colon,
    Comma,
    Backslash,
    Slash,
    Pipe,
    Questionmark,

    // '!'
    Exclamationmark,

    // `[`
    OpenBracket,

    // `]`
    CloseBracket,

    // `{`
    OpenCurlyBracket,

    // `}`
    CloseCurlyBracket,
    Backtick,
    Minus,
    Period,
    Plus,
    Equals,
    Semicolon,
    Quote,

    // ----------------------------------------------
    // Digits:
    Num0,
    Num1,
    Num2,
    Num3,
    Num4,
    Num5,
    Num6,
    Num7,
    Num8,
    Num9,

    // ----------------------------------------------
    // Letters:
    A, // Used for cmd+A (select All)
    B,
    C, // |CMD COPY|
    D, // |CMD BOOKMARK|
    E, // |CMD SEARCH|
    F, // |CMD FIND firefox & chrome|
    G, // |CMD FIND chrome|
    H, // |CMD History|
    I, // italics
    J, // |CMD SEARCH firefox/DOWNLOAD chrome|
    K, // Used for ctrl+K (delete text after cursor)
    L,
    M,
    N,
    O, // |CMD OPEN|
    P, // |CMD PRINT|
    Q,
    R, // |CMD REFRESH|
    S, // |CMD SAVE|
    T, // |CMD TAB|
    U, // Used for ctrl+U (delete text before cursor)
    V, // |CMD PASTE|
    W, // Used for ctrl+W (delete previous word)
    X, // |CMD CUT|
    Y,
    Z, // |CMD UNDO|

    // ----------------------------------------------
    // Function keys:
    F1,
    F2,
    F3,
    F4,
    F5, // |CMD REFRESH|
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    F13,
    F14,
    F15,
    F16,
    F17,
    F18,
    F19,
    F20,
    F21,
    F22,
    F23,
    F24,
    F25,
    F26,
    F27,
    F28,
    F29,
    F30,
    F31,
    F32,
    F33,
    F34,
    F35,

    // ----------------------------------------------
    // Browser / multimedia navigation keys:
    BrowserBack,
    BrowserForward,
    BrowserRefresh,
    BrowserSearch,
    BrowserHome,
    BrowserFavorites,
    BrowserStop,

    // ----------------------------------------------
    // Media control keys:
    MediaPlayPause,
    MediaTrackNext,
    MediaTrackPrevious,
    MediaStop,
    AudioVolumeMute,
    AudioVolumeDown,
    AudioVolumeUp,

    // ----------------------------------------------
    // Launch keys:
    LaunchMail,
    LaunchApp1,
    LaunchApp2,

    // ----------------------------------------------
    // Modifier keys (exposed as distinct left/right variants so that
    // games and input-capture UIs can bind them independently). egui's
    // `Modifiers` struct still collapses both sides for the common case
    // (e.g. "Ctrl+C"); these variants are emitted only as physical
    // `Event::Key` presses.
    ShiftLeft,
    ShiftRight,
    ControlLeft,
    ControlRight,
    AltLeft,
    AltRight,
    SuperLeft,
    SuperRight,

    // ----------------------------------------------
    // International keys — physical positions that only exist on
    // non-US keyboards.
    IntlBackslash,

    // ----------------------------------------------
    // Lock / System keys:
    CapsLock,
    NumLock,
    ScrollLock,
    PrintScreen,
    Pause,
    Menu,

    // ----------------------------------------------
    // Mac / other system keys:
    Fn,
    Eject,
    Help,
    Power,
    Sleep,
    Clear,
    // When adding keys, remember to also update:
    // * crates/egui-winit/src/lib.rs
    // * Key::ALL
    // * Key::from_name
    // You should test that it works using the "Input Event History" window in the egui demo app.
    // Make sure to test both natively and on web!
    // Also: don't add keys last; add them to the group they best belong to.
}

impl Key {
    /// Parses egui's key names (and the aliases egui accepts).
    pub fn from_name(key: &str) -> Option<Self> {
        Some(match key {
            "⏷" | "ArrowDown" | "Down" => Self::ArrowDown,
            "⏴" | "ArrowLeft" | "Left" => Self::ArrowLeft,
            "⏵" | "ArrowRight" | "Right" => Self::ArrowRight,
            "⏶" | "ArrowUp" | "Up" => Self::ArrowUp,

            "Escape" | "Esc" => Self::Escape,
            "Tab" => Self::Tab,
            "Backspace" => Self::Backspace,
            "Enter" | "Return" | "NumpadEnter" => Self::Enter,

            "Insert" => Self::Insert,
            "Delete" => Self::Delete,
            "Home" => Self::Home,
            "End" => Self::End,
            "PageUp" => Self::PageUp,
            "PageDown" => Self::PageDown,

            "Copy" => Self::Copy,
            "Cut" => Self::Cut,
            "Paste" => Self::Paste,

            " " | "Space" => Self::Space,
            ":" | "Colon" => Self::Colon,
            "," | "Comma" | "NumpadComma" => Self::Comma,
            "-" | "−" | "Minus" | "NumpadSubtract" => Self::Minus,
            "." | "Period" | "NumpadDecimal" => Self::Period,
            "+" | "Plus" | "NumpadAdd" => Self::Plus,
            "=" | "Equal" | "Equals" | "NumpadEqual" => Self::Equals,
            ";" | "Semicolon" => Self::Semicolon,
            "\\" | "Backslash" => Self::Backslash,
            "/" | "Slash" | "NumpadDivide" => Self::Slash,
            "|" | "Pipe" => Self::Pipe,
            "?" | "Questionmark" => Self::Questionmark,
            "!" | "Exclamationmark" => Self::Exclamationmark,
            "[" | "OpenBracket" | "BracketLeft" => Self::OpenBracket,
            "]" | "CloseBracket" | "BracketRight" => Self::CloseBracket,
            "{" | "OpenCurlyBracket" => Self::OpenCurlyBracket,
            "}" | "CloseCurlyBracket" => Self::CloseCurlyBracket,
            "`" | "Backtick" | "Backquote" | "Grave" => Self::Backtick,
            "'" | "Quote" => Self::Quote,

            "0" | "Digit0" | "Numpad0" => Self::Num0,
            "1" | "Digit1" | "Numpad1" => Self::Num1,
            "2" | "Digit2" | "Numpad2" => Self::Num2,
            "3" | "Digit3" | "Numpad3" => Self::Num3,
            "4" | "Digit4" | "Numpad4" => Self::Num4,
            "5" | "Digit5" | "Numpad5" => Self::Num5,
            "6" | "Digit6" | "Numpad6" => Self::Num6,
            "7" | "Digit7" | "Numpad7" => Self::Num7,
            "8" | "Digit8" | "Numpad8" => Self::Num8,
            "9" | "Digit9" | "Numpad9" => Self::Num9,

            "a" | "A" | "KeyA" => Self::A,
            "b" | "B" | "KeyB" => Self::B,
            "c" | "C" | "KeyC" => Self::C,
            "d" | "D" | "KeyD" => Self::D,
            "e" | "E" | "KeyE" => Self::E,
            "f" | "F" | "KeyF" => Self::F,
            "g" | "G" | "KeyG" => Self::G,
            "h" | "H" | "KeyH" => Self::H,
            "i" | "I" | "KeyI" => Self::I,
            "j" | "J" | "KeyJ" => Self::J,
            "k" | "K" | "KeyK" => Self::K,
            "l" | "L" | "KeyL" => Self::L,
            "m" | "M" | "KeyM" => Self::M,
            "n" | "N" | "KeyN" => Self::N,
            "o" | "O" | "KeyO" => Self::O,
            "p" | "P" | "KeyP" => Self::P,
            "q" | "Q" | "KeyQ" => Self::Q,
            "r" | "R" | "KeyR" => Self::R,
            "s" | "S" | "KeyS" => Self::S,
            "t" | "T" | "KeyT" => Self::T,
            "u" | "U" | "KeyU" => Self::U,
            "v" | "V" | "KeyV" => Self::V,
            "w" | "W" | "KeyW" => Self::W,
            "x" | "X" | "KeyX" => Self::X,
            "y" | "Y" | "KeyY" => Self::Y,
            "z" | "Z" | "KeyZ" => Self::Z,

            "F1" => Self::F1,
            "F2" => Self::F2,
            "F3" => Self::F3,
            "F4" => Self::F4,
            "F5" => Self::F5,
            "F6" => Self::F6,
            "F7" => Self::F7,
            "F8" => Self::F8,
            "F9" => Self::F9,
            "F10" => Self::F10,
            "F11" => Self::F11,
            "F12" => Self::F12,
            "F13" => Self::F13,
            "F14" => Self::F14,
            "F15" => Self::F15,
            "F16" => Self::F16,
            "F17" => Self::F17,
            "F18" => Self::F18,
            "F19" => Self::F19,
            "F20" => Self::F20,
            "F21" => Self::F21,
            "F22" => Self::F22,
            "F23" => Self::F23,
            "F24" => Self::F24,
            "F25" => Self::F25,
            "F26" => Self::F26,
            "F27" => Self::F27,
            "F28" => Self::F28,
            "F29" => Self::F29,
            "F30" => Self::F30,
            "F31" => Self::F31,
            "F32" => Self::F32,
            "F33" => Self::F33,
            "F34" => Self::F34,
            "F35" => Self::F35,

            "BrowserBack" => Self::BrowserBack,
            "BrowserForward" => Self::BrowserForward,
            "BrowserRefresh" => Self::BrowserRefresh,
            "BrowserSearch" => Self::BrowserSearch,
            "BrowserHome" => Self::BrowserHome,
            "BrowserFavorites" => Self::BrowserFavorites,
            "BrowserStop" => Self::BrowserStop,

            "MediaPlayPause" => Self::MediaPlayPause,
            "MediaTrackNext" => Self::MediaTrackNext,
            "MediaTrackPrevious" => Self::MediaTrackPrevious,
            "MediaStop" => Self::MediaStop,
            "AudioVolumeMute" => Self::AudioVolumeMute,
            "AudioVolumeDown" => Self::AudioVolumeDown,
            "AudioVolumeUp" => Self::AudioVolumeUp,

            "LaunchMail" => Self::LaunchMail,
            "LaunchApp1" => Self::LaunchApp1,
            "LaunchApp2" => Self::LaunchApp2,

            "ShiftLeft" => Self::ShiftLeft,
            "ShiftRight" => Self::ShiftRight,
            "ControlLeft" => Self::ControlLeft,
            "ControlRight" => Self::ControlRight,
            "AltLeft" => Self::AltLeft,
            "AltRight" => Self::AltRight,

            "SuperLeft" | "MetaLeft" | "OSLeft" => Self::SuperLeft,
            "SuperRight" | "MetaRight" | "OSRight" => Self::SuperRight,

            "IntlBackslash" => Self::IntlBackslash,

            "CapsLock" => Self::CapsLock,
            "NumLock" => Self::NumLock,
            "ScrollLock" => Self::ScrollLock,
            "PrintScreen" => Self::PrintScreen,
            "Pause" => Self::Pause,
            "ContextMenu" | "Menu" | "Apps" => Self::Menu,

            "Fn" | "FnLock" | "Globe" => Self::Fn,
            "Eject" => Self::Eject,
            "Help" => Self::Help,
            "Power" => Self::Power,
            "Sleep" | "Standby" => Self::Sleep,
            "Clear" => Self::Clear,

            _ => return None,
        })
    }

    /// egui's canonical name of the key.
    pub fn name(self) -> &'static str {
        match self {
            Self::ArrowDown => "Down",
            Self::ArrowLeft => "Left",
            Self::ArrowRight => "Right",
            Self::ArrowUp => "Up",

            Self::Escape => "Escape",
            Self::Tab => "Tab",
            Self::Backspace => "Backspace",
            Self::Enter => "Enter",

            Self::Insert => "Insert",
            Self::Delete => "Delete",
            Self::Home => "Home",
            Self::End => "End",
            Self::PageUp => "PageUp",
            Self::PageDown => "PageDown",

            Self::Copy => "Copy",
            Self::Cut => "Cut",
            Self::Paste => "Paste",

            Self::Space => "Space",
            Self::Colon => "Colon",
            Self::Comma => "Comma",
            Self::Minus => "Minus",
            Self::Period => "Period",
            Self::Plus => "Plus",
            Self::Equals => "Equals",
            Self::Semicolon => "Semicolon",
            Self::Backslash => "Backslash",
            Self::Slash => "Slash",
            Self::Pipe => "Pipe",
            Self::Questionmark => "Questionmark",
            Self::Exclamationmark => "Exclamationmark",
            Self::OpenBracket => "OpenBracket",
            Self::CloseBracket => "CloseBracket",
            Self::OpenCurlyBracket => "OpenCurlyBracket",
            Self::CloseCurlyBracket => "CloseCurlyBracket",
            Self::Backtick => "Backtick",
            Self::Quote => "Quote",

            Self::Num0 => "0",
            Self::Num1 => "1",
            Self::Num2 => "2",
            Self::Num3 => "3",
            Self::Num4 => "4",
            Self::Num5 => "5",
            Self::Num6 => "6",
            Self::Num7 => "7",
            Self::Num8 => "8",
            Self::Num9 => "9",

            Self::A => "A",
            Self::B => "B",
            Self::C => "C",
            Self::D => "D",
            Self::E => "E",
            Self::F => "F",
            Self::G => "G",
            Self::H => "H",
            Self::I => "I",
            Self::J => "J",
            Self::K => "K",
            Self::L => "L",
            Self::M => "M",
            Self::N => "N",
            Self::O => "O",
            Self::P => "P",
            Self::Q => "Q",
            Self::R => "R",
            Self::S => "S",
            Self::T => "T",
            Self::U => "U",
            Self::V => "V",
            Self::W => "W",
            Self::X => "X",
            Self::Y => "Y",
            Self::Z => "Z",
            Self::F1 => "F1",
            Self::F2 => "F2",
            Self::F3 => "F3",
            Self::F4 => "F4",
            Self::F5 => "F5",
            Self::F6 => "F6",
            Self::F7 => "F7",
            Self::F8 => "F8",
            Self::F9 => "F9",
            Self::F10 => "F10",
            Self::F11 => "F11",
            Self::F12 => "F12",
            Self::F13 => "F13",
            Self::F14 => "F14",
            Self::F15 => "F15",
            Self::F16 => "F16",
            Self::F17 => "F17",
            Self::F18 => "F18",
            Self::F19 => "F19",
            Self::F20 => "F20",
            Self::F21 => "F21",
            Self::F22 => "F22",
            Self::F23 => "F23",
            Self::F24 => "F24",
            Self::F25 => "F25",
            Self::F26 => "F26",
            Self::F27 => "F27",
            Self::F28 => "F28",
            Self::F29 => "F29",
            Self::F30 => "F30",
            Self::F31 => "F31",
            Self::F32 => "F32",
            Self::F33 => "F33",
            Self::F34 => "F34",
            Self::F35 => "F35",

            Self::BrowserBack => "BrowserBack",
            Self::BrowserForward => "BrowserForward",
            Self::BrowserRefresh => "BrowserRefresh",
            Self::BrowserSearch => "BrowserSearch",
            Self::BrowserHome => "BrowserHome",
            Self::BrowserFavorites => "BrowserFavorites",
            Self::BrowserStop => "BrowserStop",

            Self::MediaPlayPause => "MediaPlayPause",
            Self::MediaTrackNext => "MediaTrackNext",
            Self::MediaTrackPrevious => "MediaTrackPrevious",
            Self::MediaStop => "MediaStop",
            Self::AudioVolumeMute => "AudioVolumeMute",
            Self::AudioVolumeDown => "AudioVolumeDown",
            Self::AudioVolumeUp => "AudioVolumeUp",

            Self::LaunchMail => "LaunchMail",
            Self::LaunchApp1 => "LaunchApp1",
            Self::LaunchApp2 => "LaunchApp2",

            Self::ShiftLeft => "ShiftLeft",
            Self::ShiftRight => "ShiftRight",
            Self::ControlLeft => "ControlLeft",
            Self::ControlRight => "ControlRight",
            Self::AltLeft => "AltLeft",
            Self::AltRight => "AltRight",
            Self::SuperLeft => "SuperLeft",
            Self::SuperRight => "SuperRight",

            Self::IntlBackslash => "IntlBackslash",

            Self::CapsLock => "CapsLock",
            Self::NumLock => "NumLock",
            Self::ScrollLock => "ScrollLock",
            Self::PrintScreen => "PrintScreen",
            Self::Pause => "Pause",
            Self::Menu => "Menu",

            Self::Fn => "Fn",
            Self::Eject => "Eject",
            Self::Help => "Help",
            Self::Power => "Power",
            Self::Sleep => "Sleep",
            Self::Clear => "Clear",
        }
    }
}
