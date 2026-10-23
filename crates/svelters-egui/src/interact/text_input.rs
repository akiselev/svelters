//! `TextInput` (OW4, ARCH-WIDGETS §1.2, §4): a text field's behaviour on TC0 (`quire-text`).
//!
//! The unit owns the field's **session**: a `TextDocument` and one `TextView`, created lazily (an
//! unfocused, never-edited field holds none, §4.6) and kept for the node's life so undo survives
//! blur and refocus (GTK and Qt keep a field's undo stack across focus changes). It maps this
//! pass's input to TC0's `EditCommand`s through the platform's key table and rules
//! ([`TextPolicy`](crate::native::TextPolicy)), and reports what happened as [`TextIntent`]s; the
//! widget (`widgets::text_edit`) writes the prop, emits the signals and answers the host requests
//! (clipboard, PRIMARY).
//!
//! - **Keys**: the chords of each platform (Ctrl or Option words, Cmd line ends on macOS, the CUA
//!   chords, GTK's Ctrl+Shift+U hex entry), Enter (commit and submit in a one-line field), Tab
//!   (a multi-line field with `tab-focus` off inserts it), and all keys while composing.
//! - **Pointer**: own click counting at press time (`Policy::double_click_ms`), so a double or
//!   triple click selects on the press and a drag after it extends by words or lines (GTK, AppKit,
//!   WinUI; Qt extends by characters from the word's start); Shift+click extends; a middle click
//!   pastes PRIMARY where the platform has one.
//! - **Focus**: keyboard focus into a one-line field selects all of it (and programmatic focus on
//!   GTK and AppKit), the unfocused selection is kept, collapsed or cleared per platform, and blur
//!   after an edit commits.
//! - **IME**: `egui::ImeEvent` preedits and commits; AccessKit `SetValue` and
//!   `ReplaceSelectedText`.

use super::{InputCx, Intents, Unit};
use crate::native::{TextPolicy, Unfocused};
use crate::provider::StateBits;
use egui::text::CCursor;
use egui::{Event, Key, Modifiers, PointerButton, Pos2, accesskit};
use quire_text::{
    Dir, DocRules, EditCommand, EditRules, Executed, ExternalSet, Goal, HexKey, HostRequest, InputKind, LineMode, NoWrap, PreeditKind, PreeditUpdate, Rejected, Selection,
    Text, TextDocument, TextView, Unit as Step, VisualLine, VisualLines, WordStop,
};
use std::ops::Range;
use std::sync::Arc;

/// What the widget configures (from its props).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextParams {
    /// Several lines: Enter inserts a line break, Up/Down move by rows.
    pub multiline: bool,
    /// A password field: no copy, no PRIMARY, word motions go to the ends.
    pub secret: bool,
    /// Motions, selection and copy only.
    pub read_only: bool,
    /// At most this many characters (code points).
    pub max_chars: Option<u32>,
    /// A multi-line field inserts Tab instead of moving focus with it.
    pub tab_inserts: bool,
    /// Undo and redo are off (`undo={false}`).
    pub no_undo: bool,
}

/// What [`TextInput`] reports.
#[derive(Clone, Debug, PartialEq)]
pub enum TextIntent {
    /// The user changed the text.
    Edited,
    /// The selection or caret moved.
    SelectionChanged,
    /// The user finished: Enter in a one-line field (also when a default button took it), or
    /// focus left after an edit since the last commit.
    Committed,
    /// Enter in a one-line field (Ctrl+Enter in a multi-line one).
    Submitted,
    /// A command did nothing or less (TC0's reason).
    Rejected(Rejected),
    /// Something only the host can do (clipboard, PRIMARY, the bell).
    Request(HostRequest),
    /// A middle click asks to paste PRIMARY at this byte (the widget reads it).
    PastePrimary { at: usize, text: Option<String> },
    Gained,
    Lost,
}

/// The field's TC0 document, view and cached rules.
pub struct Session {
    pub doc: TextDocument,
    pub view: TextView,
    edit: EditRules,
    key: (TextPolicy, bool, TextParams),
}

impl Session {
    fn rules(policy: &crate::native::Policy, p: &TextParams) -> (Arc<DocRules>, EditRules) {
        let t = &policy.text;
        let lines = if p.multiline { LineMode::Multi } else { LineMode::Single { breaks: t.paste_breaks } };
        let mut d = DocRules::field(lines, t.undo);
        d.max_chars = p.max_chars;
        d.read_only = p.read_only;
        let mut e = EditRules::field(t.words.rules());
        e.word_move_collapses = policy.word_move_collapses;
        e.secret = p.secret;
        e.primary = t.primary;
        e.backspace = quire_text::DeleteUnit::Grapheme;
        (Arc::new(d), e)
    }

    fn new(text: &str, policy: &crate::native::Policy, p: &TextParams) -> Session {
        let (d, edit) = Session::rules(policy, p);
        let doc = TextDocument::new(text, d);
        let mut view = TextView::new(&doc);
        // egui's (and GTK's) default: the caret at the end
        view.set_selection(&doc, Selection::point(doc.text().len_bytes()));
        Session { doc, view, edit, key: (policy.text, policy.word_move_collapses, *p) }
    }

    /// Rebuilds the rules when the props or the policy changed.
    fn update(&mut self, policy: &crate::native::Policy, p: &TextParams) {
        let key = (policy.text, policy.word_move_collapses, *p);
        if key != self.key {
            let (d, e) = Session::rules(policy, p);
            self.doc.set_rules(d);
            self.edit = e;
            self.key = key;
        }
    }

    pub fn text(&self) -> &Text {
        self.doc.text()
    }

    /// The selection as char offsets `[anchor, caret]`.
    pub fn chars(&self) -> [usize; 2] {
        let p = self.view.selection().primary();
        let t = self.doc.text();
        [t.byte_to_char(p.anchor), t.byte_to_char(p.head)]
    }
}

/// How the galley lays the field's text out, for hit-testing and rows (set by the widget before
/// the unit runs: last pass's galley, where it was drawn).
#[derive(Clone)]
pub struct FieldLayout {
    pub galley: Arc<egui::Galley>,
    /// The galley's top left on screen.
    pub origin: Pos2,
    /// The galley shows the document's text char for char (no preedit, line breaks as such).
    pub exact: bool,
}

impl FieldLayout {
    /// The char under `pos` (screen).
    pub fn char_at(&self, pos: Pos2) -> usize {
        self.galley.cursor_from_pos(pos - self.origin).index.0
    }
}

/// The rows of a wrapped multi-line galley as quire-text's [`VisualLines`] (goal: x in 1/16 pt).
struct GalleyRows<'a> {
    galley: &'a egui::Galley,
    /// Per row: char range (without the line feed), paragraph, row within it.
    rows: Vec<(Range<usize>, usize, usize)>,
    page: usize,
}

impl<'a> GalleyRows<'a> {
    fn new(galley: &'a egui::Galley, page: usize) -> GalleyRows<'a> {
        let mut rows = Vec::with_capacity(galley.rows.len());
        let (mut c, mut para, mut sub) = (0usize, 0usize, 0usize);
        for r in &galley.rows {
            let n = r.row.glyphs.len();
            rows.push((c..c + n, para, sub));
            c += n;
            if r.ends_with_newline {
                c += 1;
                para += 1;
                sub = 0;
            } else {
                sub += 1;
            }
        }
        GalleyRows { galley, rows, page }
    }

    fn flat(&self, l: VisualLine) -> Option<usize> {
        self.rows.iter().position(|r| r.1 == l.line && r.2 == l.row)
    }

    fn x_of(&self, c: usize, next_row: bool) -> f32 {
        let mut cc = CCursor::new(c);
        cc.prefer_next_row = next_row;
        self.galley.pos_from_cursor(cc).min.x
    }
}

impl VisualLines for GalleyRows<'_> {
    fn locate(&self, text: &Text, byte: usize, upstream: bool) -> (VisualLine, Goal) {
        let c = text.byte_to_char(byte);
        let mut k = self.rows.iter().position(|r| r.0.contains(&c) || (c == r.0.end)).unwrap_or(self.rows.len().saturating_sub(1));
        // at a soft wrap the caret is drawn at the next row's start unless upstream
        if !upstream && k + 1 < self.rows.len() && self.rows[k].0.end == c && self.rows[k + 1].0.start == c {
            k += 1;
        }
        let r = &self.rows[k];
        let x = self.x_of(c, !upstream);
        (VisualLine { line: r.1, row: r.2 }, Goal((x.max(0.0) * 16.0) as u32))
    }

    fn offset(&self, _text: &Text, line: VisualLine, delta: isize) -> Result<VisualLine, quire_text::Edge> {
        let k = self.flat(line).unwrap_or(0) as isize + delta;
        if k < 0 {
            Err(quire_text::Edge::Start)
        } else if k as usize >= self.rows.len() {
            Err(quire_text::Edge::End)
        } else {
            let r = &self.rows[k as usize];
            Ok(VisualLine { line: r.1, row: r.2 })
        }
    }

    fn byte_at(&self, text: &Text, line: VisualLine, goal: Goal) -> (usize, bool) {
        let k = self.flat(line).unwrap_or(0);
        let (range, ..) = self.rows[k].clone();
        let soft = k + 1 < self.rows.len() && self.rows[k + 1].0.start == range.end;
        let c = if goal == Goal::END {
            range.end
        } else {
            let row = &self.galley.rows[k];
            let x = goal.0 as f32 / 16.0;
            let y = row.pos.y + row.row.size.y / 2.0;
            self.galley.cursor_from_pos(egui::vec2(x, y)).index.0.clamp(range.start, range.end)
        };
        (text.char_to_byte(c), soft && c == range.end)
    }

    fn range(&self, text: &Text, line: VisualLine) -> Range<usize> {
        let k = self.flat(line).unwrap_or(0);
        let r = &self.rows[k].0;
        text.char_to_byte(r.start)..text.char_to_byte(r.end)
    }

    fn lines_per_page(&self) -> usize {
        self.page
    }
}

/// A press and the drag after it.
#[derive(Clone, Debug)]
enum Gesture {
    /// A single press: extends from this byte.
    Chars(usize),
    /// A double press: the word pressed.
    Words(Range<usize>),
    /// A triple press: the line (or all of a one-line field).
    Lines(Range<usize>),
}

/// The TextInput unit (ARCH-WIDGETS §1.2): the field's session and its interaction state.
#[derive(Default)]
pub struct TextInput {
    session: Option<Session>,
    /// Last pass's galley (the widget sets it).
    pub layout: Option<FieldLayout>,
    /// Focus came from Tab or Shift+Tab this pass (the widget sets it: `native::tab_moved_focus`).
    pub tab_focus: bool,
    /// EV3 took this one-line field's Enter for the scope's default button (`focus::take_entered`).
    pub entered: bool,
    focused: bool,
    /// A press of ours asked for the focus that follows.
    press_focus: bool,
    edited: bool,
    gesture: Option<Gesture>,
    clicks: (u32, f64, Pos2),
    /// A middle press waiting for its PRIMARY text (the byte it was at).
    middle: Option<usize>,
    /// A Space committed the hex entry: its `Text(" ")` is not typed.
    eat_space: bool,
    /// AppKit: the selection's direction is not set (after select-all or a mouse word selection).
    undirected: bool,
    /// The last edit or caret move (input time), for the caret blink phase.
    pub last_activity: f64,
    /// Rows per page of a multi-line field (from its height).
    pub page_rows: usize,
}

impl std::fmt::Debug for TextInput {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextInput").field("session", &self.session.is_some()).field("focused", &self.focused).finish()
    }
}

impl TextInput {
    /// The session, if the field has one.
    pub fn session(&self) -> Option<&Session> {
        self.session.as_ref()
    }

    /// Creates the session from the field's `value` (lazily: the widget calls this when focus,
    /// a press or a request reaches the field), or syncs it with a `value` set from outside: an
    /// echo of our own write is a no-op, anything else replaces the text and starts a new history
    /// (GTK `set_text`, Qt `setText`: FX3).
    pub fn sync(&mut self, value: &str, policy: &crate::native::Policy, p: &TextParams, create: bool) {
        match &mut self.session {
            Some(s) => {
                s.update(policy, p);
                let same = match s.doc.text().as_str() {
                    Some(t) => t == value || (value.contains('\r') && t == quire_text::text::normalize_line_endings(value)),
                    None => s.doc.text().to_string() == value,
                };
                if !same {
                    let _ = s.doc.replace_all(value, ExternalSet::ResetHistory, Some(&mut s.view));
                    let len = s.doc.text().len_bytes();
                    let sel = s.view.selection().primary();
                    s.view.set_selection(&s.doc, Selection::single(sel.anchor.min(len), sel.head.min(len)));
                    self.edited = false;
                }
            }
            None if create => self.session = Some(Session::new(value, policy, p)),
            None => {}
        }
    }

    /// Sets the selection directly, char offsets `[anchor, caret]` (tests, the measurement
    /// harness, `set-selection`): view state, no undo step.
    pub fn set_selection_chars(&mut self, anchor: usize, caret: usize) {
        if let Some(s) = &mut self.session {
            let t = s.doc.text();
            let n = t.len_chars();
            let sel = Selection::single(t.char_to_byte(anchor.min(n)), t.char_to_byte(caret.min(n)));
            s.view.set_selection(&s.doc, sel);
        }
    }

    /// Whether an input method or the hex entry is composing.
    pub fn is_composing(&self) -> bool {
        self.session.as_ref().is_some_and(|s| s.view.composing())
    }

    /// Runs `cmd` on the session (the widget's own commands: a PRIMARY paste, a collapse asked by
    /// the selection owner rule).
    pub fn exec(&mut self, cmd: EditCommand, now: f64, out: &mut Intents<TextIntent>) {
        let layout = self.layout.clone();
        let page = self.page_rows.max(1);
        let Some(s) = &mut self.session else { return };
        let ex = run(s, cmd, layout.as_ref(), page, now);
        self.after(ex, now, out);
    }

    fn after(&mut self, ex: Executed, now: f64, out: &mut Intents<TextIntent>) {
        if let Some(a) = &ex.applied {
            self.last_activity = now;
            if a.changed_text() {
                self.edited = true;
                out.push(TextIntent::Edited);
            }
            if a.selection_changed || a.changed_text() {
                out.push(TextIntent::SelectionChanged);
            }
        }
        for r in ex.requests {
            out.push(TextIntent::Request(r));
        }
        if let Some(r) = ex.rejected {
            out.push(TextIntent::Rejected(r));
        }
    }

    fn cmd(&mut self, cmd: EditCommand, now: f64, out: &mut Intents<TextIntent>) {
        if !matches!(cmd, EditCommand::Move { extend: true, .. }) {
            self.undirected = false;
        }
        self.exec(cmd, now, out);
    }

    fn byte_of_char(&self, c: usize) -> usize {
        self.session.as_ref().map_or(0, |s| {
            let t = s.doc.text();
            t.char_to_byte(c.min(t.len_chars()))
        })
    }

    /// The byte under `pos`.
    fn byte_at(&self, pos: Pos2) -> usize {
        let c = self.layout.as_ref().map_or(0, |l| l.char_at(pos));
        // a preedit shown in the galley shifts what follows the caret
        let c = match (&self.session, &self.layout) {
            (Some(s), Some(l)) if !l.exact => {
                let caret = s.doc.text().byte_to_char(s.view.selection().primary().head);
                let n = s.view.preedit().map_or(0, |p| p.text.chars().count());
                if c > caret { c.saturating_sub(n).max(caret) } else { c }
            }
            _ => c,
        };
        self.byte_of_char(c)
    }

    fn word_at(&self, b: usize) -> Range<usize> {
        let s = self.session.as_ref().expect("session");
        s.edit.words.word_at(s.doc.text(), b)
    }

    fn line_at(&self, b: usize, multiline: bool) -> Range<usize> {
        let s = self.session.as_ref().expect("session");
        let t = s.doc.text();
        if multiline { quire_text::words::line_range_at(t, b) } else { 0..t.len_bytes() }
    }

    fn select(&mut self, anchor: usize, head: usize, now: f64, out: &mut Intents<TextIntent>) {
        self.cmd(EditCommand::SetSelection(Selection::single(anchor, head)), now, out);
    }

    fn pointer(&mut self, p: &TextParams, cx: &mut InputCx<'_>, out: &mut Intents<TextIntent>) {
        let t = &cx.policy.text;
        let on_us = cx.resp.hovered || (cx.resp.contains_pointer && cx.resp.enabled);
        let now = cx.now;
        let events: Vec<(Pos2, PointerButton, bool, Modifiers)> = cx
            .input
            .events
            .iter()
            .filter_map(|e| match e {
                Event::PointerButton { pos, button, pressed, modifiers } => Some((*pos, *button, *pressed, *modifiers)),
                _ => None,
            })
            .collect();
        let middle_paste = cx.input.events.iter().find_map(|e| match e {
            Event::MiddleClickPaste { text, .. } => Some(text.clone()),
            _ => None,
        });
        // egui-winit sends `MiddleClickPaste` with the middle press (a pass of its own under
        // kittest): it pastes where the press was; a release without it reads the provider
        if let (Some(text), Some(at)) = (&middle_paste, self.middle) {
            self.middle = None;
            out.push(TextIntent::PastePrimary { at, text: Some(text.clone()) });
        }
        for (pos, button, pressed, m) in events {
            if !pressed {
                if button == PointerButton::Primary {
                    self.gesture = None;
                }
                if button == PointerButton::Middle
                    && let Some(at) = self.middle.take()
                {
                    out.push(TextIntent::PastePrimary { at, text: None });
                }
                continue;
            }
            if !on_us || !cx.resp.rect.contains(pos) {
                continue;
            }
            match button {
                PointerButton::Primary => {
                    let (n, t0, p0) = self.clicks;
                    let close = now - t0 <= cx.policy.double_click_ms as f64 / 1000.0 && (pos - p0).length() < 6.0;
                    let count = if close { (n % 3) + 1 } else { 1 };
                    self.clicks = (count, now, pos);
                    if !cx.focus.has {
                        cx.focus.request();
                        self.press_focus = true;
                    }
                    let at = self.byte_at(pos);
                    match count {
                        1 => {
                            let head = self.session.as_ref().map_or(at, |s| s.view.selection().primary().anchor);
                            if m.shift && cx.focus.has {
                                self.select(head, at, now, out);
                            } else {
                                self.select(at, at, now, out);
                            }
                            let anchor = self.session.as_ref().map_or(at, |s| s.view.selection().primary().anchor);
                            self.gesture = Some(Gesture::Chars(anchor));
                        }
                        2 => {
                            let w = self.word_at(at);
                            self.select(w.start, w.end, now, out);
                            self.undirected = t.select_all_undirected;
                            self.gesture = Some(Gesture::Words(w));
                        }
                        _ => {
                            let l = self.line_at(at, p.multiline);
                            self.select(l.start, l.end, now, out);
                            self.undirected = t.select_all_undirected;
                            self.gesture = Some(Gesture::Lines(l));
                        }
                    }
                }
                PointerButton::Middle if t.middle_click_paste => {
                    if !cx.focus.has {
                        cx.focus.request();
                        self.press_focus = true;
                    }
                    let at = self.byte_at(pos);
                    match &middle_paste {
                        Some(text) => out.push(TextIntent::PastePrimary { at, text: Some(text.clone()) }),
                        None => self.middle = Some(at),
                    }
                }
                _ => {}
            }
        }
        // a drag after the press
        if let Some(g) = self.gesture.clone()
            && cx.input.pointer.primary_down()
            && cx.input.pointer.is_moving()
            && let Some(pos) = cx.input.pointer.latest_pos()
        {
            let at = self.byte_at(pos);
            let words = !matches!(g, Gesture::Chars(_));
            let (a, h) = match g {
                Gesture::Chars(anchor) => (anchor, at),
                Gesture::Words(w) | Gesture::Lines(w) => {
                    let lines = matches!(self.gesture, Some(Gesture::Lines(_)));
                    if !t.word_drag && !lines {
                        // Qt: by characters, from the word's start
                        (w.start, at)
                    } else {
                        let u = if lines { self.line_at(at, p.multiline) } else { self.word_at(at) };
                        if at >= w.start { (w.start, u.end.max(w.end)) } else { (w.end, u.start.min(w.start)) }
                    }
                }
            };
            // AppKit writes a mouse word selection without a direction
            let (a, h) = if self.undirected && words { (a.min(h), a.max(h)) } else { (a, h) };
            let undirected = self.undirected;
            self.select(a, h, cx.now, out);
            self.undirected = undirected;
        }
    }

    fn keys(&mut self, p: &TextParams, cx: &mut InputCx<'_>, out: &mut Intents<TextIntent>) {
        let pol = cx.policy;
        let t = pol.text;
        let now = cx.now;
        let hex = |s: &Option<Session>| s.as_ref().is_some_and(|s| s.view.preedit_kind() == PreeditKind::Hex && s.view.composing());
        let events = cx.input.events.clone();
        for e in events {
            match e {
                Event::Text(text) => {
                    if hex(&self.session) {
                        for c in text.chars() {
                            if let Some(d) = c.to_digit(16) {
                                self.cmd(EditCommand::HexEntry(HexKey::Digit(d as u8)), now, out);
                            }
                        }
                        continue;
                    }
                    if self.eat_space && text == " " {
                        self.eat_space = false;
                        continue;
                    }
                    if self.is_composing() || text.chars().all(|c| c.is_control()) {
                        continue;
                    }
                    self.cmd(EditCommand::Insert { text, kind: InputKind::Typing, distribute: false }, now, out);
                }
                Event::Copy => self.cmd(EditCommand::Copy, now, out),
                Event::Cut => self.cmd(EditCommand::Cut, now, out),
                Event::Paste(text) => self.cmd(EditCommand::Insert { text, kind: InputKind::Paste, distribute: false }, now, out),
                Event::Ime(ime) => match ime {
                    egui::ImeEvent::Preedit { text, active_range_chars } => {
                        if text.is_empty() {
                            self.cmd(EditCommand::Preedit(PreeditUpdate::Cancel), now, out);
                        } else {
                            let active = active_range_chars.map(|r| {
                                let b = |c: usize| text.char_indices().nth(c).map_or(text.len(), |(i, _)| i);
                                b(r.start)..b(r.end)
                            });
                            self.cmd(EditCommand::Preedit(PreeditUpdate::Update { text, active }), now, out);
                        }
                    }
                    egui::ImeEvent::Commit(text) => {
                        self.cmd(EditCommand::Preedit(PreeditUpdate::Commit { text, replace_prev_chars: 0, replace_next_chars: 0 }), now, out);
                    }
                    egui::ImeEvent::DeleteSurrounding { before_chars, after_chars } => {
                        self.cmd(EditCommand::Preedit(PreeditUpdate::Commit { text: String::new(), replace_prev_chars: before_chars, replace_next_chars: after_chars }), now, out);
                    }
                    _ => {}
                },
                Event::Key { key, pressed: true, modifiers: m, .. } => {
                    if hex(&self.session) {
                        let k = match key {
                            Key::Space | Key::Enter => Some(HexKey::Commit),
                            Key::Escape => Some(HexKey::Cancel),
                            Key::Backspace => Some(HexKey::Backspace),
                            _ => None,
                        };
                        if let Some(k) = k {
                            self.eat_space = key == Key::Space;
                            self.cmd(EditCommand::HexEntry(k), now, out);
                        }
                        continue;
                    }
                    if self.is_composing() {
                        continue;
                    }
                    for c in keymap(&t, p, key, m) {
                        match c {
                            Act::Cmd(c) => {
                                if p.no_undo && matches!(c, EditCommand::Undo | EditCommand::Redo) {
                                    continue;
                                }
                                if let Some(sel) = self.undirected_extend(&c) {
                                    self.select(sel.0, sel.1, now, out);
                                    continue;
                                }
                                let all = matches!(c, EditCommand::SelectAll);
                                // AppKit: a Shift+word move across the anchor stops at the anchor
                                let before = self.session.as_ref().map(|s| s.view.selection().primary());
                                let word_extend = t.word_extend_stops_at_anchor && matches!(c, EditCommand::Move { unit: Step::Word(_), extend: true, .. });
                                self.cmd(c, now, out);
                                if word_extend
                                    && let (Some(b), Some(a)) = (before, self.session.as_ref().map(|s| s.view.selection().primary()))
                                    && b.head != b.anchor
                                    && (b.head < b.anchor) != (a.head < a.anchor)
                                    && a.head != a.anchor
                                {
                                    self.select(b.anchor, b.anchor, now, out);
                                }
                                if all {
                                    self.undirected = t.select_all_undirected;
                                }
                            }
                            Act::Enter => {
                                if p.multiline {
                                    self.cmd(EditCommand::Insert { text: "\n".into(), kind: InputKind::Typing, distribute: false }, now, out);
                                } else {
                                    self.commit(out);
                                    out.push(TextIntent::Submitted);
                                    if t.return_selects_all {
                                        self.cmd(EditCommand::SelectAll, now, out);
                                        self.undirected = true;
                                    }
                                }
                            }
                            Act::Submit => {
                                self.commit(out);
                                out.push(TextIntent::Submitted);
                            }
                            Act::Tab if p.multiline && p.tab_inserts => {
                                self.cmd(EditCommand::Insert { text: "\t".into(), kind: InputKind::Typing, distribute: false }, now, out);
                            }
                            Act::Tab => {}
                            Act::Request(r) => out.push(TextIntent::Request(r)),
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// AppKit: Shift+Left/Right after select-all keeps everything selected and only fixes the
    /// anchor at the far end (WG3m pin 2).
    fn undirected_extend(&mut self, c: &EditCommand) -> Option<(usize, usize)> {
        if !self.undirected {
            return None;
        }
        let EditCommand::Move { unit: Step::Grapheme | Step::Word(_), dir, extend: true } = c else { return None };
        let s = self.session.as_ref()?;
        let r = s.view.selection().primary();
        let len = s.doc.text().len_bytes();
        if r.from() != 0 || r.to() != len {
            return None;
        }
        self.undirected = false;
        Some(if *dir == Dir::Back { (len, 0) } else { (0, len) })
    }

    fn commit(&mut self, out: &mut Intents<TextIntent>) {
        self.edited = false;
        out.push(TextIntent::Committed);
    }

    fn focus(&mut self, p: &TextParams, cx: &mut InputCx<'_>, out: &mut Intents<TextIntent>) {
        let has = cx.focus.has && cx.enabled;
        if has == self.focused {
            return;
        }
        self.focused = has;
        let now = cx.now;
        let pol = cx.policy;
        if has {
            let by_pointer = std::mem::take(&mut self.press_focus) || cx.focus.pointer_pressed;
            if let Some(s) = &mut self.session {
                s.view.set_focused(true);
                s.doc.close_undo_group();
            }
            out.push(TextIntent::Gained);
            let select = if by_pointer {
                false
            } else if self.tab_focus {
                pol.select_on_keyboard_focus
            } else {
                pol.select_on_programmatic_focus
            };
            if select && !p.multiline && self.session.is_some() {
                let len = self.session.as_ref().map_or(0, |s| s.doc.text().len_bytes());
                // as GTK and Qt: caret at the end
                if let Some(s) = &mut self.session {
                    s.view.set_selection(&s.doc, Selection::single(0, len));
                }
                self.undirected = pol.text.select_all_undirected;
                out.push(TextIntent::SelectionChanged);
            }
            self.last_activity = now;
        } else {
            self.gesture = None;
            if let Some(s) = &mut self.session {
                s.view.set_focused(false);
                s.doc.close_undo_group();
                let r = s.view.selection().primary();
                let sel = match pol.text.unfocused {
                    Unfocused::Keep => None,
                    Unfocused::Collapse => Some(Selection::point(r.head)),
                    Unfocused::Clear => Some(Selection::point(0)),
                };
                if let Some(sel) = sel
                    && sel != *s.view.selection()
                {
                    s.view.set_selection(&s.doc, sel);
                    out.push(TextIntent::SelectionChanged);
                }
            }
            if self.edited {
                self.commit(out);
            }
            out.push(TextIntent::Lost);
        }
    }
}

/// What a key does.
enum Act {
    Cmd(EditCommand),
    Enter,
    Submit,
    Tab,
    Request(HostRequest),
}

fn mv(unit: Step, dir: Dir, extend: bool) -> Act {
    Act::Cmd(EditCommand::Move { unit, dir, extend })
}

fn del(unit: Step, dir: Dir) -> Act {
    Act::Cmd(EditCommand::Delete { unit, dir })
}

/// The platform's key table (FX1's word keys, FX3's chords, WG3m's macOS keys).
fn keymap(t: &TextPolicy, p: &TextParams, key: Key, m: Modifiers) -> Vec<Act> {
    let (word, command) = if t.mac_keys { (m.alt && !m.mac_cmd && !m.ctrl, m.mac_cmd || (m.command && !m.ctrl)) } else { (m.ctrl && !m.alt, (m.ctrl || m.command) && !m.alt) };
    let shift = m.shift;
    let word_stop = Step::Word(WordStop::Native);
    let back = Dir::Back;
    let fwd = Dir::Forward;
    let one = |a| vec![a];
    match key {
        Key::ArrowLeft | Key::ArrowRight => {
            let dir = if key == Key::ArrowLeft { back } else { fwd };
            if t.mac_keys && command {
                one(mv(Step::LineEdge, dir, shift))
            } else if word {
                one(mv(word_stop, dir, shift))
            } else if m.alt || (command && !t.mac_keys) {
                vec![]
            } else {
                one(mv(Step::Grapheme, dir, shift))
            }
        }
        Key::ArrowUp | Key::ArrowDown => {
            let dir = if key == Key::ArrowUp { back } else { fwd };
            if t.mac_keys && command {
                one(mv(Step::DocEdge, dir, shift))
            } else if p.multiline && !m.alt && !command {
                one(mv(Step::VisualLine, dir, shift))
            } else if !p.multiline && t.mac_keys && !m.alt {
                // NSTextField: Up and Down go to the ends
                one(mv(Step::DocEdge, dir, shift))
            } else {
                vec![]
            }
        }
        Key::PageUp | Key::PageDown if p.multiline => one(mv(Step::Page, if key == Key::PageUp { back } else { fwd }, shift)),
        Key::Home | Key::End => {
            let dir = if key == Key::Home { back } else { fwd };
            if t.mac_keys || (command && !t.mac_keys) {
                // macOS: Home and End go to the text's ends (NSTextView scrolls; a field has one line)
                one(mv(Step::DocEdge, dir, shift))
            } else {
                one(mv(Step::LineEdge, dir, shift))
            }
        }
        Key::Backspace => {
            if t.mac_keys && command {
                one(del(Step::LineEdge, back))
            } else if word {
                one(del(word_stop, back))
            } else if !command {
                one(del(Step::Grapheme, back))
            } else {
                vec![]
            }
        }
        Key::Delete => {
            if t.cua_keys && m.shift_only() {
                one(Act::Cmd(EditCommand::Cut))
            } else if word {
                one(del(word_stop, fwd))
            } else if !command && !m.alt && !shift {
                one(del(Step::Grapheme, fwd))
            } else {
                vec![]
            }
        }
        Key::Insert if t.cua_keys && command && !shift => one(Act::Cmd(EditCommand::Copy)),
        Key::Insert if t.cua_keys && m.shift_only() => one(Act::Request(HostRequest::ReadClipboard)),
        Key::A if command && !shift => one(Act::Cmd(EditCommand::SelectAll)),
        Key::Z if command => one(Act::Cmd(if shift { EditCommand::Redo } else { EditCommand::Undo })),
        Key::Y if command && !shift && !t.mac_keys => one(Act::Cmd(EditCommand::Redo)),
        Key::U if t.hex_entry && m.ctrl && shift && !m.alt => one(Act::Cmd(EditCommand::HexEntry(HexKey::Start))),
        Key::Enter if command && p.multiline => one(Act::Submit),
        Key::Enter if !command && !m.alt => one(Act::Enter),
        Key::Tab if !m.any() => one(Act::Tab),
        _ => vec![],
    }
}

/// Executes `cmd` with the field's layout (a wrapped galley's rows when it matches the text).
fn run(s: &mut Session, cmd: EditCommand, layout: Option<&FieldLayout>, page: usize, now: f64) -> Executed {
    let time = Some((now * 1000.0) as u64);
    let rows = layout.filter(|l| l.exact && l.galley.rows.len() > 1 && l.galley.text().chars().count() == s.doc.text().len_chars()).map(|l| GalleyRows::new(&l.galley, page));
    match &rows {
        Some(r) => s.doc.execute(&mut s.view, cmd, &s.edit, r, time),
        None => s.doc.execute(&mut s.view, cmd, &s.edit, &NoWrap { tab_size: 4, page_lines: page as u32 }, time),
    }
}

impl Unit for TextInput {
    type Params = TextParams;
    type Intent = TextIntent;
    const OWNS: StateBits = StateBits(0);

    fn sense(&self, _p: &TextParams) -> egui::Sense {
        egui::Sense::click_and_drag() | egui::Sense::focusable_noninteractive()
    }

    fn input(&mut self, p: &TextParams, cx: &mut InputCx<'_>, out: &mut Intents<TextIntent>) {
        if !cx.enabled {
            self.focused = false;
            self.gesture = None;
            return;
        }
        if self.session.is_none() {
            return;
        }
        self.focus(p, cx, out);
        self.pointer(p, cx, out);
        if self.entered {
            self.entered = false;
            self.commit(out);
            out.push(TextIntent::Submitted);
        }
        // AccessKit edits reach the field focused or not (a screen reader, an automation driver)
        for r in cx.a11y.to_vec() {
            match (r.action, r.data) {
                (accesskit::Action::SetValue, Some(accesskit::ActionData::Value(v))) => {
                    self.cmd(EditCommand::SelectAll, cx.now, out);
                    self.cmd(EditCommand::ReplaceSelection(v.to_string()), cx.now, out);
                }
                (accesskit::Action::ReplaceSelectedText, Some(accesskit::ActionData::Value(v))) => {
                    self.cmd(EditCommand::ReplaceSelection(v.to_string()), cx.now, out);
                }
                _ => {}
            }
        }
        if cx.focus.has {
            self.keys(p, cx, out);
        }
    }

    fn a11y(&self, p: &TextParams, node: &mut accesskit::Node) {
        node.set_role(if p.secret {
            accesskit::Role::PasswordInput
        } else if p.multiline {
            accesskit::Role::MultilineTextInput
        } else {
            accesskit::Role::TextInput
        });
        node.add_action(accesskit::Action::Focus);
        if p.read_only {
            node.set_read_only();
        } else {
            node.add_action(accesskit::Action::SetValue);
            node.add_action(accesskit::Action::ReplaceSelectedText);
        }
    }

    fn claims(&self, p: &TextParams, key: Key, m: Modifiers) -> bool {
        // EV3's text-field rules (`focus::claims`); every key while composing
        self.is_composing() || crate::focus::claims(crate::focus::Role::Text { multiline: p.multiline }, key, m, false) || crate::focus::claims(crate::focus::Role::Text { multiline: p.multiline }, key, m, true)
    }

    fn text_input(&self, p: &TextParams) -> Option<bool> {
        Some(p.multiline)
    }

    fn composing(&self) -> bool {
        self.is_composing()
    }
}
