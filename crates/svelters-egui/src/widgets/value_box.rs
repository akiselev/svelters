//! OW1b: the number field shared by `drag-value` (the whole widget) and the value boxes of
//! `slider` and `range-slider` (their `value`/`low-value`/`high-value` parts). A composition of
//! units on one rect: Hoverable, Focusable, DragToValue (relative scrub), Steppable (keys, wheel,
//! AccessKit), and the interim [`Entry`] for typing; optional step buttons (`dec`, `inc`) with
//! Pressable auto-repeat (Qt `accelerated`).
//!
//! Signals follow ARCH-WIDGETS §2.4 and FM1 QW7's commit seam: [`BoxOut::input`] for every user
//! change of the value, [`BoxOut::commit`] at a commit point (drag released, Enter or focus lost
//! after typing, every step) when the value differs from where that interaction began, and
//! [`BoxOut::submit`] on Enter. In commit mode (`commit`, Qt `keyboardTracking=false`) typing and
//! scrubbing keep a draft that is painted, and `input` fires only at the commit.

use super::entry::Entry;
use super::numfmt::NumFormat;
use crate::interact::{self, DragToValue, Focusable, FocusParams, Hoverable, PartResponses, Press, PressParams, Pressable, Scrub, ScrubMode, ScrubParams, Step, StepParams, Steppable, Unit};
use crate::paint::{self, Align, Ua, UaRule, UaValue::*};
use crate::provider::StateBits;
use egui::{Rect, Response, Sense, accesskit};

/// A field's UA sheet (DESIGN R4: sunken, always visibly a field): `input-*` tokens, 1px
/// boundary, the 1px `focus` outline at −1 while focused (R8, rows and fields), `invalid-border`.
pub const FIELD_UA: Ua = Ua(&[
    UaRule::base(&[
        ("background-color", Token("input-bg")),
        ("color", Token("input-fg")),
        ("border-color", Token("input-border-color")),
        ("border-width", Token("stroke")),
        ("border-radius", Token("input-radius")),
        ("padding-left", Px(6.0)),
        ("padding-right", Px(6.0)),
        ("padding-top", Px(3.0)),
        ("padding-bottom", Px(3.0)),
        ("outline-style", Keyword("none")),
        ("outline-color", Token("focus")),
        ("outline-width", Px(1.0)),
        ("outline-offset", Px(-1.0)),
    ]),
    UaRule::when(StateBits::FOCUSED, &[("outline-style", Keyword("solid"))]),
    UaRule::when(StateBits::INVALID, &[("border-color", Token("invalid-border"))]),
    UaRule::when(StateBits::DISABLED, &[("color", Token("disabled-fg"))]),
    // R5: read-only looks like text
    UaRule::when(StateBits::READ_ONLY, &[("background-color", Color(egui::Color32::TRANSPARENT)), ("border-color", Color(egui::Color32::TRANSPARENT))]),
]);

/// The step buttons' UA sheet (`::part(inc)`, `::part(dec)`): flat glyph buttons inside the
/// field (Fluent NumberBox inline spin buttons), hover and press fills from the toolbar tokens;
/// `:disabled` at a limit without `wrap`.
pub const STEP_UA: Ua = Ua(&[
    UaRule::base(&[("color", Token("fg")), ("border-radius", Px(2.0)), ("font-size", Px(14.0))]),
    UaRule::when(StateBits::HOVERED, &[("background-color", Token("toolbar-hover"))]),
    UaRule::when(StateBits::PRESSED, &[("background-color", Token("toolbar-active"))]),
    UaRule::when(StateBits::DISABLED, &[("color", Token("disabled-fg")), ("background-color", Color(egui::Color32::TRANSPARENT))]),
]);

/// First repeat after this long, then every interval (Qt's spin box: 500 ms, then 100 ms; with
/// `accelerated` it speeds up).
pub const REPEAT: (f32, f32) = (0.5, 0.1);

/// What an out-of-range typed value commits to (Qt `correctionMode`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Correction {
    /// The nearest bound (Qt CorrectToNearestValue).
    #[default]
    Nearest,
    /// The value before the edit (Qt CorrectToPreviousValue).
    Previous,
}

impl Correction {
    pub fn parse(s: &str) -> Correction {
        if s == "previous" { Correction::Previous } else { Correction::Nearest }
    }
}

/// One pass's configuration of a [`ValueBox`].
#[derive(Clone, Copy, Debug)]
pub struct BoxCfg<'a> {
    pub id: egui::Id,
    /// The whole box, step buttons included.
    pub rect: Rect,
    /// The prop's value (NaN shows as `min` or 0).
    pub value: f64,
    pub step: StepParams,
    /// Scrub speed, value per point.
    pub speed: f64,
    pub fmt: NumFormat<'a>,
    pub commit_mode: bool,
    pub read_only: bool,
    pub enabled: bool,
    pub buttons: bool,
    pub accelerated: bool,
    pub correction: Correction,
    pub invalid: bool,
    /// The host's computed style for the field (the `drag-value` node's), if any.
    pub css: Option<&'a svelters_style::ComputedStyle>,
    pub dark: bool,
    /// Trace target (the node).
    pub node: u64,
}

/// What a pass of the box did.
#[derive(Debug)]
pub struct BoxOut {
    pub resp: Response,
    pub input: Option<f64>,
    pub commit: Option<f64>,
    pub submit: Option<f64>,
    pub drag_started: bool,
    pub drag_stopped: bool,
    /// The field's state bits this pass (hover, focus, press, drag, read-only, …).
    pub state: StateBits,
    /// The next auto-repeat of a held step button.
    pub repaint_after: Option<std::time::Duration>,
}

/// The number field: units, entry and draft; lives in the widget between frames.
#[derive(Debug, Default)]
pub struct ValueBox {
    hover: Hoverable,
    focus: Focusable,
    scrub: DragToValue,
    step: Steppable,
    inc: Pressable,
    dec: Pressable,
    pub entry: Entry,
    /// Commit mode: the draft shown while scrubbing or typing.
    draft: Option<f64>,
    /// The value when the drag or the edit began (commit compares with it).
    from: f64,
    bits: StateBits,
    btn_bits: [StateBits; 2],
    /// A step button stepped on its press (a click in one frame steps on the click instead).
    stepped: [bool; 2],
    /// For `a11y`: the value text and the step params of the last pass.
    shown: String,
    params: StepParams,
}

impl ValueBox {
    /// The value painted: the draft, else the prop.
    pub fn shown_value(&self, cfg: &BoxCfg) -> f64 {
        self.draft.unwrap_or(if cfg.value.is_nan() { if cfg.step.min.is_finite() { cfg.step.min } else { 0.0 } } else { cfg.value })
    }

    /// Being typed into.
    pub fn editing(&self) -> bool {
        self.entry.editing
    }

    pub fn has_draft(&self) -> bool {
        self.draft.is_some()
    }

    /// The width of `text` in the field's font (for `measure`).
    pub fn text_width(ctx: &egui::Context, css: Option<&svelters_style::ComputedStyle>, text: &str, dark: bool) -> egui::Vec2 {
        let s = paint::StyleView::new(css, FIELD_UA, StateBits::NONE, dark);
        paint::measure_text(ctx, &s, text, crate::provider::AvailableSpace::MaxContent) + paint::box_extra(&s)
    }

    fn parse(&self, cfg: &BoxCfg, text: &str) -> Option<f64> {
        let v = cfg.fmt.parse(text)?;
        let p = &cfg.step;
        let out = (!p.min.is_nan() && v < p.min) || (!p.max.is_nan() && v > p.max);
        if out && cfg.correction == Correction::Previous {
            return None;
        }
        Some(p.clamp(p.round(v)))
    }

    /// The prop takes `v` (sent as `input` when it differs, or when a draft is dropped).
    fn write(&mut self, cfg: &BoxCfg, v: f64, input: &mut Option<f64>) {
        if v != cfg.value || self.draft.is_some() {
            *input = Some(v);
        }
        self.draft = None;
    }

    /// Runs and paints the box in `ui`.
    pub fn ui(&mut self, ui: &mut egui::Ui, cfg: &BoxCfg) -> BoxOut {
        // an external write that differs from where the draft began ends it (§2.4)
        if self.draft.is_some() && cfg.value != self.from && !cfg.value.is_nan() {
            self.draft = None;
        }
        let side = cfg.rect.height();
        let nb = if cfg.buttons { 2.0 } else { 0.0 };
        let field = Rect::from_min_max(cfg.rect.min, egui::pos2((cfg.rect.max.x - nb * side).max(cfg.rect.min.x), cfg.rect.max.y));
        let editable = cfg.enabled && !cfg.read_only;
        let resp = ui.interact(field, cfg.id, Sense::click_and_drag() | Sense::FOCUSABLE);
        let cur = self.shown_value(cfg);
        let mut p = cfg.step;
        p.value = cur;
        p.active = editable;
        let sp = ScrubParams {
            value: cur,
            min: p.min,
            max: p.max,
            step: 0.0,
            mode: ScrubMode::Relative { speed: if cfg.speed.is_finite() && cfg.speed != 0.0 { cfg.speed } else { 1.0 } },
            active: editable && !self.entry.editing,
            decimals: Some(cfg.fmt.decimals.1.max(cfg.fmt.decimals.0)),
        };
        let fp = FocusParams { tab_stop: true, focus_on_click: Some(false) };
        let mut hov = interact::Intents::default();
        let mut foc = interact::Intents::default();
        let mut scr = interact::Intents::default();
        let mut stp = interact::Intents::default();
        let mut events = Vec::new();
        let mut bits = self.bits;
        let (hover, focus, scrub, step) = (&mut self.hover, &mut self.focus, &mut self.scrub, &mut self.step);
        let fc = interact::run(ui, &resp, &mut bits, &PartResponses::default(), |icx| {
            hover.input(&(), icx, &mut hov);
            focus.input(&fp, icx, &mut foc);
            scrub.input(&sp, icx, &mut scr);
            step.input(&p, icx, &mut stp);
            if icx.resp.has_focus {
                events = icx.input.events.clone();
            }
        });
        let focused = fc.has && cfg.enabled;
        self.bits = bits;
        // a click (not a drag) edits the number; keyboard focus does too (Qt, egui)
        if resp.clicked() && cfg.enabled && !focused {
            resp.request_focus();
        }
        let mut input: Option<f64> = None;
        let mut commit: Option<f64> = None;
        let mut submit: Option<f64> = None;
        let (mut drag_started, mut drag_stopped) = (false, false);
        // focus: editing begins when it is gained, and a changed text is applied when it is lost
        if focused && !self.entry.editing && !cfg.read_only {
            self.entry.begin(cfg.fmt.number(cur));
            self.from = cur;
        }
        if !focused && self.entry.editing {
            if self.entry.dirty
                && let Some(v) = self.parse(cfg, &self.entry.text.clone())
            {
                self.write(cfg, v, &mut input);
                if v != self.from {
                    commit = Some(v);
                }
            }
            self.entry.end();
        }
        let mut val = cur;
        // typing
        if self.entry.editing && focused && editable {
            let o = self.entry.input(ui.ctx(), &events);
            if o.escape {
                // restore the text and the value from before the edit
                self.entry.reset(cfg.fmt.number(self.from));
                self.entry.dirty = false;
                if self.draft.take().is_some() || cfg.value != self.from {
                    input = Some(self.from);
                }
                val = self.from;
            } else if o.edited && !cfg.commit_mode {
                if let Some(v) = self.parse(cfg, &self.entry.text.clone())
                    && v != cfg.value
                {
                    input = Some(v);
                    val = v;
                }
            }
            if o.enter {
                let v = if self.entry.dirty { self.parse(cfg, &self.entry.text.clone()) } else { None };
                let v = v.unwrap_or(val);
                self.write(cfg, v, &mut input);
                if v != self.from {
                    commit = Some(v);
                }
                submit = Some(v);
                self.from = v;
                self.entry.reset(cfg.fmt.number(v));
                self.entry.dirty = false;
                val = v;
            } else if self.entry.dirty && !stp.is_empty() {
                // a step while typing steps from the typed value (Qt)
                if let Some(v) = self.parse(cfg, &self.entry.text.clone()) {
                    val = v;
                }
            }
        }
        // steps: keys, wheel, AccessKit; each is a commit point (FM1)
        let mut stepped = false;
        for s in stp.iter() {
            let v = if let Step::Set(x) = s { p.clamp(*x) } else { p.apply(val, *s) };
            if v.is_finite() && (v != val || v != cfg.value) {
                self.write(cfg, v, &mut input);
                if v != self.from || v != val {
                    commit = Some(v);
                }
                val = v;
                stepped = true;
            }
        }
        // step buttons (not tab stops; Pressable repeat, `accelerated`)
        if cfg.buttons {
            for (k, up) in [(0usize, false), (1usize, true)] {
                let r = Self::button_rect(cfg, k);
                let br = ui.interact(r, cfg.id.with(if up { "inc" } else { "dec" }), Sense::CLICK);
                let can = editable && p.can_step(val, up);
                let pp = PressParams { repeat: Some(REPEAT), accelerate: cfg.accelerated, ..PressParams::default() };
                let mut pi = interact::Intents::default();
                let mut b = self.btn_bits[k];
                let unit = if up { &mut self.inc } else { &mut self.dec };
                interact::run(ui, &br, &mut b, &PartResponses::default(), |icx| {
                    icx.enabled = can;
                    unit.input(&pp, icx, &mut pi);
                });
                let mut n = 0;
                for i in pi.iter() {
                    match i {
                        Press::Down => {
                            n += 1;
                            self.stepped[k] = true;
                        }
                        Press::Repeat => n += 1,
                        Press::Click { count: 1, button: egui::PointerButton::Primary, .. } if !self.stepped[k] => n += 1,
                        _ => {}
                    }
                }
                if !unit.is_down() {
                    self.stepped[k] = false;
                }
                if let Some(d) = unit.next_frame() {
                    ui.ctx().request_repaint_after(d);
                }
                b.set(StateBits::DISABLED, !can);
                b.set(StateBits::HOVERED, br.contains_pointer());
                self.btn_bits[k] = b;
                if n > 0 && can {
                    let v = p.apply(val, Step::By { n: if up { n } else { -n }, page: false });
                    if v != val {
                        self.write(cfg, v, &mut input);
                        commit = Some(v);
                        val = v;
                        stepped = true;
                    }
                }
                if br.hovered() && can {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
            }
        }
        // scrub: live writes each value, commit mode keeps a draft until the release
        for s in scr.iter() {
            match *s {
                Scrub::Start => {
                    drag_started = true;
                    self.from = val;
                }
                Scrub::Scrub(v) => {
                    if cfg.commit_mode {
                        self.draft = Some(v);
                    } else {
                        input = Some(v);
                    }
                    val = v;
                }
                Scrub::Commit(v) => {
                    if cfg.commit_mode {
                        self.draft = None;
                        input = Some(v);
                    }
                    commit = Some(v);
                    self.from = v;
                }
                Scrub::Cancel { restore } => {
                    if self.draft.take().is_none() {
                        input = Some(restore);
                    }
                    val = restore;
                }
                Scrub::End => drag_stopped = true,
            }
        }
        if self.entry.editing {
            if stepped {
                self.entry.reset(cfg.fmt.number(val));
                self.entry.dirty = false;
                self.from = val;
            } else if !self.entry.dirty && input.is_none() && cfg.value != self.from && !cfg.value.is_nan() {
                // an external write while the text is untouched
                self.entry.reset(cfg.fmt.number(cfg.value));
                self.from = cfg.value;
            }
        } else if let Some(c) = commit {
            self.from = c;
        } else if !self.scrub.dragging() {
            self.from = cur;
        }
        if self.step.took_wheel() {
            ui.ctx().input_mut(|i| {
                i.smooth_scroll_delta = egui::Vec2::ZERO;
            });
        }
        if focused {
            ui.memory_mut(|m| m.set_focus_lock_filter(cfg.id, egui::EventFilter { horizontal_arrows: true, vertical_arrows: true, tab: false, escape: true }));
        }
        let mut bits = self.bits;
        bits.set(StateBits::READ_ONLY, cfg.read_only);
        bits.set(StateBits::INVALID, cfg.invalid);
        bits.set(StateBits::DISABLED, !cfg.enabled);
        self.bits = bits;
        // R10: what a press does
        if resp.hovered() && editable {
            ui.ctx().set_cursor_icon(if self.entry.editing { egui::CursorIcon::Text } else { egui::CursorIcon::ResizeHorizontal });
        }
        let shown = self.draft.unwrap_or(match input {
            Some(v) => v,
            None => cur,
        });
        let shown = if commit.is_none() && self.draft.is_none() && input.is_none() { cur } else { shown };
        self.paint(ui, cfg, field, shown, focused);
        self.params = StepParams { value: shown, ..p };
        self.shown = cfg.fmt.number(shown);
        if self.entry.editing && focused {
            let s = paint::StyleView::new(cfg.css, FIELD_UA, bits, cfg.dark);
            let g = paint::box_geom(&s, field);
            let caret = self.entry.caret_rect(ui.ctx(), &s, g.content);
            ui.ctx().output_mut(|o| o.ime = Some(egui::output::IMEOutput { purpose: egui::IMEPurpose::Normal, rect: field, cursor_rect: caret, should_interrupt_composition: false }));
        }
        BoxOut { resp, input, commit, submit, drag_started, drag_stopped, state: bits, repaint_after: None }
    }

    fn button_rect(cfg: &BoxCfg, k: usize) -> Rect {
        let side = cfg.rect.height();
        Rect::from_min_size(egui::pos2(cfg.rect.max.x - (2 - k) as f32 * side, cfg.rect.min.y), egui::vec2(side, side))
    }

    fn paint(&self, ui: &egui::Ui, cfg: &BoxCfg, field: Rect, v: f64, focused: bool) {
        if !ui.is_rect_visible(cfg.rect) {
            return;
        }
        let s = paint::StyleView::new(cfg.css, FIELD_UA, self.bits, cfg.dark);
        let mut pc = paint::PaintCx::for_node(ui.painter(), cfg.node);
        // the field's box spans the buttons too (they sit inside it)
        let g = paint::paint_box(&mut pc, &s, cfg.rect);
        let content = if cfg.buttons { Rect::from_min_max(g.content.min, egui::pos2(field.max.x.max(g.content.min.x), g.content.max.y)) } else { g.content };
        let align = Align::of(&s, Align::Start);
        if self.entry.editing && focused {
            self.entry.paint(&mut pc.part("text"), &s, content, align, true);
        } else {
            let text = cfg.fmt.display(v);
            let gal = paint::text_galley(ui.ctx(), &s, &text, None);
            let clip = pc.painter.clip_rect().intersect(content.expand(1.0));
            let painter = pc.painter.with_clip_rect(clip);
            let mut tc = paint::PaintCx::for_node(&painter, cfg.node);
            paint::paint_text(&mut tc.part("text"), &s, &gal, content, align);
        }
        if cfg.buttons {
            let side = cfg.rect.height();
            for (k, up) in [(0usize, false), (1usize, true)] {
                let r = Rect::from_min_size(egui::pos2(cfg.rect.max.x - (2 - k) as f32 * side, cfg.rect.min.y), egui::vec2(side, side)).shrink(3.0);
                let bs = paint::StyleView::ua_only(STEP_UA, self.btn_bits[k], cfg.dark);
                let name = if up { "inc" } else { "dec" };
                let mut bc = pc.part(name);
                paint::paint_box(&mut bc, &bs, r);
                glyph(&mut bc, &bs, if up { "add" } else { "remove" }, if up { "+" } else { "\u{2212}" }, r);
            }
        }
        paint::paint_outline(&mut pc, &s, cfg.rect);
    }

    /// The field's AccessKit node (role `SpinButton`): the value as text (what is shown, so
    /// VoiceOver reads it in every mode, as egui), the numeric value, bounds, step and actions.
    pub fn a11y(&self, node: &mut accesskit::Node, label: Option<&str>) {
        node.set_role(accesskit::Role::SpinButton);
        node.set_value(self.shown.clone());
        self.step.a11y(&self.params, node);
        if self.bits.contains(StateBits::READ_ONLY) {
            node.set_read_only();
        }
        if let Some(l) = label {
            node.set_label(l.to_string());
        }
    }

    /// EV3 R11: while focused, the keys the box takes itself.
    pub fn claims(&self, key: egui::Key, m: egui::Modifiers) -> bool {
        if self.scrub.claims(&ScrubParams::default(), key, m) {
            return true;
        }
        if self.entry.editing {
            // a one-line field's keys, plus Enter (apply) and Escape (restore)
            return matches!(key, egui::Key::Enter | egui::Key::Escape) && !m.command && !m.alt
                || self.step.claims(&self.params, key, m)
                || crate::focus::claims(crate::focus::Role::Text { multiline: false }, key, m, cfg!(target_os = "macos"));
        }
        self.step.claims(&self.params, key, m)
    }
}

/// A glyph from the icon registry (codicon `name`), or the `fallback` character when no icon
/// font is registered (the text primitive draws it; nothing is hand-drawn).
pub fn glyph(cx: &mut paint::PaintCx, s: &paint::StyleView, name: &str, fallback: &str, rect: Rect) {
    if crate::fonts::icon_text("codicon", name, "").is_some() {
        paint::paint_icon(cx, s, &paint::IconKey::new("codicon", name), rect);
    } else {
        let ctx = cx.painter.ctx().clone();
        let g = paint::text_galley(&ctx, s, fallback, None);
        paint::paint_text(cx, s, &g, rect, Align::Center);
    }
}
