//! `slider` (ARCH-WIDGETS §6.1; Qt QSlider parity, WIDGET-PARITY §2.7; ported from svelters-egui
//! `widgets/slider.rs`): a rail with a thumb (DragToValue, absolute: a press on the track jumps
//! there, or pages toward the pointer with `track-click="page"`; a press on the thumb grabs it),
//! Steppable keys (arrows, PageUp/PageDown, Home/End), the wheel, the value as text and a caption.
//!
//! Signals (§2.4): `changed`/`input` per change (the draft while dragging in commit mode is not
//! sent), `commit` at a commit point (release, a step), `drag-started`/`drag-stopped`. Paint parts:
//! `track`, `fill`, `rest`, `thumb`, `tick`, `mark-label` (the rail, [`rail`]), `value` (the value
//! text) and `label` (the caption).
//!
//! The geometry (rail, value, caption) depends on text sizes, so [`Kind::prepare`] computes it for
//! the events that follow; recording computes it again from its own context.

pub mod numfmt;
pub mod rail;

use crate::controls::common::{self, Units};
use crate::interact::{Arrows, DragToValue, Focusable, Hoverable, Intents, Mods, Scale, Scrub, ScrubMode, ScrubParams, Sense, StateBits, Step, StepParams, Steppable, Unit, Wheel};
use crate::kind_props;
use crate::paint::{self, Align, PaintCx, StyleView, Ua, UaRule, UaValue::*};
use crate::shim::core::{A11yCx, Event, EventCx, Handled, Kind, KindSpec, MeasureCx, NameFrom, PrepareCx, PropError, PropId, RecordCx, SignalSpec, Styles, Value, ValueType};
use crate::shim::text::TextShaper;
use numfmt::{NumFormat, auto_decimals, decimals_of};
use presto_base::accesskit;
use presto_base::event::{MouseButton, PointerPhase};
use presto_base::geom::{Point, Rect, Size, Vec2};
use presto_base::key::Key;
use rail::{GAP, Mark, Rail, Ticks};

/// The host's UA sheet: nothing of its own (the parts have theirs, `rail.rs` and [`VALUE_UA`]).
pub const UA: Ua = Ua(&[UaRule::base(&[])]);

/// `::part(value)`: the value text in the value box's metrics (6/3 padding, the field's text
/// colour), so the editable box fits the same place.
pub const VALUE_UA: Ua = Ua(&[
    UaRule::base(&[("color", Token("input-fg")), ("padding-left", Px(6.0)), ("padding-right", Px(6.0)), ("padding-top", Px(3.0)), ("padding-bottom", Px(3.0))]),
    UaRule::when(StateBits::DISABLED, &[("color", Token("disabled-fg"))]),
]);

/// The default length of the rail (egui's `slider_width`).
pub const RAIL_LEN: f32 = 100.0;

kind_props! {
    /// The slider's props.
    pub struct SliderProps;
    /// The value; dragging, the keys, the wheel change it.
    value "value": f64 = F64(0.0) ; F64, Paint, bindable;
    min "min": f64 = F64(0.0) ; F64, Layout;
    max "max": f64 = F64(100.0) ; F64, Layout;
    /// Value granularity; 0 for continuous.
    step "step": f64 = F64(0.0) ; F64, Layout;
    /// Caption after the slider (and its accessible name).
    text "text": String = Str("") ; Str, Layout;
    /// Shows the value.
    show_value "show-value": bool = Bool(true) ; Bool, Layout;
    /// PageUp/PageDown and `track-click="page"` move this much (Qt `pageStep`); 0 for ten steps.
    page_step "page-step": f64 = F64(0.0) ; F64, Property;
    /// `horizontal` (default) or `vertical` (max at the top).
    orientation "orientation": String = Enum("horizontal") ; Enum(&["horizontal", "vertical"]), Layout;
    /// Tick marks: `none` (default), `before` (above or left), `after`, `both` (Qt `tickPosition`).
    ticks "ticks": String = Enum("none") ; Enum(&["none", "before", "after", "both"]), Layout;
    /// Between ticks; 0 for `page-step` (Qt `tickInterval`).
    tick_interval "tick-interval": f64 = F64(0.0) ; F64, Paint;
    /// Marks on the rail, drawn as ticks (with labels) instead of the `tick-interval` ticks: values
    /// separated by commas, each optionally `value:label` (`"0:Low, 50, 100:High"`).
    marks "marks": String = Str("") ; Str, Layout;
    /// Dragging snaps to the nearest mark.
    snap_to_marks "snap-to-marks": bool = Bool(false) ; Bool, Property;
    /// Max at the start (Qt `invertedAppearance`).
    inverted "inverted": bool = Bool(false) ; Bool, Paint;
    /// A press on the track off the thumb: `jump` there (default) or `page` toward it.
    track_click "track-click": String = Enum("jump") ; Enum(&["jump", "page"]), Property;
    /// `linear` (default) or `log` (both ends > 0).
    scale "scale": String = Enum("linear") ; Enum(&["linear", "log"]), Paint;
    /// Commit-only mode (Qt `tracking=false`): `value` moves at the release, not while dragging.
    commit "commit": bool = Bool(false) ; Bool, Property;
    /// When the wheel steps: `focused` (default), `hover`, `off`.
    wheel "wheel": String = Enum("focused") ; Enum(&["focused", "hover", "off"]), Property;
    disabled "disabled": bool = Bool(false) ; Bool, Paint;
}

const VALUE_ARG: &[(&str, ValueType)] = &[("value", ValueType::F64)];

/// The signals, in spec order.
pub const SIGNALS: &[SignalSpec] = &[
    SignalSpec::new("changed", VALUE_ARG, "The value changed (dragging, a key, the wheel, AccessKit). Preventing it keeps the value.").cancelable(),
    SignalSpec::new("input", VALUE_ARG, "Sent after `changed`, with the value written."),
    SignalSpec::new("commit", VALUE_ARG, "A commit point: the release of a drag that changed the value, or a step."),
    SignalSpec::new("submit", VALUE_ARG, "The user committed a value: the release of a drag, or Enter while focused."),
    SignalSpec::new("drag-started", &[], ""),
    SignalSpec::new("drag-stopped", &[], ""),
];

pub static SPEC: KindSpec = KindSpec { tag: "slider", props: SliderProps::SPECS, signals: SIGNALS, slots: &[], children: false, role: accesskit::Role::Slider, name_from: NameFrom::Prop("text"), text_prop: None, focusable: true };

/// Where the slider's pieces go in its box (host-local).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Geom {
    /// The rail's box (thumb travel plus half a thumb at each end, ticks included).
    pub rail: Rect,
    /// The value's box (empty without `show-value`).
    pub value: Rect,
    /// The caption's box (empty without `text`).
    pub caption: Rect,
}

impl Geom {
    /// All rail: no value and no caption.
    fn bare(size: Size) -> Geom {
        let r = Rect::from_origin_size(Point::ZERO, size);
        Geom { rail: r, value: Rect::ZERO, caption: Rect::ZERO }
    }
}

/// Both edges inclusive (egui's `Rect::contains`).
fn contains(r: &Rect, p: Point) -> bool {
    p.x >= r.min.x && p.x <= r.max.x && p.y >= r.min.y && p.y <= r.max.y
}

fn sized(x: f32, y: f32, s: Vec2) -> Rect {
    Rect::new(x, y, x + s.x, y + s.y)
}

/// A slider.
#[derive(Debug, Default)]
pub struct Slider {
    pub props: SliderProps,
    units: Units,
    hover: Hoverable,
    focus: Focusable,
    scrub: DragToValue,
    stepper: Steppable,
    /// Commit mode: the value while dragging.
    draft: Option<f64>,
    /// `track-click="page"`: this press pages instead of dragging.
    paging: bool,
    /// This press began off the rail (on the value or the caption): it does not drag.
    off_rail: bool,
    /// The pointer is over the thumb (its `:hover`).
    thumb_hover: bool,
    /// The geometry `prepare` computed for the events.
    geom: Option<Geom>,
}

impl Slider {
    fn bounds(&self) -> (f64, f64) {
        let p = &self.props;
        let a = if p.min.is_finite() { p.min } else { 0.0 };
        let b = if p.max.is_finite() { p.max } else { a + 100.0 };
        (a.min(b), a.max(b))
    }

    fn current(&self) -> f64 {
        let (lo, _) = self.bounds();
        self.draft.unwrap_or(if self.props.value.is_nan() { lo } else { self.props.value })
    }

    fn vertical(&self) -> bool {
        self.props.orientation == "vertical"
    }

    fn marks(&self) -> Vec<Mark> {
        rail::marks_of(&self.props.marks)
    }

    /// The rail in `rect` under the props.
    fn rail(&self, rect: Rect) -> Rail {
        let p = &self.props;
        let (lo, hi) = self.bounds();
        let scale = if p.min.is_finite() && p.max.is_finite() { Scale { min: p.min, max: p.max, log: p.scale == "log" } } else { Scale { min: lo, max: hi, log: false } };
        Rail { rect, vertical: self.vertical(), inverted: p.inverted, scale, ticks: Ticks::parse(&p.ticks), labels: self.marks().iter().any(|m| !m.label.is_empty()) }
    }

    fn scrub_params(&self, r: &Rail) -> ScrubParams {
        let (lo, hi) = self.bounds();
        ScrubParams {
            value: self.current(),
            min: lo,
            max: hi,
            step: self.props.step,
            mode: ScrubMode::Absolute { track: r.travel(), thumb: r.thumb(self.current()), vertical: r.vertical, inverted: r.inverted, scale: r.scale },
            active: !self.paging && !self.off_rail,
            decimals: None,
        }
    }

    fn step_params(&self) -> StepParams {
        let (lo, hi) = self.bounds();
        StepParams {
            value: self.current(),
            min: lo,
            max: hi,
            step: self.props.step,
            page: self.props.page_step,
            wheel: Wheel::parse(&self.props.wheel),
            arrows: Arrows::Both,
            ..StepParams::default()
        }
    }

    /// The format of the value for a rail `rail_len` points long.
    fn fmt(&self, rail_len: f32) -> NumFormat<'static> {
        let (lo, hi) = self.bounds();
        let d = if self.props.step > 0.0 {
            let d = decimals_of(self.props.step);
            (d, d)
        } else {
            let a = auto_decimals((hi - lo) / rail_len.max(1.0) as f64);
            (a, a + 2)
        };
        NumFormat { decimals: d, ..NumFormat::default() }
    }

    /// The value box's size for the bounds' texts (at least 40×24).
    fn box_size(&self, shaper: &mut dyn TextShaper, v: &StyleView) -> Vec2 {
        let (lo, hi) = self.bounds();
        let f = self.fmt(RAIL_LEN);
        let extra = paint::box_extra(v);
        let a = paint::measure_text(shaper, v, &f.display(lo), None);
        let b = paint::measure_text(shaper, v, &f.display(hi), None);
        Vec2::new((a.w.max(b.w) + extra.x).max(40.0), (a.h.max(b.h) + extra.y).max(24.0))
    }

    fn caption_size(&self, shaper: &mut dyn TextShaper, v: &StyleView) -> Vec2 {
        if self.props.text.is_empty() {
            return Vec2::ZERO;
        }
        let s = paint::measure_text(shaper, v, &self.props.text, None);
        Vec2::new(s.w, s.h)
    }

    fn snap_mark(&self, v: f64) -> f64 {
        if !self.props.snap_to_marks {
            return v;
        }
        self.marks().into_iter().map(|m| m.value).min_by(|a, b| (a - v).abs().total_cmp(&(b - v).abs())).unwrap_or(v)
    }

    fn tick_interval_or_page(&self) -> f64 {
        if self.props.tick_interval > 0.0 {
            return self.props.tick_interval;
        }
        self.step_params().page_size(self.current(), true)
    }

    /// The rail, the value and the caption in a box of `size`: horizontal, rail then value then
    /// caption at the end; vertical, stacked and centred.
    fn layout(&self, shaper: &mut dyn TextShaper, styles: &Styles, size: Size) -> Geom {
        let host = common::host_view(styles, UA);
        let cap_v = host.part(styles.part_css("label"), Ua::NONE);
        let val_v = host.part(styles.part_css("value"), VALUE_UA);
        let cap = self.caption_size(shaper, &cap_v);
        let mut bx = if self.props.show_value { self.box_size(shaper, &val_v) } else { Vec2::ZERO };
        let rect = Rect::from_origin_size(Point::ZERO, size);
        let cx = rect.width() / 2.0;
        let cy = rect.height() / 2.0;
        if self.vertical() {
            let cap_h = if cap.y > 0.0 { cap.y + GAP } else { 0.0 };
            let box_h = if bx.y > 0.0 { bx.y + GAP } else { 0.0 };
            let rail_r = Rect::new(0.0, 0.0, rect.max.x, (rect.max.y - cap_h - box_h).max(rail::THUMB));
            let b = sized(cx - bx.x / 2.0, rail_r.max.y + GAP, bx);
            let c = sized(cx - cap.x / 2.0, b.max.y.max(rail_r.max.y) + GAP, cap);
            Geom { rail: rail_r, value: if bx.y > 0.0 { b } else { Rect::ZERO }, caption: if cap.y > 0.0 { c } else { Rect::ZERO } }
        } else {
            let cap_w = if cap.x > 0.0 { cap.x + GAP } else { 0.0 };
            let box_w = if bx.x > 0.0 { bx.x + GAP } else { 0.0 };
            let rail_r = Rect::new(0.0, 0.0, (rect.max.x - cap_w - box_w).max(rail::THUMB), rect.max.y);
            bx.y = bx.y.min(rect.height());
            let b = sized(rail_r.max.x + GAP, cy - bx.y / 2.0, bx);
            let c = sized(rect.max.x - cap.x, cy - cap.y / 2.0, cap);
            Geom { rail: rail_r, value: if bx.x > 0.0 { b } else { Rect::ZERO }, caption: if cap.x > 0.0 { c } else { Rect::ZERO } }
        }
    }

    /// The host's state for painting: the units' bits and `:disabled`.
    fn paint_bits(&self, state: StateBits) -> StateBits {
        let mut host = StateBits::NONE;
        for b in [StateBits::FOCUS_VISIBLE, StateBits::PRESSED, StateBits::DRAGGING, StateBits::HOVERED] {
            host.set(b, state.contains(b));
        }
        host.set(StateBits::DISABLED, self.props.disabled || state.contains(StateBits::DISABLED));
        host
    }

    /// Writes `v` as the value the user set: `changed`, the write, `input`.
    fn write(&mut self, cx: &mut EventCx, v: f64) {
        if common::emit_sync(cx, &SPEC, "changed", vec![Value::F64(v)]) {
            return;
        }
        self.props.value = v;
        cx.prop_changed(P::value.id());
        common::emit(cx, &SPEC, "input", vec![Value::F64(v)]);
    }
}

impl Kind for Slider {
    fn spec() -> &'static KindSpec {
        &SPEC
    }
    fn kind_spec(&self) -> &'static KindSpec {
        &SPEC
    }
    fn set_prop(&mut self, prop: PropId, v: &Value) -> Result<(), PropError> {
        self.props.set(prop, v)
    }
    fn get_prop(&self, prop: PropId) -> Option<Value> {
        self.props.get(prop)
    }

    fn measure(&self, cx: &mut MeasureCx, _known: taffy::Size<Option<f32>>, _avail: taffy::Size<taffy::AvailableSpace>) -> taffy::Size<f32> {
        let host = common::host_view(&cx.style, UA);
        let cap_v = host.part(cx.style.part_css("label"), Ua::NONE);
        let val_v = host.part(cx.style.part_css("value"), VALUE_UA);
        let cap = self.caption_size(cx.text, &cap_v);
        let bx = if self.props.show_value { self.box_size(cx.text, &val_v) } else { Vec2::ZERO };
        let ticks = Ticks::parse(&self.props.ticks).extra(self.marks().iter().any(|m| !m.label.is_empty()));
        let across = rail::THUMB + 4.0 + ticks;
        let s = if self.vertical() {
            let w = across.max(bx.x).max(cap.x);
            let mut h = RAIL_LEN + rail::THUMB;
            for e in [bx.y, cap.y] {
                if e > 0.0 {
                    h += GAP + e;
                }
            }
            Vec2::new(w, h)
        } else {
            let mut w = RAIL_LEN + rail::THUMB;
            for e in [bx.x, cap.x] {
                if e > 0.0 {
                    w += GAP + e;
                }
            }
            Vec2::new(w, across.max(bx.y).max(cap.y))
        };
        taffy::Size { width: s.x, height: s.y }
    }

    fn prepare(&mut self, cx: &mut PrepareCx) {
        self.geom = Some(self.layout(cx.text, &cx.style, cx.size));
    }

    fn record(&self, cx: &mut RecordCx) {
        let g = self.layout(cx.text, &cx.style, cx.size);
        let r = self.rail(g.rail);
        let host_bits = self.paint_bits(cx.style.state);
        let host = common::host_view(&cx.style, UA).in_state(host_bits);
        let shown = self.current();
        let marks = self.marks();
        let styles = &cx.style;
        let mut pc = PaintCx::new(&mut *cx.list, &mut *cx.res, &mut *cx.text, cx.scale);
        r.paint_ticks(&mut pc, styles, &host, host_bits, self.tick_interval_or_page(), &marks);
        r.paint_track(&mut pc, styles, &host, host_bits, shown, None, "fill");
        let mut tb = StateBits(host_bits.0 & (StateBits::DISABLED.0 | StateBits::FOCUS_VISIBLE.0 | StateBits::PRESSED.0 | StateBits::DRAGGING.0));
        tb.set(StateBits::HOVERED, self.thumb_hover || self.scrub.dragging());
        r.paint_thumb(&mut pc, styles, &host, "thumb", tb, shown);
        if self.props.show_value {
            let vv = host.part(styles.part_css("value"), VALUE_UA);
            let track_len = if r.vertical { r.travel().height() } else { r.travel().width() };
            let text = self.fmt(track_len).display(shown);
            let l = paint::text_layout(pc.text, &vv, &text, None);
            let content = paint::box_geom(&vv, g.value).content;
            paint::paint_text(&mut pc, &vv, &l, content, Align::of(&vv, Align::Start));
        }
        if !self.props.text.is_empty() {
            let cv = host.part(styles.part_css("label"), Ua::NONE);
            let l = paint::text_layout(pc.text, &cv, &self.props.text, None);
            paint::paint_text(&mut pc, &cv, &l, g.caption, Align::Start);
        }
    }

    fn event(&mut self, cx: &mut EventCx, ev: &Event) -> Handled {
        cx.state.set(StateBits::DISABLED, self.props.disabled);
        cx.enabled = !self.props.disabled;
        let (lo, hi) = self.bounds();
        // a value written outside the range is clamped (announced, no signal)
        if self.props.value.is_finite() && (self.props.value < lo || self.props.value > hi) {
            self.props.value = self.props.value.clamp(lo, hi);
            cx.prop_changed(P::value.id());
        }
        let geom = self.geom.unwrap_or_else(|| Geom::bare(cx.size));
        let r = self.rail(geom.rail);
        let vertical = r.vertical;
        // `track-click="page"`: a press on the track off the thumb pages toward the pointer
        let mut page_dir = None;
        if let Event::Pointer(pe) = ev
            && pe.phase == PointerPhase::Down
        {
            self.off_rail = !r.rect.contains(pe.pos);
            if cx.enabled && !self.off_rail && pe.button == Some(MouseButton::Primary) && self.props.track_click == "page" && !self.paging && !contains(&r.thumb(self.current()), pe.pos) {
                let c = r.at(self.current());
                let after = if vertical { pe.pos.y < c.y } else { pe.pos.x > c.x };
                page_dir = Some(if after != self.props.inverted { 1 } else { -1 });
            }
        }
        if page_dir.is_some() {
            self.paging = true;
        }
        let sp = self.scrub_params(&r);
        let stp = self.step_params();
        let sense = Sense::HOVER.union(self.focus.sense(&Default::default())).union(self.scrub.sense(&sp)).union(self.stepper.sense(&stp));
        let mut fs = Intents::default();
        let mut ft = Intents::default();
        let (hover, focus, scrub, stepper) = (&mut self.hover, &mut self.focus, &mut self.scrub, &mut self.stepper);
        let pass = self.units.run(cx, ev, sense, |icx| {
            hover.input(&(), icx, &mut Intents::default());
            focus.input(&Default::default(), icx, &mut Intents::default());
            scrub.input(&sp, icx, &mut fs);
            stepper.input(&stp, icx, &mut ft);
        });
        if !pass.resp.down_on {
            self.paging = false;
            self.off_rail = false;
        }
        let mut val = self.current();
        let mut input: Option<f64> = None;
        let mut commit: Option<f64> = None;
        // `submit`: the user committed a value: the release of a drag, or Enter while focused
        let mut submit: Option<f64> = None;
        let (mut started, mut stopped) = (false, false);
        if let Some(n) = page_dir {
            let v = stp.apply(val, Step::By { n, page: true });
            if v != val {
                input = Some(v);
                commit = Some(v);
                val = v;
            }
        }
        for s in fs.iter() {
            match *s {
                Scrub::Start => started = true,
                Scrub::Scrub(v) => {
                    let v = self.snap_mark(v);
                    if self.props.commit {
                        self.draft = Some(v);
                    } else {
                        input = Some(v);
                    }
                    val = v;
                }
                Scrub::Commit(v) => {
                    let v = self.snap_mark(v);
                    if self.props.commit {
                        self.draft = None;
                        input = Some(v);
                    }
                    commit = Some(v);
                    submit = Some(v);
                }
                Scrub::Cancel { restore } => {
                    if self.draft.take().is_none() {
                        input = Some(restore);
                    }
                    val = restore;
                }
                Scrub::End => stopped = true,
            }
        }
        for s in ft.iter() {
            let v = if let Step::Set(x) = s { stp.clamp(*x) } else { stp.apply(val, *s) };
            if v.is_finite() && v != val {
                self.draft = None;
                input = Some(v);
                commit = Some(v);
                val = v;
            }
        }
        if cx.focused && pass.input.key_pressed(Key::Enter) {
            submit = Some(self.draft.unwrap_or(val));
        }
        // signals: drag-started, changed/input, commit, submit, drag-stopped
        if started {
            common::emit(cx, &SPEC, "drag-started", Vec::new());
        }
        if let Some(v) = input
            && v != self.props.value
        {
            self.write(cx, v);
        }
        if let Some(v) = commit {
            common::emit(cx, &SPEC, "commit", vec![Value::F64(v)]);
        }
        if let Some(v) = submit {
            common::emit(cx, &SPEC, "submit", vec![Value::F64(v)]);
        }
        if stopped {
            common::emit(cx, &SPEC, "drag-stopped", Vec::new());
        }
        let shown = self.current();
        let thumb_hover = self.scrub.dragging() || pass.input.pointer.latest_pos().is_some_and(|p| contains(&r.thumb(shown), p));
        if thumb_hover != self.thumb_hover || !fs.is_empty() || self.draft.is_some() {
            self.thumb_hover = thumb_hover;
            cx.repaint = true;
        }
        if pass.resp.hovered || pass.resp.down_on || !fs.is_empty() || !ft.is_empty() || self.stepper.took_wheel() || page_dir.is_some() { Handled::Yes } else { Handled::No }
    }

    fn a11y(&self, cx: &mut A11yCx) {
        cx.node.set_role(accesskit::Role::Slider);
        if self.vertical() {
            cx.node.set_orientation(accesskit::Orientation::Vertical);
        }
        if !self.props.text.is_empty() {
            cx.node.set_label(self.props.text.clone());
        }
        if self.props.disabled {
            cx.node.set_disabled();
        }
        self.focus.a11y(&Default::default(), cx.node);
        self.stepper.a11y(&self.step_params(), cx.node);
    }

    fn claims_key(&self, key: Key, m: presto_base::event::Modifiers) -> bool {
        let m = Mods::of(m, false);
        let r = self.rail(self.geom.map_or(Rect::ZERO, |g| g.rail));
        self.stepper.claims(&self.step_params(), key, m) || self.scrub.claims(&self.scrub_params(&r), key, m)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controls::testkit::{self, Driver};
    use presto_base::change::ChangeClass;
    use presto_base::color::Fill;
    use presto_base::display::Prim;

    /// The default slider's size: a 120-point rail, a 40×24 value box after an 8-point gap.
    const SIZE: Size = Size::new(168.0, 24.0);

    fn driver() -> Driver<Slider> {
        let mut d = Driver::new(Slider::default(), SIZE);
        testkit::prepare(&mut d.kind, SIZE, StateBits::NONE);
        d
    }

    fn value(d: &Driver<Slider>) -> f64 {
        d.get("value").and_then(|v| v.as_f64()).unwrap()
    }

    /// The value is `want` up to the f32 rounding of pointer geometry.
    #[track_caller]
    fn assert_value(d: &Driver<Slider>, want: f64, what: &str) {
        let v = value(d);
        assert!((v - want).abs() < 1e-4, "{what}: value {v}, want {want}");
    }

    fn quad(list: &presto_base::display::DisplayList, i: usize) -> presto_base::display::Quad {
        match &list.items[i] {
            Prim::Quad(q) => q.clone(),
            p => panic!("{p:?}"),
        }
    }

    #[test]
    fn spec() {
        let s = Slider::spec();
        assert_eq!(s.tag, "slider");
        assert_eq!(s.role, accesskit::Role::Slider);
        let v = s.prop_spec(s.prop("value").unwrap()).unwrap();
        assert!(v.bindable && v.change == ChangeClass::Paint);
        assert_eq!(s.prop_spec(s.prop("max").unwrap()).unwrap().default, crate::shim::core::Default::F64(100.0));
        for (p, c) in [("min", ChangeClass::Layout), ("orientation", ChangeClass::Layout), ("show-value", ChangeClass::Layout), ("inverted", ChangeClass::Paint), ("page-step", ChangeClass::Property)] {
            assert_eq!(s.prop_spec(s.prop(p).unwrap()).unwrap().change, c, "{p}");
        }
        for sig in ["changed", "input", "commit", "submit", "drag-started", "drag-stopped"] {
            assert!(s.signal(sig).is_some(), "{sig}");
        }
        let mut k = Slider::default();
        let o = s.prop("orientation").unwrap();
        assert!(k.set_prop(o, &Value::str("diagonal")).is_err(), "an enum prop checks its names");
        k.set_prop(o, &Value::str("vertical")).unwrap();
        assert!(k.vertical());
    }

    #[test]
    fn measures_rail_and_value_box() {
        let k = Slider::default();
        assert_eq!(testkit::measure(&k, StateBits::NONE), SIZE);
        let mut v = Slider::default();
        v.props.show_value = false;
        v.props.orientation = "vertical".into();
        assert_eq!(testkit::measure(&v, StateBits::NONE), Size::new(24.0, 120.0));
        let mut t = Slider::default();
        t.props.show_value = false;
        t.props.ticks = "both".into();
        assert_eq!(testkit::measure(&t, StateBits::NONE), Size::new(120.0, 40.0), "ticks add 8 points each side");
    }

    #[test]
    fn records_track_fill_thumb_and_value() {
        let mut k = Slider::default();
        k.props.value = 50.0;
        let (list, res) = testkit::record(&k, SIZE, StateBits::NONE);
        // track, fill, (the rest is transparent), thumb disc, thumb core, the value text
        assert_eq!(testkit::kinds(&list), ["quad", "quad", "quad", "quad", "glyphs"]);
        let track = quad(&list, 0);
        assert_eq!(track.rect, Rect::new(0.0, 10.0, 120.0, 14.0));
        assert_eq!(track.radii.tl, 2.0);
        let fill = quad(&list, 1);
        assert_eq!(fill.rect, Rect::new(0.0, 10.0, 60.0, 14.0), "filled up to the thumb's centre");
        let accent = paint::ua_token(true, "accent").unwrap();
        assert_eq!(fill.fill, Fill::Solid(accent));
        let thumb = quad(&list, 2);
        assert_eq!(thumb.rect, Rect::new(50.0, 2.0, 70.0, 22.0));
        assert!(thumb.border_widths.top == 1.0 && thumb.radii.tl == 10.0);
        let core = quad(&list, 3);
        assert_eq!(core.rect, Rect::new(54.0, 6.0, 66.0, 18.0), "a 12-point core");
        assert_eq!(core.fill, Fill::Solid(accent));
        assert_eq!(res.runs.len(), 1);
        assert_eq!(res.runs[0].glyphs.len(), 2, "\"50\"");
        let Prim::Glyphs(g) = &list.items[4] else { unreachable!() };
        assert!(g.origin.x >= 128.0 + 6.0, "in the value box after the rail and the gap");
        // pressed: a 10-point core; keyboard focus: the thumb's ring last
        let (list, _) = testkit::record(&k, SIZE, StateBits::PRESSED);
        assert_eq!(quad(&list, 3).rect.width(), 10.0);
        let (list, _) = testkit::record(&k, SIZE, StateBits::FOCUSED | StateBits::FOCUS_VISIBLE);
        assert_eq!(testkit::kinds(&list), ["quad", "quad", "quad", "quad", "quad", "glyphs"]);
        assert!(quad(&list, 4).rect.min.x < 50.0, "the ring lies outside the thumb");
        // no value, ticks every page (ten steps of 1% here: 10 points apart, 11 per side)
        k.props.show_value = false;
        k.props.ticks = "after".into();
        let (list, res) = testkit::record(&k, Size::new(120.0, 32.0), StateBits::NONE);
        assert_eq!(testkit::kinds(&list).iter().filter(|k| **k == "quad").count(), 11 + 4);
        assert!(res.runs.is_empty());
    }

    #[test]
    fn records_disabled_and_caption() {
        let mut k = Slider::default();
        k.props.disabled = true;
        k.props.text = "Gain".into();
        k.props.show_value = false;
        let size = testkit::measure(&k, StateBits::NONE);
        let (list, res) = testkit::record(&k, size, StateBits::NONE);
        let grey = paint::ua_token(true, "disabled-fg").unwrap();
        assert_eq!(quad(&list, 1).fill, Fill::Solid(grey), "the fill greys out");
        assert_eq!(quad(&list, 3).fill, Fill::Solid(grey), "so does the thumb's core");
        assert_eq!(res.runs.len(), 1);
        assert_eq!(res.runs[0].glyphs.len(), 4, "the caption");
    }

    #[test]
    fn a_press_on_the_track_jumps_and_commits_on_release() {
        let mut d = driver();
        d.hover(true);
        d.down(60.0, 12.0);
        assert_value(&d, 50.0, "value");
        assert!(d.state.contains(StateBits::PRESSED) && d.state.contains(StateBits::DRAGGING));
        d.advance(30);
        d.up(60.0, 12.0);
        assert_eq!(d.take(), ["drag-started", "changed", "input", "commit", "submit", "drag-stopped"]);
        assert_eq!(d.changed, ["value"]);
        assert!(!d.focused, "egui's row does not focus on click");
    }

    #[test]
    fn dragging_the_thumb_changes_the_value() {
        let mut d = driver();
        d.set("value", Value::F64(50.0));
        d.hover(true);
        d.down(60.0, 12.0);
        assert!(d.take() == ["drag-started"], "a press on the thumb grabs it: no jump");
        d.advance(20);
        d.move_to(80.0, 12.0);
        assert_value(&d, 70.0, "value");
        let changed = d.signals.iter().find(|(n, _)| *n == "changed").and_then(|(_, a)| a.first()?.as_f64());
        assert!(changed.is_some_and(|v| (v - 70.0).abs() < 1e-4), "changed carries the value: {changed:?}");
        d.advance(20);
        d.up(80.0, 12.0);
        assert_eq!(d.take(), ["changed", "input", "commit", "submit", "drag-stopped"]);
        assert_eq!(d.changed, ["value"]);
    }

    #[test]
    fn escape_while_dragging_restores_without_a_commit() {
        let mut d = driver();
        d.set("value", Value::F64(10.0));
        d.hover(true);
        d.down(20.0, 12.0);
        d.advance(20);
        d.move_to(100.0, 12.0);
        assert_value(&d, 90.0, "live while dragging");
        assert!(d.kind.claims_key(Key::Escape, Default::default()));
        d.key(Key::Escape);
        d.up(100.0, 12.0);
        assert_value(&d, 10.0, "value");
        assert!(!d.names().contains(&"commit"));
    }

    #[test]
    fn enter_submits_the_value() {
        let mut d = driver();
        d.set("step", Value::F64(1.0));
        d.focus(true);
        d.key(Key::ArrowRight);
        d.take();
        d.key(Key::Enter);
        let subs: Vec<_> = d.signals.iter().filter(|(n, _)| *n == "submit").map(|(_, a)| a.clone()).collect();
        assert_eq!(subs, [vec![Value::F64(1.0)]], "one submit with the value");
        assert_value(&d, 1.0, "Enter does not move it");
    }

    #[test]
    fn keys_step_page_and_go_to_the_ends() {
        let mut d = driver();
        d.set("step", Value::F64(1.0));
        d.set("page-step", Value::F64(20.0));
        d.focus(true);
        d.key(Key::ArrowRight);
        assert_value(&d, 1.0, "value");
        d.key(Key::PageUp);
        assert_value(&d, 21.0, "value");
        d.key(Key::End);
        assert_value(&d, 100.0, "value");
        d.key(Key::Home);
        assert_value(&d, 0.0, "value");
        d.key(Key::ArrowDown);
        assert_value(&d, 0.0, "clamped at min: no change");
        let names = d.take();
        assert_eq!(names.iter().filter(|n| **n == "commit").count(), 4, "every step commits");
        assert_eq!(names.iter().filter(|n| **n == "changed").count(), 4);
        assert!(d.kind.claims_key(Key::ArrowLeft, Default::default()) && d.kind.claims_key(Key::Home, Default::default()));
        d.a11y_action(accesskit::Action::Increment);
        assert_value(&d, 1.0, "value");
    }

    #[test]
    fn inverted_page_clicks_and_commit_mode() {
        let mut d = driver();
        d.set("inverted", Value::Bool(true));
        d.hover(true);
        d.down(20.0, 12.0);
        d.up(20.0, 12.0);
        assert_value(&d, 90.0, "inverted: max at the start");
        d.set("inverted", Value::Bool(false));
        d.set("value", Value::F64(0.0));
        // track-click="page": toward the pointer by page-step
        d.set("track-click", Value::str("page"));
        d.set("page-step", Value::F64(20.0));
        d.advance(1000);
        d.take();
        d.down(110.0, 12.0);
        d.up(110.0, 12.0);
        assert_value(&d, 20.0, "value");
        assert_eq!(d.take(), ["changed", "input", "commit"]);
        d.set("track-click", Value::str("jump"));
        // commit mode: a draft while dragging, the value and commit at the release
        d.set("commit", Value::Bool(true));
        d.set("value", Value::F64(0.0));
        d.advance(1000);
        d.down(10.0, 12.0);
        d.advance(20);
        d.move_to(60.0, 12.0);
        assert_value(&d, 0.0, "tracking=false: not while dragging");
        assert_eq!(d.take(), ["drag-started"]);
        d.advance(20);
        d.up(60.0, 12.0);
        assert_value(&d, 50.0, "value");
        assert_eq!(d.take(), ["changed", "input", "commit", "submit", "drag-stopped"]);
    }

    #[test]
    fn a_press_on_the_value_text_does_not_drag() {
        let mut d = driver();
        d.hover(true);
        d.down(150.0, 12.0);
        d.advance(20);
        d.move_to(100.0, 12.0);
        d.up(100.0, 12.0);
        assert_value(&d, 0.0, "value");
        assert!(!d.names().contains(&"drag-started"));
    }

    #[test]
    fn disabled_takes_no_input() {
        let mut d = driver();
        d.set("disabled", Value::Bool(true));
        d.hover(true);
        d.down(60.0, 12.0);
        d.up(60.0, 12.0);
        d.focus(true);
        d.key(Key::ArrowRight);
        d.a11y_action(accesskit::Action::Increment);
        assert_value(&d, 0.0, "value");
        assert!(d.take().is_empty());
        assert!(d.state.contains(StateBits::DISABLED) && !d.state.contains(StateBits::PRESSED));
    }

    #[test]
    fn out_of_range_values_are_clamped_without_a_signal() {
        let mut d = driver();
        d.set("value", Value::F64(250.0));
        d.hover(true);
        assert_value(&d, 100.0, "value");
        assert_eq!(d.changed, ["value"]);
        assert!(d.take().is_empty());
    }

    #[test]
    fn a11y_node() {
        let mut k = Slider::default();
        k.props.text = "Gain".into();
        let n = testkit::a11y(&k, StateBits::NONE, SIZE);
        assert_eq!(n.role(), accesskit::Role::Slider);
        assert_eq!(n.label(), Some("Gain"));
        assert_eq!((n.numeric_value(), n.min_numeric_value(), n.max_numeric_value()), (Some(0.0), Some(0.0), Some(100.0)));
        assert!(n.supports_action(accesskit::Action::Increment) && !n.supports_action(accesskit::Action::Decrement), "at min");
        assert!(n.supports_action(accesskit::Action::SetValue) && n.supports_action(accesskit::Action::Focus));
        k.props.value = 50.0;
        k.props.orientation = "vertical".into();
        let n = testkit::a11y(&k, StateBits::NONE, SIZE);
        assert!(n.supports_action(accesskit::Action::Decrement));
        assert_eq!(n.orientation(), Some(accesskit::Orientation::Vertical));
    }

    #[test]
    fn a_prevented_changed_keeps_the_value() {
        let mut d = driver();
        d.set("step", Value::F64(1.0));
        d.prevent = vec!["changed"];
        d.focus(true);
        d.key(Key::ArrowRight);
        assert!(d.names().contains(&"changed") && !d.names().contains(&"input"));
        assert_value(&d, 0.0, "value");
    }
}
