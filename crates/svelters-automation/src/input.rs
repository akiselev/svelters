//! The input a session injects ([`Event`]), egui-shaped so svelters-egui's drivers convert without
//! loss (feature `egui`: [`Event`] into `egui::Event`) and other drivers (skelter-test's) translate it
//! to their platform's input without egui. Points are the scene's.

use crate::geom::{Pos2, Vec2};
use accesskit::ActionRequest;
use std::ops::{BitOr, BitOrAssign};

/// One injected input event.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// The pointer moved to a point.
    PointerMoved(Pos2),
    /// A button went down or up at a point.
    PointerButton { pos: Pos2, button: PointerButton, pressed: bool, modifiers: Modifiers },
    /// A wheel scroll where the pointer is: `delta` in `unit`s (positive `y` scrolls up, as egui
    /// sends it).
    MouseWheel { unit: MouseWheelUnit, delta: Vec2, modifiers: Modifiers },
    /// The held modifiers changed.
    ModifiersChanged(Modifiers),
    /// A key went down or up.
    Key { key: Key, pressed: bool, repeat: bool, modifiers: Modifiers },
    /// Text typed (or committed by an input method).
    Text(String),
    /// An AccessKit action request (the semantic tier).
    AccessKit(ActionRequest),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PointerButton {
    Primary,
    Secondary,
    Middle,
    Extra1,
    Extra2,
}

/// The unit of a wheel delta.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MouseWheelUnit {
    /// Points (a trackpad's pixels).
    Point,
    /// Lines (a mouse wheel's notches).
    Line,
    /// Pages.
    Page,
}

/// Held modifier keys, as egui's: `command` is the platform's shortcut modifier (Cmd on macOS,
/// Ctrl elsewhere); `mac_cmd` is the Cmd key itself.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Modifiers {
    pub alt: bool,
    pub ctrl: bool,
    pub shift: bool,
    pub mac_cmd: bool,
    pub command: bool,
}

impl Modifiers {
    pub const NONE: Modifiers = Modifiers { alt: false, ctrl: false, shift: false, mac_cmd: false, command: false };
    pub const ALT: Modifiers = Modifiers { alt: true, ..Modifiers::NONE };
    pub const CTRL: Modifiers = Modifiers { ctrl: true, ..Modifiers::NONE };
    pub const SHIFT: Modifiers = Modifiers { shift: true, ..Modifiers::NONE };
    pub const MAC_CMD: Modifiers = Modifiers { mac_cmd: true, ..Modifiers::NONE };
    pub const COMMAND: Modifiers = Modifiers { command: true, ..Modifiers::NONE };

    pub fn is_none(&self) -> bool {
        *self == Modifiers::NONE
    }
}

impl BitOr for Modifiers {
    type Output = Modifiers;
    fn bitor(self, o: Modifiers) -> Modifiers {
        Modifiers {
            alt: self.alt || o.alt,
            ctrl: self.ctrl || o.ctrl,
            shift: self.shift || o.shift,
            mac_cmd: self.mac_cmd || o.mac_cmd,
            command: self.command || o.command,
        }
    }
}

impl BitOrAssign for Modifiers {
    fn bitor_assign(&mut self, o: Modifiers) {
        *self = *self | o;
    }
}

/// A logical key, by its egui name (`"Enter"`, `"A"`, `"Tab"`, `"F5"`; skelter's key table uses the
/// same names). [`Key::from_name`] takes egui's names and aliases (`"Return"`, `"ArrowDown"`, `"-"`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Key(&'static str);

impl Key {
    pub const A: Key = Key("A");
    pub const ENTER: Key = Key("Enter");
    pub const ESCAPE: Key = Key("Escape");
    pub const TAB: Key = Key("Tab");

    /// The key one of egui's names or aliases names (`None`: no such key).
    pub fn from_name(name: &str) -> Option<Key> {
        KEYS.iter().find(|(_, aliases)| aliases.contains(&name)).map(|(n, _)| Key(n))
    }

    /// egui's name of the key (what [`Key::from_name`] takes back).
    pub fn name(self) -> &'static str {
        self.0
    }
}

/// egui 0.37's `Key::from_name` table: each key's name (`Key::name`) and the names it is parsed from.
const KEYS: &[(&str, &[&str])] = &[
    ("Down", &["⏷", "ArrowDown", "Down"]),
    ("Left", &["⏴", "ArrowLeft", "Left"]),
    ("Right", &["⏵", "ArrowRight", "Right"]),
    ("Up", &["⏶", "ArrowUp", "Up"]),
    ("Escape", &["Escape", "Esc"]),
    ("Tab", &["Tab"]),
    ("Backspace", &["Backspace"]),
    ("Enter", &["Enter", "Return", "NumpadEnter"]),
    ("Insert", &["Insert"]),
    ("Delete", &["Delete"]),
    ("Home", &["Home"]),
    ("End", &["End"]),
    ("PageUp", &["PageUp"]),
    ("PageDown", &["PageDown"]),
    ("Copy", &["Copy"]),
    ("Cut", &["Cut"]),
    ("Paste", &["Paste"]),
    ("Space", &[" ", "Space"]),
    ("Colon", &[":", "Colon"]),
    ("Comma", &[",", "Comma", "NumpadComma"]),
    ("Minus", &["-", "−", "Minus", "NumpadSubtract"]),
    ("Period", &[".", "Period", "NumpadDecimal"]),
    ("Plus", &["+", "Plus", "NumpadAdd"]),
    ("Equals", &["=", "Equal", "Equals", "NumpadEqual"]),
    ("Semicolon", &[";", "Semicolon"]),
    ("Backslash", &["\\", "Backslash"]),
    ("Slash", &["/", "Slash", "NumpadDivide"]),
    ("Pipe", &["|", "Pipe"]),
    ("Questionmark", &["?", "Questionmark"]),
    ("Exclamationmark", &["!", "Exclamationmark"]),
    ("OpenBracket", &["[", "OpenBracket", "BracketLeft"]),
    ("CloseBracket", &["]", "CloseBracket", "BracketRight"]),
    ("OpenCurlyBracket", &["{", "OpenCurlyBracket"]),
    ("CloseCurlyBracket", &["}", "CloseCurlyBracket"]),
    ("Backtick", &["`", "Backtick", "Backquote", "Grave"]),
    ("Quote", &["'", "Quote"]),
    ("0", &["0", "Digit0", "Numpad0"]),
    ("1", &["1", "Digit1", "Numpad1"]),
    ("2", &["2", "Digit2", "Numpad2"]),
    ("3", &["3", "Digit3", "Numpad3"]),
    ("4", &["4", "Digit4", "Numpad4"]),
    ("5", &["5", "Digit5", "Numpad5"]),
    ("6", &["6", "Digit6", "Numpad6"]),
    ("7", &["7", "Digit7", "Numpad7"]),
    ("8", &["8", "Digit8", "Numpad8"]),
    ("9", &["9", "Digit9", "Numpad9"]),
    ("A", &["a", "A", "KeyA"]),
    ("B", &["b", "B", "KeyB"]),
    ("C", &["c", "C", "KeyC"]),
    ("D", &["d", "D", "KeyD"]),
    ("E", &["e", "E", "KeyE"]),
    ("F", &["f", "F", "KeyF"]),
    ("G", &["g", "G", "KeyG"]),
    ("H", &["h", "H", "KeyH"]),
    ("I", &["i", "I", "KeyI"]),
    ("J", &["j", "J", "KeyJ"]),
    ("K", &["k", "K", "KeyK"]),
    ("L", &["l", "L", "KeyL"]),
    ("M", &["m", "M", "KeyM"]),
    ("N", &["n", "N", "KeyN"]),
    ("O", &["o", "O", "KeyO"]),
    ("P", &["p", "P", "KeyP"]),
    ("Q", &["q", "Q", "KeyQ"]),
    ("R", &["r", "R", "KeyR"]),
    ("S", &["s", "S", "KeyS"]),
    ("T", &["t", "T", "KeyT"]),
    ("U", &["u", "U", "KeyU"]),
    ("V", &["v", "V", "KeyV"]),
    ("W", &["w", "W", "KeyW"]),
    ("X", &["x", "X", "KeyX"]),
    ("Y", &["y", "Y", "KeyY"]),
    ("Z", &["z", "Z", "KeyZ"]),
    ("F1", &["F1"]),
    ("F2", &["F2"]),
    ("F3", &["F3"]),
    ("F4", &["F4"]),
    ("F5", &["F5"]),
    ("F6", &["F6"]),
    ("F7", &["F7"]),
    ("F8", &["F8"]),
    ("F9", &["F9"]),
    ("F10", &["F10"]),
    ("F11", &["F11"]),
    ("F12", &["F12"]),
    ("F13", &["F13"]),
    ("F14", &["F14"]),
    ("F15", &["F15"]),
    ("F16", &["F16"]),
    ("F17", &["F17"]),
    ("F18", &["F18"]),
    ("F19", &["F19"]),
    ("F20", &["F20"]),
    ("F21", &["F21"]),
    ("F22", &["F22"]),
    ("F23", &["F23"]),
    ("F24", &["F24"]),
    ("F25", &["F25"]),
    ("F26", &["F26"]),
    ("F27", &["F27"]),
    ("F28", &["F28"]),
    ("F29", &["F29"]),
    ("F30", &["F30"]),
    ("F31", &["F31"]),
    ("F32", &["F32"]),
    ("F33", &["F33"]),
    ("F34", &["F34"]),
    ("F35", &["F35"]),
    ("BrowserBack", &["BrowserBack"]),
    ("BrowserForward", &["BrowserForward"]),
    ("BrowserRefresh", &["BrowserRefresh"]),
    ("BrowserSearch", &["BrowserSearch"]),
    ("BrowserHome", &["BrowserHome"]),
    ("BrowserFavorites", &["BrowserFavorites"]),
    ("BrowserStop", &["BrowserStop"]),
    ("MediaPlayPause", &["MediaPlayPause"]),
    ("MediaTrackNext", &["MediaTrackNext"]),
    ("MediaTrackPrevious", &["MediaTrackPrevious"]),
    ("MediaStop", &["MediaStop"]),
    ("AudioVolumeMute", &["AudioVolumeMute"]),
    ("AudioVolumeDown", &["AudioVolumeDown"]),
    ("AudioVolumeUp", &["AudioVolumeUp"]),
    ("LaunchMail", &["LaunchMail"]),
    ("LaunchApp1", &["LaunchApp1"]),
    ("LaunchApp2", &["LaunchApp2"]),
    ("ShiftLeft", &["ShiftLeft"]),
    ("ShiftRight", &["ShiftRight"]),
    ("ControlLeft", &["ControlLeft"]),
    ("ControlRight", &["ControlRight"]),
    ("AltLeft", &["AltLeft"]),
    ("AltRight", &["AltRight"]),
    ("SuperLeft", &["SuperLeft", "MetaLeft", "OSLeft"]),
    ("SuperRight", &["SuperRight", "MetaRight", "OSRight"]),
    ("IntlBackslash", &["IntlBackslash"]),
    ("CapsLock", &["CapsLock"]),
    ("NumLock", &["NumLock"]),
    ("ScrollLock", &["ScrollLock"]),
    ("PrintScreen", &["PrintScreen"]),
    ("Pause", &["Pause"]),
    ("Menu", &["ContextMenu", "Menu", "Apps"]),
    ("Fn", &["Fn", "FnLock", "Globe"]),
    ("Eject", &["Eject"]),
    ("Help", &["Help"]),
    ("Power", &["Power"]),
    ("Sleep", &["Sleep", "Standby"]),
    ("Clear", &["Clear"]),
];

#[cfg(feature = "egui")]
mod egui_conv {
    use super::*;

    impl From<Modifiers> for egui::Modifiers {
        fn from(m: Modifiers) -> egui::Modifiers {
            egui::Modifiers { alt: m.alt, ctrl: m.ctrl, shift: m.shift, mac_cmd: m.mac_cmd, command: m.command }
        }
    }
    impl From<egui::Modifiers> for Modifiers {
        fn from(m: egui::Modifiers) -> Modifiers {
            Modifiers { alt: m.alt, ctrl: m.ctrl, shift: m.shift, mac_cmd: m.mac_cmd, command: m.command }
        }
    }
    impl From<PointerButton> for egui::PointerButton {
        fn from(b: PointerButton) -> egui::PointerButton {
            match b {
                PointerButton::Primary => egui::PointerButton::Primary,
                PointerButton::Secondary => egui::PointerButton::Secondary,
                PointerButton::Middle => egui::PointerButton::Middle,
                PointerButton::Extra1 => egui::PointerButton::Extra1,
                PointerButton::Extra2 => egui::PointerButton::Extra2,
            }
        }
    }
    impl From<egui::PointerButton> for PointerButton {
        fn from(b: egui::PointerButton) -> PointerButton {
            match b {
                egui::PointerButton::Primary => PointerButton::Primary,
                egui::PointerButton::Secondary => PointerButton::Secondary,
                egui::PointerButton::Middle => PointerButton::Middle,
                egui::PointerButton::Extra1 => PointerButton::Extra1,
                egui::PointerButton::Extra2 => PointerButton::Extra2,
            }
        }
    }
    impl From<MouseWheelUnit> for egui::MouseWheelUnit {
        fn from(u: MouseWheelUnit) -> egui::MouseWheelUnit {
            match u {
                MouseWheelUnit::Point => egui::MouseWheelUnit::Point,
                MouseWheelUnit::Line => egui::MouseWheelUnit::Line,
                MouseWheelUnit::Page => egui::MouseWheelUnit::Page,
            }
        }
    }
    impl From<egui::MouseWheelUnit> for MouseWheelUnit {
        fn from(u: egui::MouseWheelUnit) -> MouseWheelUnit {
            match u {
                egui::MouseWheelUnit::Point => MouseWheelUnit::Point,
                egui::MouseWheelUnit::Line => MouseWheelUnit::Line,
                egui::MouseWheelUnit::Page => MouseWheelUnit::Page,
            }
        }
    }
    impl From<Key> for egui::Key {
        fn from(k: Key) -> egui::Key {
            egui::Key::from_name(k.name()).expect("the key table is egui's")
        }
    }
    impl From<egui::Key> for Key {
        fn from(k: egui::Key) -> Key {
            Key::from_name(k.name()).expect("the key table is egui's")
        }
    }

    impl From<Event> for egui::Event {
        fn from(e: Event) -> egui::Event {
            match e {
                Event::PointerMoved(p) => egui::Event::PointerMoved(p.into()),
                Event::PointerButton { pos, button, pressed, modifiers } => {
                    egui::Event::PointerButton { pos: pos.into(), button: button.into(), pressed, modifiers: modifiers.into() }
                }
                Event::MouseWheel { unit, delta, modifiers } => egui::Event::MouseWheel {
                    unit: unit.into(),
                    delta: delta.into(),
                    phase: egui::TouchPhase::Move,
                    source: egui::MouseWheelSource::Unknown,
                    modifiers: modifiers.into(),
                },
                Event::ModifiersChanged(m) => egui::Event::ModifiersChanged(m.into()),
                Event::Key { key, pressed, repeat, modifiers } => {
                    egui::Event::Key { key: key.into(), physical_key: None, pressed, repeat, modifiers: modifiers.into() }
                }
                Event::Text(t) => egui::Event::Text(t),
                Event::AccessKit(r) => egui::Event::AccessKitActionRequest(r),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_parse_egui_names_and_aliases() {
        assert_eq!(Key::from_name("Return"), Some(Key::ENTER));
        assert_eq!(Key::from_name("ArrowDown").map(Key::name), Some("Down"));
        assert_eq!(Key::from_name("-").map(Key::name), Some("Minus"));
        assert_eq!(Key::from_name("a").map(Key::name), Some("A"));
        assert_eq!(Key::from_name("NoSuchKey"), None);
        for (name, aliases) in KEYS {
            assert_eq!(Key::from_name(name).map(Key::name), Some(*name), "{name} parses back");
            assert!(!aliases.is_empty());
        }
    }

    #[cfg(feature = "egui")]
    #[test]
    fn the_key_table_is_eguis() {
        for k in egui::Key::ALL {
            assert_eq!(Key::from(*k).name(), k.name());
        }
        for (name, aliases) in KEYS {
            for a in *aliases {
                assert_eq!(egui::Key::from_name(a).map(egui::Key::name), Some(*name), "alias {a:?}");
            }
        }
    }
}
