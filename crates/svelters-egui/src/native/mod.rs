//! FX1: native focus policy and word rules for the built-in leaves (EGUI.md W1/W2; WG3's pins 1
//! and 7, plans/milestones/FX1-egui-focus-words.md).
//!
//! The user's decision W1 is native per-platform behaviour for text, selection and keys, with GTK4
//! and Qt both references on Linux (W2). egui's own `TextEdit` stays the editing core; this module
//! changes what happens around it, without forking it:
//!
//! - Escape no longer drops keyboard focus ([`keep_focus_on_escape`]);
//! - before a focused `TextEdit` runs: the word keys (Ctrl+Left/Right with or without Shift,
//!   Ctrl+BackSpace, Ctrl+Delete) are taken out of the input and applied with the native word rules
//!   ([`text_keys`], [`words`]); egui's `TextEdit` never sees them;
//! - after a `TextEdit` ran: select-all on keyboard focus-in, the unfocused selection kept or
//!   collapsed, one selection owner at a time (X11 PRIMARY) ([`TextFocus`]);
//! - after a button or check box ran: a press focuses it ([`after_toggle`]), drawn unfocused until
//!   focus leaves it ([`focus_visible`], GTK's `:focus-visible`);
//! - FX3 (WG3b's pins 4–6): undo grouping, the CUA clipboard chords, line breaks kept by a paste,
//!   PRIMARY emptied when a selection collapses, Ctrl+Shift+U hex entry and Enter keeping focus
//!   ([`edit`], [`undo`]).
//!
//! Which toolkit is followed is a per-context setting ([`set`], [`get`]); the default comes from the
//! platform ([`Native::for_os`]): GTK on Linux and the BSDs. Windows and macOS keep egui's own
//! behaviour until their references are captured (WG3 follow-ups).

pub(crate) mod edit;
pub mod undo;
pub mod words;

use egui::text::{CCursor, CCursorRange};
use egui::text_edit::TextEditState;
use egui::{Event, Id, Key, Modifiers, Ui};
pub use undo::UndoRules;
pub use words::WordRules;

/// The toolkit whose behaviour the built-in leaves follow.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Native {
    /// GTK 4 (`GtkText`, `GtkButton`), reference `conformance/golden/linux/gtk-4.24.1.json`.
    Gtk,
    /// Qt 6 widgets (`QLineEdit`, `QPushButton`), reference `conformance/golden/linux/qt-6.12.0.json`.
    Qt,
    /// egui 0.37's own behaviour, unchanged.
    Egui,
}

/// What a [`Native`] does, one switch per behaviour WG3 measured.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Policy {
    /// Word movement and deletion rules; `None`: egui's.
    pub words: Option<WordRules>,
    /// Word movement without Shift on a selection collapses it to its end (forward) or start
    /// (backward) without moving (GTK); otherwise it moves from the caret.
    pub word_move_collapses: bool,
    /// Keyboard focus (Tab, Shift+Tab) into a single-line field selects all of it, caret at the end.
    pub select_on_keyboard_focus: bool,
    /// Programmatic focus (the `focus` method) selects all too (GTK's `grab_focus`; Qt's
    /// `setFocus(OtherFocusReason)` does not).
    pub select_on_programmatic_focus: bool,
    /// An unfocused field keeps its selection (GTK); otherwise it collapses to the caret (Qt, egui).
    pub keep_selection_unfocused: bool,
    /// Only one field holds a selection: a field that selects collapses the previous owner's
    /// selection to its start (GTK on X11, the PRIMARY selection changing hands).
    pub one_selection_owner: bool,
    /// Pressing a button or check box gives it keyboard focus.
    pub focus_on_click: bool,
    /// Escape leaves keyboard focus where it is (egui drops it).
    pub escape_keeps_focus: bool,
    /// FX3: undo grouping; `None`: egui's (time-based `Undoer`).
    pub undo: Option<UndoRules>,
    /// Ctrl+Insert copies, Shift+Insert pastes, Shift+Delete cuts (egui-winit maps them on Windows
    /// only).
    pub cua_clipboard_keys: bool,
    /// A paste into a single-line field keeps its line breaks (egui turns them into spaces).
    pub paste_keeps_breaks: bool,
    /// A focused field whose selection collapses empties the PRIMARY selection (GTK; Qt leaves
    /// the last text there).
    pub primary_clears_on_collapse: bool,
    /// What Ctrl+Shift+U does in a text field.
    pub ctrl_shift_u: ShiftU,
    /// Enter in a single-line field keeps keyboard focus (egui ends editing and drops it).
    pub enter_keeps_focus: bool,
}

/// Ctrl+Shift+U in a text field ([`Policy::ctrl_shift_u`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShiftU {
    /// egui's: Ctrl+U, delete to the line start (it ignores Shift).
    Egui,
    /// Nothing (Qt without an input method).
    Nothing,
    /// Hex Unicode entry (GTK's built-in input method; IBus has the same).
    HexEntry,
}

const GTK: Policy = Policy {
    words: Some(WordRules::Pango),
    word_move_collapses: true,
    select_on_keyboard_focus: true,
    select_on_programmatic_focus: true,
    keep_selection_unfocused: true,
    one_selection_owner: true,
    focus_on_click: true,
    escape_keeps_focus: true,
    undo: Some(UndoRules::Gtk),
    cua_clipboard_keys: true,
    paste_keeps_breaks: true,
    primary_clears_on_collapse: true,
    ctrl_shift_u: ShiftU::HexEntry,
    enter_keeps_focus: true,
};

const QT: Policy = Policy {
    words: Some(WordRules::Qt),
    word_move_collapses: false,
    select_on_keyboard_focus: true,
    select_on_programmatic_focus: false,
    keep_selection_unfocused: false,
    one_selection_owner: false,
    focus_on_click: true,
    escape_keeps_focus: true,
    undo: Some(UndoRules::Qt),
    cua_clipboard_keys: true,
    paste_keeps_breaks: true,
    primary_clears_on_collapse: false,
    ctrl_shift_u: ShiftU::Nothing,
    enter_keeps_focus: true,
};

const EGUI: Policy = Policy {
    words: None,
    word_move_collapses: false,
    select_on_keyboard_focus: false,
    select_on_programmatic_focus: false,
    keep_selection_unfocused: false,
    one_selection_owner: false,
    focus_on_click: false,
    escape_keeps_focus: false,
    undo: None,
    cua_clipboard_keys: false,
    paste_keeps_breaks: false,
    primary_clears_on_collapse: false,
    ctrl_shift_u: ShiftU::Egui,
    enter_keeps_focus: false,
};

impl Native {
    /// The per-platform table: Linux (and the BSDs) follow GTK; the others keep egui's behaviour
    /// until WG3 has their references.
    pub fn for_os(os: egui::os::OperatingSystem) -> Native {
        match os {
            egui::os::OperatingSystem::Nix => Native::Gtk,
            _ => Native::Egui,
        }
    }

    pub fn policy(self) -> &'static Policy {
        match self {
            Native::Gtk => &GTK,
            Native::Qt => &QT,
            Native::Egui => &EGUI,
        }
    }
}

fn setting_id() -> Id {
    Id::unique("svelters-egui/native")
}

fn owner_id() -> Id {
    Id::unique("svelters-egui/native/selection-owner")
}

/// Makes the leaves of `ctx` follow `native` (default: [`Native::for_os`] of `ctx.os()`).
pub fn set(ctx: &egui::Context, native: Native) {
    ctx.data_mut(|d| d.insert_temp(setting_id(), native));
}

/// The toolkit the leaves of `ctx` follow.
pub fn get(ctx: &egui::Context) -> Native {
    ctx.data(|d| d.get_temp(setting_id())).unwrap_or_else(|| Native::for_os(ctx.os()))
}

pub(crate) fn policy(ui: &Ui) -> &'static Policy {
    get(ui.ctx()).policy()
}

fn tab_pass_id() -> Id {
    Id::unique("svelters-egui/native/tab-pass")
}

/// Notes a pass whose input holds Tab or Shift+Tab, for [`tab_moved_focus`].
fn note_tab(ui: &Ui) {
    let tab = ui.input(|i| i.events.iter().any(|e| matches!(e, Event::Key { key: Key::Tab, pressed: true, modifiers, .. } if !modifiers.any() || modifiers.shift_only())));
    if tab {
        let pass = ui.ctx().cumulative_pass_nr();
        ui.data_mut(|d| d.insert_temp(tab_pass_id(), pass));
    }
}

/// Whether focus gained in this pass came from Tab or Shift+Tab: egui moves it in the pass of the
/// key (Tab) or the next one (Shift+Tab). Other focus changes (a restore after a viewport change,
/// assistive technology) do not select.
fn tab_moved_focus(ui: &Ui) -> bool {
    let now = ui.ctx().cumulative_pass_nr();
    ui.data(|d| d.get_temp::<u64>(tab_pass_id())).is_some_and(|p| p <= now && now - p <= 2)
}

fn plain_escape(ui: &Ui) -> bool {
    ui.input(|i| i.events.iter().any(|e| matches!(e, Event::Key { key: Key::Escape, pressed: true, modifiers, .. } if !modifiers.any())))
}

/// When this pass's plain Escape took keyboard focus from widget `id` (egui's `Memory` drops focus
/// on it), gives it back. Called before a `TextEdit` runs (else it collapses its selection for want
/// of focus) and after the other leaves (their id is known only then).
pub(crate) fn keep_focus_on_escape(ui: &Ui, id: Id) {
    if !policy(ui).escape_keeps_focus {
        return;
    }
    if plain_escape(ui) && ui.memory(|m| m.focused().is_none() && m.had_focus_last_frame(id)) {
        ui.memory_mut(|m| m.request_focus(id));
    }
}

fn pointer_focus_id() -> Id {
    Id::unique("svelters-egui/native/pointer-focus")
}

/// Before a button or check box runs in `ui` (its own child `Ui`): GTK's `:focus-visible`. A
/// widget focused by a press is drawn as it would be unfocused (egui draws a focused widget as
/// pressed), until focus leaves it; keyboard focus is drawn as egui draws it.
pub(crate) fn focus_visible(ui: &mut Ui) {
    if !policy(ui).focus_on_click {
        return;
    }
    // the id the button or check box takes (both use `next_auto_id`, egui 0.37)
    let id = ui.next_auto_id();
    let by_pointer = ui.data(|d| d.get_temp::<Id>(pointer_focus_id())) == Some(id);
    if !by_pointer || !ui.memory(|m| m.has_focus(id)) {
        return;
    }
    // egui styles a widget by its response of the previous pass
    let Some(prev) = ui.ctx().read_response(id) else { return };
    if prev.is_pointer_button_down_on() || prev.clicked() {
        return;
    }
    let w = &mut ui.visuals_mut().widgets;
    w.active = if prev.hovered() || prev.highlighted() { w.hovered } else { w.inactive };
}

/// After a button or check box ran: a press focuses it (GTK `focus-on-click`, Qt `StrongFocus`).
pub(crate) fn after_toggle(ui: &Ui, resp: &egui::Response) {
    if !policy(ui).focus_on_click {
        return;
    }
    let focused = ui.memory(|m| m.has_focus(resp.id));
    if resp.is_pointer_button_down_on() {
        if !focused {
            resp.request_focus();
        }
        ui.data_mut(|d| d.insert_temp(pointer_focus_id(), resp.id));
    } else if !focused && ui.data(|d| d.get_temp::<Id>(pointer_focus_id())) == Some(resp.id) {
        ui.data_mut(|d| d.remove::<Id>(pointer_focus_id()));
    }
}

fn is_word_key(key: Key, m: Modifiers) -> bool {
    // Ctrl is the word modifier on Linux and Windows (egui-winit sets `ctrl` and `command`)
    if !m.ctrl || m.alt || m.mac_cmd {
        return false;
    }
    match key {
        Key::ArrowLeft | Key::ArrowRight => true,
        Key::Backspace | Key::Delete => !m.shift,
        _ => false,
    }
}

fn taken_id(id: Id) -> Id {
    id.with("svelters-native/taken")
}

/// Notes key events taken out of the input before field `id`'s `TextEdit` ran (FX1's word keys,
/// FX3's undo and clipboard chords), for [`give_back`].
pub(crate) fn note_taken(ui: &Ui, id: Id, events: impl IntoIterator<Item = Event>) {
    ui.data_mut(|d| d.get_temp_mut_or_default::<Vec<Event>>(taken_id(id)).extend(events));
}

/// After field `id`'s `TextEdit` ran: puts the keys taken before it back into the input, so that
/// EV3's key claim (`focus::record`, right after the leaf) sees them as the field's
/// (`focus::consumed_keys`) and removes them before anything else in the pass does.
pub(crate) fn give_back(ui: &Ui, id: Id) {
    let taken = ui.data_mut(|d| d.remove_temp::<Vec<Event>>(taken_id(id))).unwrap_or_default();
    if !taken.is_empty() {
        ui.ctx().input_mut(|i| i.events.extend(taken));
    }
}

fn char_to_byte(text: &str, c: usize) -> usize {
    text.char_indices().nth(c).map_or(text.len(), |(b, _)| b)
}

/// Before focused `TextEdit` `id` runs: takes this pass's word keys out of the input and applies
/// them to `text` and the field's selection with the native word rules. Returns whether the text
/// changed (the `TextEdit` will not report it: it did not make the change).
pub(crate) fn text_keys(ui: &Ui, id: Id, text: &mut String, password: bool) -> bool {
    let p = policy(ui);
    let Some(rules) = p.words else { return false };
    if !ui.is_enabled() || !ui.memory(|m| m.has_focus(id)) {
        return false;
    }
    let keys: Vec<(Key, Modifiers)> = ui.ctx().input_mut(|i| {
        let mut taken = Vec::new();
        i.events.retain(|e| match e {
            Event::Key { key, pressed: true, modifiers, .. } if is_word_key(*key, *modifiers) => {
                taken.push((*key, *modifiers));
                false
            }
            _ => true,
        });
        taken
    });
    if keys.is_empty() {
        return false;
    }
    note_taken(ui, id, keys.iter().map(|&(key, modifiers)| Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers }));
    let ctx = ui.ctx();
    let mut st = TextEditState::load(ctx, id).unwrap_or_default();
    let len = text.chars().count();
    // no state yet: egui's default puts the caret at the end
    let (mut anchor, mut caret) = st.cursor.char_range().map_or((len, len), |r| (r.secondary.index.0.min(len), r.primary.index.0.min(len)));
    let mut changed = false;
    for (key, m) in keys {
        let len = text.chars().count();
        // a password field's text is not shown: its words are not either (GTK and Qt move to the ends)
        let next = |t: &str, at| if password { len } else { rules.next(t, at) };
        let prev = |t: &str, at| if password { 0 } else { rules.prev(t, at) };
        match key {
            Key::ArrowLeft | Key::ArrowRight => {
                let forward = key == Key::ArrowRight;
                if anchor != caret && !m.shift && p.word_move_collapses {
                    caret = if forward { anchor.max(caret) } else { anchor.min(caret) };
                } else {
                    caret = if forward { next(text, caret) } else { prev(text, caret) };
                }
                if !m.shift {
                    anchor = caret;
                }
            }
            _ => {
                let (lo, hi) = if anchor != caret {
                    (anchor.min(caret), anchor.max(caret))
                } else {
                    let to = if key == Key::Backspace { prev(text, caret) } else { next(text, caret) };
                    (caret.min(to), caret.max(to))
                };
                if lo < hi {
                    let (b0, b1) = (char_to_byte(text, lo), char_to_byte(text, hi));
                    text.replace_range(b0..b1, "");
                    changed = true;
                }
                anchor = lo;
                caret = lo;
            }
        }
    }
    st.cursor.set_char_range(Some(CCursorRange::two(CCursor::new(anchor), CCursor::new(caret))));
    st.store(ctx, id);
    ui.ctx().request_repaint();
    changed
}

/// A `TextEdit`'s focus and selection before it ran, for [`TextFocus::after`].
pub(crate) struct TextFocus {
    id: Id,
    focused: bool,
    range: Option<CCursorRange>,
}

impl TextFocus {
    pub(crate) fn before(ui: &Ui, id: Id) -> TextFocus {
        note_tab(ui);
        TextFocus {
            id,
            focused: ui.memory(|m| m.has_focus(id)),
            range: TextEditState::load(ui.ctx(), id).and_then(|s| s.cursor.char_range()),
        }
    }

    /// After the `TextEdit` ran (and after a programmatic focus request of this pass, `requested`):
    /// select-all on keyboard focus-in, the unfocused selection kept, one selection owner.
    pub(crate) fn after(self, ui: &Ui, resp: &egui::Response, len: usize, multiline: bool, requested: bool) {
        let p = policy(ui);
        let ctx = ui.ctx();
        let id = self.id;
        let focused = ui.memory(|m| m.has_focus(id));
        let mut st = TextEditState::load(ctx, id).unwrap_or_default();
        let before = st.cursor.char_range();
        if !focused && !self.focused && p.keep_selection_unfocused {
            // egui collapses an unfocused field's selection to its caret every pass
            if let Some(r) = self.range.filter(|r| !r.is_empty())
                && before != Some(r)
            {
                st.cursor.set_char_range(Some(r));
            }
        }
        // gained this pass: Tab gives focus while the widget runs, Shift+Tab before it (egui defers
        // it a frame), so ask egui's memory rather than the state before the run
        let gained = focused && !ui.memory(|m| m.had_focus_last_frame(id));
        if gained && !multiline {
            let pointer = resp.is_pointer_button_down_on() || resp.clicked() || resp.dragged() || resp.drag_started();
            let select = if requested {
                p.select_on_programmatic_focus
            } else {
                !pointer && p.select_on_keyboard_focus && tab_moved_focus(ui)
            };
            if select {
                st.cursor.set_char_range(Some(CCursorRange::two(CCursor::new(0), CCursor::new(len))));
            }
        }
        let now = st.cursor.char_range();
        if p.one_selection_owner && focused && now.is_some_and(|r| !r.is_empty()) {
            let owner: Option<Id> = ctx.data(|d| d.get_temp(owner_id()));
            if owner != Some(id) {
                if let Some(o) = owner
                    && let Some(mut os) = TextEditState::load(ctx, o)
                    && let Some(r) = os.cursor.char_range().filter(|r| !r.is_empty())
                {
                    let start = r.primary.index.0.min(r.secondary.index.0);
                    os.cursor.set_char_range(Some(CCursorRange::one(CCursor::new(start))));
                    os.store(ctx, o);
                }
                ctx.data_mut(|d| d.insert_temp(owner_id(), id));
            }
        }
        if now != before {
            st.store(ctx, id);
            // FX3: putting back the range the field had before this pass (egui collapses an
            // unfocused field's selection on every pass) changes nothing on screen; a repaint for
            // it would repaint forever while an unfocused GTK field holds a selection
            if now != self.range {
                ctx.request_repaint();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fx1_platform_table() {
        use egui::os::OperatingSystem as Os;
        assert_eq!(Native::for_os(Os::Nix), Native::Gtk);
        assert_eq!(Native::for_os(Os::Windows), Native::Egui);
        assert_eq!(Native::for_os(Os::Mac), Native::Egui);
        // where GTK and Qt differ, the switches say so
        assert_ne!(GTK.words, QT.words);
        assert!(GTK.keep_selection_unfocused && !QT.keep_selection_unfocused);
        assert!(GTK.focus_on_click && QT.focus_on_click && GTK.escape_keeps_focus && QT.escape_keeps_focus);
    }
}
