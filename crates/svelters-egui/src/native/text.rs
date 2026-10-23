//! OW4: the text-field half of the platform policy (ARCH-WIDGETS §4.4, W1/W2/W6): which TC0
//! (`quire-text`) rule sets svelters' own `text-edit` runs with on each platform, and the word
//! boundaries quire-text leaves to the host.
//!
//! The values come from the native references: GTK 4.24.1 and Qt 6.12.0 (WG3, FX1, FX3), AppKit
//! NSTextField 15.7.5 (WG3m) and Win32/WinUI (WG3w). quire-text holds the shared rules (Pango and
//! Qt word stops, the GTK and Qt undo models: FX1's `native/words.rs` and FX3's `native/undo.rs`
//! moved there); this module holds what the references showed beyond them:
//!
//! - **double-click words** ([`GTK_WORDS`], [`QT_WORDS`], [`MAC_WORDS`]): TC0 derived `word_at`
//!   for Pango and Qt without a measurement (its open question 11). WG3's pin 3 (`multi-click`)
//!   measures it: GTK selects from the previous word boundary to the next one, start or end
//!   (`gtk_text_select_word`: `bar-` for a click on `-`, `foo ` for a click on a space); Qt
//!   selects the word run at the click less its trailing spaces (`selectWordAtPos`: `-` alone,
//!   `foo` for the space after it); AppKit selects the word, or the one character between words.
//! - **macOS word stops**: Pango's (word ends forward, word starts backward, dictionary CJK) with
//!   `.`, `_` and `'` between letters or digits inside the word (`baz.qux`, `snake_case`, `3.14`).

use egui::os::OperatingSystem;
use quire_text::words::native::NativeWords;
use quire_text::{LineBreaks, PrimaryRule, Text, UndoRules, WordBoundaries, WordRules, WordStop};
use std::ops::Range;

/// Whose word rules a field follows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextWords {
    /// GTK over Pango's word attributes; double-click as `gtk_text_select_word`.
    Gtk,
    /// Qt's `SkipWords`; double-click as `selectWordAtPos`.
    Qt,
    /// AppKit (NSTextField): Pango-like stops with `.` `_` `'` inside words.
    Mac,
    /// VS Code's word classifier (egui's own fields had no native rules; `Native::Egui`).
    Code,
}

impl TextWords {
    /// The quire-text rules (cached by the caller in its `EditRules`).
    pub fn rules(self) -> WordRules {
        match self {
            TextWords::Gtk => WordRules::Custom(&GTK_WORDS),
            TextWords::Qt => WordRules::Custom(&QT_WORDS),
            TextWords::Mac => WordRules::Custom(&MAC_WORDS),
            TextWords::Code => WordRules::default(),
        }
    }
}

/// What happens to an unfocused field's selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unfocused {
    /// It stays (GTK, NSTextView: each field shows its own).
    Keep,
    /// It collapses to the caret (Qt, Win32, egui).
    Collapse,
    /// It goes, caret at the start (NSTextField: an unfocused field has no field editor).
    Clear,
}

/// The text-field rules of a platform (`Policy::text`).
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct TextPolicy {
    /// Undo grouping (quire-text's GTK, Qt or Monaco model).
    pub undo: UndoRules,
    /// Word stops and double-click words.
    pub words: TextWords,
    /// Line breaks pasted into a one-line field (GTK, Qt and AppKit keep them).
    pub paste_breaks: LineBreaks,
    /// When the PRIMARY selection is written (GTK: set and cleared; Qt: set; elsewhere none).
    pub primary: PrimaryRule,
    /// A middle click pastes PRIMARY at the pointer (X11, Wayland); otherwise it does nothing,
    /// not even focus (AppKit).
    pub middle_click_paste: bool,
    /// GTK's Ctrl+Shift+U hex Unicode entry.
    pub hex_entry: bool,
    /// The IBM CUA chords: Ctrl+Insert copies, Shift+Insert pastes, Shift+Delete cuts.
    pub cua_keys: bool,
    /// macOS keys: Option moves and deletes by word, Cmd+Left/Right to the line ends, Cmd+Up/Down
    /// to the text's ends (else Ctrl by word, Home/End, Ctrl+Home/End).
    pub mac_keys: bool,
    /// A double-click drag extends by words (GTK, AppKit, WinUI); otherwise by characters from
    /// the word's start (Qt).
    pub word_drag: bool,
    /// The unfocused field's selection.
    pub unfocused: Unfocused,
    /// Enter in a one-line field selects all of it (NSTextField ends editing so).
    pub return_selects_all: bool,
    /// After select-all, Shift+Left/Right keeps everything selected and only fixes the anchor at
    /// the far end (AppKit's undetermined direction).
    pub select_all_undirected: bool,
    /// Shift+word motion that would cross the anchor stops at it first (AppKit, WG3m pin 2).
    pub word_extend_stops_at_anchor: bool,
}

pub(crate) const GTK: TextPolicy = TextPolicy {
    undo: UndoRules::Gtk,
    words: TextWords::Gtk,
    paste_breaks: LineBreaks::Keep,
    primary: PrimaryRule::SetAndClear,
    middle_click_paste: true,
    hex_entry: true,
    cua_keys: true,
    mac_keys: false,
    word_drag: true,
    unfocused: Unfocused::Keep,
    return_selects_all: false,
    select_all_undirected: false,
    word_extend_stops_at_anchor: false,
};

pub(crate) const QT: TextPolicy = TextPolicy {
    undo: UndoRules::Qt,
    words: TextWords::Qt,
    paste_breaks: LineBreaks::Keep,
    primary: PrimaryRule::Set,
    middle_click_paste: true,
    hex_entry: false,
    cua_keys: true,
    mac_keys: false,
    word_drag: false,
    unfocused: Unfocused::Collapse,
    return_selects_all: false,
    select_all_undirected: false,
    word_extend_stops_at_anchor: false,
};

/// WG3w: Win32 `EDIT` and WinUI `TextBox`; WinUI's word and drag rules (the modern control), Qt's
/// undo model until a Windows golden pins pins 4-6.
pub(crate) const WINDOWS: TextPolicy = TextPolicy {
    undo: UndoRules::Qt,
    words: TextWords::Qt,
    paste_breaks: LineBreaks::Keep,
    primary: PrimaryRule::Off,
    middle_click_paste: false,
    hex_entry: false,
    cua_keys: true,
    mac_keys: false,
    word_drag: true,
    unfocused: Unfocused::Collapse,
    return_selects_all: false,
    select_all_undirected: false,
    word_extend_stops_at_anchor: false,
};

/// WG3m: NSTextField (TextKit 1). Undo: one step per typing run, split at deletions, caret moves
/// and pastes, a run with spaces one step: Qt's model.
pub(crate) const MAC: TextPolicy = TextPolicy {
    undo: UndoRules::Qt,
    words: TextWords::Mac,
    paste_breaks: LineBreaks::Keep,
    primary: PrimaryRule::Off,
    middle_click_paste: false,
    hex_entry: false,
    cua_keys: false,
    mac_keys: true,
    word_drag: true,
    unfocused: Unfocused::Clear,
    return_selects_all: true,
    select_all_undirected: true,
    word_extend_stops_at_anchor: true,
};

/// `Native::Egui`: what egui's own `TextEdit` did (time-grouped undo, VS Code-like words, line
/// breaks turned into spaces), with the platform's keys.
pub(crate) const fn egui(os: OperatingSystem) -> TextPolicy {
    let nix = matches!(os, OperatingSystem::Nix);
    TextPolicy {
        undo: UndoRules::Monaco,
        words: TextWords::Code,
        paste_breaks: LineBreaks::Spaces,
        primary: if nix { PrimaryRule::Set } else { PrimaryRule::Off },
        middle_click_paste: nix,
        hex_entry: false,
        cua_keys: matches!(os, OperatingSystem::Windows),
        mac_keys: matches!(os, OperatingSystem::Mac),
        word_drag: false,
        unfocused: Unfocused::Collapse,
        return_selects_all: false,
        select_all_undirected: false,
        word_extend_stops_at_anchor: false,
    }
}

// ---- word boundaries ----

/// GTK: Pango's stops; double-click per `gtk_text_select_word`.
pub struct GtkWords;
/// Qt: `SkipWords`; double-click per `QWidgetLineControl::selectWordAtPos`.
pub struct QtWords;
/// AppKit: Pango-like stops with `.`, `_` and `'` inside words.
pub struct MacWords;

pub static GTK_WORDS: GtkWords = GtkWords;
pub static QT_WORDS: QtWords = QtWords;
pub static MAC_WORDS: MacWords = MacWords;

/// The chars of line `r` of `text` and the char column of byte `at` in it.
fn line_chars(text: &Text, r: &Range<usize>, at: usize) -> (Vec<char>, usize) {
    let s = text.slice(r.clone()).to_cow();
    let chars: Vec<char> = s.chars().collect();
    let col = text.byte_to_char(at.clamp(r.start, r.end)) - text.byte_to_char(r.start);
    (chars, col)
}

fn col_to_byte(text: &Text, r: &Range<usize>, col: usize) -> usize {
    text.char_to_byte(text.byte_to_char(r.start) + col).min(r.end)
}

/// A word motion over logical lines, as quire-text's native rules do: from a line's end forward
/// to the first stop of the next line, from a line's start back to the last stop of the previous
/// one (GtkTextView). `f(chars, col)` is the stop within one line.
fn across_lines(text: &Text, at: usize, forward: bool, f: impl Fn(&[char], usize) -> usize) -> usize {
    let mut line = text.byte_to_line(at);
    let mut r = text.line_range(line);
    let start = if forward {
        if at >= r.end {
            if line + 1 >= text.len_lines() {
                return text.len_bytes();
            }
            line += 1;
            r = text.line_range(line);
            if r.is_empty() {
                return r.start;
            }
            r.start
        } else {
            at
        }
    } else if at <= r.start {
        if line == 0 {
            return 0;
        }
        line -= 1;
        r = text.line_range(line);
        if r.is_empty() {
            return r.start;
        }
        r.end
    } else {
        at
    };
    let (chars, col) = line_chars(text, &r, start);
    col_to_byte(text, &r, f(&chars, col))
}

fn pango(chars: &[char], at: usize, forward: bool) -> usize {
    let s: String = chars.iter().collect();
    if forward { NativeWords::Pango.next(&s, at) } else { NativeWords::Pango.prev(&s, at) }
}

fn qt(chars: &[char], at: usize, forward: bool) -> usize {
    let s: String = chars.iter().collect();
    if forward { NativeWords::Qt.next(&s, at) } else { NativeWords::Qt.prev(&s, at) }
}

/// The word boundaries of a line under Pango (every word start and end), with 0 and the end.
fn pango_boundaries(chars: &[char]) -> Vec<usize> {
    let n = chars.len();
    let mut b = vec![0, n];
    for i in 0..=n {
        b.push(pango(chars, i, true));
        b.push(pango(chars, i, false));
    }
    b.sort_unstable();
    b.dedup();
    b
}

/// The word at byte `at` of `text` by a char-level rule over its line.
fn in_line_word(text: &Text, at: usize, f: impl Fn(&[char], usize) -> (usize, usize)) -> Range<usize> {
    let at = text.clamp_to_boundary(at.min(text.len_bytes()));
    let r = text.line_range(text.byte_to_line(at));
    let (chars, col) = line_chars(text, &r, at);
    let (a, b) = f(&chars, col);
    col_to_byte(text, &r, a)..col_to_byte(text, &r, b)
}

/// GTK's double click (`gtk_text_select_word`, measured by pin 3): from the last word boundary
/// (start or end) before the click to the first one after it.
pub fn gtk_word_at(chars: &[char], at: usize) -> (usize, usize) {
    let n = chars.len();
    let at = at.min(n);
    let b = pango_boundaries(chars);
    let lo = b.iter().copied().filter(|&x| x < at).max().unwrap_or(0);
    let hi = b.iter().copied().filter(|&x| x > at).min().unwrap_or(n);
    (lo, hi)
}

/// Qt's double click (`selectWordAtPos`, measured by pin 3): the `SkipWords` run before the
/// position after the click, to that run's next stop less trailing white space.
pub fn qt_word_at(chars: &[char], at: usize) -> (usize, usize) {
    let n = chars.len();
    let at = at.min(n);
    let next = (at + 1).min(n);
    let c = qt(chars, next, false);
    let mut end = qt(chars, c, true);
    while end > at && end > 0 && chars[end - 1].is_whitespace() {
        end -= 1;
    }
    (c.min(end), end.max(c))
}

fn word_char(c: char) -> bool {
    c.is_alphanumeric()
}

fn emoji(c: char) -> bool {
    matches!(c as u32, 0x1F000..=0x1FAFF | 0x2600..=0x27BF)
}

/// AppKit's text with the characters that join a word (`.`, `_`, `'` between letters or digits)
/// replaced by a letter, so Pango's stops see one word. Forward (`split_emoji`), each emoji is a
/// word of its own (`x😀y` is three stops: a katakana stand-in, which Pango splits from Latin by
/// script class); backward, Pango's own classes match AppKit (WG3m pin 1, both measured).
fn mac_mapped_with(chars: &[char], split_emoji: bool) -> Vec<char> {
    let mut out: Vec<char> = chars.iter().map(|&c| if emoji(c) && split_emoji { '\u{30A2}' } else { c }).collect();
    for i in 1..chars.len().saturating_sub(1) {
        if matches!(chars[i], '.' | '_' | '\'' | '\u{2019}') && word_char(chars[i - 1]) && word_char(chars[i + 1]) {
            out[i] = 'a';
        }
    }
    out
}

fn mac_mapped(chars: &[char]) -> Vec<char> {
    mac_mapped_with(chars, true)
}

/// AppKit's double click (pin 3): the word at the click; between words, the one character
/// (a run of white space).
pub fn mac_word_at(chars: &[char], at: usize) -> (usize, usize) {
    let n = chars.len();
    if n == 0 {
        return (0, 0);
    }
    let at = at.min(n - 1);
    let m = mac_mapped(chars);
    let c = chars[at];
    if word_char(m[at]) {
        let a = pango(&m, at + 1, false);
        let b = pango(&m, a, true);
        if a <= at && at < b {
            return (a, b);
        }
        return (at, at + 1);
    }
    if c.is_whitespace() {
        let mut a = at;
        while a > 0 && chars[a - 1].is_whitespace() {
            a -= 1;
        }
        let mut b = at + 1;
        while b < n && chars[b].is_whitespace() {
            b += 1;
        }
        return (a, b);
    }
    (at, at + 1)
}

impl WordBoundaries for GtkWords {
    fn next(&self, text: &Text, at: usize, stop: WordStop) -> usize {
        WordRules::Pango.next_native(text, at, stop)
    }
    fn prev(&self, text: &Text, at: usize, stop: WordStop) -> usize {
        WordRules::Pango.prev_native(text, at, stop)
    }
    fn word_at(&self, text: &Text, at: usize) -> Range<usize> {
        in_line_word(text, at, gtk_word_at)
    }
}

impl WordBoundaries for QtWords {
    fn next(&self, text: &Text, at: usize, stop: WordStop) -> usize {
        WordRules::Qt.next_native(text, at, stop)
    }
    fn prev(&self, text: &Text, at: usize, stop: WordStop) -> usize {
        WordRules::Qt.prev_native(text, at, stop)
    }
    fn word_at(&self, text: &Text, at: usize) -> Range<usize> {
        in_line_word(text, at, qt_word_at)
    }
}

impl WordBoundaries for MacWords {
    fn next(&self, text: &Text, at: usize, _stop: WordStop) -> usize {
        across_lines(text, at, true, |c, i| pango(&mac_mapped(c), i, true))
    }
    fn prev(&self, text: &Text, at: usize, _stop: WordStop) -> usize {
        across_lines(text, at, false, |c, i| pango(&mac_mapped_with(c, false), i, false))
    }
    fn word_at(&self, text: &Text, at: usize) -> Range<usize> {
        in_line_word(text, at, mac_word_at)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PIN3: &str = "foo bar-baz qux.quux 日本語 end";

    fn chars(s: &str) -> Vec<char> {
        s.chars().collect()
    }

    /// WG3 pin 3 (`multi-click`), the double clicks of each golden (char offsets): the
    /// measurement TC0 left open (its question 11).
    #[test]
    fn ow4_double_click_words_match_the_goldens() {
        let c = chars(PIN3);
        // (click at, GTK 4.24.1, Qt 6.12.0, NSTextField 15.7.5)
        let rows = [
            (5, (4, 7), (4, 7), (4, 7)),
            (7, (4, 8), (7, 8), (7, 8)),
            (3, (0, 4), (0, 3), (3, 4)),
            (14, (12, 15), (12, 15), (12, 20)),
            (22, (21, 24), (21, 24), (21, 24)),
        ];
        for (at, g, q, m) in rows {
            assert_eq!(gtk_word_at(&c, at), g, "gtk at {at}");
            assert_eq!(qt_word_at(&c, at), q, "qt at {at}");
            assert_eq!(mac_word_at(&c, at), m, "mac at {at}");
        }
        // pin 5 (`primary`): a double click inside `source`
        let p = chars("primary source words");
        assert_eq!(gtk_word_at(&p, 9), (8, 14));
        assert_eq!(qt_word_at(&p, 9), (8, 14));
    }

    /// WG3m pin 1: `.` and `_` between letters or digits do not split an AppKit word; `,` is not
    /// one.
    #[test]
    fn ow4_mac_word_stops() {
        let t = Text::from("foo  bar, baz.qux snake_case 3.14");
        let next = |at| MAC_WORDS.next(&t, at, WordStop::Native);
        assert_eq!(next(0), 3);
        assert_eq!(next(3), 8);
        assert_eq!(next(8), 17, "baz.qux is one word");
        assert_eq!(next(17), 28, "snake_case is one word");
        assert_eq!(next(28), 33, "3.14 is one word");
        assert_eq!(MAC_WORDS.prev(&t, 33, WordStop::Native), 29);
        assert_eq!(MAC_WORDS.prev(&t, 17, WordStop::Native), 10);
    }
}
