//! How text fields behave per platform: the text rows of svelters-egui's FX1 `Policy`
//! (`svelters-egui/src/native/mod.rs`, measured by WG3 against GTK 4, Qt 6, Win32 and AppKit), in
//! quire-text's vocabulary (word rules, undo grouping, PRIMARY), plus the key map flavour and the
//! password mask.

use quire_text::{EditRules, PrimaryRule, UndoRules, WordRules};

/// Whose text-field behaviour a field follows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Platform {
    /// GTK 4 (`GtkText`), the Linux default (reference `conformance/golden/linux/gtk-4.24.1.json`).
    Gtk,
    /// Qt 6 widgets (`QLineEdit`, reference `conformance/golden/linux/qt-6.12.0.json`).
    Qt,
    /// Windows (FX1's row: Qt-like word stops, until WG3w's references pin more).
    Windows,
    /// macOS (FX1's row: Option word motions, Cmd line and document motions).
    Mac,
}

impl Platform {
    /// The platform this build runs on (GTK's behaviour on Linux and the BSDs).
    pub const fn current() -> Platform {
        if cfg!(target_os = "macos") {
            Platform::Mac
        } else if cfg!(windows) {
            Platform::Windows
        } else {
            Platform::Gtk
        }
    }

    pub const fn is_mac(self) -> bool {
        matches!(self, Platform::Mac)
    }
}

/// One platform's text-field behaviour. `#[non_exhaustive]`: built from [`TextPolicy::of`] and then
/// changed field by field (the `styleHint` equivalent).
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub struct TextPolicy {
    pub platform: Platform,
    /// Word movement and deletion rules (quire-text's `WordRules::Pango` / `Qt`).
    pub words: NativeWords,
    /// Word movement without Shift on a selection collapses it to its edge without moving (GTK).
    pub word_move_collapses: bool,
    /// Keyboard focus (Tab, Shift+Tab) into a single-line field selects all of it, caret at the end.
    pub select_on_keyboard_focus: bool,
    /// Programmatic focus selects all too (GTK's `grab_focus`; Qt's `setFocus` does not).
    pub select_on_programmatic_focus: bool,
    /// An unfocused field keeps its selection (GTK, AppKit); otherwise it collapses to the caret.
    pub keep_selection_unfocused: bool,
    /// Only one field holds a selection: a field that selects collapses the previous owner's to its
    /// start (GTK on X11, the PRIMARY selection changing hands).
    pub one_selection_owner: bool,
    /// When the PRIMARY selection is written (X11 and Wayland only).
    pub primary: PrimaryRule,
    /// A middle click pastes PRIMARY at the click (X11 and Wayland).
    pub middle_click_paste: bool,
    /// Undo grouping (FX3: GTK's and Qt's measured models; Monaco's elsewhere until measured).
    pub undo: UndoRules,
    /// Ctrl+Shift+U starts GTK's hex Unicode entry.
    pub hex_entry: bool,
    /// A full caret blink cycle in ms; 0 for no blink.
    pub caret_blink_ms: u32,
    /// The caret stops blinking after this long without input (GTK `gtk-cursor-blink-timeout`).
    pub caret_blink_timeout_ms: Option<u32>,
    /// The longest gap between the clicks of a double or triple click.
    pub double_click_ms: u32,
    /// What a password field shows for each character.
    pub mask: char,
    /// The host's layer over this platform's text keys (S2); `None`: the platform's keys as they are.
    pub keys: Option<std::sync::Arc<super::keys::TextKeymap>>,
}

/// The word rules a platform uses (quire-text's `WordRules` minus the VS Code classifier, which
/// fields do not use; `Copy`, so the policy stays `Copy`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NativeWords {
    Pango,
    Qt,
}

impl NativeWords {
    pub fn rules(self) -> WordRules {
        match self {
            NativeWords::Pango => WordRules::Pango,
            NativeWords::Qt => WordRules::Qt,
        }
    }
}

const GTK: TextPolicy = TextPolicy {
    platform: Platform::Gtk,
    words: NativeWords::Pango,
    word_move_collapses: true,
    select_on_keyboard_focus: true,
    select_on_programmatic_focus: true,
    keep_selection_unfocused: true,
    one_selection_owner: true,
    primary: PrimaryRule::SetAndClear,
    middle_click_paste: true,
    undo: UndoRules::Gtk,
    hex_entry: true,
    caret_blink_ms: 1200,
    caret_blink_timeout_ms: Some(10_000),
    double_click_ms: 400,
    // GtkText's default invisible char, U+25CF BLACK CIRCLE
    mask: '\u{25CF}',
    keys: None,
};

const QT: TextPolicy = TextPolicy {
    platform: Platform::Qt,
    words: NativeWords::Qt,
    word_move_collapses: false,
    select_on_keyboard_focus: true,
    select_on_programmatic_focus: false,
    keep_selection_unfocused: false,
    one_selection_owner: false,
    primary: PrimaryRule::Set,
    middle_click_paste: true,
    undo: UndoRules::Qt,
    hex_entry: false,
    caret_blink_ms: 1000,
    caret_blink_timeout_ms: None,
    double_click_ms: 400,
    mask: '\u{25CF}',
    keys: None,
};

const WINDOWS: TextPolicy = TextPolicy {
    platform: Platform::Windows,
    words: NativeWords::Qt,
    word_move_collapses: false,
    select_on_keyboard_focus: true,
    select_on_programmatic_focus: false,
    keep_selection_unfocused: false,
    one_selection_owner: false,
    primary: PrimaryRule::Off,
    middle_click_paste: false,
    undo: UndoRules::Monaco,
    hex_entry: false,
    caret_blink_ms: 1060,
    caret_blink_timeout_ms: None,
    double_click_ms: 500,
    mask: '\u{25CF}',
    keys: None,
};

const MAC: TextPolicy = TextPolicy {
    platform: Platform::Mac,
    words: NativeWords::Qt,
    word_move_collapses: false,
    select_on_keyboard_focus: true,
    select_on_programmatic_focus: true,
    keep_selection_unfocused: true,
    one_selection_owner: false,
    primary: PrimaryRule::Off,
    middle_click_paste: false,
    undo: UndoRules::Monaco,
    hex_entry: false,
    caret_blink_ms: 1060,
    caret_blink_timeout_ms: None,
    double_click_ms: 500,
    // NSSecureTextField's bullet
    mask: '\u{2022}',
    keys: None,
};

impl TextPolicy {
    pub const fn of(p: Platform) -> TextPolicy {
        match p {
            Platform::Gtk => GTK,
            Platform::Qt => QT,
            Platform::Windows => WINDOWS,
            Platform::Mac => MAC,
        }
    }

    /// The text rows of the widget substrate's platform policy (`crate::interact::Policy`, the same FX1
    /// table), over this platform's word rules, undo grouping, PRIMARY and mask.
    pub fn from_policy(platform: Platform, p: &crate::interact::Policy) -> TextPolicy {
        TextPolicy {
            word_move_collapses: p.word_move_collapses,
            select_on_keyboard_focus: p.select_on_keyboard_focus,
            select_on_programmatic_focus: p.select_on_programmatic_focus,
            keep_selection_unfocused: p.keep_selection_unfocused,
            one_selection_owner: p.one_selection_owner,
            caret_blink_ms: p.caret_blink_ms,
            double_click_ms: p.double_click_ms,
            ..TextPolicy::of(platform)
        }
    }

    /// quire-text's command rules for a field under this policy.
    pub fn edit_rules(&self, secret: bool) -> EditRules {
        let mut r = EditRules::field(self.words.rules());
        r.word_move_collapses = self.word_move_collapses;
        r.secret = secret;
        r.primary = self.primary;
        r
    }
}

impl Default for TextPolicy {
    fn default() -> Self {
        TextPolicy::of(Platform::current())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fx1_rows_carry_over() {
        let (g, q) = (TextPolicy::of(Platform::Gtk), TextPolicy::of(Platform::Qt));
        assert_ne!(g.words, q.words);
        assert!(g.keep_selection_unfocused && !q.keep_selection_unfocused);
        assert!(g.word_move_collapses && !q.word_move_collapses);
        assert!(g.select_on_programmatic_focus && !q.select_on_programmatic_focus);
        let r = g.edit_rules(true);
        assert!(r.secret && r.word_move_collapses && !r.smart_home);
        assert_eq!(TextPolicy::of(Platform::Mac).primary, PrimaryRule::Off);
    }
}
