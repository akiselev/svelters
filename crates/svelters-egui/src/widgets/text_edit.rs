//! `text-edit`, svelters' own (OW4, ARCH-WIDGETS §4, §6.1): box, text (galley, selection, caret,
//! preedit); Hoverable, Focusable, [`TextInput`] on TC0 (`quire-text`); paint parts
//! `placeholder` (`::placeholder`), `selection` (`::selection`), `caret`, `preedit`. Replaces the
//! `egui::TextEdit` wrapper of `leaf.rs` (deleted in this lane, with FX1's and FX3's code around it).
//!
//! **Look** (DESIGN R4/R5/R8/R9/R10, UA [`UA`]): a sunken field (`input-bg` fill, `input-border-color`
//! boundary, radius `input-radius`, an inset `0 1px 1px` shadow), the border `input-border-color-focus`
//! while focused, `invalid-border` while `:invalid`; read-only as text (no fill, border or inset);
//! disabled at 40% with `not-allowed`; the I-beam cursor; the keyboard focus ring (`:focus-visible`,
//! 1px at offset −1). 26px tall at the default density (padding 4/6, 16px lines, 1px border).
//!
//! **Behaviour** is [`TextInput`]'s, with the platform's rules (`native::TextPolicy`): word stops,
//! undo grouping, paste, PRIMARY, hex entry, focus selection. Signals as the built-in row:
//! `changed` (every user edit), `gained-focus`, `lost-focus`, `commit` (Enter in a one-line field,
//! also when a default button takes it, or blur after an edit), `submit` (Enter; Ctrl+Enter in a
//! multi-line field), plus the extras `rejected` and `selection-changed`.

use crate::interact::{FocusParams, Focusable, Hoverable, TextInput, TextIntent, TextParams};
use crate::paint::{self, StyleView, Ua, UaRule, UaValue::*};
use crate::provider::{A11yCx, AvailableSpace, Event, MeasureCx, Part, StateBits, Widget, WidgetCx};
use egui::text::CCursor;
use egui::{Color32, Pos2, Rect, Vec2, pos2, vec2};
use quire_text::{EditCommand, HostRequest, InputKind, Rejected, Selection};
use std::sync::Arc;
use svelters_style::{Color, Shadow};

/// R4: the sunken field's inset (dark: `.25`).
const FIELD_INSET: &[Shadow] = &[Shadow { x: 0.0, y: 1.0, blur: 1.0, spread: 0.0, color: Some(Color { r: 0.0, g: 0.0, b: 0.0, a: 0.25 }), inset: true }];

/// The field's UA sheet (DESIGN §1.3 input box: 26px, radius 4; R4, R5, R8, R9, R10).
pub const UA: Ua = Ua(&[
    UaRule::base(&[
        ("background-color", Token("input-bg")),
        ("color", Token("input-fg")),
        ("border-color", Token("input-border-color")),
        ("border-width", Token("stroke")),
        ("border-radius", Token("input-radius")),
        ("box-shadow", Shadows(FIELD_INSET)),
        ("padding-left", Px(6.0)),
        ("padding-right", Px(6.0)),
        ("padding-top", Px(4.0)),
        ("padding-bottom", Px(4.0)),
        ("font-size", Token("font-size")),
        ("line-height", Px(16.0)),
        ("cursor", Keyword("text")),
        ("outline-style", Keyword("none")),
        ("outline-color", Token("focus")),
        ("outline-width", Px(1.0)),
        ("outline-offset", Px(-1.0)),
        // the intrinsic width of a field (egui's `text_edit_width`, which layouts were made with)
        ("--sv-field-width", Px(280.0)),
    ]),
    UaRule::when(StateBits::FOCUSED, &[("border-color", Token("input-border-color-focus"))]),
    UaRule::when(StateBits::FOCUS_VISIBLE, &[("outline-style", Keyword("solid"))]),
    UaRule::when(StateBits::INVALID, &[("border-color", Token("invalid-border"))]),
    UaRule::when(StateBits::READ_ONLY, &[("background-color", Color(Color32::TRANSPARENT)), ("border-color", Color(Color32::TRANSPARENT)), ("box-shadow", Shadows(&[]))]),
    UaRule::when(StateBits::DISABLED, &[("opacity", Number(0.4)), ("cursor", Keyword("not-allowed")), ("box-shadow", Shadows(&[]))]),
]);

/// `::placeholder`: VS Code's `input.placeholderForeground`.
pub const PLACEHOLDER_UA: Ua = Ua(&[UaRule::base(&[("color", Token("placeholder"))])]);
/// `::selection`: `editor.selectionBackground` (the text keeps its colour unless `color` is set).
pub const SELECTION_UA: Ua = Ua(&[UaRule::base(&[("background-color", Token("selection"))])]);
/// `::part(caret)`: `editorCursor.foreground`, 1px (`width`).
pub const CARET_UA: Ua = Ua(&[UaRule::base(&[("background-color", Token("caret")), ("width", Px(1.0))])]);
/// `::part(preedit)`: the composing text, underlined in the text colour.
pub const PREEDIT_UA: Ua = Ua(&[]);

/// A single- or multi-line text field (`QLineEdit`, `QPlainTextEdit`).
#[svelters::widget(namespace = "sv", replaces = "text-edit", caps = FOCUSABLE | PAINTS_BOX)]
pub struct TextEdit {
    /// The edited text; typing changes it.
    #[prop]
    text: String,
    /// Shown while the text is empty; also the default accessible name.
    #[prop]
    hint_text: String,
    /// `hint-text` by its HTML name (QLineEdit `placeholderText`); wins when set.
    #[prop]
    placeholder: String,
    /// Several lines: Enter inserts a line break.
    #[prop(measure)]
    multiline: bool,
    /// Hides the characters (`echo="password"`).
    #[prop]
    password: bool,
    /// `normal`, `password` or `none` (QLineEdit `echoMode`; `none` shows nothing).
    #[prop(enumeration = "normal|password|none", default = "normal")]
    echo: String,
    /// At most this many characters (code points); 0 for none.
    #[prop]
    char_limit: i64,
    /// `char-limit` by its HTML name; wins when above 0.
    #[prop]
    maxlength: i64,
    /// Motions, selection and copy only (QLineEdit `readOnly`, `:read-only`).
    #[prop]
    read_only: bool,
    /// Undo and redo (`undo={false}` turns the history off).
    #[prop(default = true)]
    undo: bool,
    /// A multi-line field's Tab moves focus (otherwise it inserts a tab; Ctrl+Tab always moves).
    #[prop(default = true)]
    tab_focus: bool,
    /// A multi-line field's wrapping: `word`, `char` or `none`.
    #[prop(enumeration = "word|char|none", default = "word", measure)]
    wrap: String,
    /// A multi-line field's height in lines.
    #[prop(default = 4, measure)]
    rows: i64,
    /// The selection's start, in characters (a write selects).
    #[prop]
    selection_start: i64,
    /// The selection's end, in characters (equal to `selection-start` without a selection).
    #[prop]
    selection_end: i64,
    /// The user edited since the last `commit` (QLineEdit `isModified`); the app may clear it.
    #[prop]
    modified: bool,
    /// Undo would do something now (read-only, observable).
    #[prop]
    can_undo: bool,
    /// Redo would do something now (read-only, observable).
    #[prop]
    can_redo: bool,
    /// Copy would copy something now: a selection outside a password field (read-only, observable).
    #[prop]
    can_copy: bool,
    /// The input method's purpose: `text`, `decimal`, `numeric`, … (`password` fields always ask
    /// for a password IME).
    #[prop(default = "text")]
    inputmode: String,
    /// The user edited the text; carries the new text (once per frame).
    #[event]
    changed: Event<String>,
    /// Took keyboard focus.
    #[event]
    gained_focus: Event<()>,
    /// Lost keyboard focus.
    #[event]
    lost_focus: Event<()>,
    /// The user committed the text (Enter, or blur after an edit); carries the text.
    #[event]
    commit: Event<String>,
    /// Enter in a one-line field (Ctrl+Enter in a multi-line one); after `commit`.
    #[event]
    submit: Event<String>,
    /// An edit was refused or cut short: `max-chars`, `read-only`, `filtered`, `line-break`,
    /// `secret` (QLineEdit `inputRejected`).
    #[event]
    rejected: Event<String>,
    /// The selection or caret moved (read `selection-start` and `selection-end`).
    #[event]
    selection_changed: Event<()>,
    #[behavior]
    hover: Hoverable,
    #[behavior(params = focus_params)]
    focus: Focusable,
    #[behavior(params = text_params)]
    input: TextInput,
    /// `::placeholder`: `color`, `font-*`.
    #[part(name = "placeholder")]
    placeholder_part: Part,
    /// `::selection`: `background-color`, `color`.
    #[part(name = "selection")]
    selection_part: Part,
    /// `::part(caret)`: `background-color`, `width`.
    #[part(name = "caret")]
    caret_part: Part,
    /// `::part(preedit)`: `color`, the underline.
    #[part(name = "preedit")]
    preedit_part: Part,
    /// Scroll offset of the text in its box (one-line: x; multi-line: y).
    #[state]
    scroll: Vec2,
    /// The selection as last announced (chars), so a prop write is told from our own.
    #[state]
    seen_sel: (i64, i64),
    /// A commit happened (`:user-invalid` shows from then on).
    #[state]
    committed: bool,
}

/// What a text field shows: its text, masked, with line breaks of a one-line field as spaces and a
/// preedit at the caret.
struct Shown {
    text: String,
    /// The preedit's char range in `text`.
    preedit: Option<std::ops::Range<usize>>,
    /// `text` is the document's char for char.
    exact: bool,
}

impl TextEdit {
    fn focus_params(&self) -> FocusParams {
        // TextInput takes focus on a primary press itself (a middle click on macOS takes none)
        FocusParams { tab_stop: true, focus_on_click: Some(false) }
    }

    fn secret(&self) -> bool {
        self.password || self.echo != "normal"
    }

    fn text_params(&self) -> TextParams {
        let max = if self.maxlength > 0 { self.maxlength } else { self.char_limit };
        TextParams {
            multiline: self.multiline,
            secret: self.secret(),
            read_only: self.read_only,
            max_chars: (max > 0).then(|| max.min(u32::MAX as i64) as u32),
            tab_inserts: self.multiline && !self.tab_focus,
            no_undo: !self.undo,
        }
    }

    fn hint(&self) -> &str {
        if self.placeholder.is_empty() { &self.hint_text } else { &self.placeholder }
    }

    fn wrap_width(&self, content: f32) -> Option<f32> {
        (self.multiline && self.wrap != "none").then_some(content)
    }

    /// What the field shows now.
    fn shown(&self, s: &StyleView) -> Shown {
        let session = self.input.session();
        let mut text = match session {
            Some(x) => x.text().to_string(),
            None => self.text.clone(),
        };
        let mut exact = true;
        if self.echo == "none" {
            return Shown { text: String::new(), preedit: None, exact: false };
        }
        if self.secret() {
            let mask = s.keyword("--sv-password-char").and_then(|k| k.trim_matches('"').chars().next()).unwrap_or('\u{2022}');
            text = text.chars().map(|_| mask).collect();
        } else if !self.multiline && text.contains('\n') {
            // a one-line field shows the line breaks it keeps (GTK, Qt) as spaces, char for char
            text = text.replace('\n', " ");
        }
        let mut preedit = None;
        if let Some(x) = session
            && let Some(p) = x.view.preedit()
        {
            let caret = x.text().byte_to_char(x.view.selection().primary().head);
            let at = text.char_indices().nth(caret).map_or(text.len(), |(i, _)| i);
            text.insert_str(at, &p.text);
            preedit = Some(caret..caret + p.text.chars().count());
            exact = false;
        }
        Shown { text, preedit, exact }
    }

    fn line_height(s: &StyleView, g: &egui::Galley) -> f32 {
        let row = g.rows.first().map_or(s.font_size(), |r| r.row.size.y);
        s.px("line-height").filter(|l| *l > 0.0).unwrap_or(row).max(row)
    }

    /// Where the galley goes in `content` (scrolled).
    fn origin(&self, s: &StyleView, g: &egui::Galley, content: Rect) -> Pos2 {
        if self.multiline {
            let lh = Self::line_height(s, g);
            let row = g.rows.first().map_or(lh, |r| r.row.size.y);
            pos2(content.left(), content.top() + (lh - row) / 2.0 - self.scroll.y)
        } else {
            pos2(content.left() - self.scroll.x, content.center().y - g.size().y / 2.0)
        }
    }

    fn field_layout(&self, ctx: &egui::Context, s: &StyleView, content: Rect) -> (Shown, Arc<egui::Galley>, Pos2) {
        let shown = self.shown(s);
        let g = paint::text_galley(ctx, s, &shown.text, self.wrap_width(content.width()));
        let o = self.origin(s, &g, content);
        (shown, g, o)
    }

    /// The caret's char in the shown text and the selection's char range.
    fn caret_and_range(&self, shown: &Shown) -> Option<(usize, std::ops::Range<usize>)> {
        let s = self.input.session()?;
        let [a, h] = s.chars();
        let (lo, hi) = (a.min(h), a.max(h));
        Some(match &shown.preedit {
            Some(p) => (p.end, lo..hi),
            None => (h, lo..hi),
        })
    }

    /// Keeps the caret inside the content box (single-line: horizontally; multi-line: vertically).
    fn reveal(&mut self, s: &StyleView, g: &egui::Galley, content: Rect, caret: usize) {
        let r = g.pos_from_cursor(cursor(caret));
        if self.multiline {
            let lh = Self::line_height(s, g);
            let (top, bottom) = (r.min.y, r.min.y + lh);
            if top < self.scroll.y {
                self.scroll.y = top;
            } else if bottom > self.scroll.y + content.height() {
                self.scroll.y = bottom - content.height();
            }
            let max = (g.size().y - content.height()).max(0.0);
            self.scroll.y = self.scroll.y.clamp(0.0, max);
            self.scroll.x = 0.0;
        } else {
            let x = r.min.x;
            let w = content.width().max(1.0);
            if x < self.scroll.x {
                self.scroll.x = x;
            } else if x > self.scroll.x + w - 1.0 {
                self.scroll.x = x - w + 1.0;
            }
            let max = (g.size().x - w + 1.0).max(0.0);
            self.scroll.x = self.scroll.x.clamp(0.0, max);
            self.scroll.y = 0.0;
        }
    }

    /// The selection as char offsets `[anchor, caret]` (tests, the measurement harness).
    pub fn selection(&self) -> Option<[usize; 2]> {
        self.input.session().map(|s| s.chars())
    }

    /// The galley's screen position of char `at` (`frac` of the way to the next char), as last
    /// drawn: where a click lands on it.
    pub fn char_point(&self, at: usize, frac: f32) -> Option<Pos2> {
        let l = self.input.layout.as_ref()?;
        let n = l.galley.text().chars().count();
        let r0 = l.galley.pos_from_cursor(cursor(at.min(n)));
        let x1 = if at < n { l.galley.pos_from_cursor(cursor(at + 1)).min.x } else { r0.min.x + 6.0 };
        Some(l.origin + vec2(r0.min.x + (x1 - r0.min.x) * frac, r0.center().y))
    }
}

fn cursor(c: usize) -> CCursor {
    let mut k = CCursor::new(c);
    k.prefer_next_row = true;
    k
}

fn rejected_name(r: Rejected) -> &'static str {
    match r {
        Rejected::ReadOnly => "read-only",
        Rejected::MaxChars => "max-chars",
        Rejected::Filtered => "filtered",
        Rejected::LineBreak => "line-break",
        Rejected::Secret => "secret",
    }
}

fn collapse_id() -> egui::Id {
    egui::Id::unique("svelters-egui/text-edit/collapse")
}

impl Widget for TextEdit {
    fn measure(&mut self, cx: &mut MeasureCx<'_>, _avail: AvailableSpace) -> egui::Vec2 {
        let s = cx.style(UA);
        let g = paint::text_galley(cx.ctx, &s, "Ag", None);
        let lh = Self::line_height(&s, &g);
        let lines = if self.multiline { self.rows.max(1) as f32 } else { 1.0 };
        let w = s.px("--sv-field-width").unwrap_or(280.0);
        vec2(w, lh * lines) + paint::box_extra(&s)
    }

    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx<'_>) -> egui::Response {
        let ctx = ui.ctx().clone();
        let id = cx.id();
        let policy = crate::native::policy_of(&ctx);
        let params = self.text_params();
        let now = ctx.input(|i| i.time);
        // the prop mirror (§2.3)
        let common = cx.node().common();
        let invalid = common.invalid;
        cx.set_state(StateBits::READ_ONLY, self.read_only);
        cx.set_state(StateBits::INVALID, invalid);
        cx.set_state(StateBits::USER_INVALID, invalid && self.committed);
        crate::native::note_tab(ui);
        self.input.entered = crate::focus::take_entered(ui, id);
        let resp = cx.interact(ui, self.units_sense());
        // the session is made when the field is first focused, pressed or asked (§4.6)
        let pressed_here = resp.contains_pointer() && ctx.input(|i| i.pointer.any_pressed());
        let asked = ctx.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::AccessKitActionRequest(r) if r.target_node == id.accesskit_id())));
        let create = resp.has_focus() || pressed_here || asked || self.input.entered;
        self.input.sync(&self.text, &policy, &params, create);
        // GTK: another field took the selection (one owner, X11 PRIMARY): ours collapses to its start
        if ctx.data(|d| d.get_temp::<egui::Id>(collapse_id())) == Some(id) {
            ctx.data_mut(|d| d.remove::<egui::Id>(collapse_id()));
            if let Some([a, h]) = self.selection() {
                let lo = a.min(h);
                self.input.set_selection_chars(lo, lo);
            }
        }
        // a selection written from JS
        if let Some([a, h]) = self.selection()
            && (self.selection_start, self.selection_end) != self.seen_sel
        {
            let (lo, hi) = ((a.min(h)) as i64, (a.max(h)) as i64);
            if (self.selection_start, self.selection_end) != (lo, hi) {
                self.input.set_selection_chars(self.selection_start.max(0) as usize, self.selection_end.max(0) as usize);
            }
        }
        // last pass's geometry for hit-testing and rows
        let host = cx.style(UA);
        let content = paint::box_geom(&host, cx.rect()).content;
        {
            let (shown, g, o) = self.field_layout(&ctx, &host, content);
            self.input.layout = Some(crate::interact::text_input::FieldLayout { galley: g.clone(), origin: o, exact: shown.exact });
            let lh = Self::line_height(&host, &g).max(1.0);
            self.input.page_rows = (content.height() / lh).floor().max(1.0) as usize;
        }
        self.input.tab_focus = crate::native::tab_moved_focus(ui);
        if resp.has_focus() {
            // arrows and (per platform) Escape stay the field's; Tab moves focus unless it inserts
            let filter = egui::EventFilter { tab: params.tab_inserts, horizontal_arrows: true, vertical_arrows: true, escape: policy.escape_keeps_focus };
            ctx.memory_mut(|m| m.set_focus_lock_filter(id, filter));
        }
        let mut fired = self.run_units(cx, ui, &resp);
        // a middle-click paste reads PRIMARY (egui-winit sends it with the press, else the provider)
        let pastes: Vec<(usize, Option<String>)> = fired.input.iter().filter_map(|i| if let TextIntent::PastePrimary { at, text } = i { Some((*at, text.clone())) } else { None }).collect();
        for (at, text) in pastes {
            let t = text.or_else(|| crate::primary::provider().and_then(|p| p.get())).unwrap_or_default();
            if t.is_empty() {
                continue;
            }
            self.input.exec(EditCommand::SetSelection(Selection::point(at)), now, &mut fired.input);
            self.input.exec(EditCommand::Insert { text: t, kind: InputKind::Paste, distribute: false }, now, &mut fired.input);
        }
        let focused = resp.has_focus() || fired.focus.iter().any(|f| matches!(f, crate::interact::Focus::Gained(_)));
        let mut edited = false;
        let mut sel_changed = false;
        for i in fired.input.iter() {
            match i {
                TextIntent::Edited => edited = true,
                TextIntent::SelectionChanged => sel_changed = true,
                TextIntent::Request(HostRequest::SetClipboard(s)) => ctx.copy_text(s.clone()),
                TextIntent::Request(HostRequest::ReadClipboard) => ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaste),
                TextIntent::Request(HostRequest::SetPrimary(s)) if focused => match crate::primary::provider() {
                    Some(p) => p.set(s),
                    None => ctx.output_mut(|o| o.commands.push(egui::OutputCommand::TextSelectionSettled(s.clone()))),
                },
                TextIntent::Request(HostRequest::ClearPrimary) if focused => {
                    let owner: Option<egui::Id> = ctx.data(|d| d.get_temp(crate::native::owner_id()));
                    if owner.is_none_or(|o| o == id) {
                        match crate::primary::provider() {
                            Some(p) => p.set(""),
                            None => ctx.output_mut(|o| o.commands.push(egui::OutputCommand::TextSelectionSettled(String::new()))),
                        }
                    }
                }
                _ => {}
            }
        }
        // the prop and the signals, in the order they happened
        if edited && let Some(s) = self.input.session() {
            let t = s.text().to_string();
            cx.set(&mut self.text, t);
            cx.set(&mut self.modified, true);
        }
        let text = self.text.clone();
        for i in fired.input.into_iter() {
            match i {
                TextIntent::Edited => {}
                TextIntent::Gained => {
                    cx.dispatch(&self.gained_focus, ());
                }
                TextIntent::Lost => {
                    cx.dispatch(&self.lost_focus, ());
                }
                TextIntent::Committed => {
                    self.committed = true;
                    cx.set(&mut self.modified, false);
                    cx.dispatch(&self.commit, text.clone());
                }
                TextIntent::Submitted => {
                    cx.dispatch(&self.submit, text.clone());
                }
                TextIntent::Rejected(r) => {
                    cx.dispatch(&self.rejected, rejected_name(r).to_string());
                }
                _ => {}
            }
        }
        if edited {
            cx.dispatch(&self.changed, text.clone());
        }
        // NSTextField: an unfocused field has no selection (no field editor)
        if !focused
            && policy.text.unfocused == crate::native::Unfocused::Clear
            && self.selection().is_some_and(|s| s != [0, 0])
        {
            self.input.set_selection_chars(0, 0);
        }
        // the derived props
        if let Some(s) = self.input.session() {
            let [a, h] = s.chars();
            let (lo, hi) = (a.min(h) as i64, a.max(h) as i64);
            let (cu, cr) = (s.doc.can_undo() && self.undo, s.doc.can_redo() && self.undo);
            let copy = hi > lo && !self.secret();
            cx.set(&mut self.selection_start, lo);
            cx.set(&mut self.selection_end, hi);
            self.seen_sel = (lo, hi);
            cx.set(&mut self.can_undo, cu);
            cx.set(&mut self.can_redo, cr);
            cx.set(&mut self.can_copy, copy);
            if sel_changed {
                cx.dispatch(&self.selection_changed, ());
            }
            // GTK: one field holds a selection; the previous owner's collapses
            if policy.one_selection_owner && focused && hi > lo {
                let owner: Option<egui::Id> = ctx.data(|d| d.get_temp(crate::native::owner_id()));
                if owner != Some(id) {
                    ctx.data_mut(|d| {
                        if let Some(o) = owner {
                            d.insert_temp(collapse_id(), o);
                        }
                        d.insert_temp(crate::native::owner_id(), id);
                    });
                    ctx.request_repaint();
                }
            }
        }
        cx.set_state(StateBits::PLACEHOLDER_SHOWN, self.text.is_empty() && !self.hint().is_empty());
        cx.set_state(StateBits::USER_INVALID, invalid && self.committed);
        cx.commit_state();

        // paint
        let host = cx.style(UA);
        if host.state().contains(StateBits::HOVERED)
            && let Some(c) = host.cursor()
        {
            ctx.set_cursor_icon(c);
        }
        if host.opacity() < 1.0 {
            ui.multiply_opacity(host.opacity());
        }
        let geom = if ui.is_rect_visible(cx.rect()) { cx.paint_box(ui, UA) } else { paint::box_geom(&host, cx.rect()) };
        let content = geom.content;
        let (shown, g, mut o) = self.field_layout(&ctx, &host, content);
        let caret = self.caret_and_range(&shown);
        if let Some((c, _)) = &caret
            && focused
        {
            let before = self.scroll;
            self.reveal(&host, &g, content, *c);
            if self.scroll != before {
                o = self.origin(&host, &g, content);
            }
        }
        if self.multiline && resp.hovered() {
            // the wheel scrolls a multi-line field that overflows
            let dy = ctx.input(|i| i.smooth_scroll_delta.y);
            let max = (g.size().y - content.height()).max(0.0);
            if dy != 0.0 && max > 0.0 {
                self.scroll.y = (self.scroll.y - dy).clamp(0.0, max);
                ctx.input_mut(|i| i.smooth_scroll_delta.y = 0.0);
                o = self.origin(&host, &g, content);
            }
        }
        let painter = ui.painter_at(geom.padding.intersect(ui.clip_rect()));
        let mut pc = paint::PaintCx::for_node(&painter, cx.node().id().to_raw());
        pc.tag = "text-edit";
        if shown.text.is_empty() && !self.hint().is_empty() {
            let ph = cx.part(&host, "placeholder", PLACEHOLDER_UA);
            let pg = paint::text_galley(&ctx, &ph, self.hint(), self.wrap_width(content.width()));
            let po = self.origin(&ph, &pg, content);
            let color = ph.color("color").unwrap_or(Color32::GRAY);
            painter.galley(po, pg, color);
        }
        // the selection under the text (focused, or kept unfocused per platform)
        if let Some((_, range)) = &caret
            && !range.is_empty()
            && !shown.text.is_empty()
        {
            let sel = cx.part(&host, "selection", SELECTION_UA);
            let bg = sel.color("background-color").unwrap_or(Color32::from_rgba_unmultiplied(38, 79, 120, 255));
            let bg = if focused { bg } else { bg.gamma_multiply(0.6) };
            for r in selection_rects(&g, range.clone()) {
                painter.rect_filled(r.translate(o.to_vec2()), 0.0, bg);
            }
        }
        let color = host.color("color").unwrap_or(Color32::PLACEHOLDER);
        if !shown.text.is_empty() {
            painter.galley(o, g.clone(), color);
        }
        // the preedit, underlined (thicker for its active range)
        if let Some(p) = &shown.preedit {
            let pe = cx.part(&host, "preedit", PREEDIT_UA);
            let uc = pe.color("color").unwrap_or(color);
            for r in selection_rects(&g, p.clone()) {
                let r = r.translate(o.to_vec2());
                painter.line_segment([r.left_bottom(), r.right_bottom()], egui::Stroke::new(1.0, uc));
            }
        }
        // the caret, blinking at the platform's rate (none after 10 s idle, as GTK)
        let mut caret_rect = None;
        if focused && let Some((c, _)) = &caret {
            let cp = cx.part(&host, "caret", CARET_UA);
            let r = g.pos_from_cursor(cursor(*c)).translate(o.to_vec2());
            let lh = Self::line_height(&host, &g);
            let w = cp.px("width").unwrap_or(1.0).max(1.0);
            let x = (r.min.x * pc.ppp).round() / pc.ppp;
            let rect = Rect::from_min_size(pos2(x, r.center().y - lh / 2.0), vec2(w, lh));
            caret_rect = Some(rect);
            let period = policy.caret_blink_ms as f64 / 1000.0;
            let idle = now - self.input.last_activity;
            let on = period <= 0.0 || idle > 10.0 || (idle % period) < period / 2.0;
            if on && !self.read_only {
                painter.rect_filled(rect, 0.0, cp.color("background-color").unwrap_or(color));
            }
            if period > 0.0 && idle <= 10.0 {
                let left = period / 2.0 - (idle % (period / 2.0));
                ctx.request_repaint_after(std::time::Duration::from_secs_f64(left.max(0.01)));
            }
        }
        if ui.is_rect_visible(cx.rect()) {
            cx.paint_outline(ui, UA);
        }
        // IME: where the caret is (the candidate window), every frame the field is focused
        if focused && let Some(cr) = caret_rect {
            let purpose = if self.secret() { egui::IMEPurpose::Password } else { egui::IMEPurpose::Normal };
            let rect = cx.rect();
            ctx.output_mut(|out| out.ime = Some(egui::output::IMEOutput { purpose, rect, cursor_rect: cr, should_interrupt_composition: false }));
        }
        // the next pass hit-tests against what was drawn
        self.input.layout = Some(crate::interact::text_input::FieldLayout { galley: g.clone(), origin: o, exact: shown.exact });
        // AccessKit: value (masked), placeholder, text runs, caret and selection (§5)
        let (enabled, hint) = (cx.enabled(), self.hint().to_string());
        let masked = if self.secret() { shown.text.clone() } else { self.text.clone() };
        resp.widget_info(|| egui::WidgetInfo::text_edit(enabled, masked.clone(), masked.clone(), hint.clone()));
        if let Some(s) = self.input.session()
            && shown.exact
        {
            let [a, h] = s.chars();
            let range = egui::text::CCursorRange::two(CCursor::new(a), CCursor::new(h));
            egui::text_selection::accesskit_text::update_accesskit_for_text_widget(&ctx, id, Some(range), egui::emath::TSTransform::from_translation(o.to_vec2()), &g);
        }
        resp
    }

    fn a11y(&self, _cx: &A11yCx<'_>, node: &mut egui::accesskit::Node) {
        self.units_a11y(node);
        let p = self.text_params();
        if !self.hint().is_empty() {
            node.set_placeholder(self.hint().to_string());
        }
        if p.secret {
            node.set_role(egui::accesskit::Role::PasswordInput);
        }
    }

    fn consume_key(&self, key: egui::Key, m: egui::Modifiers) -> bool {
        self.units_claims(key, m)
    }
}

/// The rects (galley coordinates) covering chars `range` of `g`, one per row.
fn selection_rects(g: &egui::Galley, range: std::ops::Range<usize>) -> Vec<Rect> {
    let mut out = Vec::new();
    let mut c = 0usize;
    for (k, r) in g.rows.iter().enumerate() {
        let n = r.row.glyphs.len();
        let (rs, re) = (c, c + n);
        let lo = range.start.max(rs);
        let hi = range.end.min(re + usize::from(r.ends_with_newline));
        if lo < hi || (lo == hi && range.start < rs && range.end > re) {
            let x0 = g.pos_from_cursor(cursor(lo)).min.x;
            let x1 = if range.end > re {
                // past the row's end: to its end (and a space's width for the line feed)
                r.pos.x + r.row.size.x + if r.ends_with_newline { 4.0 } else { 0.0 }
            } else {
                let mut e = CCursor::new(hi);
                e.prefer_next_row = false;
                g.pos_from_cursor(e).min.x
            };
            let y0 = r.pos.y;
            out.push(Rect::from_min_max(pos2(x0, y0), pos2(x1.max(x0), y0 + r.row.size.y)));
        }
        let _ = k;
        c = re + usize::from(r.ends_with_newline);
    }
    out
}

/// OW4: a text field's hint (its default accessible name), if `x` is one.
pub(crate) fn hint_of(x: &crate::provider::ExtWidget) -> Option<String> {
    x.with(|t: &TextEdit| t.hint().to_string()).filter(|h| !h.is_empty())
}

/// OW4: the selection of text field `x` as char offsets `[anchor, caret]` (`None`: not a text
/// field, or no session yet).
pub(crate) fn selection_of(x: &crate::provider::ExtWidget) -> Option<[usize; 2]> {
    x.with(|t: &TextEdit| t.selection()).flatten()
}

/// OW4: sets the selection of text field `x` (char offsets), creating its session.
pub(crate) fn set_selection_of(x: &crate::provider::ExtWidget, ctx: &egui::Context, anchor: usize, caret: usize) -> bool {
    let policy = crate::native::policy_of(ctx);
    x.with_mut(|t: &mut TextEdit| {
        let p = t.text_params();
        let text = t.text.clone();
        t.input.sync(&text, &policy, &p, true);
        t.input.set_selection_chars(anchor, caret);
    })
    .is_some()
}

/// OW4: where a click on char `at` of text field `x` lands (`frac` into it), as last drawn.
pub(crate) fn char_point_of(x: &crate::provider::ExtWidget, at: usize, frac: f32) -> Option<Pos2> {
    x.with(|t: &TextEdit| t.char_point(at, frac)).flatten()
}

impl crate::EguiBackend {
    fn with_field<R>(&self, h: u64, f: impl FnOnce(&crate::provider::ExtWidget) -> Option<R>) -> Option<R> {
        let scene = self.0.scene.borrow();
        let i = scene.ix(scene.live(h).ok()?);
        match scene.w(i) {
            crate::scene::W::Ext(x) if x.entry().replaces == Some("text-edit") => f(x),
            _ => None,
        }
    }

    /// OW4: the selection of text field `h` as char offsets `[anchor, caret]` (`None`: not a
    /// text field, or not focused, pressed or edited yet).
    pub fn text_selection(&self, h: u64) -> Option<[usize; 2]> {
        self.with_field(h, selection_of)
    }

    /// OW4: sets the selection of text field `h` (char offsets; view state, no undo step), as a
    /// test or an automation driver sets up a field. False when `h` is not a text field.
    pub fn set_text_selection(&self, h: u64, anchor: usize, caret: usize) -> bool {
        let Some(ctx) = self.ctx() else { return false };
        let ok = self.with_field(h, |x| set_selection_of(x, &ctx, anchor, caret).then_some(())).is_some();
        ctx.request_repaint();
        ok
    }

    /// OW4: where a click on char `at` of text field `h` lands, `frac` of the way into it, as last
    /// drawn (screen points).
    pub fn text_char_point(&self, h: u64, at: usize, frac: f32) -> Option<Pos2> {
        self.with_field(h, |x| char_point_of(x, at, frac))
    }
}
