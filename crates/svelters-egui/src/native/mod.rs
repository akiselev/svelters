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
//!   focus leaves it ([`focus_visible`], GTK's `:focus-visible`).
//!
//! Which toolkit is followed is a per-context setting ([`set`], [`get`]); the default comes from the
//! platform ([`Native::for_os`]): GTK on Linux and the BSDs. Windows and macOS keep egui's own
//! behaviour until their references are captured (WG3 follow-ups).

// FX3s: the GTK and Qt undo models, unused until svelters' own text field (see the module docs)
#[allow(dead_code)]
pub(crate) mod undo;
pub mod words;

use egui::text::{CCursor, CCursorRange};
use egui::text_edit::TextEditState;
use egui::{Event, Id, Key, Modifiers, Ui};
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
#[derive(Clone, Copy, Debug, PartialEq)]
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
    /// OW2: scrolling and sashes (`interact::Scrollable`, `interact::Resizable`).
    pub scroll: ScrollPolicy,
}

/// OW2 (ARCH-WIDGETS §1.2, §3.3): how scrollables and sashes behave on a platform. Sizes and
/// colours are CSS (WIDGET-PARITY P9); this is timing and gesture behaviour only.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollPolicy {
    /// Who continues a trackpad flick after the fingers leave.
    pub kinetic: Kinetic,
    /// Bars over the content (fading) or beside it (a gutter, always shown when it overflows).
    pub bar: BarMode,
    /// What a primary press on the track (beside the thumb) does.
    pub track_click: TrackClick,
    /// Shift+press on the track does the other [`TrackClick`] (GTK).
    pub shift_track_click_inverts: bool,
    /// Points one arrow key (or one scroll-bar arrow step) scrolls.
    pub line: f32,
    /// Overlay bars: how long they stay after the last scroll or hover, then fade out over
    /// [`ScrollPolicy::bar_fade`] seconds (VS Code: 0.5 s, then 0.8 s).
    pub bar_hold: f32,
    pub bar_fade: f32,
    /// R11: hovering a sash shows its hover bar and grip after this many seconds (VS Code 0.3 s).
    pub sash_hover_delay: f32,
}

/// Who continues a trackpad flick (ARCH-WIDGETS §3.3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kinetic {
    /// The OS sends momentum events after the fingers lift (macOS, Windows precision touchpads).
    Os,
    /// svelters decays the release velocity itself (Linux touchpads through libinput, as GTK).
    Own,
    /// No momentum (Qt widgets without `QScroller`).
    None,
}

/// Where scroll bars sit (`--sv-scrollbar-mode` / `scrollbar-gutter`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BarMode {
    /// Over the content, thumb only, shown while hovered or scrolling and then faded (VS Code,
    /// GTK 4, macOS).
    Overlay,
    /// Beside the content in a gutter with a track, shown whenever the content overflows (Qt
    /// Fusion, Windows classic).
    Classic,
}

/// A primary press on the track (Qt `SH_ScrollBar_LeftClickAbsolutePosition`, GTK
/// `gtk-primary-button-warps-slider`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrackClick {
    /// Scrolls one page towards the press, repeating while held.
    Page,
    /// Moves the thumb's centre to the press and drags it from there.
    Jump,
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
    scroll: ScrollPolicy {
        kinetic: Kinetic::Own,
        bar: BarMode::Overlay,
        track_click: TrackClick::Jump,
        shift_track_click_inverts: true,
        line: 40.0,
        bar_hold: 0.5,
        bar_fade: 0.8,
        sash_hover_delay: 0.3,
    },
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
    scroll: ScrollPolicy {
        kinetic: Kinetic::None,
        bar: BarMode::Classic,
        track_click: TrackClick::Page,
        shift_track_click_inverts: false,
        line: 40.0,
        bar_hold: 0.5,
        bar_fade: 0.8,
        sash_hover_delay: 0.3,
    },
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
    scroll: ScrollPolicy {
        kinetic: Kinetic::Os,
        bar: BarMode::Overlay,
        track_click: TrackClick::Jump,
        shift_track_click_inverts: false,
        line: 40.0,
        bar_hold: 0.5,
        bar_fade: 0.8,
        sash_hover_delay: 0.3,
    },
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

fn policy(ui: &Ui) -> &'static Policy {
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
