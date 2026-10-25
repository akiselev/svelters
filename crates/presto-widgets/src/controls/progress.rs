//! `progress-bar` (ARCH-WIDGETS §6.1, ported from svelters-egui `widgets/progress.rs`): box, text;
//! no units.
//!
//! Paint parts `track`, `fill` and `label` (WIDGET-PARITY §2.9). VS Code's progress is a thin bar
//! in `progress` (`progressBar.background`): the track is 4 px tall, centred, unless the bar shows
//! text (`text`, `format` or `show-percentage`), when it takes the node's height and the label is
//! centred on it. Busy (`animate`, `indeterminate`, or a `value` with `min == max`) is `:busy` and
//! `:indeterminate` and a sweep: a 30 % segment crossing the track every 1.5 s ([`sweep`]). The
//! recording is the sweep's frame at t = 0 (the source's reduced-motion frame, the segment not yet
//! on the track); running it is a compositor animation (lane MOTION-CSS).

use crate::kind_props;
use crate::paint::{self, Align, PaintCx, StyleView, Ua, UaRule, UaValue::*};
use crate::shim::core::{A11yCx, Event, EventCx, Handled, Kind, KindSpec, MeasureCx, NameFrom, PropError, PropId, RecordCx, Styles, Value};
use presto_base::accesskit;
use presto_base::color::{Color, Fill};
use presto_base::display::{BorderStyle, Prim, Quad};
use presto_base::geom::{Corners, Insets, Rect, Vec2};

pub const UA: Ua = Ua(&[UaRule::base(&[("text-align", Keyword("center")), ("width", Px(100.0))])]);

/// `::part(track)`.
pub const TRACK: Ua = Ua(&[UaRule::base(&[("background-color", Token("control-bg")), ("border-radius", Px(2.0)), ("height", Px(4.0))])]);

/// `::part(fill)`.
pub const FILL: Ua = Ua(&[UaRule::base(&[("background-color", Token("progress"))])]);

/// The sweep's period in seconds (`--sv-busy-period`).
pub const BUSY_PERIOD: f64 = 1.5;

kind_props! {
    /// The progress bar's props.
    pub struct ProgressProps;
    /// Done, from 0 to 1.
    fraction "fraction": f64 = F64(0.0) ; F64, Paint;
    /// Text on the bar.
    text "text": String = Str("") ; Str, Layout;
    /// Shows the percentage (when `text` is empty).
    show_percentage "show-percentage": bool = Bool(false) ; Bool, Layout;
    /// Busy: progress without a known fraction (a sweep).
    animate "animate": bool = Bool(false) ; Bool, Paint;
    /// WIDGET-PARITY §2.9: the value in `min..max`, the alternative to `fraction` (Qt's form).
    value "value": Option<f64> = None ; F64, Layout;
    min "min": f64 = F64(0.0) ; F64, Layout;
    max "max": f64 = F64(100.0) ; F64, Layout;
    /// Busy, as `animate` (and `:indeterminate`); also when `value` is set and `min == max`.
    indeterminate "indeterminate": bool = Bool(false) ; Bool, Paint;
    /// The bar's axis (`[orientation="vertical"]` styles it).
    orientation "orientation": String = Enum("horizontal") ; Enum(&["horizontal", "vertical"]), Layout;
    /// Fills from the end (right, or top when vertical) instead.
    inverted "inverted": bool = Bool(false) ; Bool, Paint;
    /// The label's text when `text` is empty: `%p` percent, `%v` value, `%m` maximum (`%%` a
    /// percent sign); shown when set or with `show-percentage` (`%p%`).
    format "format": String = Str("") ; Str, Layout;
    disabled "disabled": bool = Bool(false) ; Bool, Paint;
}

pub static SPEC: KindSpec = KindSpec { tag: "progress-bar", props: ProgressProps::SPECS, signals: &[], slots: &[], children: false, role: accesskit::Role::ProgressIndicator, name_from: NameFrom::None, text_prop: None, focusable: false };

/// Qt's `QProgressBar::format`: `%p` percent, `%v` value, `%m` maximum, `%%` a percent sign.
pub fn format_progress(fmt: &str, fraction: f32, value: f64, max: f64) -> String {
    let num = |v: f64| if v.fract() == 0.0 { format!("{}", v as i64) } else { format!("{v}") };
    let mut out = String::new();
    let mut it = fmt.chars().peekable();
    while let Some(c) = it.next() {
        match (c, it.peek().copied()) {
            ('%', Some('p')) => out.push_str(&format!("{}", (fraction * 100.0).round() as i64)),
            ('%', Some('v')) => out.push_str(&num(value)),
            ('%', Some('m')) => out.push_str(&num(max)),
            ('%', Some('%')) => out.push('%'),
            _ => {
                out.push(c);
                continue;
            }
        }
        it.next();
    }
    out
}

/// The busy sweep's span along the track (0..1 from where the fill starts) at `t` seconds: a 30 %
/// segment entering at the start and leaving at the end once per [`BUSY_PERIOD`].
pub fn sweep(t: f64) -> (f32, f32) {
    let t = (t / BUSY_PERIOD).fract() as f32;
    let x = -0.3 + t * 1.3;
    (x.max(0.0), (x + 0.3).min(1.0))
}

/// A progress bar (horizontal, or vertical with `orientation`).
#[derive(Debug, Default)]
pub struct ProgressBar {
    pub props: ProgressProps,
}

impl ProgressBar {
    fn fraction(&self) -> f32 {
        let p = &self.props;
        let f = match p.value {
            Some(v) if p.max > p.min => (v - p.min) / (p.max - p.min),
            Some(_) => 0.0,
            None => p.fraction,
        };
        if f.is_finite() { f.clamp(0.0, 1.0) as f32 } else { 0.0 }
    }

    /// Busy: `animate`, `indeterminate`, or Qt's `minimum == maximum` with a value.
    pub fn busy(&self) -> bool {
        let p = &self.props;
        p.animate || p.indeterminate || (p.value.is_some() && p.min == p.max)
    }

    fn vertical(&self) -> bool {
        self.props.orientation == "vertical"
    }

    fn label(&self) -> Option<String> {
        let p = &self.props;
        if !p.text.is_empty() {
            Some(p.text.clone())
        } else if !p.format.is_empty() || p.show_percentage {
            let fmt = if p.format.is_empty() { "%p%" } else { p.format.as_str() };
            Some(format_progress(fmt, self.fraction(), p.value.unwrap_or(f64::from(self.fraction()) * 100.0), p.max))
        } else {
            None
        }
    }

    /// The node's state with the prop mirrors (`:busy`, `:indeterminate`, `:disabled`).
    fn mirror(&self, state: &mut crate::interact::StateBits) {
        use crate::interact::StateBits;
        let busy = self.busy();
        state.set(StateBits::BUSY, busy);
        state.set(StateBits::INDETERMINATE, busy);
        state.set(StateBits::DISABLED, self.props.disabled);
    }

    fn view<'a>(&self, s: &Styles<'a>) -> StyleView<'a> {
        let mut state = s.state;
        self.mirror(&mut state);
        StyleView::new(s.computed, UA, state, s.dark)
    }
}

/// A filled quad (the fill part).
fn fill_quad(r: Rect, radius: f32, color: Color) -> Prim {
    Prim::Quad(Quad { rect: r, radii: Corners::all(radius), fill: Fill::Solid(color), border_widths: Insets::ZERO, border_color: Color::TRANSPARENT, border_style: BorderStyle::Solid })
}

impl Kind for ProgressBar {
    fn parts() -> &'static [&'static str] {
        &["track", "fill", "label"]
    }
    fn prop_state(&self) -> crate::interact::StateBits {
        { let mut s = crate::interact::StateBits::NONE; self.mirror(&mut s); s }
    }
    fn prop_states() -> crate::interact::StateBits {
        crate::interact::StateBits(crate::interact::StateBits::BUSY.0 | crate::interact::StateBits::INDETERMINATE.0)
    }
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
        let s = self.view(&cx.style);
        let track = s.part(cx.style.part_css("track"), TRACK);
        let w = s.px("width").unwrap_or(100.0);
        let th = track.px("height").unwrap_or(4.0);
        let along = match self.label() {
            Some(t) => {
                let t = paint::measure_text(cx.text, &s, &t, None);
                Vec2::new(w.max(t.w + 8.0), (t.h + 4.0).max(th))
            }
            None => Vec2::new(w, th),
        };
        let size = if self.vertical() { Vec2::new(along.y, along.x) } else { along };
        taffy::Size { width: size.x, height: size.y }
    }

    fn record(&self, cx: &mut RecordCx) {
        let host = self.view(&cx.style);
        let track = host.part(cx.style.part_css("track"), TRACK);
        let fill = host.part(cx.style.part_css("fill"), FILL);
        let r = cx.rect();
        let label = self.label();
        let (vertical, inverted) = (self.vertical(), self.props.inverted);
        let cross = if vertical { r.width() } else { r.height() };
        let th = if label.is_some() { cross } else { track.px("height").unwrap_or(4.0).min(cross) };
        let (mx, my) = ((r.min.x + r.max.x) / 2.0, (r.min.y + r.max.y) / 2.0);
        let tr = if vertical { Rect::new(mx - th / 2.0, r.min.y, mx + th / 2.0, r.max.y) } else { Rect::new(r.min.x, my - th / 2.0, r.max.x, my + th / 2.0) };
        let mut pc = PaintCx::of(cx);
        paint::paint_box(&mut pc, &track, tr);
        let color = fill.color("background-color").unwrap_or(Color::TRANSPARENT);
        let radius = track.px("border-radius").unwrap_or(0.0);
        // the fill's span along the bar, 0..1 from where it starts (left, or bottom when vertical)
        let (a, b) = if self.busy() { sweep(0.0) } else { (0.0, self.fraction()) };
        let (a, b) = if inverted { (1.0 - b, 1.0 - a) } else { (a, b) };
        let fr = if vertical { Rect::new(tr.min.x, tr.max.y - b * tr.height(), tr.max.x, tr.max.y - a * tr.height()) } else { Rect::new(tr.min.x + a * tr.width(), tr.min.y, tr.min.x + b * tr.width(), tr.max.y) };
        if !fr.is_empty() && color.a > 0.0 {
            pc.push(fill_quad(fr, radius, color), fr);
        }
        if let Some(t) = &label {
            let l = paint::text_layout(pc.text, &host, t, None);
            paint::paint_text(&mut pc, &host, &l, r, Align::of(&host, Align::Center));
        }
    }

    fn event(&mut self, cx: &mut EventCx, _ev: &Event) -> Handled {
        self.mirror(cx.state);
        Handled::No
    }

    fn a11y(&self, cx: &mut A11yCx) {
        let n = &mut *cx.node;
        n.set_role(accesskit::Role::ProgressIndicator);
        let p = &self.props;
        if self.busy() {
            n.set_busy();
        } else if let Some(v) = p.value {
            n.set_numeric_value(v);
            n.set_min_numeric_value(p.min);
            n.set_max_numeric_value(p.max);
        } else {
            n.set_numeric_value(f64::from(self.fraction()));
            n.set_min_numeric_value(0.0);
            n.set_max_numeric_value(1.0);
        }
        if let Some(t) = self.label() {
            n.set_value(t);
        }
        if self.vertical() {
            n.set_orientation(accesskit::Orientation::Vertical);
        }
        if p.disabled {
            n.set_disabled();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controls::testkit::{self, Driver};
    use crate::interact::StateBits;
    use presto_base::change::ChangeClass;
    use presto_base::geom::Size;

    fn bar(f: f64) -> ProgressBar {
        let mut b = ProgressBar::default();
        b.props.fraction = f;
        b
    }

    #[test]
    fn spec() {
        let s = ProgressBar::spec();
        assert_eq!((s.tag, s.role), ("progress-bar", accesskit::Role::ProgressIndicator));
        assert_eq!(s.prop_spec(s.prop("fraction").unwrap()).unwrap().change, ChangeClass::Paint);
        assert_eq!(s.prop_spec(s.prop("value").unwrap()).unwrap().change, ChangeClass::Layout);
        assert_eq!(s.prop_spec(s.prop("max").unwrap()).unwrap().default, crate::shim::core::Default::F64(100.0));
        let mut b = ProgressBar::default();
        let v = s.prop("value").unwrap();
        b.set_prop(v, &Value::F64(3.0)).unwrap();
        assert_eq!(b.props.value, Some(3.0));
        b.set_prop(v, &Value::List(Vec::new())).unwrap();
        assert_eq!(b.props.value, None, "unset");
    }

    #[test]
    fn qt_format() {
        assert_eq!(format_progress("%p%", 0.5, 50.0, 100.0), "50%");
        assert_eq!(format_progress("%v of %m", 0.3, 3.0, 10.0), "3 of 10");
        assert_eq!(format_progress("%v", 0.25, 2.5, 10.0), "2.5");
        assert_eq!(format_progress("100%% %x", 1.0, 0.0, 0.0), "100% %x");
    }

    #[test]
    fn the_sweep_crosses_the_track() {
        assert_eq!(sweep(0.0), (0.0, 0.0), "t = 0: not yet on the track");
        let (a, b) = sweep(0.75);
        assert!((a - 0.35).abs() < 1e-5 && (b - 0.65).abs() < 1e-5);
        assert_eq!(sweep(1.5), sweep(0.0), "once per period");
    }

    #[test]
    fn measures_a_thin_track_or_a_text_bar() {
        assert_eq!(testkit::measure(&bar(0.5), StateBits::NONE), Size::new(100.0, 4.0));
        let mut v = bar(0.5);
        v.props.orientation = "vertical".into();
        assert_eq!(testkit::measure(&v, StateBits::NONE), Size::new(4.0, 100.0));
        let mut t = bar(0.5);
        t.props.show_percentage = true;
        assert!(testkit::measure(&t, StateBits::NONE).h > 4.0);
    }

    #[test]
    fn records_track_and_fill() {
        let size = Size::new(100.0, 10.0);
        let (list, _) = testkit::record(&bar(0.5), size, StateBits::NONE);
        assert_eq!(testkit::kinds(&list), ["quad", "quad"]);
        let (Prim::Quad(track), Prim::Quad(fill)) = (&list.items[0], &list.items[1]) else { unreachable!() };
        assert_eq!(track.rect, Rect::new(0.0, 3.0, 100.0, 7.0), "4px tall, centred");
        assert_eq!(track.radii, Corners::all(2.0));
        assert_eq!(fill.rect, Rect::new(0.0, 3.0, 50.0, 7.0));
        assert_eq!(fill.fill, Fill::Solid(paint::ua_token(true, "progress").unwrap()));
        // inverted fills from the end; vertical from the bottom
        let mut b = bar(0.25);
        b.props.inverted = true;
        let (list, _) = testkit::record(&b, size, StateBits::NONE);
        let Prim::Quad(fill) = &list.items[1] else { unreachable!() };
        assert_eq!((fill.rect.min.x, fill.rect.max.x), (75.0, 100.0));
        let mut b = bar(0.25);
        b.props.orientation = "vertical".into();
        let (list, _) = testkit::record(&b, Size::new(10.0, 100.0), StateBits::NONE);
        let Prim::Quad(fill) = &list.items[1] else { unreachable!() };
        assert_eq!(fill.rect, Rect::new(3.0, 75.0, 7.0, 100.0));
        // value in min..max
        let mut b = ProgressBar::default();
        b.props.value = Some(30.0);
        b.props.max = 60.0;
        let (list, _) = testkit::record(&b, size, StateBits::NONE);
        let Prim::Quad(fill) = &list.items[1] else { unreachable!() };
        assert_eq!(fill.rect.max.x, 50.0);
    }

    #[test]
    fn a_text_bar_takes_the_height_and_centres_its_label() {
        let mut b = bar(0.5);
        b.props.format = "%p%".into();
        let (list, res) = testkit::record(&b, Size::new(100.0, 20.0), StateBits::NONE);
        assert_eq!(testkit::kinds(&list), ["quad", "quad", "glyphs"]);
        let Prim::Quad(track) = &list.items[0] else { unreachable!() };
        assert_eq!(track.rect, Rect::new(0.0, 0.0, 100.0, 20.0));
        let ids: String = res.runs[0].glyphs.iter().map(|g| char::from_u32(g.id).unwrap()).collect();
        assert_eq!(ids, "50%");
    }

    #[test]
    fn busy_records_the_static_frame_and_mirrors_its_state() {
        let mut b = bar(0.5);
        b.props.indeterminate = true;
        let (list, _) = testkit::record(&b, Size::new(100.0, 10.0), StateBits::NONE);
        assert_eq!(testkit::kinds(&list), ["quad"], "the track; the sweep is not on it at t = 0");
        let mut d = Driver::new(b, Size::new(100.0, 10.0));
        assert_eq!(d.send(Event::Hover(true)), Handled::No);
        assert!(d.state.contains(StateBits::BUSY | StateBits::INDETERMINATE));
        d.set("indeterminate", Value::Bool(false));
        d.send(Event::Hover(false));
        assert!(!d.state.intersects(StateBits::BUSY | StateBits::INDETERMINATE));
        // Qt: min == max with a value is busy
        let mut q = ProgressBar::default();
        q.props.value = Some(1.0);
        q.props.max = 0.0;
        assert!(q.busy());
    }

    #[test]
    fn a11y_numeric_value() {
        let n = testkit::a11y(&bar(0.25), StateBits::NONE, Size::new(100.0, 4.0));
        assert_eq!(n.role(), accesskit::Role::ProgressIndicator);
        assert_eq!((n.numeric_value(), n.min_numeric_value(), n.max_numeric_value()), (Some(0.25), Some(0.0), Some(1.0)));
        let mut b = ProgressBar::default();
        b.props.value = Some(30.0);
        b.props.max = 60.0;
        let n = testkit::a11y(&b, StateBits::NONE, Size::new(100.0, 4.0));
        assert_eq!((n.numeric_value(), n.max_numeric_value()), (Some(30.0), Some(60.0)));
        b.props.animate = true;
        let n = testkit::a11y(&b, StateBits::NONE, Size::new(100.0, 4.0));
        assert!(n.is_busy() && n.numeric_value().is_none());
    }
}
