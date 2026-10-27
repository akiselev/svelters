//! [`TextField`]: the `text-edit` kind's state (single- and multi-line, password, character limit,
//! placeholder, read-only), and the base of the value boxes, the hex field and the cell editor.
//!
//! It owns a [`TextEditor`], the layout of its shown text, its scroll offset and its caret, and
//! answers the `Kind` hooks of architecture §4.2: `measure`, `prepare` (layout and scrolling),
//! `record` (selection, glyphs, placeholder, preedit underline; the caret is a fast commit),
//! `event` (keys, IME, pointer, focus), `text_input` (it is the platform's
//! [`TextInputClient`]) and `a11y`. skelter-core's `Kind` trait and its contexts are not there yet,
//! so the hooks take what they need as arguments; the consolidation wraps them in `impl Kind`.
//!
//! Signals for JS ([`FieldSignal`]) keep svelters' `text-edit` names (svelters-egui
//! `kinds/builtin.rs`): `changed` on every user change, `commit` when an edited field is committed
//! (Enter or blur; `bind:text|commit` follows it), `submit` on Enter in a single-line field; new
//! here, `cancel` on Escape and `select` when the selection changes. [`TEXT_EDIT_PROPS`] and
//! [`TEXT_EDIT_SIGNALS`] are the kind's spec rows for skelter-core's `KindSpec`.

use super::caret::CaretState;
use super::editor::{EditorConfig, FocusReason, TextEditor, TextRequest};
use super::keys::KeyAction;
use super::lines::GeometryLines;
use super::policy::TextPolicy;
use super::runs::TextRanges;
use super::shim::{Clipboard, Layouter, RecordSink, TextGeometry};
use super::shown::Shown;
use super::utf16::{floor_boundary, str_byte_to_utf16, str_utf16_to_byte};
use skelter_base::accesskit;
use skelter_base::change::ChangeClass;
use skelter_base::color::{Color, Fill};
use skelter_base::display::{Line, LineCap, Prim, Quad};
use skelter_base::event::{ImeEvent, KeyEvent, Modifiers, MouseButton, PointerPhase};
use skelter_base::geom::{Affine, Corners, Insets, Point, Rect, RoundedRect, Size, Vec2};
use skelter_base::input::{InputPurpose, TextInputClient, Utf16Range};
use skelter_base::time::{Span, Time};
use quire_text::{NoWrap, Rejected};
use std::borrow::Cow;
use std::ops::Range;
use std::sync::Arc;

/// The most runs a field sends: skelter-core's cap on one node's virtual children.
const MAX_RUNS: usize = skelter_core::a11y::MAX_VIRTUAL_CHILDREN;

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

/// WCAG 2 relative luminance of `c` (alpha ignored).
fn luminance(c: Color) -> f32 {
    let lin = |v: f32| if v <= 0.040_45 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) };
    0.2126 * lin(c.r) + 0.7152 * lin(c.g) + 0.0722 * lin(c.b)
}

/// WCAG 2 contrast ratio of `a` and `b` (1 to 21).
fn contrast(a: Color, b: Color) -> f32 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// The colour of selected text on `fill`: `None` (keep `text`) when `text` reads on it (4.5:1, or a
/// translucent fill), else black or white, whichever contrasts more. VS Code's high-contrast sets
/// pair their light or dark selection with `editor.selectionForeground` the same way.
pub(crate) fn selection_text(text: Color, fill: Color) -> Option<Color> {
    if fill.a < 1.0 || contrast(text, fill) >= 4.5 {
        return None;
    }
    let (black, white) = (Color::rgba(0.0, 0.0, 0.0, 1.0), Color::rgba(1.0, 1.0, 1.0, 1.0));
    Some(if contrast(black, fill) >= contrast(white, fill) { black } else { white })
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
    /// The click count of a press, when the dispatcher counts clicks (skelter-core does); `None`: the
    /// field counts with the platform's double-click time.
    pub count: Option<u32>,
}

impl FieldPointer {
    pub fn new(phase: PointerPhase, button: Option<MouseButton>, buttons: u32, local: Point) -> Self {
        FieldPointer { phase, button, buttons, modifiers: Modifiers::default(), local, count: None }
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
    /// The content box in node coordinates, and the node's local → window transform.
    content: Rect,
    to_window: Affine,
    scroll: Vec2,
    caret: CaretState,
    clicks: ClickCounter,
    signals: Vec<FieldSignal>,
    /// The user changed the text since focus or the last `change`.
    edited: bool,
    /// Content needs recording (text, selection, preedit or scroll changed).
    repaint: bool,
    /// A scroll change needs a re-record (false: the host moves recorded content, as a
    /// self-scrolling kind's compositor offset does; [`TextField::record_content`]).
    scroll_repaints: bool,
    /// A non-empty selection was last recorded: a caret move needs no re-record unless one was or is
    /// (the caret itself goes out through the fast channel).
    selection_painted: bool,
    /// The laid-out text as accessibility runs, rebuilt with the layout.
    ranges: TextRanges,
    /// The accessibility node is stale (text, selection, preedit, scroll or layout changed; never a
    /// caret blink).
    a11y_changed: bool,
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
            to_window: Affine::IDENTITY,
            scroll: Vec2::ZERO,
            caret: CaretState::default(),
            clicks: ClickCounter::default(),
            signals: Vec::new(),
            edited: false,
            repaint: true,
            scroll_repaints: true,
            selection_painted: false,
            ranges: TextRanges::default(),
            a11y_changed: true,
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

    /// The node's local → window transform, as last prepared.
    pub fn window_transform(&self) -> Affine {
        self.to_window
    }

    pub fn scroll_offset(&self) -> Vec2 {
        self.scroll
    }

    /// Takes the scroll offset the host holds (a compositor offset a wheel or fling moved); the next
    /// prepare clamps it.
    pub fn set_scroll_offset(&mut self, v: Vec2) {
        self.scroll = v;
    }

    /// The largest scroll offset on each axis (the text, one point past it for the caret at its end,
    /// beyond the content box).
    pub fn scroll_max(&self) -> Vec2 {
        let Some(g) = &self.geom else { return Vec2::ZERO };
        let s = g.size();
        Vec2::new((s.w + 1.0 - self.content.width()).max(0.0), (s.h - self.content.height()).max(0.0))
    }

    /// Whether a scroll needs a re-record (default `true`); a host that records with
    /// [`TextField::record_content`] and moves the content itself turns it off.
    pub fn set_scroll_repaints(&mut self, on: bool) {
        self.scroll_repaints = on;
    }

    pub fn take_signals(&mut self) -> Vec<FieldSignal> {
        std::mem::take(&mut self.signals)
    }

    /// Whether `record` must run again (and clears the flag).
    pub fn take_repaint(&mut self) -> bool {
        std::mem::take(&mut self.repaint)
    }

    /// Whether the accessibility node must be sent again (and clears the flag).
    pub fn take_a11y(&mut self) -> bool {
        std::mem::take(&mut self.a11y_changed)
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
            self.a11y_changed = true;
            self.caret.restart();
            // a collapsed caret moving is the fast channel's: no re-record
            let selected = !self.editor.selection().range().is_empty();
            if c.text || c.external || c.preedit || selected || self.selection_painted {
                self.repaint = true;
            }
            self.selection_painted = selected;
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
        // the runs follow the layout (O(text) in carets: here, not in every `a11y`)
        let words: Vec<usize> = self.editor.word_starts().into_iter().map(|b| self.shown.to_shown(b)).collect();
        self.ranges = TextRanges::build(&self.shown.text, self.geom.as_deref(), &words);
        self.a11y_changed = true;
    }

    /// Intrinsic content size: the text (or the placeholder, or one empty row) at `max_width`.
    pub fn measure(&mut self, lay: &mut dyn Layouter, max_width: Option<f32>) -> Size {
        let wrap = if self.wrap && self.editor.config().multiline { max_width } else { None };
        let s = lay.layout(&self.shown.text, wrap).size();
        let p = if self.placeholder.is_empty() { Size::ZERO } else { lay.layout(&self.placeholder, wrap).size() };
        let row = lay.layout(" ", None).size();
        Size::new(s.w.max(p.w), s.h.max(p.h).max(row.h))
    }

    /// After layout: the content box (node coordinates) and the node's local → window transform;
    /// lays the text out if needed and scrolls the caret into view.
    pub fn prepare(&mut self, lay: &mut dyn Layouter, content: Rect, to_window: Affine) {
        if content.width() != self.content.width() && self.wrap {
            self.stale = true;
        }
        if content != self.content || to_window != self.to_window {
            self.repaint = true;
        }
        self.content = content;
        self.to_window = to_window;
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
        self.repaint |= self.scroll_repaints && before != self.scroll;
        self.a11y_changed |= before != self.scroll;
    }

    fn clamp_scroll(&mut self) {
        let Some(g) = &self.geom else { return };
        let s = g.size();
        // one point past the text for the caret at its end
        let max = Vec2::new((s.w + 1.0 - self.content.width()).max(0.0), (s.h - self.content.height()).max(0.0));
        let c = Vec2::new(self.scroll.x.clamp(0.0, max.x), self.scroll.y.clamp(0.0, max.y));
        self.repaint |= self.scroll_repaints && c != self.scroll;
        self.a11y_changed |= c != self.scroll;
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
        if self.editor.preedit().is_some() {
            return Some(g.caret_rect(self.preedit_caret()?, false));
        }
        let s = self.editor.selection();
        if !s.range().is_empty() {
            return None;
        }
        Some(g.caret_rect(self.shown.to_shown(s.head), false))
    }

    /// The shown byte of the caret in a composition: the end of its active clause.
    fn preedit_caret(&self) -> Option<usize> {
        let pre = self.editor.preedit()?;
        let p = self.shown.preedit_range()?;
        let a = pre.active.as_ref()?;
        Some(if self.editor.config().password { p.start + pre.text[..a.end].chars().count() * self.editor.policy().mask.len_utf8() } else { p.start + a.end })
    }

    /// The caret in node coordinates, `None` when hidden or scrolled away.
    pub fn caret_local(&self) -> Option<Rect> {
        let r = self.caret_in_layout()?.translate(self.to_node());
        r.intersects(&self.content.inflate(1.0)).then_some(r)
    }

    /// The caret in window coordinates.
    pub fn caret_rect(&self) -> Option<Rect> {
        self.caret_local().map(|r| self.to_window.apply_rect(&r))
    }

    /// The caret's node-local rect when it moved, hid or restarts its blink: what the kind hands to
    /// skelter-core's fast channel (`EventCx::set_caret`).
    pub fn caret_update(&mut self) -> Option<Option<Rect>> {
        let r = self.caret_local();
        self.caret.changed(r)
    }

    /// The IME candidate area (window coordinates): the composition, else the caret.
    pub fn ime_area(&self) -> Option<Rect> {
        let g = self.geom.as_ref().filter(|_| !self.stale)?;
        let r = match self.shown.preedit_range() {
            Some(p) => g.selection_rects(p).into_iter().reduce(|a, b| a.union(&b))?,
            None => self.caret_in_layout().unwrap_or_else(|| g.caret_rect(self.shown.to_shown(self.editor.caret()), false)),
        };
        Some(self.to_window.apply_rect(&r.translate(self.to_node())))
    }

    // ---- record ----

    /// Records the field's content (node coordinates): the selection behind the text, the glyphs or
    /// the placeholder, and the preedit underline, clipped to the content box.
    pub fn record(&self, list: &mut dyn RecordSink, colors: &FieldColors) {
        self.record_at(list, colors, self.to_node(), Some(self.content));
    }

    /// Records the content as [`TextField::record`] does, unscrolled and unclipped: for a host that
    /// places it in a scrolled space (a self-scrolling kind's content space, where the scroll is a
    /// compositor offset of [`TextField::scroll_offset`] and the scroll layer clips).
    pub fn record_content(&self, list: &mut dyn RecordSink, colors: &FieldColors) {
        self.record_at(list, colors, self.content.min.to_vec(), None);
    }

    fn record_at(&self, list: &mut dyn RecordSink, colors: &FieldColors, o: Vec2, clip: Option<Rect>) {
        let Some(g) = &self.geom else { return };
        if let Some(clip) = clip {
            list.push(Prim::PushClip(RoundedRect { rect: clip, radii: Corners::default() }), clip);
        }
        let s = self.editor.selection();
        let focused = self.editor.focused();
        let mut selected = Vec::new();
        let mut selected_text = None;
        if !s.range().is_empty() && !self.editor.composing() && (focused || self.editor.policy().keep_selection_unfocused) {
            let fill = if focused { colors.selection } else { colors.selection_inactive };
            selected_text = selection_text(colors.text, fill);
            for r in g.selection_rects(self.shown.range_to_shown(s.range())) {
                let r = r.translate(o);
                selected.push(r);
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
            // text the selection fill would hide (a light fill under light text): the selected
            // glyphs again, clipped to the selection, in a colour that reads on the fill
            if let Some(c) = selected_text {
                for &r in &selected {
                    list.push(Prim::PushClip(RoundedRect { rect: r, radii: Corners::default() }), r);
                    g.record_glyphs(&mut *list, Point::ZERO + o, c);
                    list.push(Prim::PopClip, r);
                }
            }
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
        if let Some(clip) = clip {
            list.push(Prim::PopClip, clip);
        }
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

    /// Another field took the one selection (GTK): collapse this one's to its start.
    pub fn lose_selection(&mut self) {
        self.editor.collapse_to_start();
        self.after();
    }

    /// The field holds a non-empty selection.
    pub fn has_selection(&self) -> bool {
        !self.editor.selection().range().is_empty()
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

    /// The runs of the shown text: the layout's, or unplaced rows while there is no layout.
    fn a11y_ranges(&self) -> Cow<'_, TextRanges> {
        if self.geom.is_some() && !self.stale { Cow::Borrowed(&self.ranges) } else { Cow::Owned(TextRanges::build(&self.shown.text, None, &[])) }
    }

    /// The runs sent: all of them, or, past the virtual-children cap, the whole rows in view with a
    /// viewport of margin on each side (as Chromium's virtualised editors do).
    fn a11y_window(&self, r: &TextRanges) -> Range<usize> {
        let n = r.runs.len();
        if n <= MAX_RUNS {
            return 0..n;
        }
        let h = self.content.height().max(1.0);
        let (top, bottom) = (self.scroll.y - h, self.scroll.y + 2.0 * h);
        let mut a = r.runs.iter().position(|q| q.rect.max.y >= top).unwrap_or(n - 1);
        while a > 0 && r.runs[a].chunk > 0 {
            a -= 1;
        }
        let mut b = r.runs[a..].iter().position(|q| q.rect.min.y > bottom).map_or(n, |k| a + k);
        while b < n && r.runs[b].chunk > 0 {
            b += 1;
        }
        if b - a > MAX_RUNS {
            b = a + MAX_RUNS;
            while b > a + 1 && r.runs[b].chunk > 0 {
                b -= 1;
            }
        }
        a..b
    }

    /// The field's AccessKit node, and its text as `Role::TextRun` nodes (plan `qc-a11y` §3.1) for
    /// the owner to add as virtual children, in order: `run_id(k)` is the id the `k`-th gets. The
    /// runs carry the shown text (a password field's masks), the selection names them, and a
    /// single-line field keeps its `value` (UIA's ValuePattern, macOS `AXValue`); a multi-line one
    /// leaves it to the runs.
    pub fn a11y(&self, node: &mut accesskit::Node, run_id: &dyn Fn(usize) -> accesskit::NodeId) -> Vec<accesskit::Node> {
        let cfg = self.editor.config();
        node.set_role(if cfg.password {
            accesskit::Role::PasswordInput
        } else if cfg.multiline {
            accesskit::Role::MultilineTextInput
        } else {
            accesskit::Role::TextInput
        });
        if !cfg.multiline {
            node.set_value(if cfg.password { self.shown.text.clone() } else { self.editor.text() });
        }
        if !self.placeholder.is_empty() {
            node.set_placeholder(self.placeholder.clone());
        }
        if cfg.read_only {
            node.set_read_only();
        } else {
            node.add_action(accesskit::Action::SetValue);
            node.add_action(accesskit::Action::ReplaceSelectedText);
        }
        node.add_action(accesskit::Action::SetTextSelection);
        node.add_action(accesskit::Action::Focus);

        let ranges = self.a11y_ranges();
        let w = self.a11y_window(&ranges);
        let o = self.to_node();
        let mut runs = Vec::with_capacity(w.len());
        for (k, r) in ranges.runs[w.clone()].iter().enumerate() {
            let mut n = accesskit::Node::new(accesskit::Role::TextRun);
            n.set_text_direction(accesskit::TextDirection::LeftToRight);
            let b = r.rect.translate(o);
            n.set_bounds(accesskit::Rect { x0: b.min.x as f64, y0: b.min.y as f64, x1: b.max.x as f64, y1: b.max.y as f64 });
            n.set_value(r.text.clone());
            n.set_character_lengths(r.lengths.clone());
            n.set_character_positions(r.positions.clone());
            n.set_character_widths(r.widths.clone());
            n.set_word_starts(r.word_starts.clone());
            if r.chunk > 0 && k > 0 {
                n.set_previous_on_line(run_id(k - 1));
            }
            if r.chunk + 1 < r.chunks && w.start + k + 1 < w.end {
                n.set_next_on_line(run_id(k + 1));
            }
            runs.push(n);
        }
        // in a composition the caret is at its active clause's end, which the runs hold
        let s = self.editor.selection();
        let (anchor, focus) = match self.preedit_caret() {
            Some(c) => (c, c),
            None => (self.shown.to_shown(s.anchor), self.shown.to_shown(s.head)),
        };
        let pos = |b: usize| {
            let (k, c) = match ranges.position(b) {
                (k, _) if k < w.start => (w.start, 0),
                (k, _) if k >= w.end => (w.end - 1, ranges.runs[w.end - 1].lengths.len()),
                kc => kc,
            };
            accesskit::TextPosition { node: run_id(k - w.start), character_index: c }
        };
        node.set_text_selection(accesskit::TextSelection { anchor: pos(anchor), focus: pos(focus) });
        runs
    }

    /// An AccessKit action on the field: `SetValue`, `ReplaceSelectedText` and `SetTextSelection`,
    /// whose positions name runs; `run_of` gives a run id's index among the runs [`TextField::a11y`]
    /// returned. Whether the field took it.
    pub fn a11y_action(&mut self, req: &accesskit::ActionRequest, run_of: &dyn Fn(accesskit::NodeId) -> Option<usize>) -> bool {
        let took = match (req.action, &req.data) {
            (accesskit::Action::SetValue, Some(accesskit::ActionData::Value(v))) => {
                self.editor.set_value_by_user(v);
                true
            }
            (accesskit::Action::ReplaceSelectedText, Some(accesskit::ActionData::Value(v))) => {
                self.editor.replace_selection(v);
                true
            }
            (accesskit::Action::SetTextSelection, Some(accesskit::ActionData::SetTextSelection(sel))) => {
                let bytes = {
                    let ranges = self.a11y_ranges();
                    let w = self.a11y_window(&ranges);
                    let byte = |p: &accesskit::TextPosition| run_of(p.node).filter(|k| w.start + k < w.end).map(|k| self.shown.to_doc(ranges.byte(w.start + k, p.character_index)));
                    byte(&sel.anchor).zip(byte(&sel.focus))
                };
                match bytes {
                    Some((a, h)) => {
                        self.editor.set_selection(a, h);
                        true
                    }
                    None => false,
                }
            }
            _ => false,
        };
        self.after();
        took
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
        self.to_window.apply_rect(&r.translate(self.to_node()))
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
        let local = self.to_window.inverse()?.apply(p) - self.to_node();
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
    use skelter_base::event::{KeyLocation, KeyState, Modifiers};
    use skelter_base::key::Key;
    use crate::text::shim::ListSink;

    const LAY: MonoLayouter = MonoLayouter { advance: 10.0, line_height: 20.0 };

    fn field(text: &str, cfg: EditorConfig) -> (TextField, MonoLayouter) {
        let mut f = TextField::new(text, cfg, TextPolicy::of(Platform::Gtk));
        let mut lay = LAY;
        // a 100×20 content box at (4, 2) in a node placed at (50, 60) in the window
        f.prepare(&mut lay, Rect::new(4.0, 2.0, 104.0, 22.0), Affine::translate(50.0, 60.0));
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
        assert_eq!(f.caret_update(), Some(Some(Rect::new(4.0, 2.0, 5.0, 22.0))), "node coordinates");
        assert_eq!(f.caret_update(), None);
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
    fn selected_text_reads_on_the_selection_fill() {
        let hex = |v: &str| crate::theme::parse_color(v).unwrap();
        assert_eq!(selection_text(hex("#CCCCCC"), hex("#264F78")), None, "Dark Modern keeps its text colour");
        assert_eq!(selection_text(hex("#3B3B3B"), hex("#ADD6FF")), None, "Light Modern too");
        assert_eq!(selection_text(hex("#FFFFFF"), hex("#F3F518")), Some(Color::rgba(0.0, 0.0, 0.0, 1.0)), "HC Dark's yellow: black text");
        assert_eq!(selection_text(hex("#292929"), hex("#0F4A85")), Some(Color::rgba(1.0, 1.0, 1.0, 1.0)), "HC Light's navy: white text");
        assert_eq!(selection_text(hex("#FFFFFF"), hex("#F3F51880")), None, "a translucent fill keeps the text");
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
            // "world" is 60..110 in the layout; the 100-point box scrolled 11 to show the caret at 110
            Prim::Quad(q) => assert_eq!(q.rect, Rect::new(53.0, 2.0, 103.0, 22.0)),
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
        f.prepare(&mut lay, Rect::new(0.0, 0.0, 40.0, 60.0), Affine::IDENTITY);
        let m = f.measure(&mut lay, Some(40.0));
        assert_eq!(m, Size::new(40.0, 60.0), "the placeholder wraps to three rows");
        f.focus_in(FocusReason::Pointer);
        f.set_text("abcdefgh");
        f.prepare(&mut lay, Rect::new(0.0, 0.0, 40.0, 60.0), Affine::IDENTITY);
        f.editor.set_selection(1, 1);
        press(&mut f, &mut lay, Key::ArrowDown, Modifiers::default(), None);
        assert_eq!(f.editor.caret(), 5, "down moves to the next wrapped row");
        press(&mut f, &mut lay, Key::End, Modifiers::default(), None);
        assert_eq!(f.editor.caret(), 8);
    }

    fn ax(f: &TextField) -> (accesskit::Node, Vec<accesskit::Node>) {
        let mut n = accesskit::Node::new(accesskit::Role::TextInput);
        let runs = f.a11y(&mut n, &|k| accesskit::NodeId(1000 + k as u64));
        (n, runs)
    }

    fn select(f: &mut TextField, anchor: (usize, usize), focus: (usize, usize)) -> bool {
        let pos = |(k, c): (usize, usize)| accesskit::TextPosition { node: accesskit::NodeId(1000 + k as u64), character_index: c };
        let req = accesskit::ActionRequest {
            action: accesskit::Action::SetTextSelection,
            target_tree: accesskit::TreeId::ROOT,
            target_node: accesskit::NodeId(1),
            data: Some(accesskit::ActionData::SetTextSelection(accesskit::TextSelection { anchor: pos(anchor), focus: pos(focus) })),
        };
        f.a11y_action(&req, &|id| id.0.checked_sub(1000).map(|k| k as usize))
    }

    #[test]
    fn runs_carry_the_text_and_the_selection_names_them() {
        let (mut f, mut lay) = field("hello wörld", EditorConfig::default());
        f.focus_in(FocusReason::Pointer);
        // the end: "ö" is two bytes
        f.editor.set_selection(12, 12);
        f.take_a11y();
        press(&mut f, &mut lay, Key::Exclamationmark, Modifiers::default(), Some("!"));
        assert!(f.take_a11y(), "typing re-sends the node");
        press(&mut f, &mut lay, Key::ArrowLeft, Modifiers::SHIFT, None);
        assert!(f.take_a11y(), "a selection change re-sends it");
        let (n, runs) = ax(&f);
        assert_eq!(n.value(), Some("hello wörld!"));
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].role(), accesskit::Role::TextRun);
        assert_eq!(runs[0].value(), Some("hello wörld!"));
        assert_eq!(runs[0].character_lengths().len(), 12);
        assert_eq!(runs[0].character_lengths()[7], 2, "ö is two bytes");
        assert_eq!(runs[0].word_starts(), &[0, 6]);
        assert_eq!(runs[0].character_positions().unwrap()[1], 10.0);
        let sel = n.text_selection().unwrap();
        assert_eq!((sel.anchor.node, sel.anchor.character_index), (accesskit::NodeId(1000), 12));
        assert_eq!((sel.focus.node, sel.focus.character_index), (accesskit::NodeId(1000), 11));
        assert!(n.supports_action(accesskit::Action::SetTextSelection));
        // AT moves the selection: "hello" (characters 0..5)
        assert!(select(&mut f, (0, 0), (0, 5)));
        let s = f.editor.selection();
        assert_eq!((s.anchor, s.head), (0, 5));
        assert!(f.take_a11y());
    }

    #[test]
    fn an_empty_field_has_one_empty_run_and_a_password_no_clear_text() {
        let (f, _) = field("", EditorConfig::default());
        let (n, runs) = ax(&f);
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].value(), Some(""));
        assert!(runs[0].character_lengths().is_empty());
        assert_eq!(n.text_selection().map(|s| s.focus.character_index), Some(0));

        let (p, _) = field("hunter2", EditorConfig { password: true, ..EditorConfig::default() });
        let (n, runs) = ax(&p);
        assert_eq!(n.role(), accesskit::Role::PasswordInput);
        for v in std::iter::once(n.value()).chain(runs.iter().map(|r| r.value())) {
            let v = v.unwrap_or("");
            assert!(!v.contains("hunter") && !v.contains('2'), "clear text left the field: {v:?}");
        }
        assert_eq!(runs[0].character_lengths().len(), 7, "one mask a character");
        assert!(runs[0].word_starts().is_empty());
    }

    #[test]
    fn multiline_runs_end_in_line_feeds_and_leave_value_unset() {
        let cfg = EditorConfig { multiline: true, ..EditorConfig::default() };
        let mut f = TextField::new("ab\ncd", cfg, TextPolicy::of(Platform::Gtk));
        let mut lay = LAY;
        f.prepare(&mut lay, Rect::new(4.0, 2.0, 104.0, 42.0), Affine::IDENTITY);
        let (n, runs) = ax(&f);
        assert_eq!(n.role(), accesskit::Role::MultilineTextInput);
        assert_eq!(n.value(), None, "consumers read the runs");
        assert_eq!(runs.iter().map(|r| r.value().unwrap()).collect::<Vec<_>>(), vec!["ab\n", "cd"]);
        assert_eq!(runs[0].character_widths().unwrap()[2], 0.0);
        // bounds in node space: the content box's origin
        let b = runs[1].bounds().unwrap();
        assert_eq!((b.x0, b.y0, b.x1, b.y1), (4.0, 22.0, 24.0, 42.0));
        // a selection after the line feed starts the second run
        f.editor.set_selection(3, 4);
        let (n, _) = ax(&f);
        let s = n.text_selection().unwrap();
        assert_eq!((s.anchor.node, s.anchor.character_index, s.focus.character_index), (accesskit::NodeId(1001), 0, 1));
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
