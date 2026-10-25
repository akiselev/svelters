//! [`TextField`]: the `text-edit` kind's state (single- and multi-line, password, character limit,
//! placeholder, read-only), and the base of the value boxes, the hex field and the cell editor.
//!
//! It owns a [`TextEditor`], the layout of its shown text, its scroll offset and its caret, and
//! answers the `Kind` hooks of architecture §4.2: `measure`, `prepare` (layout and scrolling),
//! `record` (selection, glyphs, placeholder, preedit underline; the caret is a fast commit),
//! `event` (keys, IME, pointer, focus), `text_input` (it is the platform's
//! [`TextInputClient`]) and `a11y`. presto-core's `Kind` trait and its contexts are not there yet,
//! so the hooks take what they need as arguments; the consolidation wraps them in `impl Kind`.
//!
//! Signals for JS ([`FieldSignal`]) keep svelters' `text-edit` names (svelters-egui
//! `kinds/builtin.rs`): `changed` on every user change, `commit` when an edited field is committed
//! (Enter or blur; `bind:text|commit` follows it), `submit` on Enter in a single-line field; new
//! here, `cancel` on Escape and `select` when the selection changes. [`TEXT_EDIT_PROPS`] and
//! [`TEXT_EDIT_SIGNALS`] are the kind's spec rows for presto-core's `KindSpec`.

use super::caret::CaretState;
use super::editor::{EditorConfig, FocusReason, TextEditor, TextRequest};
use super::keys::KeyAction;
use super::lines::GeometryLines;
use super::policy::TextPolicy;
use super::shim::{Clipboard, Layouter, RecordSink, TextGeometry};
use super::shown::Shown;
use super::utf16::{floor_boundary, str_byte_to_utf16, str_utf16_to_byte};
use presto_base::accesskit;
use presto_base::change::ChangeClass;
use presto_base::color::{Color, Fill};
use presto_base::commit::FastCommit;
use presto_base::display::{Line, LineCap, Prim, Quad};
use presto_base::event::{ImeEvent, KeyEvent, Modifiers, MouseButton, PointerPhase};
use presto_base::geom::{Corners, Insets, Point, Rect, RoundedRect, Size, Vec2};
use presto_base::ids::{Revision, WindowId};
use presto_base::input::{InputPurpose, TextInputClient, Utf16Range};
use presto_base::time::{Span, Time};
use quire_text::{NoWrap, Rejected};
use std::ops::Range;
use std::sync::Arc;

/// The `text-edit` kind's props and their change classes (architecture §4.3), with svelters'
/// names; `read-only` and `wrap` are new.
pub const TEXT_EDIT_PROPS: &[(&str, ChangeClass)] = &[
    ("text", ChangeClass::Layout),
    ("hint-text", ChangeClass::Layout),
    ("multiline", ChangeClass::Layout),
    ("password", ChangeClass::Layout),
    ("wrap", ChangeClass::Layout),
    ("char-limit", ChangeClass::Paint),
    ("read-only", ChangeClass::Paint),
];

/// The `text-edit` kind's signals, by [`FieldSignal::name`].
pub const TEXT_EDIT_SIGNALS: &[&str] = &["changed", "commit", "submit", "cancel", "select"];

/// What the field tells JS.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FieldSignal {
    /// The user edited the text (an undo too).
    Changed,
    /// An edited field was committed: Enter, or focus lost after an edit.
    Commit,
    /// Enter in a single-line field (after `Commit`).
    Submit,
    /// Escape.
    Cancel,
    Select,
    /// A command did nothing or less (read-only, the character limit, a filter): the field may flash.
    Rejected(Rejected),
}

impl FieldSignal {
    /// The spec's name; `None` for what stays native (a rejected command flashes the field).
    pub fn name(&self) -> Option<&'static str> {
        Some(match self {
            FieldSignal::Changed => "changed",
            FieldSignal::Commit => "commit",
            FieldSignal::Submit => "submit",
            FieldSignal::Cancel => "cancel",
            FieldSignal::Select => "select",
            FieldSignal::Rejected(_) => return None,
        })
    }
}

/// The colours `record` paints with, from the field's computed style and theme tokens.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FieldColors {
    pub text: Color,
    pub placeholder: Color,
    pub selection: Color,
    /// The selection of an unfocused field (GTK, AppKit keep it, dimmed).
    pub selection_inactive: Color,
    pub preedit_underline: Color,
}

/// Pointer input as the field takes it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FieldPointer {
    pub phase: PointerPhase,
    pub button: Option<MouseButton>,
    /// Buttons held after the event (primary = 1, secondary = 2, middle = 4).
    pub buttons: u32,
    pub modifiers: Modifiers,
    /// Node coordinates.
    pub local: Point,
    /// The click count of a press, when the dispatcher counts clicks (presto-core does); `None`: the
    /// field counts with the platform's double-click time.
    pub count: Option<u32>,
    /// The node's origin in window coordinates (`pos - local`), for the IME's window-space queries.
    pub window_origin: Option<Point>,
}

impl FieldPointer {
    pub fn new(phase: PointerPhase, button: Option<MouseButton>, buttons: u32, local: Point) -> Self {
        FieldPointer { phase, button, buttons, modifiers: Modifiers::default(), local, count: None, window_origin: None }
    }
}

/// Counts the clicks of double and triple clicks.
#[derive(Clone, Copy, Debug, Default)]
struct ClickCounter {
    last: Option<(Time, Point)>,
    count: u32,
}

impl ClickCounter {
    /// Points the pointer may move between the clicks of a multi-click.
    const SLOP: f32 = 4.0;

    fn press(&mut self, now: Time, pos: Point, double_ms: u32) -> u32 {
        let near = self.last.is_some_and(|(t, p)| now - t <= Span::from_millis(double_ms as i64) && (pos - p).length() <= Self::SLOP);
        self.count = if near && self.count < 3 { self.count + 1 } else { 1 };
        self.last = Some((now, pos));
        self.count
    }
}

/// One `text-edit` (or a field inside a value box, hex field or cell editor). See the module docs.
pub struct TextField {
    pub editor: TextEditor,
    placeholder: String,
    /// Multi-line text wraps at the content width.
    wrap: bool,
    geom: Option<Arc<dyn TextGeometry>>,
    placeholder_geom: Option<Arc<dyn TextGeometry>>,
    /// The layout no longer matches the shown text (or the width).
    stale: bool,
    shown: Shown,
    /// The content box in node coordinates, and the node's origin in window coordinates.
    content: Rect,
    origin: Point,
    scroll: Vec2,
    caret: CaretState,
    clicks: ClickCounter,
    signals: Vec<FieldSignal>,
    /// The user changed the text since focus or the last `change`.
    edited: bool,
    /// Content needs recording (text, selection, preedit or scroll changed).
    repaint: bool,
}

impl TextField {
    pub fn new(text: &str, cfg: EditorConfig, policy: TextPolicy) -> Self {
        let editor = TextEditor::new(text, cfg, policy);
        let shown = editor.shown();
        TextField {
            editor,
            placeholder: String::new(),
            wrap: cfg.multiline,
            geom: None,
            placeholder_geom: None,
            stale: true,
            shown,
            content: Rect::ZERO,
            origin: Point::ZERO,
            scroll: Vec2::ZERO,
            caret: CaretState::default(),
            clicks: ClickCounter::default(),
            signals: Vec::new(),
            edited: false,
            repaint: true,
        }
    }

    // ---- props ----

    /// The `text` prop (not a `changed`).
    pub fn set_text(&mut self, text: &str) {
        self.editor.set_text(text);
        self.after();
    }

    pub fn text(&self) -> String {
        self.editor.text()
    }

    pub fn set_placeholder(&mut self, p: &str) {
        if p != self.placeholder {
            self.placeholder = p.to_string();
            self.placeholder_geom = None;
            self.stale = true;
        }
    }

    pub fn placeholder(&self) -> &str {
        &self.placeholder
    }

    pub fn set_config(&mut self, cfg: EditorConfig) {
        self.wrap = cfg.multiline;
        self.editor.set_config(cfg);
        self.after();
        self.stale = true;
    }

    /// Multi-line text wraps (the default) or scrolls sideways.
    pub fn set_wrap(&mut self, wrap: bool) {
        if wrap != self.wrap {
            self.wrap = wrap;
            self.stale = true;
        }
    }

    /// Multi-line text wraps at the content width.
    pub fn wraps(&self) -> bool {
        self.wrap
    }

    /// The text as laid out and drawn (masked, with the preedit).
    pub fn shown_text(&self) -> &str {
        &self.shown.text
    }

    /// The node's origin in window coordinates, as last known.
    pub fn window_origin(&self) -> Point {
        self.origin
    }

    pub fn set_window_origin(&mut self, o: Point) {
        self.origin = o;
    }

    pub fn scroll_offset(&self) -> Vec2 {
        self.scroll
    }

    pub fn take_signals(&mut self) -> Vec<FieldSignal> {
        std::mem::take(&mut self.signals)
    }

    /// Whether `record` must run again (and clears the flag).
    pub fn take_repaint(&mut self) -> bool {
        std::mem::take(&mut self.repaint)
    }

    // ---- after every operation ----

    /// Folds the editor's changes into signals, layout staleness and the caret restart.
    fn after(&mut self) {
        let c = self.editor.take_changes();
        if c.relayout() {
            self.stale = true;
            self.shown = self.editor.shown();
        }
        if c.text {
            self.edited = true;
            self.signals.push(FieldSignal::Changed);
        }
        if c.selection && !self.signals.contains(&FieldSignal::Select) {
            self.signals.push(FieldSignal::Select);
        }
        if c.text || c.external || c.selection || c.preedit {
            self.caret.restart();
            self.repaint = true;
        }
        if let Some(r) = self.editor.take_rejected() {
            self.signals.push(FieldSignal::Rejected(r));
        }
    }

    // ---- layout ----

    fn wrap_width(&self) -> Option<f32> {
        (self.wrap && self.editor.config().multiline).then(|| self.content.width().max(1.0))
    }

    fn relayout(&mut self, lay: &mut dyn Layouter) {
        if !self.stale && self.geom.is_some() {
            return;
        }
        let w = self.wrap_width();
        self.geom = Some(lay.layout(&self.shown.text, w));
        if !self.placeholder.is_empty() && self.placeholder_geom.is_none() {
            self.placeholder_geom = Some(lay.layout(&self.placeholder, w));
        }
        self.stale = false;
        self.repaint = true;
    }

    /// Intrinsic content size: the text (or the placeholder, or one empty row) at `max_width`.
    pub fn measure(&mut self, lay: &mut dyn Layouter, max_width: Option<f32>) -> Size {
        let wrap = if self.wrap && self.editor.config().multiline { max_width } else { None };
        let s = lay.layout(&self.shown.text, wrap).size();
        let p = if self.placeholder.is_empty() { Size::ZERO } else { lay.layout(&self.placeholder, wrap).size() };
        let row = lay.layout(" ", None).size();
        Size::new(s.w.max(p.w), s.h.max(p.h).max(row.h))
    }

    /// After layout: the content box (node coordinates) and the node's window origin; lays the text
    /// out if needed and scrolls the caret into view.
    pub fn prepare(&mut self, lay: &mut dyn Layouter, content: Rect, origin: Point) {
        if content.width() != self.content.width() && self.wrap {
            self.stale = true;
        }
        if content != self.content || origin != self.origin {
            self.repaint = true;
        }
        self.content = content;
        self.origin = origin;
        self.relayout(lay);
        if let Some(b) = self.editor.take_reveal() {
            self.reveal(self.shown.to_shown(b));
        }
        self.clamp_scroll();
    }

    fn reveal(&mut self, shown_byte: usize) {
        let Some(g) = &self.geom else { return };
        let r = g.caret_rect(shown_byte, false);
        let (w, h) = (self.content.width(), self.content.height());
        let before = self.scroll;
        if r.min.x < self.scroll.x {
            self.scroll.x = r.min.x;
        } else if r.max.x > self.scroll.x + w {
            self.scroll.x = r.max.x - w;
        }
        if r.min.y < self.scroll.y {
            self.scroll.y = r.min.y;
        } else if r.max.y > self.scroll.y + h {
            self.scroll.y = r.max.y - h;
        }
        self.repaint |= before != self.scroll;
    }

    fn clamp_scroll(&mut self) {
        let Some(g) = &self.geom else { return };
        let s = g.size();
        // one point past the text for the caret at its end
        let max = Vec2::new((s.w + 1.0 - self.content.width()).max(0.0), (s.h - self.content.height()).max(0.0));
        let c = Vec2::new(self.scroll.x.clamp(0.0, max.x), self.scroll.y.clamp(0.0, max.y));
        self.repaint |= c != self.scroll;
        self.scroll = c;
    }

    /// Layout coordinates → node coordinates.
    fn to_node(&self) -> Vec2 {
        self.content.min.to_vec() - self.scroll
    }

    /// The document byte under node point `p`.
    fn hit(&self, lay: &mut dyn Layouter, p: Point) -> usize {
        let g = match (&self.geom, self.stale) {
            (Some(g), false) => g.clone(),
            _ => lay.layout(&self.shown.text, self.wrap_width()),
        };
        self.shown.to_doc(g.hit_test(p - self.to_node()).byte)
    }

    /// The caret in layout coordinates: `None` when unfocused, over a selection, or while the IME
    /// hides it.
    fn caret_in_layout(&self) -> Option<Rect> {
        let g = self.geom.as_ref().filter(|_| !self.stale)?;
        if !self.editor.focused() {
            return None;
        }
        if let Some(pre) = self.editor.preedit() {
            let p = self.shown.preedit_range()?;
            let a = pre.active.as_ref()?;
            let at = if self.editor.config().password { p.start + pre.text[..a.end].chars().count() * self.editor.policy().mask.len_utf8() } else { p.start + a.end };
            return Some(g.caret_rect(at, false));
        }
        let s = self.editor.selection();
        if !s.range().is_empty() {
            return None;
        }
        Some(g.caret_rect(self.shown.to_shown(s.head), false))
    }

    /// The caret in window coordinates, clipped out when scrolled away.
    pub fn caret_rect(&self) -> Option<Rect> {
        let r = self.caret_in_layout()?.translate(self.to_node());
        let clip = self.content.inflate(1.0);
        r.intersects(&clip).then(|| r.translate(self.origin.to_vec()))
    }

    /// The caret's fast commit, when it moved, hid or restarts its blink (architecture §7.3).
    pub fn caret_commit(&mut self, window: WindowId, revision: Revision) -> Option<FastCommit> {
        let r = self.caret_rect();
        self.caret.update(window, r, revision)
    }

    /// The IME candidate area (window coordinates): the composition, else the caret.
    pub fn ime_area(&self) -> Option<Rect> {
        let g = self.geom.as_ref().filter(|_| !self.stale)?;
        let r = match self.shown.preedit_range() {
            Some(p) => g.selection_rects(p).into_iter().reduce(|a, b| a.union(&b))?,
            None => self.caret_in_layout().unwrap_or_else(|| g.caret_rect(self.shown.to_shown(self.editor.caret()), false)),
        };
        Some(r.translate(self.to_node()).translate(self.origin.to_vec()))
    }

    // ---- record ----

    /// Records the field's content (node coordinates): the selection behind the text, the glyphs or
    /// the placeholder, and the preedit underline, clipped to the content box.
    pub fn record(&self, list: &mut dyn RecordSink, colors: &FieldColors) {
        let Some(g) = &self.geom else { return };
        let o = self.to_node();
        let clip = self.content;
        list.push(Prim::PushClip(RoundedRect { rect: clip, radii: Corners::default() }), clip);
        let s = self.editor.selection();
        let focused = self.editor.focused();
        if !s.range().is_empty() && !self.editor.composing() && (focused || self.editor.policy().keep_selection_unfocused) {
            let fill = if focused { colors.selection } else { colors.selection_inactive };
            for r in g.selection_rects(self.shown.range_to_shown(s.range())) {
                let r = r.translate(o);
                list.push(
                    Prim::Quad(Quad {
                        rect: r,
                        radii: Corners::default(),
                        fill: Fill::Solid(fill),
                        border_widths: Insets::default(),
                        border_color: Color::TRANSPARENT,
                        border_style: Default::default(),
                    }),
                    r,
                );
            }
        }
        if self.editor.is_empty() && !self.editor.composing() {
            if let Some(p) = &self.placeholder_geom {
                p.record_glyphs(&mut *list, Point::ZERO + o, colors.placeholder);
            }
        } else {
            g.record_glyphs(&mut *list, Point::ZERO + o, colors.text);
        }
        if let (Some(p), Some(pre)) = (self.shown.preedit_range(), self.editor.preedit()) {
            let mask = self.editor.config().password.then(|| self.editor.policy().mask.len_utf8());
            let active = pre.active.as_ref().filter(|a| !a.is_empty()).map(|a| match mask {
                Some(ml) => p.start + pre.text[..a.start].chars().count() * ml..p.start + pre.text[..a.end].chars().count() * ml,
                None => p.start + a.start..p.start + a.end,
            });
            let mut underline = |r: Rect, width: f32| {
                let y = r.max.y - width / 2.0;
                let (a, b) = (Point::new(r.min.x, y) + o, Point::new(r.max.x, y) + o);
                list.push(
                    Prim::Line(Line { from: a, to: b, width, color: colors.preedit_underline, cap: LineCap::Butt, dash: None, wavy: false }),
                    Rect::new(a.x, a.y - width, b.x, b.y + width),
                );
            };
            for r in g.selection_rects(p) {
                underline(r, 1.0);
            }
            if let Some(a) = active {
                for r in g.selection_rects(a) {
                    underline(r, 2.0);
                }
            }
        }
        list.push(Prim::PopClip, clip);
    }

    // ---- events ----

    /// Sets the editor's clock and lays out if a previous event left the layout stale, so motions
    /// see the text as it is now.
    fn begin(&mut self, now: Time, lay: &mut dyn Layouter) {
        self.editor.set_time(now);
        self.relayout(lay);
    }

    fn page_rows(&self) -> usize {
        let row = self.geom.as_ref().filter(|g| g.line_count() > 0).map_or(16.0, |g| g.line(0).height.max(1.0));
        ((self.content.height() / row).floor() as usize).max(1)
    }

    /// A key press while focused (after the §11.2 route sent it to the keymap). Returns what the
    /// field did with it; `Ignore` bubbles.
    pub fn key(&mut self, ev: &KeyEvent, now: Time, lay: &mut dyn Layouter) -> KeyAction {
        self.begin(now, lay);
        let action = match &self.geom {
            Some(g) => {
                let g = g.clone();
                let lines = GeometryLines { geom: &*g, shown: &self.shown, page: self.page_rows() };
                self.editor.key(ev, &lines)
            }
            None => self.editor.key(ev, &NoWrap::default()),
        };
        match action {
            KeyAction::Submit => {
                self.commit_change();
                self.signals.push(FieldSignal::Submit);
            }
            KeyAction::Cancel => self.signals.push(FieldSignal::Cancel),
            _ => {}
        }
        self.after();
        self.relayout(lay);
        if let Some(b) = self.editor.take_reveal() {
            self.reveal(self.shown.to_shown(b));
            self.clamp_scroll();
        }
        action
    }

    /// A platform IME event (winit-style).
    pub fn ime(&mut self, ev: &ImeEvent, now: Time, lay: &mut dyn Layouter) {
        self.begin(now, lay);
        self.editor.ime(ev);
        self.after();
        self.relayout(lay);
    }

    /// A pointer event (node coordinates). Returns whether the field took it.
    pub fn pointer(&mut self, ev: &FieldPointer, now: Time, lay: &mut dyn Layouter) -> bool {
        self.begin(now, lay);
        if let Some(o) = ev.window_origin {
            self.origin = o;
        }
        let local = ev.local;
        let took = match (ev.phase, ev.button) {
            (PointerPhase::Down, Some(MouseButton::Primary)) => {
                let n = match ev.count {
                    Some(n) => n,
                    None => self.clicks.press(now, local, self.editor.policy().double_click_ms),
                };
                let at = self.hit(lay, local);
                self.editor.pointer_down(at, n, ev.modifiers.contains(Modifiers::SHIFT));
                true
            }
            (PointerPhase::Down, Some(MouseButton::Middle)) => {
                let at = self.hit(lay, local);
                self.editor.middle_click(at)
            }
            (PointerPhase::Move, _) if self.editor.dragging() && ev.buttons & 1 != 0 => {
                let at = self.hit(lay, local);
                self.editor.pointer_drag(at);
                true
            }
            (PointerPhase::Up | PointerPhase::Cancel, _) if self.editor.dragging() => {
                self.editor.pointer_up();
                true
            }
            _ => false,
        };
        self.after();
        self.relayout(lay);
        if let Some(b) = self.editor.take_reveal() {
            self.reveal(self.shown.to_shown(b));
            self.clamp_scroll();
        }
        took
    }

    pub fn focus_in(&mut self, reason: FocusReason) {
        self.edited = false;
        self.editor.focus_in(reason);
        self.after();
        self.repaint = true;
    }

    /// Focus left: an edited field commits (`change`).
    pub fn focus_out(&mut self) {
        self.editor.focus_out();
        self.commit_change();
        self.after();
        self.repaint = true;
    }

    fn commit_change(&mut self) {
        if std::mem::take(&mut self.edited) {
            self.signals.push(FieldSignal::Commit);
        }
    }

    /// Serves the editor's requests with the platform clipboard (the answers paste at once).
    pub fn serve(&mut self, clipboard: &mut dyn Clipboard) {
        for r in self.editor.take_requests() {
            match r {
                TextRequest::SetClipboard(s) => clipboard.set_text(&s),
                TextRequest::ReadClipboard => {
                    if let Some(t) = clipboard.get_text().filter(|t| !t.is_empty()) {
                        self.editor.paste(&t);
                    }
                }
                TextRequest::SetPrimary(s) => clipboard.set_primary(&s),
                TextRequest::ClearPrimary => clipboard.clear_primary(),
                TextRequest::ReadPrimary => {
                    if let Some(t) = clipboard.get_primary().filter(|t| !t.is_empty()) {
                        self.editor.paste_primary(&t);
                    }
                }
                // the platform's bell: the kind forwards it when the platform has one
                TextRequest::Beep => {}
            }
        }
        self.after();
    }

    // ---- accessibility ----

    pub fn a11y(&self, node: &mut accesskit::Node) {
        let cfg = self.editor.config();
        node.set_role(if cfg.password {
            accesskit::Role::PasswordInput
        } else if cfg.multiline {
            accesskit::Role::MultilineTextInput
        } else {
            accesskit::Role::TextInput
        });
        let value = if cfg.password { self.shown.text.clone() } else { self.editor.text() };
        node.set_value(value);
        if !self.placeholder.is_empty() {
            node.set_placeholder(self.placeholder.clone());
        }
        if cfg.read_only {
            node.set_read_only();
        } else {
            node.add_action(accesskit::Action::SetValue);
            node.add_action(accesskit::Action::ReplaceSelectedText);
        }
        node.add_action(accesskit::Action::Focus);
    }

    /// An AccessKit action on the field.
    pub fn a11y_action(&mut self, req: &accesskit::ActionRequest) {
        if let Some(accesskit::ActionData::Value(v)) = &req.data {
            match req.action {
                accesskit::Action::SetValue => self.editor.set_value_by_user(v),
                accesskit::Action::ReplaceSelectedText => self.editor.replace_selection(v),
                _ => {}
            }
        }
        self.after();
    }

    // ---- the IME document ↔ the layout (geometry queries of TextInputClient) ----

    /// The IME's document text (unmasked, the preedit spliced in).
    fn ime_text(&self) -> String {
        let t = self.editor.text();
        Shown::build(&t, None, self.editor.preedit().map(|p| (self.editor.caret(), p.text.as_str()))).text
    }

    /// A byte of the IME document → a byte of the laid-out text (they have the same chars; a
    /// password field's are masks).
    fn ime_to_layout(&self, ime: &str, b: usize) -> usize {
        if !self.editor.config().password {
            return b;
        }
        let k = ime[..floor_boundary(ime, b)].chars().count();
        self.shown.text.char_indices().nth(k).map_or(self.shown.text.len(), |(i, _)| i)
    }

    fn layout_to_ime(&self, ime: &str, s: usize) -> usize {
        if !self.editor.config().password {
            return s.min(ime.len());
        }
        let k = self.shown.text[..floor_boundary(&self.shown.text, s)].chars().count();
        ime.char_indices().nth(k).map_or(ime.len(), |(i, _)| i)
    }

    fn layout_rect_to_window(&self, r: Rect) -> Rect {
        r.translate(self.to_node()).translate(self.origin.to_vec())
    }
}

impl TextInputClient for TextField {
    fn text_for_range(&self, range: Utf16Range) -> Option<String> {
        self.editor.ime_text_for_range(range)
    }

    fn selected_range(&self) -> Option<Utf16Range> {
        self.editor.ime_selected_range()
    }

    fn marked_range(&self) -> Option<Utf16Range> {
        self.editor.ime_marked_range()
    }

    fn replace_text_in_range(&mut self, range: Option<Utf16Range>, text: &str) {
        self.editor.ime_replace(range, text);
        self.after();
    }

    fn replace_and_mark_text_in_range(&mut self, range: Option<Utf16Range>, text: &str, new_selected: Option<Utf16Range>) {
        self.editor.ime_replace_and_mark(range, text, new_selected);
        self.after();
    }

    fn unmark_text(&mut self) {
        self.editor.ime_unmark();
        self.after();
    }

    fn bounds_for_range(&self, range: Utf16Range) -> Option<Rect> {
        let g = self.geom.as_ref().filter(|_| !self.stale)?;
        let ime = self.ime_text();
        let a = self.ime_to_layout(&ime, str_utf16_to_byte(&ime, range.start));
        let b = self.ime_to_layout(&ime, str_utf16_to_byte(&ime, range.end.max(range.start)));
        let r = if a == b { Some(g.caret_rect(a, false)) } else { g.selection_rects(a..b).into_iter().reduce(|x, y| x.union(&y)) }?;
        Some(self.layout_rect_to_window(r))
    }

    fn character_index_for_point(&self, p: Point) -> Option<usize> {
        let g = self.geom.as_ref().filter(|_| !self.stale)?;
        let local = p - self.origin.to_vec() - self.to_node();
        let s = g.hit_test(local).byte;
        let ime = self.ime_text();
        Some(str_byte_to_utf16(&ime, self.layout_to_ime(&ime, s)))
    }

    fn surrounding_text(&self, max_bytes: usize) -> Option<(String, Range<usize>)> {
        self.editor.ime_surrounding(max_bytes)
    }

    fn purpose(&self) -> InputPurpose {
        self.editor.purpose()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::policy::Platform;
    use crate::text::shim::MonoLayouter;
    use presto_base::event::{KeyLocation, KeyState, Modifiers};
    use presto_base::key::Key;
    use crate::text::shim::ListSink;

    const LAY: MonoLayouter = MonoLayouter { advance: 10.0, line_height: 20.0 };

    fn field(text: &str, cfg: EditorConfig) -> (TextField, MonoLayouter) {
        let mut f = TextField::new(text, cfg, TextPolicy::of(Platform::Gtk));
        let mut lay = LAY;
        // a 100×20 content box at (4, 2) in a node placed at (50, 60) in the window
        f.prepare(&mut lay, Rect::new(4.0, 2.0, 104.0, 22.0), Point::new(50.0, 60.0));
        (f, lay)
    }

    fn press(f: &mut TextField, lay: &mut MonoLayouter, k: Key, m: Modifiers, text: Option<&str>) -> KeyAction {
        let ev = KeyEvent { key: Some(k), physical: 0, text: text.map(str::to_string), state: KeyState::Pressed, repeat: false, location: KeyLocation::Standard, modifiers: m };
        f.key(&ev, Time(0), lay)
    }

    fn ptr(phase: PointerPhase, button: Option<MouseButton>, buttons: u32, local: Point) -> FieldPointer {
        FieldPointer::new(phase, button, buttons, local)
    }

    #[test]
    fn caret_scrolls_into_view_and_commits_fast() {
        let (mut f, mut lay) = field("", EditorConfig::default());
        f.focus_in(FocusReason::Pointer);
        assert_eq!(f.caret_rect(), Some(Rect::new(54.0, 62.0, 55.0, 82.0)));
        assert!(f.caret_commit(WindowId(1), Revision(1)).is_some());
        assert_eq!(f.caret_commit(WindowId(1), Revision(1)), None);
        for _ in 0..15 {
            press(&mut f, &mut lay, Key::X, Modifiers::default(), Some("x"));
        }
        // 15 chars = 150 points in a 100-point box: scrolled so the caret is at the right edge
        assert_eq!(f.scroll_offset().x, 51.0);
        let c = f.caret_rect().unwrap();
        assert!(c.max.x <= 50.0 + 104.0 + 0.001);
        assert!(f.take_signals().iter().filter(|s| **s == FieldSignal::Changed).count() == 15);
        press(&mut f, &mut lay, Key::Home, Modifiers::default(), None);
        assert_eq!(f.scroll_offset().x, 0.0);
    }

    #[test]
    fn signals_follow_the_input_element() {
        let (mut f, mut lay) = field("ab", EditorConfig::default());
        f.focus_in(FocusReason::Pointer);
        press(&mut f, &mut lay, Key::C, Modifiers::default(), Some("c"));
        assert_eq!(press(&mut f, &mut lay, Key::Enter, Modifiers::default(), Some("\r")), KeyAction::Submit);
        let s = f.take_signals();
        assert!(s.contains(&FieldSignal::Changed) && s.contains(&FieldSignal::Commit) && s.contains(&FieldSignal::Submit));
        // blur after no edit: no change
        f.focus_out();
        assert!(!f.take_signals().contains(&FieldSignal::Commit));
        // a prop write is not an input
        f.set_text("zz");
        assert!(!f.take_signals().contains(&FieldSignal::Changed));
        let mut cfg = EditorConfig::default();
        cfg.read_only = true;
        f.set_config(cfg);
        f.focus_in(FocusReason::Pointer);
        press(&mut f, &mut lay, Key::Q, Modifiers::default(), Some("q"));
        assert!(f.take_signals().contains(&FieldSignal::Rejected(Rejected::ReadOnly)));
    }

    #[test]
    fn pointer_selects_and_records_selection_behind_text() {
        let (mut f, mut lay) = field("hello world", EditorConfig::default());
        f.focus_in(FocusReason::Pointer);
        // double click on "world" (node x = 4 + 7.5 cells)
        let p = Point::new(4.0 + 75.0, 10.0);
        f.pointer(&ptr(PointerPhase::Down, Some(MouseButton::Primary), 1, p), Time(0), &mut lay);
        f.pointer(&ptr(PointerPhase::Up, Some(MouseButton::Primary), 0, p), Time(1_000_000), &mut lay);
        f.pointer(&ptr(PointerPhase::Down, Some(MouseButton::Primary), 1, p), Time(50_000_000), &mut lay);
        let s = f.editor.selection();
        assert_eq!((s.anchor, s.head), (6, 11));
        assert_eq!(f.caret_rect(), None, "no caret over a selection");
        let colors = FieldColors {
            text: Color::BLACK,
            placeholder: Color::BLACK,
            selection: Color::rgba(0.2, 0.4, 0.8, 1.0),
            selection_inactive: Color::rgba(0.5, 0.5, 0.5, 1.0),
            preedit_underline: Color::BLACK,
        };
        let mut sink = ListSink::default();
        f.record(&mut sink, &colors);
        let list = sink.list;
        assert!(matches!(list.items[0], Prim::PushClip(_)));
        match &list.items[1] {
            Prim::Quad(q) => assert_eq!(q.rect, Rect::new(64.0, 2.0, 114.0, 22.0)),
            p => panic!("expected the selection quad, got {p:?}"),
        }
        assert!(matches!(list.items.last(), Some(Prim::PopClip)));
    }

    #[test]
    fn preedit_is_underlined_and_reported_to_the_ime() {
        let (mut f, mut lay) = field("ab", EditorConfig::default());
        f.focus_in(FocusReason::Pointer);
        f.editor.set_selection(1, 1);
        f.ime(&ImeEvent::Preedit { text: "かな".into(), cursor: Some((0, 3)) }, Time(0), &mut lay);
        let colors = FieldColors { text: Color::BLACK, placeholder: Color::BLACK, selection: Color::BLACK, selection_inactive: Color::BLACK, preedit_underline: Color::WHITE };
        let mut sink = ListSink::default();
        f.record(&mut sink, &colors);
        let list = sink.list;
        let lines: Vec<f32> = list.items.iter().filter_map(|p| if let Prim::Line(l) = p { Some(l.width) } else { None }).collect();
        assert_eq!(lines, vec![1.0, 2.0], "the composition, then its active clause");
        // the IME places its candidates under the composition: chars 1..3 of the IME document
        assert_eq!(f.marked_range(), Some(1..3));
        let b = f.bounds_for_range(1..3).unwrap();
        assert_eq!(b, Rect::new(50.0 + 4.0 + 10.0, 62.0, 50.0 + 4.0 + 30.0, 82.0));
        assert_eq!(f.ime_area(), Some(b));
        assert_eq!(f.character_index_for_point(Point::new(50.0 + 4.0 + 31.0, 70.0)), Some(3));
        f.replace_text_in_range(None, "仮名");
        assert_eq!(f.text(), "a仮名b");
    }

    #[test]
    fn multiline_up_down_on_wrapped_rows_and_placeholder() {
        let cfg = EditorConfig { multiline: true, ..EditorConfig::default() };
        let mut f = TextField::new("", cfg, TextPolicy::of(Platform::Qt));
        let mut lay = LAY;
        f.set_placeholder("Type here");
        // 40 points wide: 4 columns
        f.prepare(&mut lay, Rect::new(0.0, 0.0, 40.0, 60.0), Point::ZERO);
        let m = f.measure(&mut lay, Some(40.0));
        assert_eq!(m, Size::new(40.0, 60.0), "the placeholder wraps to three rows");
        f.focus_in(FocusReason::Pointer);
        f.set_text("abcdefgh");
        f.prepare(&mut lay, Rect::new(0.0, 0.0, 40.0, 60.0), Point::ZERO);
        f.editor.set_selection(1, 1);
        press(&mut f, &mut lay, Key::ArrowDown, Modifiers::default(), None);
        assert_eq!(f.editor.caret(), 5, "down moves to the next wrapped row");
        press(&mut f, &mut lay, Key::End, Modifiers::default(), None);
        assert_eq!(f.editor.caret(), 8);
    }

    struct Board {
        text: Option<String>,
        primary: Option<String>,
    }

    impl Clipboard for Board {
        fn get_text(&mut self) -> Option<String> {
            self.text.clone()
        }
        fn set_text(&mut self, t: &str) {
            self.text = Some(t.to_string());
        }
        fn get_primary(&mut self) -> Option<String> {
            self.primary.clone()
        }
        fn set_primary(&mut self, t: &str) {
            self.primary = Some(t.to_string());
        }
        fn clear_primary(&mut self) {
            self.primary = None;
        }
    }

    #[test]
    fn clipboard_and_primary_through_the_platform() {
        let (mut f, mut lay) = field("copy me", EditorConfig::default());
        let mut board = Board { text: None, primary: None };
        f.focus_in(FocusReason::Keyboard);
        f.serve(&mut board);
        assert_eq!(board.primary.as_deref(), Some("copy me"), "GTK: keyboard focus selects all and owns PRIMARY");
        press(&mut f, &mut lay, Key::C, Modifiers::CTRL, Some("c"));
        press(&mut f, &mut lay, Key::End, Modifiers::default(), None);
        press(&mut f, &mut lay, Key::V, Modifiers::CTRL, Some("v"));
        f.serve(&mut board);
        assert_eq!(f.text(), "copy mecopy me");
        assert_eq!(board.primary, None, "GTK gives PRIMARY up when the selection collapses");
    }
}
