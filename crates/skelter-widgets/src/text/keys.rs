//! Keys in text fields: the IME-first route (architecture §11.2) and each platform's key map from a
//! [`KeyEvent`] to a quire-text [`EditCommand`] or a field action.
//!
//! The maps follow the platforms' own bindings: GTK 4's `GtkText` and Qt's `QLineEdit`/`QTextEdit`
//! (Ctrl word motions, Ctrl+Backspace/Delete, Shift+Delete/Insert and Ctrl+Insert clipboard keys,
//! Ctrl+Shift+U hex entry on GTK), Windows' edit controls (the same, plus Ctrl+Y redo), and AppKit's
//! standard key bindings (`StandardKeyBinding.dict`: Option word motions, Cmd line and document
//! motions, the Emacs Ctrl keys, Home/End scroll without moving the caret).
//!
//! [`TextKeymap`] (S2) is the same maps as data: [`TextKeymap::defaults`] lists a platform's chords
//! and what each does, and a host layer of entries binds a chord to another [`TextAction`], unbinds
//! it (the key bubbles) or leaves it to the input method. A field resolves a key through its
//! policy's layer first ([`TextPolicy::keys`](super::policy::TextPolicy::keys)), then the platform's map.

use super::policy::Platform;
use skelter_base::event::{KeyEvent, KeyState, Modifiers};
use skelter_base::key::Key;
use quire_text::{Dir, EditCommand, HexKey, InputKind, Unit, WordStop};

/// Where a key goes while a field has focus (architecture §11.2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyRoute {
    /// To the input method, before any keymap: a composition is active, or an IME input source is
    /// active and the key is printable (so `jj`-style chords cannot break Japanese input).
    Ime,
    /// Through the keymaps: the command layer's, then the focused kind's ([`map_key`]), then JS.
    Keymap,
}

/// The §11.2 route of `ev`. `composing`: the field shows a preedit; `ime_active`: the platform
/// reports an input method source (IME enabled).
pub fn route(ev: &KeyEvent, composing: bool, ime_active: bool) -> KeyRoute {
    if composing {
        return KeyRoute::Ime;
    }
    let m = ev.modifiers;
    let chord = m.contains(Modifiers::CTRL) || m.contains(Modifiers::SUPER) || m.contains(Modifiers::ALT);
    if ime_active && !chord && ev.text.as_deref().is_some_and(is_printable) {
        KeyRoute::Ime
    } else {
        KeyRoute::Keymap
    }
}

/// What a key does in a field.
#[derive(Clone, Debug, PartialEq)]
pub enum KeyAction {
    /// A quire-text command.
    Edit(EditCommand),
    /// Enter in a single-line field: the kind's `submit` (and a dialog's default button).
    Submit,
    /// Escape: the kind's `cancel` (a cell editor drops its edit; dialogs close).
    Cancel,
    /// Taken by the field with no edit (AppKit's Home/End scroll; a key held for the IME).
    Consumed,
    /// Not the field's: it bubbles (Tab, unbound chords, Up/Down in a single-line field).
    Ignore,
}

/// What [`map_key`] needs to know about the field.
#[derive(Clone, Copy, Debug)]
pub struct KeyCx {
    pub platform: Platform,
    pub multiline: bool,
    /// Tab inserts a tab (a code field); otherwise it moves focus.
    pub accepts_tab: bool,
    /// GTK's Ctrl+Shift+U entry is available.
    pub hex_entry: bool,
    /// A hex entry is in progress (the field's preedit is GTK's hex entry).
    pub hex_active: bool,
}

fn is_printable(t: &str) -> bool {
    !t.is_empty() && t.chars().all(|c| !c.is_control())
}

fn mv(unit: Unit, dir: Dir, extend: bool) -> KeyAction {
    KeyAction::Edit(EditCommand::Move { unit, dir, extend })
}

fn del(unit: Unit, dir: Dir) -> KeyAction {
    KeyAction::Edit(EditCommand::Delete { unit, dir })
}

fn typed(text: &str) -> KeyAction {
    let kind = if text == " " { InputKind::TypingFirstSpace } else { InputKind::Typing };
    KeyAction::Edit(EditCommand::Insert { text: text.to_string(), kind, distribute: false })
}

/// Maps a key press to what the field does (releases do nothing). Text-producing keys without a
/// command modifier type their text.
pub fn map_key(cx: KeyCx, ev: &KeyEvent) -> KeyAction {
    if ev.state == KeyState::Released {
        return KeyAction::Ignore;
    }
    if cx.hex_active {
        return hex_key(ev);
    }
    match cx.platform {
        Platform::Mac => map_mac(cx, ev),
        _ => map_pc(cx, ev),
    }
}

/// GTK's hex entry: digits, Backspace, Space/Enter to commit, Escape to cancel; any other key ends
/// it and does nothing (GtkIMContextSimple).
fn hex_key(ev: &KeyEvent) -> KeyAction {
    let cmd = |k| KeyAction::Edit(EditCommand::HexEntry(k));
    match ev.key {
        Some(Key::Backspace) => cmd(HexKey::Backspace),
        Some(Key::Space | Key::Enter) => cmd(HexKey::Commit),
        Some(Key::Escape) => cmd(HexKey::Cancel),
        _ => match ev.text.as_deref().and_then(|t| {
            let mut c = t.chars();
            let d = c.next()?.to_digit(16)?;
            c.next().is_none().then_some(d)
        }) {
            Some(d) => cmd(HexKey::Digit(d as u8)),
            None => cmd(HexKey::Cancel),
        },
    }
}

fn type_or_ignore(ev: &KeyEvent, allow_alt: bool) -> KeyAction {
    let m = ev.modifiers;
    let blocked = m.contains(Modifiers::CTRL) || m.contains(Modifiers::SUPER) || (!allow_alt && m.contains(Modifiers::ALT));
    match ev.text.as_deref() {
        Some(t) if !blocked && is_printable(t) => typed(t),
        _ => KeyAction::Ignore,
    }
}

fn map_pc(cx: KeyCx, ev: &KeyEvent) -> KeyAction {
    let m = ev.modifiers;
    let ctrl = m.contains(Modifiers::CTRL);
    let shift = m.contains(Modifiers::SHIFT);
    let alt = m.contains(Modifiers::ALT);
    let sup = m.contains(Modifiers::SUPER);
    if sup {
        return KeyAction::Ignore;
    }
    let word = Unit::Word(WordStop::Native);
    let Some(key) = ev.key else { return type_or_ignore(ev, false) };
    if alt && !ctrl {
        // Alt chords are menu mnemonics; AltGr text arrives with `ALT_GR`, not `ALT`
        return KeyAction::Ignore;
    }
    match key {
        Key::ArrowLeft | Key::ArrowRight => {
            let dir = if key == Key::ArrowLeft { Dir::Back } else { Dir::Forward };
            mv(if ctrl { word } else { Unit::Grapheme }, dir, shift)
        }
        Key::ArrowUp | Key::ArrowDown if cx.multiline && !ctrl => {
            mv(Unit::VisualLine, if key == Key::ArrowUp { Dir::Back } else { Dir::Forward }, shift)
        }
        Key::Home | Key::End => {
            let dir = if key == Key::Home { Dir::Back } else { Dir::Forward };
            mv(if ctrl { Unit::DocEdge } else { Unit::LineEdge }, dir, shift)
        }
        Key::PageUp | Key::PageDown if cx.multiline && !ctrl => {
            mv(Unit::Page, if key == Key::PageUp { Dir::Back } else { Dir::Forward }, shift)
        }
        Key::Backspace => del(if ctrl { word } else { Unit::Grapheme }, Dir::Back),
        Key::Delete if shift && !ctrl => KeyAction::Edit(EditCommand::Cut),
        Key::Delete => del(if ctrl { word } else { Unit::Grapheme }, Dir::Forward),
        Key::Insert if shift && !ctrl => KeyAction::Edit(EditCommand::Paste),
        Key::Insert if ctrl && !shift => KeyAction::Edit(EditCommand::Copy),
        Key::Copy => KeyAction::Edit(EditCommand::Copy),
        Key::Cut => KeyAction::Edit(EditCommand::Cut),
        Key::Paste => KeyAction::Edit(EditCommand::Paste),
        Key::A if ctrl && !shift => KeyAction::Edit(EditCommand::SelectAll),
        Key::C if ctrl && !shift => KeyAction::Edit(EditCommand::Copy),
        Key::X if ctrl && !shift => KeyAction::Edit(EditCommand::Cut),
        Key::V if ctrl && !shift => KeyAction::Edit(EditCommand::Paste),
        Key::Z if ctrl && !shift => KeyAction::Edit(EditCommand::Undo),
        Key::Z if ctrl && shift => KeyAction::Edit(EditCommand::Redo),
        Key::Y if ctrl && !shift => KeyAction::Edit(EditCommand::Redo),
        Key::U if ctrl && shift && cx.hex_entry => KeyAction::Edit(EditCommand::HexEntry(HexKey::Start)),
        Key::Enter if cx.multiline && !ctrl => typed("\n"),
        Key::Enter if !ctrl => KeyAction::Submit,
        Key::Escape => KeyAction::Cancel,
        Key::Tab if cx.accepts_tab && !ctrl && !shift => typed("\t"),
        Key::Tab => KeyAction::Ignore,
        _ if ctrl => KeyAction::Ignore,
        _ => type_or_ignore(ev, false),
    }
}

fn map_mac(cx: KeyCx, ev: &KeyEvent) -> KeyAction {
    let m = ev.modifiers;
    let cmd = m.contains(Modifiers::SUPER);
    let opt = m.contains(Modifiers::ALT);
    let ctrl = m.contains(Modifiers::CTRL);
    let shift = m.contains(Modifiers::SHIFT);
    let word = Unit::Word(WordStop::Native);
    let Some(key) = ev.key else { return type_or_ignore(ev, true) };
    match key {
        Key::ArrowLeft | Key::ArrowRight => {
            let dir = if key == Key::ArrowLeft { Dir::Back } else { Dir::Forward };
            let unit = if cmd { Unit::LineEdge } else if opt { word } else { Unit::Grapheme };
            mv(unit, dir, shift)
        }
        Key::ArrowUp | Key::ArrowDown => {
            let dir = if key == Key::ArrowUp { Dir::Back } else { Dir::Forward };
            if cmd {
                mv(Unit::DocEdge, dir, shift)
            } else if cx.multiline {
                mv(Unit::VisualLine, dir, shift)
            } else {
                // NSTextField: Up and Down go to the start and the end
                mv(Unit::DocEdge, dir, shift)
            }
        }
        // scrollToBeginningOfDocument: / scrollToEndOfDocument: (the caret stays); with Shift
        // moveToBeginningOfDocumentAndModifySelection: and its End twin
        Key::Home | Key::End if shift => mv(Unit::DocEdge, if key == Key::Home { Dir::Back } else { Dir::Forward }, true),
        Key::Home | Key::End => KeyAction::Consumed,
        Key::PageUp | Key::PageDown if cx.multiline => {
            mv(Unit::Page, if key == Key::PageUp { Dir::Back } else { Dir::Forward }, shift)
        }
        Key::Backspace if cmd => del(Unit::LineEdge, Dir::Back),
        Key::Backspace if opt => del(word, Dir::Back),
        Key::Backspace => del(Unit::Grapheme, Dir::Back),
        Key::Delete if opt => del(word, Dir::Forward),
        Key::Delete => del(Unit::Grapheme, Dir::Forward),
        Key::A if cmd && !shift => KeyAction::Edit(EditCommand::SelectAll),
        Key::C if cmd && !shift => KeyAction::Edit(EditCommand::Copy),
        Key::X if cmd && !shift => KeyAction::Edit(EditCommand::Cut),
        Key::V if cmd && !shift => KeyAction::Edit(EditCommand::Paste),
        Key::Z if cmd && !shift => KeyAction::Edit(EditCommand::Undo),
        Key::Z if cmd && shift => KeyAction::Edit(EditCommand::Redo),
        Key::Copy => KeyAction::Edit(EditCommand::Copy),
        Key::Cut => KeyAction::Edit(EditCommand::Cut),
        Key::Paste => KeyAction::Edit(EditCommand::Paste),
        // the Emacs keys of the standard bindings
        Key::A if ctrl => mv(Unit::LineEdge, Dir::Back, shift),
        Key::E if ctrl => mv(Unit::LineEdge, Dir::Forward, shift),
        Key::B if ctrl => mv(Unit::Grapheme, Dir::Back, shift),
        Key::F if ctrl => mv(Unit::Grapheme, Dir::Forward, shift),
        Key::P if ctrl && cx.multiline => mv(Unit::VisualLine, Dir::Back, shift),
        Key::N if ctrl && cx.multiline => mv(Unit::VisualLine, Dir::Forward, shift),
        Key::H if ctrl => del(Unit::Grapheme, Dir::Back),
        Key::D if ctrl => del(Unit::Grapheme, Dir::Forward),
        Key::K if ctrl => del(Unit::LineEdge, Dir::Forward),
        Key::Enter if cx.multiline && !cmd => typed("\n"),
        Key::Enter if !cmd => KeyAction::Submit,
        Key::Escape => KeyAction::Cancel,
        Key::Tab if cx.accepts_tab && !ctrl && !shift && !opt => typed("\t"),
        Key::Tab => KeyAction::Ignore,
        _ if cmd || ctrl => KeyAction::Ignore,
        _ => type_or_ignore(ev, true),
    }
}

/// A key with the modifiers that pick a binding (Shift, Ctrl, Alt, Super; AltGr is text, not a chord).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct KeyChord {
    pub key: Key,
    pub mods: Modifiers,
}

impl KeyChord {
    const MASK: u8 = Modifiers::SHIFT.0 | Modifiers::CTRL.0 | Modifiers::ALT.0 | Modifiers::SUPER.0;
    pub const fn new(key: Key, mods: Modifiers) -> KeyChord {
        KeyChord { key, mods: Modifiers(mods.0 & Self::MASK) }
    }
    /// The chord of a key event (`None` for a key with no logical key: text only).
    pub fn of(ev: &KeyEvent) -> Option<KeyChord> {
        ev.key.map(|k| KeyChord::new(k, ev.modifiers))
    }
}

/// What a chord does in a text field: a [`KeyAction`], or leaving the key to the input method
/// ([`TextEditor::route`](super::editor::TextEditor::route) sends it there; the field takes no action).
#[derive(Clone, Debug, PartialEq)]
pub enum TextAction {
    Key(KeyAction),
    PassToIme,
}

impl From<KeyAction> for TextAction {
    fn from(a: KeyAction) -> TextAction {
        TextAction::Key(a)
    }
}

/// The keys the platform maps bind (every other key types its text or is not the field's).
const MAPPED: &[Key] = &[
    Key::ArrowLeft, Key::ArrowRight, Key::ArrowUp, Key::ArrowDown, Key::Home, Key::End, Key::PageUp, Key::PageDown,
    Key::Backspace, Key::Delete, Key::Insert, Key::Copy, Key::Cut, Key::Paste, Key::Enter, Key::Escape, Key::Tab,
    Key::A, Key::B, Key::C, Key::D, Key::E, Key::F, Key::H, Key::K, Key::N, Key::P, Key::U, Key::V, Key::X, Key::Y, Key::Z,
];

/// A host layer over a platform's text keys (S2): entries in the order bound, at most one per chord.
/// Empty: the platform's keys as they are.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TextKeymap {
    pub entries: Vec<(KeyChord, TextAction)>,
}

impl TextKeymap {
    pub fn new() -> TextKeymap {
        TextKeymap::default()
    }

    /// The platform's table for a field like `cx`: every chord of the keys its map binds, with what
    /// it does there (chords that are not the field's left out), built from the platform's map.
    pub fn defaults(cx: KeyCx) -> Vec<(KeyChord, TextAction)> {
        let cx = KeyCx { hex_active: false, ..cx };
        let mut out = Vec::new();
        for &key in MAPPED {
            for m in 0..16u8 {
                let ev = KeyEvent { key: Some(key), physical: 0, text: None, state: KeyState::Pressed, repeat: false, location: skelter_base::event::KeyLocation::Standard, modifiers: Modifiers(m) };
                let a = map_key(cx, &ev);
                if a != KeyAction::Ignore {
                    out.push((KeyChord::new(key, Modifiers(m)), TextAction::Key(a)));
                }
            }
        }
        out
    }

    /// [`TextKeymap::defaults`] with this layer applied: what a keymap editor lists for `cx`.
    pub fn table(&self, cx: KeyCx) -> Vec<(KeyChord, TextAction)> {
        let mut t: Vec<_> = TextKeymap::defaults(cx).into_iter().filter(|(c, _)| self.get(*c).is_none()).collect();
        t.extend(self.entries.iter().filter(|(_, a)| *a != TextAction::Key(KeyAction::Ignore)).cloned());
        t
    }

    /// The layer's entry for `chord`.
    pub fn get(&self, chord: KeyChord) -> Option<&TextAction> {
        self.entries.iter().find(|(c, _)| *c == chord).map(|(_, a)| a)
    }

    /// Binds `chord` to `action`, replacing the layer's entry for it.
    pub fn bind(&mut self, chord: KeyChord, action: impl Into<TextAction>) {
        self.reset(chord);
        self.entries.push((chord, action.into()));
    }

    /// Unbinds `chord`: the field leaves it ([`KeyAction::Ignore`]), so it bubbles to the app.
    pub fn unbind(&mut self, chord: KeyChord) {
        self.bind(chord, KeyAction::Ignore);
    }

    /// Drops the layer's entry for `chord`: the platform's binding again.
    pub fn reset(&mut self, chord: KeyChord) {
        self.entries.retain(|(c, _)| *c != chord);
    }

    /// What `ev` does in a field like `cx`: the layer's entry for its chord, else the platform's
    /// map ([`map_key`]). Releases and GTK's hex entry in progress never reach the layer.
    pub fn resolve(&self, cx: KeyCx, ev: &KeyEvent) -> TextAction {
        if ev.state == KeyState::Released || cx.hex_active {
            return TextAction::Key(map_key(cx, ev));
        }
        match KeyChord::of(ev).and_then(|c| self.get(c)) {
            Some(a) => a.clone(),
            None => TextAction::Key(map_key(cx, ev)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use skelter_base::event::KeyLocation;

    fn ev(key: Key, m: Modifiers, text: Option<&str>) -> KeyEvent {
        KeyEvent { key: Some(key), physical: 0, text: text.map(str::to_string), state: KeyState::Pressed, repeat: false, location: KeyLocation::Standard, modifiers: m }
    }

    fn cx(platform: Platform) -> KeyCx {
        KeyCx { platform, multiline: false, accepts_tab: false, hex_entry: platform == Platform::Gtk, hex_active: false }
    }

    #[test]
    fn the_text_keymap_is_the_platform_map_as_data_with_a_host_layer() {
        let pc = cx(Platform::Windows);
        let ctrl_a = KeyChord::new(Key::A, Modifiers::CTRL);
        let defaults = TextKeymap::defaults(pc);
        // built from the platform's map: Ctrl+A selects all, a plain A is not a binding (it types)
        assert!(defaults.contains(&(ctrl_a, TextAction::Key(KeyAction::Edit(EditCommand::SelectAll)))));
        assert!(!defaults.iter().any(|(c, _)| *c == KeyChord::new(Key::A, Modifiers::default())));
        assert!(TextKeymap::defaults(cx(Platform::Mac)).contains(&(KeyChord::new(Key::A, Modifiers::SUPER), TextAction::Key(KeyAction::Edit(EditCommand::SelectAll)))));
        let mut layer = TextKeymap::new();
        assert_eq!(layer.resolve(pc, &ev(Key::A, Modifiers::CTRL, Some("a"))), TextAction::Key(KeyAction::Edit(EditCommand::SelectAll)));
        // unbound: the chord bubbles; AltGr does not change the chord
        layer.unbind(ctrl_a);
        assert_eq!(layer.resolve(pc, &ev(Key::A, Modifiers::CTRL.union(Modifiers::ALT_GR), Some("a"))), TextAction::Key(KeyAction::Ignore));
        assert!(!layer.table(pc).iter().any(|(c, _)| *c == ctrl_a));
        // rebound, and a chord left to the input method
        layer.bind(ctrl_a, KeyAction::Edit(EditCommand::Undo));
        let ctrl_space = KeyChord::new(Key::Space, Modifiers::CTRL);
        layer.bind(ctrl_space, TextAction::PassToIme);
        assert_eq!(layer.entries.len(), 2, "one entry per chord");
        assert_eq!(layer.resolve(pc, &ev(Key::A, Modifiers::CTRL, Some("a"))), TextAction::Key(KeyAction::Edit(EditCommand::Undo)));
        assert_eq!(layer.resolve(pc, &ev(Key::Space, Modifiers::CTRL, Some(" "))), TextAction::PassToIme);
        assert!(layer.table(pc).contains(&(ctrl_space, TextAction::PassToIme)));
        // reset: the platform's binding again; other keys keep the platform's map
        layer.reset(ctrl_a);
        assert_eq!(layer.resolve(pc, &ev(Key::A, Modifiers::CTRL, Some("a"))), TextAction::Key(KeyAction::Edit(EditCommand::SelectAll)));
        assert_eq!(layer.resolve(pc, &ev(Key::B, Modifiers::default(), Some("b"))), TextAction::Key(typed("b")));
    }

    #[test]
    fn ime_first_routes_printable_keys_while_an_ime_is_active() {
        let j = ev(Key::J, Modifiers::default(), Some("j"));
        assert_eq!(route(&j, false, false), KeyRoute::Keymap);
        assert_eq!(route(&j, false, true), KeyRoute::Ime);
        // a chord still reaches the keymap with an IME source active
        assert_eq!(route(&ev(Key::S, Modifiers::CTRL, Some("s")), false, true), KeyRoute::Keymap);
        // while composing, everything goes to the IME (Enter and Escape included)
        assert_eq!(route(&ev(Key::Enter, Modifiers::default(), None), true, false), KeyRoute::Ime);
    }

    #[test]
    fn word_keys_follow_the_platform() {
        let w = |dir| KeyAction::Edit(EditCommand::Move { unit: Unit::Word(WordStop::Native), dir, extend: false });
        assert_eq!(map_key(cx(Platform::Gtk), &ev(Key::ArrowLeft, Modifiers::CTRL, None)), w(Dir::Back));
        assert_eq!(map_key(cx(Platform::Mac), &ev(Key::ArrowRight, Modifiers::ALT, None)), w(Dir::Forward));
        // Cmd+Left on macOS is the line's start; Alt+Left elsewhere is not the field's
        assert_eq!(
            map_key(cx(Platform::Mac), &ev(Key::ArrowLeft, Modifiers::SUPER, None)),
            KeyAction::Edit(EditCommand::Move { unit: Unit::LineEdge, dir: Dir::Back, extend: false })
        );
        assert_eq!(map_key(cx(Platform::Windows), &ev(Key::ArrowLeft, Modifiers::ALT, None)), KeyAction::Ignore);
        assert_eq!(
            map_key(cx(Platform::Qt), &ev(Key::Backspace, Modifiers::CTRL, None)),
            KeyAction::Edit(EditCommand::Delete { unit: Unit::Word(WordStop::Native), dir: Dir::Back })
        );
    }

    #[test]
    fn clipboard_undo_and_field_keys() {
        let pc = cx(Platform::Windows);
        assert_eq!(map_key(pc, &ev(Key::Delete, Modifiers::SHIFT, None)), KeyAction::Edit(EditCommand::Cut));
        assert_eq!(map_key(pc, &ev(Key::Insert, Modifiers::SHIFT, None)), KeyAction::Edit(EditCommand::Paste));
        assert_eq!(map_key(pc, &ev(Key::Y, Modifiers::CTRL, Some("y"))), KeyAction::Edit(EditCommand::Redo));
        assert_eq!(map_key(pc, &ev(Key::Enter, Modifiers::default(), Some("\r"))), KeyAction::Submit);
        assert_eq!(map_key(KeyCx { multiline: true, ..pc }, &ev(Key::Enter, Modifiers::default(), Some("\r"))), typed("\n"));
        assert_eq!(map_key(pc, &ev(Key::Tab, Modifiers::default(), Some("\t"))), KeyAction::Ignore);
        assert_eq!(map_key(pc, &ev(Key::Escape, Modifiers::default(), None)), KeyAction::Cancel);
        assert_eq!(map_key(pc, &ev(Key::A, Modifiers::default(), Some("a"))), typed("a"));
        // AltGr text types; a Ctrl chord does not
        assert_eq!(map_key(pc, &ev(Key::Q, Modifiers::ALT_GR, Some("@"))), typed("@"));
        assert_eq!(map_key(pc, &ev(Key::Q, Modifiers::CTRL, Some("q"))), KeyAction::Ignore);
        // Option text types on macOS
        assert_eq!(map_key(cx(Platform::Mac), &ev(Key::G, Modifiers::ALT, Some("©"))), typed("©"));
        assert_eq!(map_key(cx(Platform::Mac), &ev(Key::Home, Modifiers::default(), None)), KeyAction::Consumed);
    }

    #[test]
    fn gtk_hex_entry_keys() {
        let g = cx(Platform::Gtk);
        assert_eq!(
            map_key(g, &ev(Key::U, Modifiers::CTRL.union(Modifiers::SHIFT), Some("U"))),
            KeyAction::Edit(EditCommand::HexEntry(HexKey::Start))
        );
        assert_eq!(map_key(cx(Platform::Qt), &ev(Key::U, Modifiers::CTRL.union(Modifiers::SHIFT), Some("U"))), KeyAction::Ignore);
        let h = KeyCx { hex_active: true, ..g };
        assert_eq!(map_key(h, &ev(Key::E, Modifiers::default(), Some("e"))), KeyAction::Edit(EditCommand::HexEntry(HexKey::Digit(14))));
        assert_eq!(map_key(h, &ev(Key::Space, Modifiers::default(), Some(" "))), KeyAction::Edit(EditCommand::HexEntry(HexKey::Commit)));
        assert_eq!(map_key(h, &ev(Key::G, Modifiers::default(), Some("g"))), KeyAction::Edit(EditCommand::HexEntry(HexKey::Cancel)));
    }
}
