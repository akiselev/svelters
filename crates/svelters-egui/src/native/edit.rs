//! FX3: editing keys around egui's `TextEdit` per the native references (WG3b pins 4–6,
//! `plans/milestones/FX3-text-editing.md`). Like FX1's word keys, everything happens before or after
//! `TextEdit` runs; `TextEdit` itself is not forked.
//!
//! Before it runs ([`before`]), on the focused field:
//! - **Undo and Redo** (Ctrl+Z; Ctrl+Shift+Z, Ctrl+Y) are taken out of the input and answered by
//!   the field's [`History`] with the toolkit's grouping ([`super::undo`]);
//! - **the CUA chords** become clipboard commands, as egui-winit does on Windows only: Ctrl+Insert
//!   copies (`Event::Copy`), Shift+Delete cuts (`Event::Cut`), Shift+Insert pastes
//!   (`ViewportCommand::RequestPaste`: the integration reads its clipboard and sends
//!   `Event::Paste`, as for Ctrl+V);
//! - **a paste with line breaks into a single-line field** keeps them (egui turns them into spaces);
//! - **Ctrl+Shift+U** starts GTK's hex Unicode entry: hex digits collect in a preedit drawn at the
//!   caret (the text is unchanged meanwhile), Space or Enter commits the character, Escape cancels,
//!   BackSpace deletes a digit. Where there is no hex entry (Qt), Ctrl+Shift+U does nothing; egui
//!   would take it for Ctrl+U (delete to the line start).
//!
//! After it ran ([`after`]): the edit or caret move of the pass goes into the history; (GTK) a
//! selection that collapsed empties the PRIMARY selection, and a middle click that pasted nothing
//! leaves the caret where it was.

use super::undo::{EditKind, History, Snap};
use super::{Policy, ShiftU, policy};
use egui::text::{CCursor, CCursorRange};
use egui::text_edit::{TextEditOutput, TextEditState};
use egui::{Event, Id, ImeEvent, Key, Modifiers, PointerButton, Ui};
use std::sync::{Arc, Mutex};

/// Per field: the text and selection at the end of the last pass, the history, the hex entry.
#[derive(Default)]
struct Field {
    last: Option<Snap>,
    history: Option<History>,
    /// Ctrl+Shift+U's digits while it is open.
    hex: Option<String>,
    /// The pass in which Space committed a hex entry: its text (`" "`) is not typed.
    eat_space: Option<u64>,
}

type Shared = Arc<Mutex<Field>>;

fn field(ui: &Ui, id: Id) -> Shared {
    ui.data_mut(|d| d.get_temp_mut_or_insert_with(id.with("svelters-native/edit"), || Arc::new(Mutex::new(Field::default()))).clone())
}

fn lock(f: &Shared) -> std::sync::MutexGuard<'_, Field> {
    f.lock().unwrap_or_else(|e| e.into_inner())
}

/// What [`before`] learned for [`after`].
pub(crate) struct EditPass {
    id: Id,
    kind: EditKind,
    /// A middle button event reached this pass.
    middle: bool,
}

fn range(ui: &Ui, id: Id, len: usize) -> (usize, usize) {
    TextEditState::load(ui.ctx(), id)
        .and_then(|s| s.cursor.char_range())
        .map_or((len, len), |r| (r.secondary.index.0.min(len), r.primary.index.0.min(len)))
}

fn store_range(ui: &Ui, id: Id, anchor: usize, caret: usize) {
    let mut st = TextEditState::load(ui.ctx(), id).unwrap_or_default();
    st.cursor.set_char_range(Some(CCursorRange::two(CCursor::new(anchor), CCursor::new(caret))));
    st.store(ui.ctx(), id);
}

fn kind_of(events: &[Event]) -> EditKind {
    let mut kind = EditKind::Other;
    for e in events {
        match e {
            Event::Paste(_) | Event::MiddleClickPaste { .. } => return EditKind::Paste,
            Event::Cut => return EditKind::Cut,
            Event::Key { key: Key::Backspace, pressed: true, modifiers: m, .. } => {
                kind = if m.ctrl { EditKind::WordBack } else { EditKind::Backspace };
            }
            Event::Key { key: Key::Delete, pressed: true, modifiers: m, .. } => {
                kind = if m.shift_only() {
                    EditKind::Cut
                } else if m.ctrl {
                    EditKind::WordForward
                } else {
                    EditKind::Delete
                };
            }
            Event::Text(_) | Event::Ime(ImeEvent::Commit(_)) if kind == EditKind::Other => kind = EditKind::Typing,
            _ => {}
        }
    }
    kind
}

/// `text` with chars `[lo, hi)` replaced by `s`, at most `char_limit` chars in all (0: no limit);
/// the caret after the inserted text.
fn replace(text: &mut String, lo: usize, hi: usize, s: &str, char_limit: usize) -> usize {
    let len = text.chars().count();
    let room = if char_limit > 0 { char_limit.saturating_sub(len - (hi - lo)) } else { usize::MAX };
    let s: String = s.chars().take(room).collect();
    let b0 = super::char_to_byte(text, lo);
    let b1 = super::char_to_byte(text, hi);
    text.replace_range(b0..b1, &s);
    lo + s.chars().count()
}

enum Action {
    Undo,
    Redo,
    Insert(String),
}

fn ctrl_shift_u(m: &Modifiers) -> bool {
    m.ctrl && m.shift && !m.alt && !m.mac_cmd
}

/// Before text field `id` runs (after FX1's Escape handling, before its word keys). The second
/// value is true if the text changed (the caller announces it).
pub(crate) fn before(ui: &Ui, id: Id, text: &mut String, multiline: bool, password: bool, char_limit: usize) -> (EditPass, bool) {
    let p: &Policy = policy(ui);
    let f = field(ui, id);
    let mut f = lock(&f);
    let (anchor, caret) = range(ui, id, text.chars().count());
    let cur = Snap::new(text, anchor, caret);
    match f.last.take() {
        // set from outside (a prop, a binding): the history no longer applies
        Some(last) if last.text != cur.text => f.history = None,
        // moved since (FX1 selects on focus after the last pass's record)
        Some(last) => {
            if let Some(h) = f.history.as_mut() {
                h.moved(&last, &cur);
            }
        }
        None => {}
    }
    f.last = Some(cur);
    let middle = ui.input(|i| i.events.iter().any(|e| matches!(e, Event::PointerButton { button: PointerButton::Middle, .. })));
    let mut pass = EditPass { id, kind: EditKind::Other, middle };
    let focused = ui.is_enabled() && ui.memory(|m| m.has_focus(id));
    if !focused {
        f.hex = None;
        return (pass, false);
    }
    let undo = p.undo.filter(|_| !password);
    let now = ui.ctx().cumulative_pass_nr();
    // a Space that committed a hex entry in the last pass: its text may come in this one
    let mut eat_space = f.eat_space.take().is_some_and(|n| now <= n + 1);
    let mut committed = false;
    let mut actions = Vec::new();
    let mut request_paste = false;
    let mut hex = f.hex.take();
    ui.ctx().input_mut(|i| {
        pass.kind = kind_of(&i.events);
        if i.pointer.button_released(PointerButton::Middle) {
            pass.kind = EditKind::Paste;
        }
        let events = std::mem::take(&mut i.events);
        for e in events {
            // Ctrl+Shift+U's hex entry takes the text and every key while it is open
            if let Some(h) = hex.as_mut() {
                match &e {
                    Event::Text(t) => {
                        for c in t.chars().filter(|c| c.is_ascii_hexdigit()) {
                            if h.len() < 8 {
                                h.push(c.to_ascii_lowercase());
                            }
                        }
                        continue;
                    }
                    Event::Key { key, pressed, .. } => {
                        if *pressed {
                            match key {
                                Key::Space | Key::Enter => {
                                    if let Some(c) = u32::from_str_radix(h, 16).ok().and_then(char::from_u32) {
                                        actions.push(Action::Insert(c.to_string()));
                                        committed = true;
                                    }
                                    eat_space = *key == Key::Space;
                                    hex = None;
                                }
                                Key::Escape => hex = None,
                                Key::Backspace if h.pop().is_none() => hex = None,
                                _ => {}
                            }
                        }
                        continue;
                    }
                    _ => {}
                }
            }
            match &e {
                Event::Text(t) if t == " " && eat_space => {
                    eat_space = false;
                    continue;
                }
                Event::Key { key: Key::U, pressed, modifiers, .. } if ctrl_shift_u(modifiers) && p.ctrl_shift_u != ShiftU::Egui => {
                    if *pressed && p.ctrl_shift_u == ShiftU::HexEntry {
                        hex = Some(String::new());
                    }
                    continue;
                }
                Event::Key { key, pressed: true, modifiers: m, .. } if p.cua_clipboard_keys && !m.alt && !m.mac_cmd => match key {
                    Key::Insert if m.ctrl && !m.shift => {
                        i.events.push(Event::Copy);
                        continue;
                    }
                    Key::Insert if m.shift_only() => {
                        request_paste = true;
                        continue;
                    }
                    Key::Delete if m.shift_only() => {
                        i.events.push(Event::Cut);
                        continue;
                    }
                    _ => {}
                },
                _ => {}
            }
            if undo.is_some()
                && let Event::Key { key, pressed, modifiers: m, .. } = &e
                && m.command
                && !m.alt
            {
                let a = match key {
                    Key::Z if !m.shift => Some(Action::Undo),
                    Key::Z | Key::Y => Some(Action::Redo),
                    _ => None,
                };
                if let Some(a) = a {
                    if *pressed {
                        actions.push(a);
                    }
                    continue;
                }
            }
            if !multiline
                && p.paste_keeps_breaks
                && let Event::Paste(t) = &e
                && t.contains(['\n', '\r'])
            {
                actions.push(Action::Insert(t.clone()));
                continue;
            }
            i.events.push(e);
        }
    });
    if request_paste {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::RequestPaste);
    }
    if committed {
        pass.kind = EditKind::Typing;
    }
    f.hex = hex;
    f.eat_space = eat_space.then_some(now);
    if actions.is_empty() {
        return (pass, false);
    }
    let (mut anchor, mut caret) = (anchor, caret);
    let mut changed = false;
    for a in actions {
        match a {
            Action::Insert(s) => {
                let (lo, hi) = (anchor.min(caret), anchor.max(caret));
                caret = replace(text, lo, hi, &s, char_limit);
                anchor = caret;
                changed = true;
            }
            Action::Undo | Action::Redo => {
                let Some(rules) = undo else { continue };
                let cur = Snap::new(text, anchor, caret);
                if let Some(last) = f.last.clone()
                    && last.text != cur.text
                {
                    // an insertion earlier in this pass goes in first
                    f.history.get_or_insert_with(|| History::new(rules)).edit(&last, &cur, pass.kind);
                }
                let h = f.history.get_or_insert_with(|| History::new(rules));
                let r = if matches!(a, Action::Undo) { h.undo(&cur) } else { h.redo(&cur) };
                let next = r.unwrap_or(cur);
                if next.text != *text {
                    *text = next.text.clone();
                    changed = true;
                }
                (anchor, caret) = (next.anchor, next.caret);
                f.last = Some(next);
            }
        }
    }
    store_range(ui, id, anchor, caret);
    ui.ctx().request_repaint();
    (pass, changed)
}

/// After text field `pass.id` ran (and the PRIMARY paste): records the pass's edit or caret move,
/// empties PRIMARY when a GTK selection collapsed, and draws an open hex entry.
pub(crate) fn after(ui: &Ui, pass: EditPass, out: &TextEditOutput, text: &str, password: bool) {
    let p = policy(ui);
    let id = pass.id;
    let f = field(ui, id);
    let mut f = lock(&f);
    let len = text.chars().count();
    let (anchor, caret) = range(ui, id, len);
    let now = Snap::new(text, anchor, caret);
    let Some(last) = f.last.take() else {
        f.last = Some(now);
        return;
    };
    let focused = ui.memory(|m| m.has_focus(id));
    // GTK: a middle click that pasted nothing (PRIMARY empty) focuses without moving the caret
    let now = if pass.middle && p.primary_clears_on_collapse && now.text == last.text && (now.anchor, now.caret) != (last.anchor, last.caret) {
        store_range(ui, id, last.anchor, last.caret);
        Snap { anchor: last.anchor, caret: last.caret, ..now }
    } else {
        now
    };
    if let Some(rules) = p.undo.filter(|_| !password) {
        let h = f.history.get_or_insert_with(|| History::new(rules));
        if h.rules() != rules {
            *h = History::new(rules);
        }
        h.edit(&last, &now, pass.kind);
    }
    let collapsed = last.anchor != last.caret && now.anchor == now.caret;
    if collapsed && focused && p.primary_clears_on_collapse && !password {
        let owner: Option<Id> = ui.data(|d| d.get_temp(super::owner_id()));
        if owner.is_none_or(|o| o == id) {
            crate::primary::clear(ui, id);
        }
    }
    f.last = Some(now);
    if let Some(h) = f.hex.as_ref().filter(|_| focused) {
        preedit(ui, out, caret, h);
    }
}

/// GTK draws the open hex entry as an underlined `u` and its digits at the caret.
fn preedit(ui: &Ui, out: &TextEditOutput, caret: usize, digits: &str) {
    let at = out.galley_pos + out.galley.pos_from_cursor(CCursor::new(caret)).min.to_vec2();
    let font = egui::TextStyle::Body.resolve(ui.style());
    let color = ui.visuals().text_color();
    let painter = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Tooltip, out.response.response.id.with("preedit")));
    let galley = painter.layout_no_wrap(format!("u{digits}"), font, color);
    let rect = egui::Rect::from_min_size(at, galley.size());
    painter.rect_filled(rect.expand(1.0), 0.0, ui.visuals().extreme_bg_color);
    painter.galley(rect.min, galley, color);
    painter.line_segment([rect.left_bottom(), rect.right_bottom()], egui::Stroke::new(1.0, color));
}
