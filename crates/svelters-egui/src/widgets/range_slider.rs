//! `range-slider`, svelters' own (OW1b, ARCH-WIDGETS §6.1, WIDGET-PARITY §2.7): a rail with two
//! thumbs (`low`, `high`) selecting `low..high`; they meet but never cross. Each thumb is a tab
//! stop with its own units (Hoverable, Focusable, DragToValue, Steppable) and AccessKit `Slider`
//! node named `<name> low`/`<name> high`; a press on the rail off the thumbs moves the nearer one
//! there. With `show-value`, a value box (a [`ValueBox`], AccessKit `SpinButton`) sits at each end.
//!
//! Signals: `changed(low, high)` (and `input`) per change, `commit(low, high)` at a commit point,
//! `drag-started`/`drag-stopped`. Parts: `track`, `range`, `rest`, `low`, `high`, `tick`,
//! `low-value`, `high-value`.

use super::numfmt::{NumFormat, auto_decimals, decimals_of};
use super::rail::{self, GAP, Rail, Ticks};
use super::value_box::{BoxCfg, Correction, ValueBox};
use crate::interact::{self, Arrows, DragToValue, FocusParams, Focusable, Hoverable, PartResponses, Scale, Scrub, ScrubMode, ScrubParams, Step, StepParams, Steppable, Unit, Wheel};
use crate::paint::{self, Ua, UaRule};
use crate::provider::{AvailableSpace, Event, MeasureCx, Part, StateBits, Widget, WidgetCx};

pub const UA: Ua = Ua(&[UaRule::base(&[])]);

/// One thumb: its units, state and value box.
#[derive(Debug, Default)]
pub struct Handle {
    hover: Hoverable,
    focus: Focusable,
    scrub: DragToValue,
    step: Steppable,
    bits: StateBits,
    field: ValueBox,
    focused: bool,
    field_focused: bool,
}

/// What the range slider keeps between frames.
#[derive(Debug, Default)]
pub struct RangeState {
    rail: Rail,
    h: [Handle; 2],
    /// A press on the rail moves this thumb.
    rail_scrub: DragToValue,
    rail_target: usize,
}

/// A two-thumb slider.
#[svelters::widget(namespace = "sv", replaces = "range-slider", caps = FOCUSABLE | EDITABLE | PAINTS_BOX)]
pub struct RangeSlider {
    /// The lower end; kept within `min` and `high`.
    #[prop(bind)]
    low: f64,
    /// The upper end; kept within `low` and `max`.
    #[prop(bind, default = 100.0)]
    high: f64,
    #[prop(default = 0.0, measure)]
    min: f64,
    #[prop(default = 100.0, measure)]
    max: f64,
    /// Value granularity; 0 for continuous.
    #[prop(measure)]
    step: f64,
    /// Caption after the slider (each thumb is named `<text> low`/`<text> high`).
    #[prop(measure)]
    text: String,
    /// Shows an editable value box at each end.
    #[prop(default = true, measure)]
    show_value: bool,
    /// PageUp/PageDown; 0 for ten steps.
    #[prop]
    page_step: f64,
    /// `horizontal` (default) or `vertical`.
    #[prop(default = "horizontal".to_string(), measure, enumeration = "horizontal|vertical")]
    orientation: String,
    /// Tick marks: `none`, `before`, `after`, `both`.
    #[prop(default = "none".to_string(), measure, enumeration = "none|before|after|both")]
    ticks: String,
    /// Between ticks; 0 for `page-step`.
    #[prop]
    tick_interval: f64,
    /// Max at the start.
    #[prop]
    inverted: bool,
    /// `linear` (default) or `log`.
    #[prop(default = "linear".to_string(), enumeration = "linear|log")]
    scale: String,
    /// Commit-only mode: `low`/`high` move at the release.
    #[prop(rename = "commit")]
    commit_mode: bool,
    /// When the wheel steps: `focused` (default), `hover`, `off`.
    #[prop(default = "focused".to_string(), enumeration = "focused|hover|off")]
    wheel: String,
    /// The user moved a thumb (or the widget clamped a written value); carries both ends.
    #[event]
    changed: Event<Vec<f64>>,
    #[event]
    input: Event<()>,
    #[event]
    commit: Event<()>,
    #[event]
    drag_started: Event<()>,
    #[event]
    drag_stopped: Event<()>,
    #[part]
    track: Part,
    #[part]
    range: Part,
    #[part]
    rest: Part,
    #[part]
    low_thumb: Part,
    #[part]
    high_thumb: Part,
    #[part]
    tick: Part,
    #[part]
    low_value: Part,
    #[part]
    high_value: Part,
    #[state]
    st: RangeState,
}

impl RangeSlider {
    /// The rail's ends: a NaN or infinite end falls back (0, or 1 past the other), ±1e300 at most.
    fn bounds(&self) -> (f64, f64) {
        let a = if self.min.is_finite() { self.min } else { 0.0 }.clamp(-1e300, 1e300);
        let b = if self.max.is_finite() { self.max } else if a < 1.0 { 1.0 } else { a + 1.0 }.clamp(-1e300, 1e300);
        (a.min(b), a.max(b))
    }

    /// The two ends as shown: NaN falls back to its end of the rail; ordered and within it.
    fn ends(&self) -> (f64, f64) {
        let (lo, hi) = self.bounds();
        let l = if self.low.is_finite() { self.low } else { lo }.clamp(lo, hi);
        let h = if self.high.is_finite() { self.high } else { hi }.clamp(lo, hi);
        (l.min(h), h.max(l))
    }

    fn step_params(&self, k: usize, v: (f64, f64)) -> StepParams {
        let (lo, hi) = self.bounds();
        let (min, max) = if k == 0 { (lo, v.1) } else { (v.0, hi) };
        StepParams { value: if k == 0 { v.0 } else { v.1 }, min, max, step: self.step, page: self.page_step, wheel: Wheel::parse(&self.wheel), arrows: Arrows::Both, ..StepParams::default() }
    }

    fn fmt(&self, len: f32) -> NumFormat<'static> {
        let (lo, hi) = self.bounds();
        let d = if self.step > 0.0 && self.step.is_finite() {
            let d = decimals_of(self.step);
            (d, d)
        } else {
            let a = auto_decimals((hi - lo) / len.max(1.0) as f64);
            (a, a + 2)
        };
        NumFormat { decimals: d, ..NumFormat::default() }
    }

    fn box_size(&self, ctx: &egui::Context, dark: bool) -> egui::Vec2 {
        let (lo, hi) = self.bounds();
        let f = self.fmt(super::slider::RAIL_LEN);
        let a = ValueBox::text_width(ctx, None, &f.display(lo), dark).max(ValueBox::text_width(ctx, None, &f.display(hi), dark));
        egui::vec2(a.x.max(40.0), a.y.max(24.0))
    }

    fn snap(&self, v: f64) -> f64 {
        let (lo, hi) = self.bounds();
        ScrubParams { min: lo, max: hi, step: self.step, ..ScrubParams::default() }.snap(v)
    }
}

impl Widget for RangeSlider {
    fn measure(&mut self, cx: &mut MeasureCx<'_>, _avail: AvailableSpace) -> egui::Vec2 {
        let dark = cx.ctx.theme() == egui::Theme::Dark;
        let s = cx.style(UA);
        let cap = if self.text.is_empty() { egui::Vec2::ZERO } else { paint::measure_text(cx.ctx, &s, &self.text, AvailableSpace::MaxContent) };
        let bx = if self.show_value { self.box_size(cx.ctx, dark) } else { egui::Vec2::ZERO };
        let across = rail::THUMB + 4.0 + Ticks::parse(&self.ticks).extra(false);
        let mut w = super::slider::RAIL_LEN + rail::THUMB;
        for e in [bx.x, bx.x, cap.x] {
            if e > 0.0 {
                w += GAP + e;
            }
        }
        if self.orientation == "vertical" {
            return egui::vec2(across.max(bx.x).max(cap.x), w);
        }
        egui::vec2(w, across.max(bx.y).max(cap.y))
    }

    fn ui(&mut self, ui: &mut egui::Ui, cx: &mut WidgetCx<'_>) -> egui::Response {
        let dark = ui.ctx().theme() == egui::Theme::Dark;
        let (lo, hi) = self.bounds();
        let vertical = self.orientation == "vertical";
        let rect = cx.rect();
        let cap = if self.text.is_empty() { egui::Vec2::ZERO } else { paint::measure_text(ui.ctx(), &cx.style(UA), &self.text, AvailableSpace::MaxContent) };
        let bx = if self.show_value { self.box_size(ui.ctx(), dark).min(rect.size()) } else { egui::Vec2::ZERO };
        // [low box] rail [high box] [caption]; vertical: top to bottom, the high end first
        let (rail_rect, boxes, cap_rect) = {
            let main = |r: egui::Rect| if vertical { r.height() } else { r.width() };
            let start = if bx.x > 0.0 { (if vertical { bx.y } else { bx.x }) + GAP } else { 0.0 };
            let end = start + if cap.x > 0.0 { (if vertical { cap.y } else { cap.x }) + GAP } else { 0.0 };
            let len = (main(rect) - start - end).max(rail::THUMB);
            if vertical {
                let rail_r = egui::Rect::from_min_size(egui::pos2(rect.min.x, rect.min.y + start), egui::vec2(rect.width(), len));
                let hb = egui::Rect::from_min_size(egui::pos2(rect.center().x - bx.x / 2.0, rect.min.y), bx);
                let lb = egui::Rect::from_min_size(egui::pos2(rect.center().x - bx.x / 2.0, rail_r.max.y + GAP), bx);
                let c = egui::Rect::from_min_size(egui::pos2(rect.center().x - cap.x / 2.0, lb.max.y.max(rail_r.max.y) + GAP), cap);
                (rail_r, [lb, hb], c)
            } else {
                let lb = egui::Rect::from_min_size(egui::pos2(rect.min.x, rect.center().y - bx.y / 2.0), bx);
                let rail_r = egui::Rect::from_min_size(egui::pos2(rect.min.x + start, rect.min.y), egui::vec2(len, rect.height()));
                let hb = egui::Rect::from_min_size(egui::pos2(rail_r.max.x + GAP, rect.center().y - bx.y / 2.0), bx);
                let c = egui::Rect::from_min_size(egui::pos2(rect.max.x - cap.x, rect.center().y - cap.y / 2.0), cap);
                (rail_r, [lb, hb], c)
            }
        };
        self.st.rail = Rail { rect: rail_rect, vertical, inverted: self.inverted, scale: Scale { min: lo, max: hi, log: self.scale == "log" }, ticks: Ticks::parse(&self.ticks), labels: false };
        let r = self.st.rail;
        let id = cx.id();
        let enabled = cx.enabled();
        let (mut l, mut h) = self.ends();
        let before = (l, h);
        let (mut started, mut stopped) = (false, false);
        let mut commit = false;
        // the rail: a press off the thumbs moves the nearer thumb there
        let rail_resp = ui.interact(rail_rect, id, egui::Sense::CLICK | egui::Sense::DRAG);
        let mut target = self.st.rail_target;
        if !self.st.rail_scrub.dragging()
            && let Some(p) = rail_resp.interact_pointer_pos()
        {
            let (dl, dh) = (r.at(l).distance(p), r.at(h).distance(p));
            target = if dh < dl || (dh == dl && (if vertical { p.y < r.at(h).y } else { p.x > r.at(h).x })) { 1 } else { 0 };
            self.st.rail_target = target;
        }
        let tv = if target == 0 { l } else { h };
        let sp = ScrubParams {
            value: tv,
            min: if target == 0 { lo } else { l },
            max: if target == 0 { h } else { hi },
            step: self.step,
            mode: ScrubMode::Absolute { track: r.travel(), thumb: egui::Rect::NOTHING, vertical, inverted: self.inverted, scale: r.scale },
            active: enabled,
            decimals: None,
        };
        let mut ri = interact::Intents::default();
        let mut rb = StateBits::NONE;
        let rs = &mut self.st.rail_scrub;
        interact::run(ui, &rail_resp, &mut rb, &PartResponses::default(), |icx| rs.input(&sp, icx, &mut ri));
        let apply = |k: usize, v: f64, l: &mut f64, h: &mut f64| {
            if k == 0 {
                *l = v.clamp(lo, *h);
            } else {
                *h = v.clamp(*l, hi);
            }
        };
        for s in ri.iter() {
            match *s {
                Scrub::Start => started = true,
                Scrub::Scrub(v) => apply(target, v, &mut l, &mut h),
                Scrub::Commit(_) => commit = true,
                Scrub::Cancel { restore } => apply(target, restore, &mut l, &mut h),
                Scrub::End => stopped = true,
            }
        }
        // the thumbs
        let names = {
            let c = cx.node().common();
            let base = if !self.text.is_empty() {
                self.text.clone()
            } else if !c.accessible_name.is_empty() {
                c.accessible_name.clone()
            } else {
                c.tooltip.clone()
            };
            if base.is_empty() { ["low".to_string(), "high".to_string()] } else { [format!("{base} low"), format!("{base} high")] }
        };
        let mut thumb_resps = Vec::new();
        for k in 0..2 {
            let v = if k == 0 { l } else { h };
            let tid = id.with(if k == 0 { "low" } else { "high" });
            let tr = ui.interact(r.thumb(v), tid, egui::Sense::click_and_drag());
            let pstep = self.step_params(k, (l, h));
            let psc = ScrubParams {
                value: v,
                min: pstep.min,
                max: pstep.max,
                step: self.step,
                mode: ScrubMode::Absolute { track: r.travel(), thumb: r.thumb(v), vertical, inverted: self.inverted, scale: r.scale },
                active: enabled,
                decimals: None,
            };
            let fp = FocusParams::default();
            let hd = &mut self.st.h[k];
            let (mut a, mut b, mut c, mut d) = (interact::Intents::default(), interact::Intents::default(), interact::Intents::default(), interact::Intents::default());
            let mut bits = hd.bits;
            let (hv, fo, sc, stp) = (&mut hd.hover, &mut hd.focus, &mut hd.scrub, &mut hd.step);
            let fc = interact::run(ui, &tr, &mut bits, &PartResponses::default(), |icx| {
                hv.input(&(), icx, &mut a);
                fo.input(&fp, icx, &mut b);
                sc.input(&psc, icx, &mut c);
                stp.input(&pstep, icx, &mut d);
            });
            hd.bits = bits;
            hd.focused = fc.has;
            if hd.step.took_wheel() {
                ui.ctx().input_mut(|i| {
                    i.smooth_scroll_delta = egui::Vec2::ZERO;
                });
            }
            if tr.has_focus() {
                ui.memory_mut(|m| m.set_focus_lock_filter(tid, egui::EventFilter { horizontal_arrows: true, vertical_arrows: true, tab: false, escape: false }));
            }
            for s in c.iter() {
                match *s {
                    Scrub::Start => started = true,
                    Scrub::Scrub(x) => apply(k, x, &mut l, &mut h),
                    Scrub::Commit(_) => commit = true,
                    Scrub::Cancel { restore } => apply(k, restore, &mut l, &mut h),
                    Scrub::End => stopped = true,
                }
            }
            for s in d.iter() {
                let cur = if k == 0 { l } else { h };
                let p = self.step_params(k, (l, h));
                let x = if let Step::Set(x) = s { self.snap(*x) } else { p.apply(cur, *s) };
                if x.is_finite() && x != cur {
                    apply(k, x, &mut l, &mut h);
                    commit = true;
                }
            }
            thumb_resps.push(tr);
        }
        // the value boxes
        let mut box_resps = Vec::new();
        if self.show_value {
            let len = if vertical { r.travel().height() } else { r.travel().width() };
            for k in 0..2 {
                let v = if k == 0 { l } else { h };
                let mut field = std::mem::take(&mut self.st.h[k].field);
                let out = {
                    let cfg = BoxCfg {
                        id: id.with(if k == 0 { "low-value" } else { "high-value" }),
                        rect: boxes[k],
                        value: v,
                        step: StepParams { arrows: Arrows::Vertical, ..self.step_params(k, (l, h)) },
                        speed: (hi - lo) / len.max(1.0) as f64,
                        fmt: self.fmt(len),
                        commit_mode: self.commit_mode,
                        read_only: false,
                        enabled,
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
                self.st.h[k].field = field;
                if let Some(x) = out.input {
                    apply(k, self.snap(x), &mut l, &mut h);
                }
                commit |= out.commit.is_some();
                started |= out.drag_started;
                stopped |= out.drag_stopped;
                self.st.h[k].field_focused = out.resp.has_focus();
                box_resps.push(out.resp);
            }
        }
        // writes: both ends are stored (an end the host left at NaN takes the value it was shown at)
        if started {
            cx.dispatch(&self.drag_started, ());
        }
        if (l, h) != before && !cx.dispatch(&self.changed, vec![l, h]).prevented() {
            // EV1: a Rust filter may prevent the write (the default action of `changed`)
            if l != self.low {
                cx.set(&mut self.low, l);
            }
            if h != self.high {
                cx.set(&mut self.high, h);
            }
            cx.dispatch(&self.input, ());
        }
        if commit {
            cx.dispatch(&self.commit, ());
        }
        if stopped {
            cx.dispatch(&self.drag_stopped, ());
        }
        // host bits: hover over the rail or a thumb, focus on a thumb or box
        let any_focus = self.st.h.iter().any(|x| x.focused || x.field_focused);
        cx.set_state(StateBits::HOVERED, rail_resp.hovered() || thumb_resps.iter().any(|t| t.hovered()));
        cx.set_state(StateBits::FOCUSED, any_focus);
        cx.set_state(StateBits::FOCUS_VISIBLE, self.st.h.iter().any(|x| x.bits.contains(StateBits::FOCUS_VISIBLE)));
        cx.set_state(StateBits::DRAGGING, self.st.rail_scrub.dragging() || self.st.h.iter().any(|x| x.scrub.dragging()));
        cx.commit_state();
        // paint
        if ui.is_rect_visible(rect) {
            let host = if enabled { StateBits::NONE } else { StateBits::DISABLED };
            let mut pc = cx.paint_cx(ui);
            let interval = if self.tick_interval > 0.0 { self.tick_interval } else { self.step_params(0, (l, h)).page_size(l, true) };
            r.paint_ticks(&mut pc, host, dark, interval, &[]);
            r.paint_track(&mut pc, host, dark, h, Some((l, h)), "range");
            for (k, name) in [(0usize, "low-thumb"), (1usize, "high-thumb")] {
                let hd = &self.st.h[k];
                let mut b = host | StateBits(hd.bits.0 & (StateBits::FOCUS_VISIBLE.0 | StateBits::PRESSED.0 | StateBits::DRAGGING.0 | StateBits::HOVERED.0));
                if self.st.rail_scrub.dragging() && self.st.rail_target == k {
                    b |= StateBits::DRAGGING;
                }
                r.paint_thumb(&mut pc, name, b, dark, if k == 0 { l } else { h });
            }
            if !self.text.is_empty() {
                let s = cx.style(UA);
                let g = paint::text_galley(ui.ctx(), &s, &self.text, None);
                paint::paint_text(&mut pc.part("label"), &s, &g, cap_rect, paint::Align::Start);
            }
        }
        // AccessKit: the rail is a group; each thumb a named `Slider`, each box a `SpinButton`
        ui.ctx().accesskit_node_builder(id, |n| n.set_role(egui::accesskit::Role::Group));
        for (k, tr) in thumb_resps.iter().enumerate() {
            let v = if k == 0 { l } else { h };
            let label = names[k].clone();
            tr.widget_info(|| egui::WidgetInfo::slider(enabled, v, &label));
            let p = StepParams { value: v, ..self.step_params(k, (l, h)) };
            let hd = &self.st.h[k];
            ui.ctx().accesskit_node_builder(tr.id, |n| {
                n.set_role(egui::accesskit::Role::Slider);
                n.set_label(names[k].clone());
                if vertical {
                    n.set_orientation(egui::accesskit::Orientation::Vertical);
                }
                hd.step.a11y(&p, n);
                hd.focus.a11y(&FocusParams::default(), n);
            });
        }
        for (k, b) in box_resps.iter().enumerate() {
            ui.ctx().accesskit_node_builder(b.id, |n| self.st.h[k].field.a11y(n, None));
            b.clone().labelled_by(thumb_resps[k].id);
        }
        // the focused piece speaks for the widget (EV3 records it, as egui's own)
        let mut resp = rail_resp;
        let mut focused = false;
        for t in thumb_resps.into_iter().chain(box_resps) {
            let f = t.has_focus() || t.gained_focus() || t.lost_focus();
            focused |= f;
            resp = if f { t.union(resp) } else { resp.union(t) };
        }
        if !focused {
            // the rail itself is not a tab stop (each thumb is)
            resp.sense.remove(egui::Sense::FOCUSABLE);
        }
        resp
    }

    fn consume_key(&self, key: egui::Key, m: egui::Modifiers) -> bool {
        self.st.h.iter().enumerate().any(|(k, hd)| {
            if hd.field_focused {
                hd.field.claims(key, m)
            } else if hd.focused {
                let (l, h) = self.ends();
                hd.step.claims(&self.step_params(k, (l, h)), key, m) || hd.scrub.claims(&ScrubParams::default(), key, m)
            } else {
                false
            }
        })
    }
}
