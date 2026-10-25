//! The table's built-in cell editor (OW5b; svelters-egui's `views/table.rs` used an
//! `egui::TextEdit::singleline` here): a single-line [`TextField`] over the edited cell. Enter
//! commits, Escape reverts, Tab and Shift+Tab commit and move to the next or previous editable
//! cell, and focus leaving it commits (Qt's default). The table turns a [`CellEnd`] into its
//! `end_edit` (commit or revert, then move by `step`).

use super::editor::{EditorConfig, FocusReason};
use super::field::TextField;
use super::keys::KeyAction;
use super::policy::TextPolicy;
use super::shim::Layouter;
use skelter_base::event::{KeyEvent, KeyState, Modifiers};
use skelter_base::key::Key;
use skelter_base::time::Time;

/// How an edit ends.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CellEnd {
    /// Commit the editor's text (else revert to the cell's value).
    pub commit: bool,
    /// Editable cells to move by afterwards (Tab +1, Shift+Tab −1).
    pub step: i32,
    pub text: String,
}

pub struct CellEditor {
    pub field: TextField,
    /// The cell's text when the edit began.
    value: String,
    end: Option<CellEnd>,
}

impl CellEditor {
    /// Starts editing a cell holding `value`. `typed`: the text that started the edit (typing on a
    /// cell replaces it, caret at the end); empty for F2, a double click or `edit` (the value, all
    /// selected).
    pub fn begin(value: &str, typed: &str, policy: TextPolicy) -> Self {
        let start = if typed.is_empty() { value } else { typed };
        let mut field = TextField::new(start, EditorConfig::default(), policy);
        field.focus_in(FocusReason::Programmatic);
        let len = start.len();
        if typed.is_empty() {
            field.editor.set_selection(0, len);
        } else {
            field.editor.set_selection(len, len);
        }
        field.take_signals();
        CellEditor { field, value: value.to_string(), end: None }
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    /// The edit asked to end (once).
    pub fn take_end(&mut self) -> Option<CellEnd> {
        self.end.take()
    }

    fn finish(&mut self, commit: bool, step: i32) {
        if self.end.is_none() {
            let text = if commit { self.field.text() } else { self.value.clone() };
            self.end = Some(CellEnd { commit, step, text });
        }
    }

    pub fn key(&mut self, ev: &KeyEvent, now: Time, lay: &mut dyn Layouter) -> KeyAction {
        if ev.state == KeyState::Pressed && ev.key == Some(Key::Tab) && !self.field.editor.composing() {
            let back = ev.modifiers.contains(Modifiers::SHIFT);
            self.finish(true, if back { -1 } else { 1 });
            return KeyAction::Consumed;
        }
        let action = self.field.key(ev, now, lay);
        match action {
            KeyAction::Submit => self.finish(true, 0),
            KeyAction::Cancel => self.finish(false, 0),
            _ => {}
        }
        self.field.take_signals();
        action
    }

    /// Focus left the editor (a click elsewhere): commit.
    pub fn focus_out(&mut self) {
        self.field.focus_out();
        self.field.take_signals();
        self.finish(true, 0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::policy::Platform;
    use crate::text::shim::MonoLayouter;
    use skelter_base::event::KeyLocation;

    fn ev(k: Key, m: Modifiers, text: Option<&str>) -> KeyEvent {
        KeyEvent { key: Some(k), physical: 0, text: text.map(str::to_string), state: KeyState::Pressed, repeat: false, location: KeyLocation::Standard, modifiers: m }
    }

    #[test]
    fn enter_escape_tab_and_blur_end_the_edit() {
        let mut lay = MonoLayouter { advance: 8.0, line_height: 16.0 };
        let p = TextPolicy::of(Platform::Qt);
        let mut c = CellEditor::begin("old", "", p);
        c.key(&ev(Key::N, Modifiers::default(), Some("n")), Time(0), &mut lay);
        assert_eq!(c.field.text(), "n", "F2 selects the value: typing replaces it");
        c.key(&ev(Key::Enter, Modifiers::default(), Some("\r")), Time(0), &mut lay);
        assert_eq!(c.take_end(), Some(CellEnd { commit: true, step: 0, text: "n".into() }));

        let mut c = CellEditor::begin("old", "x", p);
        assert_eq!((c.field.text().as_str(), c.field.editor.caret()), ("x", 1));
        c.key(&ev(Key::Escape, Modifiers::default(), None), Time(0), &mut lay);
        assert_eq!(c.take_end(), Some(CellEnd { commit: false, step: 0, text: "old".into() }));

        let mut c = CellEditor::begin("v", "", p);
        c.key(&ev(Key::Tab, Modifiers::SHIFT, None), Time(0), &mut lay);
        assert_eq!(c.take_end().map(|e| (e.commit, e.step)), Some((true, -1)));

        let mut c = CellEditor::begin("v", "w", p);
        c.focus_out();
        assert_eq!(c.take_end().map(|e| e.text), Some("w".into()));
    }
}
