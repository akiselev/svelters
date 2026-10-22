//! `slider`, svelters' own (OW1b, ARCH-WIDGETS §6.1; Qt QSlider parity, WIDGET-PARITY §2.7): a
//! rail with a thumb (DragToValue, absolute: a press on the track jumps there, or pages toward the
//! pointer with `track-click="page"`; a press on the thumb grabs it), Steppable keys (arrows,
//! PageUp/PageDown, Home/End), the wheel, and an optional value box (a [`ValueBox`]) and caption.
//!
//! Signals (§2.4): `changed`/`input` per change (the draft while dragging in commit mode is not
//! sent), `commit` at a commit point (release, a step, Enter in the value box), `submit` (Enter in
//! the value box), `drag-started`/`drag-stopped`. Parts (paint parts until OW-CSS; ARCH lists
//! `track` and `thumb` as node parts, see the milestone): `track`, `fill`, `rest`, `thumb`,
//! `tick`, `mark-label`, `value`.

use super::numfmt::{NumFormat, auto_decimals, decimals_of};
use super::rail::{self, GAP, Mark, Rail, Ticks};
use super::value_box::{BoxCfg, Correction, ValueBox};
use crate::interact::{Arrows, DragToValue, Focusable, Hoverable, Scale, Scrub, ScrubMode, ScrubParams, Step, StepParams, Steppable, Wheel};
use crate::paint::{self, Ua, UaRule};
use crate::provider::{A11yCx, AvailableSpace, Event, MeasureCx, Part, StateBits, Widget, WidgetCx};

/// The host's UA sheet: only the caption's text (the parts have their own, `rail.rs`).
pub const UA: Ua = Ua(&[UaRule::base(&[])]);

/// The default length of the rail (egui's `slider_width`).
pub const RAIL_LEN: f32 = 100.0;

/// A slider.
#[svelters::widget(namespace = "sv", replaces = "slider", caps = FOCUSABLE | EDITABLE | PAINTS_BOX)]
pub struct Slider {
    /// The value; dragging, the keys, the wheel or the value box change it.
    #[prop(bind)]
    value: f64,
    #[prop(default = 0.0, measure)]
    min: f64,
    #[prop(default = 100.0, measure)]
    max: f64,
    /// Value granularity; 0 for continuous.
    #[prop(measure)]
    step: f64,
    /// Caption after the slider (and its accessible name).
    #[prop(measure)]
    text: String,
    /// Shows the value box.
    #[prop(default = true, measure)]
    show_value: bool,
    /// PageUp/PageDown and `track-click="page"` move this much (Qt `pageStep`); 0 for ten steps.
    #[prop]
    page_step: f64,
    /// `horizontal` (default) or `vertical` (max at the top).
    #[prop(default = "horizontal".to_string(), measure, enumeration = "horizontal|vertical")]
    orientation: String,
    /// Tick marks: `none` (default), `before` (above or left), `after`, `both` (Qt `tickPosition`).
    #[prop(default = "none".to_string(), measure, enumeration = "none|before|after|both")]
    ticks: String,
    /// Between ticks; 0 for `page-step` (Qt `tickInterval`).
    #[prop]
    tick_interval: f64,
    /// Marks on the rail, drawn as ticks (with labels) instead of the `tick-interval` ticks:
    /// values separated by commas, each optionally `value:label` (`"0:Low, 50, 100:High"`).
    #[prop(measure)]
    marks: String,
    /// Dragging snaps to the nearest mark.
    #[prop]
    snap_to_marks: bool,
    /// Max at the start (Qt `invertedAppearance`).
    #[prop]
    inverted: bool,
    /// A press on the track off the thumb: `jump` there (default) or `page` toward it.
    #[prop(default = "jump".to_string(), enumeration = "jump|page")]
    track_click: String,
    /// `linear` (default) or `log` (both ends > 0).
    #[prop(default = "linear".to_string(), enumeration = "linear|log")]
    scale: String,
    /// Commit-only mode (Qt `tracking=false`): `value` moves at the release, not while dragging.
    #[prop(rename = "commit")]
    commit_mode: bool,
    /// When the wheel steps: `focused` (default), `hover`, `off`.
    #[prop(default = "focused".to_string(), enumeration = "focused|hover|off")]
    wheel: String,
    #[event]
    changed: Event<f64>,
    #[event]
    input: Event<f64>,
    #[event]
    commit: Event<f64>,
    #[event]
    submit: Event<f64>,
    #[event]
    drag_started: Event<()>,
    #[event]
    drag_stopped: Event<()>,
    #[part]
    track: Part,
    #[part]
    fill: Part,
    #[part]
    rest: Part,
    #[part]
    thumb: Part,
    #[part]
    tick: Part,
    #[part]
    mark_label: Part,
    #[part]
    value_box: Part,
    #[behavior]
    hover: Hoverable,
    #[behavior]
    focus: Focusable,
    #[behavior(params = scrub_params)]
    scrub: DragToValue,
    #[behavior(params = step_params)]
    stepper: Steppable,
    #[state]
    st: SliderState,
}

/// What the slider keeps between frames.
#[derive(Debug, Default)]
pub struct SliderState {
    rail: Rail,
    field: ValueBox,
    /// Commit mode: the value while dragging.
    draft: Option<f64>,
    /// `track-click="page"`: this press pages instead of dragging.
    paging: bool,
    field_focused: bool,
    shown: f64,
}

impl Slider {
    fn bounds(&self) -> (f64, f64) {
        let a = if self.min.is_finite() { self.min } else { 0.0 };
        let b = if self.max.is_finite() { self.max } else { a + 100.0 };
        (a.min(b), a.max(b))
    }

    fn current(&self) -> f64 {
        let (lo, _) = self.bounds();
        self.st.draft.unwrap_or(if self.value.is_nan() { lo } else { self.value })
    }

    fn vertical(&self) -> bool {
        self.orientation == "vertical"
    }

    fn marks(&self) -> Vec<Mark> {
        rail::marks_of(&self.marks)
    }

    fn scrub_params(&self) -> ScrubParams {
        let (lo, hi) = self.bounds();
        let r = &self.st.rail;
        ScrubParams {
            value: self.current(),
            min: lo,
            max: hi,
            step: self.step,
            mode: ScrubMode::Absolute { track: r.travel(), thumb: r.thumb(self.current()), vertical: r.vertical, inverted: r.inverted, scale: r.scale },
            active: !self.st.paging,
            decimals: None,
        }
    }

    fn step_params(&self) -> StepParams {
        let (lo, hi) = self.bounds();
        StepParams {
            value: self.current(),
            min: lo,
            max: hi,
            step: self.step,
            page: self.page_step,
            wheel: Wheel::parse(&self.wheel),
            arrows: Arrows::Both,
            ..StepParams::default()
        }
    }

    fn fmt(&self, rail_len: f32) -> NumFormat<'static> {
        let (lo, hi) = self.bounds();
        let d = if self.step > 0.0 {
            let d = decimals_of(self.step);
            (d, d)
        } else {
            let a = auto_decimals((hi - lo) / rail_len.max(1.0) as f64);
            (a, a + 2)
        };
        NumFormat { decimals: d, ..NumFormat::default() }
    }

    /// The value box's size for the bounds' texts.
    fn box_size(&self, ctx: &egui::Context, dark: bool, rail_len: f32) -> egui::Vec2 {
        let (lo, hi) = self.bounds();
        let f = self.fmt(rail_len);
        let a = ValueBox::text_width(ctx, None, &f.display(lo), dark).max(ValueBox::text_width(ctx, None, &f.display(hi), dark));
        egui::vec2(a.x.max(40.0), a.y.max(24.0))
    }

    fn snap_mark(&self, v: f64) -> f64 {
        if !self.snap_to_marks {
            return v;
        }
        self.marks().into_iter().map(|m| m.value).min_by(|a, b| (a - v).abs().total_cmp(&(b - v).abs())).unwrap_or(v)
    }
}

impl Widget for Slider {
    fn measure(&mut self, cx: &mut MeasureCx<'_>, _avail: AvailableSpace) -> egui::Vec2 {
        let dark = cx.ctx.theme() == egui::Theme::Dark;
        let s = cx.style(UA);
        let cap = if self.text.is_empty() { egui::Vec2::ZERO } else { paint::measure_text(cx.ctx, &s, &self.text, AvailableSpace::MaxContent) };
        let bx = if self.show_value { self.box_size(cx.ctx, dark, RAIL_LEN) } else { egui::Vec2::ZERO };
        let ticks = Ticks::parse(&self.ticks).extra(self.marks().iter().any(|m| !m.label.is_empty()));
        let across = rail::THUMB + 4.0 + ticks;
        if self.vertical() {
            let w = across.max(bx.x).max(cap.x);
            let mut h = RAIL_LEN + rail::THUMB;
            for e in [bx.y, cap.y] {
                if e > 0.0 {
                    h += GAP + e;
                }
            }
            egui::vec2(w, h)
        } else {
            let mut w = RAIL_LEN + rail::THUMB;
            for e in [bx.x, cap.x] {
                if e > 0.0 {
                    w += GAP + e;
                }
            }
            egui::vec2(w, across.max(bx.y).max(cap.y))
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx<'_>) -> egui::Response {
        let dark = ui.ctx().theme() == egui::Theme::Dark;
        let (lo, hi) = self.bounds();
        // a value written outside the range is clamped when drawn (announced, no event)
        if self.value.is_finite() && (self.value < lo || self.value > hi) {
            let v = self.value.clamp(lo, hi);
            cx.set(&mut self.value, v);
        }
        // layout: rail, value box, caption
        let rect = cx.rect();
        let cap = if self.text.is_empty() { egui::Vec2::ZERO } else { paint::measure_text(ui.ctx(), &cx.style(UA), &self.text, AvailableSpace::MaxContent) };
        let vertical = self.vertical();
        let mut bx = if self.show_value { self.box_size(ui.ctx(), dark, RAIL_LEN) } else { egui::Vec2::ZERO };
        let (rail_rect, box_rect, cap_rect) = if vertical {
            let cap_h = if cap.y > 0.0 { cap.y + GAP } else { 0.0 };
            let box_h = if bx.y > 0.0 { bx.y + GAP } else { 0.0 };
            let rail_r = egui::Rect::from_min_max(rect.min, egui::pos2(rect.max.x, (rect.max.y - cap_h - box_h).max(rect.min.y + rail::THUMB)));
            let b = egui::Rect::from_min_size(egui::pos2(rect.center().x - bx.x / 2.0, rail_r.max.y + GAP), bx);
            let c = egui::Rect::from_min_size(egui::pos2(rect.center().x - cap.x / 2.0, b.max.y.max(rail_r.max.y) + GAP), cap);
            (rail_r, b, c)
        } else {
            let cap_w = if cap.x > 0.0 { cap.x + GAP } else { 0.0 };
            let box_w = if bx.x > 0.0 { bx.x + GAP } else { 0.0 };
            let rail_r = egui::Rect::from_min_max(rect.min, egui::pos2((rect.max.x - cap_w - box_w).max(rect.min.x + rail::THUMB), rect.max.y));
            bx.y = bx.y.min(rect.height());
            let b = egui::Rect::from_min_size(egui::pos2(rail_r.max.x + GAP, rect.center().y - bx.y / 2.0), bx);
            let c = egui::Rect::from_min_size(egui::pos2(rect.max.x - cap.x, rect.center().y - cap.y / 2.0), cap);
            (rail_r, b, c)
        };
        let marks = self.marks();
        self.st.rail = Rail {
            rect: rail_rect,
            vertical,
            inverted: self.inverted,
            scale: Scale { min: self.min, max: self.max, log: self.scale == "log" },
            ticks: Ticks::parse(&self.ticks),
            labels: marks.iter().any(|m| !m.label.is_empty()),
        };
        if !(self.min.is_finite() && self.max.is_finite()) {
            self.st.rail.scale = Scale { min: lo, max: hi, log: false };
        }
        let id = cx.id();
        let resp = ui.interact(rail_rect, id, self.units_sense());
        // `track-click="page"`: a press off the thumb pages toward the pointer
        let pressed_now = ui.input(|i| i.pointer.primary_pressed()) || resp.clicked();
        let page_dir = if self.track_click == "page" && pressed_now && !self.st.paging {
            resp.interact_pointer_pos().filter(|p| !self.st.rail.thumb(self.current()).contains(*p)).map(|p| {
                let c = self.st.rail.at(self.current());
                let after = if vertical { p.y < c.y } else { p.x > c.x };
                if after != self.inverted { 1 } else { -1 }
            })
        } else {
            None
        };
        if page_dir.is_some() {
            self.st.paging = true;
        }
        let fired = self.run_units(cx, ui, &resp);
        if self.st.paging && !resp.is_pointer_button_down_on() {
            self.st.paging = false;
        }
        let sp = self.step_params();
        let mut val = self.current();
        let mut input: Option<f64> = None;
        let mut commit: Option<f64> = None;
        let mut submit: Option<f64> = None;
        let (mut started, mut stopped) = (false, false);
        if let Some(n) = page_dir {
            let v = sp.apply(val, Step::By { n, page: true });
            if v != val {
                input = Some(v);
                commit = Some(v);
                val = v;
            }
        }
        for s in fired.scrub.iter() {
            match *s {
                Scrub::Start => started = true,
                Scrub::Scrub(v) => {
                    let v = self.snap_mark(v);
                    if self.commit_mode {
                        self.st.draft = Some(v);
                    } else {
                        input = Some(v);
                    }
                    val = v;
                }
                Scrub::Commit(v) => {
                    let v = self.snap_mark(v);
                    if self.commit_mode {
                        self.st.draft = None;
                        input = Some(v);
                    }
                    commit = Some(v);
                }
                Scrub::Cancel { restore } => {
                    if self.st.draft.take().is_none() {
                        input = Some(restore);
                    }
                    val = restore;
                }
                Scrub::End => stopped = true,
            }
        }
        for s in fired.stepper.iter() {
            let v = if let Step::Set(x) = s { sp.clamp(*x) } else { sp.apply(val, *s) };
            if v.is_finite() && v != val {
                self.st.draft = None;
                input = Some(v);
                commit = Some(v);
                val = v;
            }
        }
        if self.stepper.took_wheel() {
            ui.ctx().input_mut(|i| {
                i.smooth_scroll_delta = egui::Vec2::ZERO;
            });
        }
        if resp.has_focus() {
            ui.memory_mut(|m| m.set_focus_lock_filter(id, egui::EventFilter { horizontal_arrows: true, vertical_arrows: true, tab: false, escape: self.scrub.dragging() }));
        }
        // the value box
        let mut field_resp = None;
        if self.show_value {
            let shown = self.st.draft.unwrap_or(input.unwrap_or(self.current()));
            let track_len = if vertical { self.st.rail.travel().height() } else { self.st.rail.travel().width() };
            let mut field = std::mem::take(&mut self.st.field);
            let out = {
                let cfg = BoxCfg {
                    id: id.with("value"),
                    rect: box_rect,
                    value: shown,
                    step: StepParams { arrows: Arrows::Vertical, ..sp },
                    speed: (hi - lo) / track_len.max(1.0) as f64,
                    fmt: self.fmt(track_len),
                    commit_mode: self.commit_mode,
                    read_only: false,
                    enabled: cx.enabled(),
                    buttons: false,
                    accelerated: false,
                    correction: Correction::Nearest,
                    invalid: false,
                    css: None,
                    dark,
                    node: cx.node().id().to_raw(),
                };
                field.ui(ui, &cfg)
            };
            self.st.field = field;
            if let Some(v) = out.input {
                input = Some(v);
                val = v;
            }
            if let Some(v) = out.commit {
                commit = Some(v);
            }
            if out.submit.is_some() {
                submit = out.submit;
            }
            started |= out.drag_started;
            stopped |= out.drag_stopped;
            field_resp = Some(out.resp);
        }
        let _ = val;
        // writes and signals: drag-started, changed/input, commit, submit, drag-stopped
        if started {
            cx.dispatch(&self.drag_started, ());
        }
        if let Some(v) = input
            && v != self.value
        {
            // EV1: a Rust filter may prevent the write (the default action of `changed`)
            if !cx.dispatch(&self.changed, v).prevented() {
                cx.set(&mut self.value, v);
                cx.dispatch(&self.input, v);
            }
        }
        if let Some(v) = commit {
            cx.dispatch(&self.commit, v);
        }
        if let Some(v) = submit {
            cx.dispatch(&self.submit, v);
        }
        if stopped {
            cx.dispatch(&self.drag_stopped, ());
        }
        // paint
        let mut host = StateBits::NONE;
        for b in [StateBits::FOCUS_VISIBLE, StateBits::PRESSED, StateBits::DRAGGING, StateBits::HOVERED] {
            host.set(b, cx.state(b));
        }
        host.set(StateBits::DISABLED, !cx.enabled());
        let shown = self.current();
        self.st.shown = shown;
        if ui.is_rect_visible(rect) {
            let mut pc = cx.paint_cx(ui);
            let r = self.st.rail;
            r.paint_ticks(&mut pc, host, dark, self.tick_interval_or_page(), &marks);
            r.paint_track(&mut pc, host, dark, shown, None, "fill");
            let mut tb = StateBits(host.0 & (StateBits::DISABLED.0 | StateBits::FOCUS_VISIBLE.0 | StateBits::PRESSED.0 | StateBits::DRAGGING.0));
            tb.set(StateBits::HOVERED, ui.rect_contains_pointer(r.thumb(shown)) || self.scrub.dragging());
            r.paint_thumb(&mut pc, "thumb", tb, dark, shown);
            if !self.text.is_empty() {
                let s = cx.style(UA);
                let g = paint::text_galley(ui.ctx(), &s, &self.text, None);
                paint::paint_text(&mut pc.part("label"), &s, &g, cap_rect, paint::Align::Start);
            }
        }
        // AccessKit: the rail is the `Slider`, the value box a `SpinButton` named by it
        let enabled = cx.enabled();
        let text = self.text.clone();
        resp.widget_info(|| egui::WidgetInfo::slider(enabled, shown, &text));
        ui.ctx().accesskit_node_builder(id, |n| {
            n.set_role(egui::accesskit::Role::Slider);
            if vertical {
                n.set_orientation(egui::accesskit::Orientation::Vertical);
            }
            self.units_a11y(n);
        });
        self.st.field_focused = false;
        match field_resp {
            Some(f) => {
                let fid = f.id;
                ui.ctx().accesskit_node_builder(fid, |n| self.st.field.a11y(n, None));
                let focused = f.has_focus() || f.gained_focus() || f.lost_focus();
                self.st.field_focused = f.has_focus();
                if text.is_empty() && focused {
                    resp.clone().labelled_by(fid);
                } else {
                    f.clone().labelled_by(id);
                }
                if focused { f.union(resp) } else { resp.union(f) }
            }
            None => resp,
        }
    }

    fn a11y(&self, _cx: &A11yCx<'_>, _node: &mut egui::accesskit::Node) {}

    fn consume_key(&self, key: egui::Key, m: egui::Modifiers) -> bool {
        if self.st.field_focused { self.st.field.claims(key, m) } else { self.units_claims(key, m) }
    }
}

impl Slider {
    fn tick_interval_or_page(&self) -> f64 {
        if self.tick_interval > 0.0 {
            return self.tick_interval;
        }
        let sp = self.step_params();
        sp.page_size(self.current(), true)
    }
}
