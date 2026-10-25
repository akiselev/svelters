//! [`TextEditor`]: the editing state of one text field, on quire-text: a [`TextDocument`] and its one
//! [`TextView`], the platform's [`TextPolicy`], and what quire-text leaves to its host: keys (through
//! [`super::keys`]), IME events and the [`TextInputClient`](presto_base::input::TextInputClient)
//! operations in document-wide UTF-16 offsets, pointer selection (click, double and triple click,
//! drags by word and line, the middle-click PRIMARY paste), focus rules, and the clipboard and
//! PRIMARY requests the platform serves ([`TextRequest`]).
//!
//! The editor is layout-free: motions that need rows take a [`VisualLines`] (quire-text's
//! [`NoWrap`] when the text is not laid out; [`super::lines::GeometryLines`] over a layout), and the
//! IME's geometry queries are answered by the field that owns the layout ([`super::field`]).

use super::keys::{self, KeyAction, KeyCx};
use super::policy::TextPolicy;
use super::shown::Shown;
use super::utf16::{floor_boundary, str_byte_to_utf16, str_utf16_to_byte};
use presto_base::event::{ImeEvent, KeyEvent};
use presto_base::input::{InputPurpose, Utf16Range};
use presto_base::time::Time;
use quire_text::{
    DocRules, EditCommand, EditRules, ExternalSet, HostRequest, InputFilter, InputKind, LineMode, NoWrap, Preedit, PreeditKind,
    PreeditUpdate, Rejected, SelRange, Selection, TextDocument, TextView, VisualLines,
};
use std::ops::Range;
use std::sync::Arc;

/// What kind of field an editor backs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EditorConfig {
    pub multiline: bool,
    /// A password field: masked, no copy or cut, no PRIMARY, word motions go to the ends.
    pub password: bool,
    /// At most this many characters (code points, GTK's `max-length`).
    pub max_chars: Option<u32>,
    pub read_only: bool,
    /// What the IME is told the field is for.
    pub purpose: InputPurpose,
    /// Tab inserts a tab instead of moving focus.
    pub accepts_tab: bool,
}

impl Default for EditorConfig {
    fn default() -> Self {
        EditorConfig { multiline: false, password: false, max_chars: None, read_only: false, purpose: InputPurpose::Text, accepts_tab: false }
    }
}

/// Something only the platform can do for the field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TextRequest {
    SetClipboard(String),
    /// Read the clipboard and answer with [`TextEditor::paste`].
    ReadClipboard,
    /// Own PRIMARY with this text.
    SetPrimary(String),
    ClearPrimary,
    /// Read PRIMARY and answer with [`TextEditor::paste_primary`] (a middle click).
    ReadPrimary,
    Beep,
}

/// What changed since [`TextEditor::take_changes`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Changes {
    /// The user changed the text (an `input` event).
    pub text: bool,
    /// The text was set from outside ([`TextEditor::set_text`]).
    pub external: bool,
    pub selection: bool,
    /// The preedit changed (shown text and caret move; the document did not change).
    pub preedit: bool,
}

impl Changes {
    /// The shown text differs: relayout.
    pub fn relayout(&self) -> bool {
        self.text || self.external || self.preedit
    }
}

/// How focus arrived.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FocusReason {
    /// Tab or Shift+Tab.
    Keyboard,
    Pointer,
    /// The `focus` method, autofocus.
    Programmatic,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DragUnit {
    Char,
    Word,
    Line,
}

#[derive(Clone, Debug)]
struct Drag {
    unit: DragUnit,
    /// The range the press selected; the drag extends from it.
    origin: Range<usize>,
}

/// One field's editing state. See the module docs.
#[derive(Clone, Debug)]
pub struct TextEditor {
    doc: TextDocument,
    view: TextView,
    policy: TextPolicy,
    rules: EditRules,
    cfg: EditorConfig,
    /// Applied to inserted text (a hex or identifier field).
    filter: Option<InputFilter>,
    ime_enabled: bool,
    /// A middle click waits for PRIMARY to paste here.
    paste_at: Option<usize>,
    drag: Option<Drag>,
    requests: Vec<TextRequest>,
    changes: Changes,
    rejected: Option<Rejected>,
    reveal: Option<usize>,
    now_ms: u64,
}

fn doc_rules(cfg: &EditorConfig, policy: &TextPolicy) -> DocRules {
    let mut r = DocRules::field(if cfg.multiline { LineMode::Multi } else { LineMode::SINGLE }, policy.undo);
    r.max_chars = cfg.max_chars;
    r.read_only = cfg.read_only;
    r
}

impl TextEditor {
    pub fn new(text: &str, cfg: EditorConfig, policy: TextPolicy) -> Self {
        let doc = TextDocument::new(text, Arc::new(doc_rules(&cfg, &policy)));
        let view = TextView::new(&doc);
        TextEditor {
            doc,
            view,
            rules: policy.edit_rules(cfg.password),
            policy,
            cfg,
            filter: None,
            ime_enabled: false,
            paste_at: None,
            drag: None,
            requests: Vec::new(),
            changes: Changes::default(),
            rejected: None,
            reveal: None,
            now_ms: 0,
        }
    }

    pub fn config(&self) -> &EditorConfig {
        &self.cfg
    }

    pub fn policy(&self) -> &TextPolicy {
        &self.policy
    }

    /// Changes the field's kind (a prop write: `multiline`, `password`, `max-chars`, `read-only`).
    pub fn set_config(&mut self, cfg: EditorConfig) {
        if cfg == self.cfg {
            return;
        }
        let mask_changed = cfg.password != self.cfg.password;
        self.cfg = cfg;
        self.doc.set_rules(Arc::new(doc_rules(&self.cfg, &self.policy)));
        self.rebuild_rules();
        if mask_changed {
            self.changes.external = true;
        }
    }

    pub fn set_policy(&mut self, policy: TextPolicy) {
        self.policy = policy;
        self.doc.set_rules(Arc::new(doc_rules(&self.cfg, &self.policy)));
        self.rebuild_rules();
    }

    /// A pure filter for inserted text (typing, paste, IME commit, drop).
    pub fn set_filter(&mut self, filter: Option<InputFilter>) {
        self.filter = filter;
        self.rebuild_rules();
    }

    fn rebuild_rules(&mut self) {
        self.rules = self.policy.edit_rules(self.cfg.password);
        self.rules.filter = self.filter;
    }

    pub fn document(&self) -> &TextDocument {
        &self.doc
    }

    pub fn view(&self) -> &TextView {
        &self.view
    }

    pub fn text(&self) -> String {
        self.doc.text().to_string()
    }

    pub fn is_empty(&self) -> bool {
        self.doc.text().is_empty()
    }

    /// The primary selection range (anchor and head, bytes).
    pub fn selection(&self) -> SelRange {
        self.view.selection().primary()
    }

    /// The caret (the primary head).
    pub fn caret(&self) -> usize {
        self.selection().head
    }

    pub fn focused(&self) -> bool {
        self.view.focused()
    }

    /// An IME composition (or GTK's hex entry) is showing.
    pub fn composing(&self) -> bool {
        self.view.composing()
    }

    pub fn preedit(&self) -> Option<&Preedit> {
        self.view.preedit()
    }

    /// GTK's hex entry is in progress.
    pub fn hex_active(&self) -> bool {
        self.view.composing() && self.view.preedit_kind() == PreeditKind::Hex
    }

    pub fn ime_enabled(&self) -> bool {
        self.ime_enabled
    }

    pub fn purpose(&self) -> InputPurpose {
        if self.cfg.password { InputPurpose::Password } else { self.cfg.purpose }
    }

    /// The host clock for undo grouping (Monaco's pause rule).
    pub fn set_time(&mut self, now: Time) {
        self.now_ms = now.0 / 1_000_000;
    }

    pub fn take_requests(&mut self) -> Vec<TextRequest> {
        std::mem::take(&mut self.requests)
    }

    pub fn take_changes(&mut self) -> Changes {
        std::mem::take(&mut self.changes)
    }

    /// Why the last command did nothing (or less); the field may flash.
    pub fn take_rejected(&mut self) -> Option<Rejected> {
        self.rejected.take()
    }

    /// The byte to scroll into view after the last edit or motion.
    pub fn take_reveal(&mut self) -> Option<usize> {
        self.reveal.take()
    }

    /// The shown text: masked for a password field, the preedit spliced in at the caret.
    pub fn shown(&self) -> Shown {
        let text = self.text();
        let mask = self.cfg.password.then_some(self.policy.mask);
        Shown::build(&text, mask, self.preedit().map(|p| (self.caret(), p.text.as_str())))
    }

    /// The IME's document: the text with the preedit spliced in, never masked.
    fn ime_doc(&self) -> Shown {
        let text = self.text();
        Shown::build(&text, None, self.preedit().map(|p| (self.caret(), p.text.as_str())))
    }

    fn push(&mut self, r: TextRequest) {
        // a drag sets PRIMARY at every step: only the last one matters
        if matches!(r, TextRequest::SetPrimary(_) | TextRequest::ClearPrimary) {
            self.requests.retain(|q| !matches!(q, TextRequest::SetPrimary(_) | TextRequest::ClearPrimary));
        }
        self.requests.push(r);
    }

    /// Runs a quire-text command through the view, recording what changed and what the host must do.
    pub fn execute(&mut self, cmd: EditCommand, layout: &dyn VisualLines) {
        let sel = self.view.selection_version();
        let pre = self.view.preedit().cloned();
        let ex = self.doc.execute(&mut self.view, cmd, &self.rules, layout, Some(self.now_ms));
        for r in ex.requests {
            match r {
                HostRequest::SetClipboard(s) => self.push(TextRequest::SetClipboard(s)),
                HostRequest::ReadClipboard => self.push(TextRequest::ReadClipboard),
                HostRequest::SetPrimary(s) => self.push(TextRequest::SetPrimary(s)),
                HostRequest::ClearPrimary => self.push(TextRequest::ClearPrimary),
                HostRequest::Beep => self.push(TextRequest::Beep),
                // fields keep local undo (`UndoMode::Local`): quire-text asks only in host mode
                HostRequest::Undo | HostRequest::Redo => {}
            }
        }
        if ex.applied.as_ref().is_some_and(|a| a.changed_text()) {
            self.changes.text = true;
        }
        if self.view.selection_version() != sel {
            self.changes.selection = true;
        }
        if self.view.preedit() != pre.as_ref() {
            self.changes.preedit = true;
        }
        if ex.rejected.is_some() {
            self.rejected = ex.rejected;
        }
        if ex.reveal.is_some() {
            self.reveal = ex.reveal;
        }
    }

    fn run(&mut self, cmd: EditCommand) {
        self.execute(cmd, &NoWrap::default());
    }

    /// Sets the text from outside (the `text` prop): a new history, the caret kept where it can be.
    /// Not an `input`.
    pub fn set_text(&mut self, text: &str) {
        if self.doc.text() == text {
            return;
        }
        if self.doc.replace_all(text, ExternalSet::ResetHistory, Some(&mut self.view)).is_ok() {
            self.changes.external = true;
            self.changes.selection = true;
        }
    }

    /// Sets the selection from outside (`selectionStart`/`selectionEnd`, AccessKit), clamped.
    pub fn set_selection(&mut self, anchor: usize, head: usize) {
        let v = self.view.selection_version();
        self.view.set_selection(&self.doc, Selection::single(anchor, head));
        self.changes.selection |= self.view.selection_version() != v;
    }

    pub fn select_all(&mut self) {
        self.run(EditCommand::SelectAll);
    }

    /// The clipboard's text, answering [`TextRequest::ReadClipboard`].
    pub fn paste(&mut self, text: &str) {
        self.run(EditCommand::Insert { text: text.to_string(), kind: InputKind::Paste, distribute: false });
    }

    /// PRIMARY's text, answering [`TextRequest::ReadPrimary`]: pasted where the middle click was.
    pub fn paste_primary(&mut self, text: &str) {
        if let Some(at) = self.paste_at.take() {
            self.run(EditCommand::SetSelection(Selection::point(at)));
        }
        self.paste(text);
    }

    /// Replaces the whole text as a user edit (AccessKit `SetValue`): undoable, an `input`.
    pub fn set_value_by_user(&mut self, text: &str) {
        self.run(EditCommand::SelectAll);
        self.run(EditCommand::ReplaceSelection(text.to_string()));
    }

    /// AccessKit `ReplaceSelectedText`, a drop.
    pub fn replace_selection(&mut self, text: &str) {
        self.run(EditCommand::ReplaceSelection(text.to_string()));
    }

    /// The key-map context of this field.
    pub fn key_cx(&self) -> KeyCx {
        KeyCx {
            platform: self.policy.platform,
            multiline: self.cfg.multiline,
            accepts_tab: self.cfg.accepts_tab,
            hex_entry: self.policy.hex_entry && !self.cfg.password,
            hex_active: self.hex_active(),
        }
    }

    /// A key press in the focused field. Edits are applied; the action says what the kind does with
    /// the rest (submit, cancel, bubble). While an IME composes, keys are the IME's (§11.2).
    pub fn key(&mut self, ev: &KeyEvent, layout: &dyn VisualLines) -> KeyAction {
        if self.composing() && !self.hex_active() {
            return KeyAction::Consumed;
        }
        let action = keys::map_key(self.key_cx(), ev);
        if let KeyAction::Edit(cmd) = &action {
            self.drag = None;
            self.execute(cmd.clone(), layout);
        }
        action
    }

    /// The route of a key for this field (architecture §11.2).
    pub fn route(&self, ev: &KeyEvent) -> keys::KeyRoute {
        keys::route(ev, self.composing() && !self.hex_active(), self.ime_enabled)
    }

    /// A platform IME event (winit-style; the own backends speak [`TextInputClient`] instead).
    ///
    /// [`TextInputClient`]: presto_base::input::TextInputClient
    pub fn ime(&mut self, ev: &ImeEvent) {
        match ev {
            ImeEvent::Enabled => self.ime_enabled = true,
            ImeEvent::Disabled => {
                self.ime_enabled = false;
                if self.composing() {
                    self.run(EditCommand::Preedit(PreeditUpdate::Cancel));
                }
            }
            ImeEvent::Preedit { text, cursor } => {
                if text.is_empty() {
                    self.run(EditCommand::Preedit(PreeditUpdate::Cancel));
                } else {
                    self.begin_composition();
                    let active = cursor.map(|(s, e)| floor_boundary(text, s.min(e))..floor_boundary(text, s.max(e)));
                    self.run(EditCommand::Preedit(PreeditUpdate::Update { text: text.clone(), active }));
                }
            }
            ImeEvent::Commit(t) => self.commit(t, 0, 0),
            ImeEvent::DeleteSurrounding { before, after } => {
                let text = self.text();
                let c = self.caret();
                let prev = text[floor_boundary(&text, c.saturating_sub(*before))..c].chars().count();
                let next = text[c..floor_boundary(&text, (c + after).min(text.len()))].chars().count();
                self.commit("", prev, next);
            }
        }
    }

    fn commit(&mut self, text: &str, prev: usize, next: usize) {
        self.run(EditCommand::Preedit(PreeditUpdate::Commit { text: text.to_string(), replace_prev_chars: prev, replace_next_chars: next }));
    }

    /// A composition starting over a selection replaces it (AppKit, TSF): quire-text draws a
    /// preedit at the caret without deleting the selection, so delete it first.
    fn begin_composition(&mut self) {
        if !self.composing() && !self.selection().range().is_empty() {
            self.run(EditCommand::ReplaceSelection(String::new()));
        }
    }

    // ---- pointer ----

    /// A primary press at document byte `at` with `count` clicks (1, 2, 3+); `extend`: Shift.
    pub fn pointer_down(&mut self, at: usize, count: u32, extend: bool) {
        let len = self.doc.text().len_bytes();
        let at = self.doc.text().clamp_to_boundary(at.min(len));
        let (unit, r) = match count {
            0 | 1 => (DragUnit::Char, at..at),
            2 if !self.cfg.password => (DragUnit::Word, self.rules.words.word_at(self.doc.text(), at)),
            _ => (DragUnit::Line, self.line_at(at)),
        };
        let sel = if count <= 1 && extend {
            let a = self.selection().anchor;
            self.drag = Some(Drag { unit, origin: a..a });
            Selection::single(a, at)
        } else {
            self.drag = Some(Drag { unit, origin: r.clone() });
            Selection::single(r.start, r.end)
        };
        self.run(EditCommand::SetSelection(sel));
    }

    /// The pointer moved to document byte `at` with the button held.
    pub fn pointer_drag(&mut self, at: usize) {
        let Some(d) = self.drag.clone() else { return };
        let at = self.doc.text().clamp_to_boundary(at.min(self.doc.text().len_bytes()));
        let r = match d.unit {
            DragUnit::Char => at..at,
            DragUnit::Word => self.rules.words.word_at(self.doc.text(), at),
            DragUnit::Line => self.line_at(at),
        };
        let sel = if r.start < d.origin.start {
            Selection::single(d.origin.end, r.start)
        } else {
            Selection::single(d.origin.start, r.end.max(d.origin.end))
        };
        self.run(EditCommand::SetSelection(sel));
    }

    pub fn pointer_up(&mut self) {
        self.drag = None;
    }

    pub fn dragging(&self) -> bool {
        self.drag.is_some()
    }

    /// A middle click at `at`: pastes PRIMARY there where the platform has it.
    pub fn middle_click(&mut self, at: usize) -> bool {
        if !self.policy.middle_click_paste || self.cfg.read_only {
            return false;
        }
        self.paste_at = Some(at);
        self.push(TextRequest::ReadPrimary);
        true
    }

    fn line_at(&self, at: usize) -> Range<usize> {
        let t = self.doc.text();
        if self.cfg.multiline {
            let r = t.line_range(t.byte_to_line(at));
            // with its line feed, as triple click selects it
            r.start..(r.end + 1).min(t.len_bytes())
        } else {
            0..t.len_bytes()
        }
    }

    // ---- focus ----

    pub fn focus_in(&mut self, reason: FocusReason) {
        if self.view.set_focused(true) {
            self.doc.close_undo_group();
        }
        let select = !self.cfg.multiline
            && match reason {
                FocusReason::Keyboard => self.policy.select_on_keyboard_focus,
                FocusReason::Programmatic => self.policy.select_on_programmatic_focus,
                FocusReason::Pointer => false,
            };
        if select {
            let len = self.doc.text().len_bytes();
            self.set_selection(0, len);
        }
    }

    pub fn focus_out(&mut self) {
        let composing = self.composing();
        if self.view.set_focused(false) {
            self.doc.close_undo_group();
        }
        self.changes.preedit |= composing;
        self.drag = None;
        if !self.policy.keep_selection_unfocused {
            let h = self.caret();
            self.set_selection(h, h);
        }
    }

    /// Another field took the selection (GTK's one selection owner): collapse to the start.
    pub fn collapse_to_start(&mut self) {
        let s = self.selection().from();
        self.set_selection(s, s);
    }

    // ---- the TextInputClient operations (document-wide UTF-16 offsets) ----

    pub fn ime_text_for_range(&self, r: Utf16Range) -> Option<String> {
        let d = self.ime_doc();
        let a = str_utf16_to_byte(&d.text, r.start);
        let b = str_utf16_to_byte(&d.text, r.end.max(r.start));
        Some(d.text[a..b].to_string())
    }

    pub fn ime_selected_range(&self) -> Option<Utf16Range> {
        let d = self.ime_doc();
        let r = match (d.preedit_range(), self.preedit()) {
            (Some(p), Some(pre)) => match &pre.active {
                Some(a) => p.start + a.start..p.start + a.end,
                None => p.end..p.end,
            },
            _ => {
                let s = self.selection();
                d.to_shown(s.from())..d.to_shown(s.to())
            }
        };
        Some(str_byte_to_utf16(&d.text, r.start)..str_byte_to_utf16(&d.text, r.end))
    }

    pub fn ime_marked_range(&self) -> Option<Utf16Range> {
        if self.hex_active() {
            return None;
        }
        let d = self.ime_doc();
        let p = d.preedit_range()?;
        Some(str_byte_to_utf16(&d.text, p.start)..str_byte_to_utf16(&d.text, p.end))
    }

    /// The document range of an IME-document UTF-16 range, and whether it is the marked text.
    fn ime_to_doc(&self, r: Utf16Range) -> (Range<usize>, bool) {
        let d = self.ime_doc();
        let (a, b) = (str_utf16_to_byte(&d.text, r.start), str_utf16_to_byte(&d.text, r.end.max(r.start)));
        (d.to_doc(a)..d.to_doc(b), d.preedit_range() == Some(a..b))
    }

    pub fn ime_replace(&mut self, range: Option<Utf16Range>, text: &str) {
        match range {
            None => self.commit(text, 0, 0),
            Some(r) => {
                let (dr, marked) = self.ime_to_doc(r);
                if self.composing() && !marked {
                    self.run(EditCommand::Preedit(PreeditUpdate::Cancel));
                }
                if !marked {
                    self.run(EditCommand::SetSelection(Selection::single(dr.start, dr.end)));
                }
                self.commit(text, 0, 0);
            }
        }
    }

    pub fn ime_replace_and_mark(&mut self, range: Option<Utf16Range>, text: &str, new_selected: Option<Utf16Range>) {
        if let Some(r) = range {
            let (dr, marked) = self.ime_to_doc(r);
            if !marked {
                // reconversion of existing text: it becomes the composition
                if self.composing() {
                    self.run(EditCommand::Preedit(PreeditUpdate::Cancel));
                }
                self.run(EditCommand::SetSelection(Selection::single(dr.start, dr.end)));
            }
        }
        if text.is_empty() {
            self.run(EditCommand::Preedit(PreeditUpdate::Cancel));
            return;
        }
        self.begin_composition();
        let active = new_selected.map(|r| str_utf16_to_byte(text, r.start)..str_utf16_to_byte(text, r.end.max(r.start)));
        self.run(EditCommand::Preedit(PreeditUpdate::Update { text: text.to_string(), active }));
    }

    pub fn ime_unmark(&mut self) {
        if self.composing() && !self.hex_active() {
            let t = self.preedit().map(|p| p.text.clone()).unwrap_or_default();
            self.commit(&t, 0, 0);
        }
    }

    /// Text around the caret (at most `max_bytes`) and the selection in it, in bytes; the preedit is
    /// not part of it (Wayland text-input-v3). `None` for a password field.
    pub fn ime_surrounding(&self, max_bytes: usize) -> Option<(String, Range<usize>)> {
        if self.cfg.password {
            return None;
        }
        let text = self.text();
        let s = self.selection();
        let start = if text.len() <= max_bytes { 0 } else { floor_boundary(&text, s.head.saturating_sub(max_bytes / 2)) };
        let end = floor_boundary(&text, (start + max_bytes).min(text.len()));
        let clamp = |b: usize| b.clamp(start, end) - start;
        Some((text[start..end].to_string(), clamp(s.from())..clamp(s.to())))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::policy::Platform;
    use presto_base::event::{KeyLocation, KeyState, Modifiers};
    use presto_base::key::Key;

    fn ed(text: &str, p: Platform) -> TextEditor {
        TextEditor::new(text, EditorConfig::default(), TextPolicy::of(p))
    }

    fn key(e: &mut TextEditor, k: Key, m: Modifiers, text: Option<&str>) -> KeyAction {
        let ev = KeyEvent { key: Some(k), physical: 0, text: text.map(str::to_string), state: KeyState::Pressed, repeat: false, location: KeyLocation::Standard, modifiers: m };
        e.key(&ev, &NoWrap::default())
    }

    fn typ(e: &mut TextEditor, s: &str) {
        for c in s.chars() {
            let t = c.to_string();
            key(e, Key::A, Modifiers::default(), Some(&t));
        }
    }

    fn sel(e: &TextEditor) -> (usize, usize) {
        let s = e.selection();
        (s.anchor, s.head)
    }

    #[test]
    fn typing_motion_and_deletion() {
        let mut e = ed("", Platform::Gtk);
        e.focus_in(FocusReason::Pointer);
        typ(&mut e, "hello world");
        assert_eq!(e.text(), "hello world");
        assert!(e.take_changes().text);
        key(&mut e, Key::ArrowLeft, Modifiers::CTRL, None);
        assert_eq!(e.caret(), 6);
        key(&mut e, Key::Backspace, Modifiers::CTRL, None);
        assert_eq!(e.text(), "world");
        key(&mut e, Key::End, Modifiers::SHIFT, None);
        assert_eq!(sel(&e), (0, 5));
        typ(&mut e, "x");
        assert_eq!(e.text(), "x");
        key(&mut e, Key::Z, Modifiers::CTRL, Some("z"));
        assert_eq!(e.text(), "world");
    }

    #[test]
    fn single_line_strips_breaks_and_enter_submits() {
        let mut e = ed("ab", Platform::Windows);
        e.focus_in(FocusReason::Pointer);
        e.paste("x\ny");
        assert_eq!(e.text(), "xyab");
        assert_eq!(key(&mut e, Key::Enter, Modifiers::default(), Some("\r")), KeyAction::Submit);
        let mut m = TextEditor::new("", EditorConfig { multiline: true, ..EditorConfig::default() }, TextPolicy::of(Platform::Windows));
        m.focus_in(FocusReason::Pointer);
        typ(&mut m, "a");
        key(&mut m, Key::Enter, Modifiers::default(), Some("\r"));
        typ(&mut m, "b");
        assert_eq!(m.text(), "a\nb");
        key(&mut m, Key::ArrowUp, Modifiers::default(), None);
        assert_eq!(m.caret(), 1);
    }

    #[test]
    fn char_limit_password_and_read_only() {
        let mut e = TextEditor::new("", EditorConfig { max_chars: Some(3), ..EditorConfig::default() }, TextPolicy::of(Platform::Gtk));
        e.paste("abcdef");
        assert_eq!(e.text(), "abc");
        assert_eq!(e.take_rejected(), Some(Rejected::MaxChars));

        let mut p = TextEditor::new("secret", EditorConfig { password: true, ..EditorConfig::default() }, TextPolicy::of(Platform::Gtk));
        assert_eq!(p.shown().text, "\u{25CF}".repeat(6));
        p.select_all();
        key(&mut p, Key::C, Modifiers::CTRL, Some("c"));
        assert!(p.take_requests().iter().all(|r| !matches!(r, TextRequest::SetClipboard(_))));
        assert_eq!(p.purpose(), InputPurpose::Password);
        assert_eq!(p.ime_surrounding(64), None);

        let mut r = TextEditor::new("fixed", EditorConfig { read_only: true, ..EditorConfig::default() }, TextPolicy::of(Platform::Qt));
        typ(&mut r, "x");
        assert_eq!(r.text(), "fixed");
        key(&mut r, Key::End, Modifiers::SHIFT, None);
        assert_eq!(sel(&r), (0, 5), "motions work in a read-only field");
    }

    #[test]
    fn clipboard_and_primary_requests() {
        let mut e = ed("one two", Platform::Gtk);
        e.focus_in(FocusReason::Pointer);
        e.pointer_down(1, 2, false);
        assert_eq!(sel(&e), (0, 3));
        let r = e.take_requests();
        assert_eq!(r, vec![TextRequest::SetPrimary("one".into())]);
        key(&mut e, Key::X, Modifiers::CTRL, Some("x"));
        assert!(e.take_requests().contains(&TextRequest::SetClipboard("one".into())));
        assert_eq!(e.text(), " two");
        key(&mut e, Key::V, Modifiers::CTRL, Some("v"));
        assert_eq!(e.take_requests(), vec![TextRequest::ReadClipboard]);
        e.paste("ONE");
        assert_eq!(e.text(), "ONE two");
        assert!(e.middle_click(7));
        assert_eq!(e.take_requests(), vec![TextRequest::ReadPrimary]);
        e.paste_primary("!");
        assert_eq!(e.text(), "ONE two!");
        // no PRIMARY on Windows
        let mut w = ed("abc", Platform::Windows);
        w.pointer_down(1, 2, false);
        assert!(w.take_requests().is_empty());
        assert!(!w.middle_click(0));
    }

    #[test]
    fn multi_click_and_drag_by_words() {
        let mut e = ed("alpha beta gamma", Platform::Qt);
        e.pointer_down(7, 2, false);
        assert_eq!(sel(&e), (6, 10));
        e.pointer_drag(13);
        assert_eq!(sel(&e), (6, 16));
        e.pointer_drag(2);
        assert_eq!(sel(&e), (10, 0));
        e.pointer_up();
        e.pointer_down(3, 3, false);
        assert_eq!(sel(&e), (0, 16), "triple click in a single-line field selects all");
        e.pointer_down(2, 1, false);
        e.pointer_down(9, 1, true);
        assert_eq!(sel(&e), (2, 9), "Shift+click extends");
    }

    #[test]
    fn focus_rules_per_platform() {
        let mut g = ed("text", Platform::Gtk);
        g.focus_in(FocusReason::Keyboard);
        assert_eq!(sel(&g), (0, 4));
        g.focus_out();
        assert_eq!(sel(&g), (0, 4), "GTK keeps an unfocused selection");
        let mut q = ed("text", Platform::Qt);
        q.focus_in(FocusReason::Programmatic);
        assert_eq!(sel(&q), (0, 0), "Qt's setFocus does not select");
        q.focus_in(FocusReason::Keyboard);
        assert_eq!(sel(&q), (0, 4));
        q.focus_out();
        assert_eq!(sel(&q), (4, 4), "Qt collapses an unfocused selection");
    }

    #[test]
    fn ime_composition_through_events() {
        let mut e = ed("ab", Platform::Gtk);
        e.focus_in(FocusReason::Pointer);
        e.set_selection(1, 1);
        e.ime(&ImeEvent::Enabled);
        e.ime(&ImeEvent::Preedit { text: "にほ".into(), cursor: Some((6, 6)) });
        assert!(e.composing());
        assert_eq!(e.text(), "ab", "the preedit is not in the document");
        assert_eq!(e.shown().text, "aにほb");
        // keys are the IME's while it composes
        assert_eq!(key(&mut e, Key::Backspace, Modifiers::default(), None), KeyAction::Consumed);
        e.ime(&ImeEvent::Commit("日本".into()));
        assert!(!e.composing());
        assert_eq!(e.text(), "a日本b");
        assert_eq!(e.caret(), 7);
        // one undo step for the commit
        key(&mut e, Key::Z, Modifiers::CTRL, Some("z"));
        assert_eq!(e.text(), "ab");
        // a composition over a selection replaces it
        e.set_selection(0, 2);
        e.ime(&ImeEvent::Preedit { text: "x".into(), cursor: None });
        assert_eq!(e.text(), "");
        e.ime(&ImeEvent::Preedit { text: String::new(), cursor: None });
        assert!(!e.composing());
        // DeleteSurrounding counts bytes
        e.set_text("héllo");
        e.set_selection(3, 3);
        // two bytes before the caret are `é`, one after is `l`
        e.ime(&ImeEvent::DeleteSurrounding { before: 2, after: 1 });
        assert_eq!(e.text(), "hlo");
    }

    #[test]
    fn text_input_client_offsets_are_utf16() {
        // 😀 is two UTF-16 units and four bytes
        let mut e = ed("a😀b", Platform::Mac);
        e.focus_in(FocusReason::Pointer);
        e.set_selection(5, 5);
        assert_eq!(e.ime_selected_range(), Some(3..3));
        assert_eq!(e.ime_text_for_range(1..3).as_deref(), Some("😀"));
        e.ime_replace_and_mark(None, "かな", Some(2..2));
        assert_eq!(e.ime_marked_range(), Some(3..5));
        assert_eq!(e.ime_selected_range(), Some(5..5));
        assert_eq!(e.ime_text_for_range(0..6).as_deref(), Some("a😀かなb"));
        e.ime_replace(None, "仮名");
        assert_eq!(e.text(), "a😀仮名b");
        assert_eq!(e.ime_marked_range(), None);
        assert_eq!(e.ime_selected_range(), Some(5..5));
        // replacing a range outside a composition (an autocorrection)
        e.ime_replace(Some(0..1), "A");
        assert_eq!(e.text(), "A😀仮名b");
        // reconversion: existing text becomes the marked text
        e.ime_replace_and_mark(Some(3..5), "かめい", Some(3..3));
        assert_eq!(e.text(), "A😀b");
        assert_eq!(e.ime_marked_range(), Some(3..6));
        e.ime_unmark();
        assert_eq!(e.text(), "A😀かめいb");
        let (around, s) = e.ime_surrounding(1024).unwrap();
        assert_eq!(around, "A😀かめいb");
        assert_eq!(s, 14..14);
    }

    #[test]
    fn gtk_hex_entry() {
        let mut e = ed("", Platform::Gtk);
        e.focus_in(FocusReason::Pointer);
        key(&mut e, Key::U, Modifiers::CTRL.union(Modifiers::SHIFT), Some("U"));
        assert!(e.hex_active());
        for c in ["e", "9"] {
            key(&mut e, Key::A, Modifiers::default(), Some(c));
        }
        assert_eq!(e.shown().text, "ue9");
        key(&mut e, Key::Space, Modifiers::default(), Some(" "));
        assert_eq!(e.text(), "\u{e9}");
        assert!(!e.hex_active());
    }
}
