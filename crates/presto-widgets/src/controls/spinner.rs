//! `spinner` (ARCH-WIDGETS §6.1, ported from the spinner of svelters-egui `widgets/progress.rs`): a
//! busy indicator, a 270° arc in `progress` turning once a second while `spinning` (`:busy`).
//!
//! The arc is the source's polyline: 24 segments ([`arc`]) of a stroke an eighth of the side wide,
//! recorded as round-capped lines. The recording is the frame at t = 0 (the source's
//! reduced-motion frame, the arc starting at 3 o'clock); turning it is a compositor rotation
//! (lane MOTION-CSS), so a turning spinner never re-records.

use crate::kind_props;
use crate::paint::{PaintCx, StyleView, Ua, UaRule, UaValue::*};
use crate::shim::core::{A11yCx, Event, EventCx, Handled, Kind, KindSpec, MeasureCx, NameFrom, PropError, PropId, RecordCx, Styles, Value};
use presto_base::accesskit;
use presto_base::color::Color;
use presto_base::display::{Line, LineCap, Prim};
use presto_base::geom::{Point, Rect};

/// The spinner's UA: the arc's colour and default size.
pub const UA: Ua = Ua(&[UaRule::base(&[("color", Token("progress")), ("width", Px(16.0))])]);

/// The arc's segments.
pub const SEGMENTS: usize = 24;

kind_props! {
    /// The spinner's props.
    pub struct SpinnerProps;
    /// Side in points (default: CSS `width`, else 16); not a positive finite number is no size.
    size "size": Option<f64> = None ; F64, Layout;
    /// Turns (and is `:busy`); a stopped spinner keeps its space and draws nothing.
    spinning "spinning": bool = Bool(true) ; Bool, Paint;
}

pub static SPEC: KindSpec = KindSpec { tag: "spinner", props: SpinnerProps::SPECS, signals: &[], slots: &[], children: false, role: accesskit::Role::ProgressIndicator, name_from: NameFrom::None, text_prop: None, focusable: false };

/// The arc's points in a `d`-sided square centred at `c`, with a stroke `w` wide, starting at
/// `start` radians (0: 3 o'clock, clockwise in y-down coordinates) and spanning 1.5π.
pub fn arc(c: Point, d: f32, w: f32, start: f32) -> Vec<Point> {
    let radius = (d - w) / 2.0;
    (0..=SEGMENTS)
        .map(|k| {
            let a = start + 1.5 * std::f32::consts::PI * k as f32 / SEGMENTS as f32;
            Point::new(c.x + radius * a.cos(), c.y + radius * a.sin())
        })
        .collect()
}

/// Paints the arc at t = 0 in `r` under `s`.
pub fn paint(pc: &mut PaintCx, s: &StyleView, r: Rect) {
    let d = r.width().min(r.height());
    let color = s.color("color").unwrap_or(Color::TRANSPARENT);
    if d <= 0.0 || color.a <= 0.0 {
        return;
    }
    let w = pc.snap_width((d / 8.0).max(1.0));
    let c = Point::new((r.min.x + r.max.x) / 2.0, (r.min.y + r.max.y) / 2.0);
    let pts = arc(c, d, w, 0.0);
    for seg in pts.windows(2) {
        let (a, b) = (seg[0], seg[1]);
        let ink = Rect::new(a.x.min(b.x), a.y.min(b.y), a.x.max(b.x), a.y.max(b.y)).inflate(w / 2.0);
        pc.push(Prim::Line(Line { from: a, to: b, width: w, color, cap: LineCap::Round, dash: None, wavy: false }), ink);
    }
}

/// A busy indicator.
#[derive(Debug, Default)]
pub struct Spinner {
    pub props: SpinnerProps,
}

impl Spinner {
    fn view<'a>(&self, s: &Styles<'a>) -> StyleView<'a> {
        let mut state = s.state;
        state.set(crate::interact::StateBits::BUSY, self.props.spinning);
        StyleView::new(s.computed, UA, state, s.dark)
    }
}

impl Kind for Spinner {
    fn prop_state(&self) -> crate::interact::StateBits {
        { let mut s = crate::interact::StateBits::NONE; s.set(crate::interact::StateBits::BUSY, self.props.spinning); s }
    }
    fn prop_states() -> crate::interact::StateBits {
        crate::interact::StateBits::BUSY
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
        let d = self.props.size.filter(|s| s.is_finite() && *s > 0.0).map(|s| s as f32).or_else(|| self.view(&cx.style).px("width")).unwrap_or(16.0);
        taffy::Size { width: d, height: d }
    }

    fn record(&self, cx: &mut RecordCx) {
        if !self.props.spinning {
            return;
        }
        let s = self.view(&cx.style);
        let r = cx.rect();
        paint(&mut PaintCx::of(cx), &s, r);
    }

    fn event(&mut self, cx: &mut EventCx, _ev: &Event) -> Handled {
        cx.state.set(crate::interact::StateBits::BUSY, self.props.spinning);
        Handled::No
    }

    fn a11y(&self, cx: &mut A11yCx) {
        cx.node.set_role(accesskit::Role::ProgressIndicator);
        if self.props.spinning {
            cx.node.set_busy();
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

    #[test]
    fn spec() {
        let s = Spinner::spec();
        assert_eq!((s.tag, s.role), ("spinner", accesskit::Role::ProgressIndicator));
        assert_eq!(s.prop_spec(s.prop("size").unwrap()).unwrap().change, ChangeClass::Layout);
        assert_eq!(s.prop_spec(s.prop("spinning").unwrap()).unwrap().default, crate::shim::core::Default::Bool(true));
    }

    #[test]
    fn measures_its_side() {
        let mut s = Spinner::default();
        assert_eq!(testkit::measure(&s, StateBits::NONE), Size::new(16.0, 16.0));
        s.props.size = Some(24.0);
        assert_eq!(testkit::measure(&s, StateBits::NONE), Size::new(24.0, 24.0));
        s.props.size = Some(-1.0);
        assert_eq!(testkit::measure(&s, StateBits::NONE), Size::new(16.0, 16.0), "not a size");
    }

    #[test]
    fn records_the_arc_as_segments() {
        let s = Spinner::default();
        let (list, _) = testkit::record(&s, Size::new(16.0, 16.0), StateBits::NONE);
        assert_eq!(list.items.len(), SEGMENTS);
        assert!(testkit::kinds(&list).iter().all(|k| *k == "line"));
        let (Prim::Line(first), Prim::Line(last)) = (&list.items[0], &list.items[SEGMENTS - 1]) else { unreachable!() };
        // side 16: a 2pt stroke on radius 7, from 3 o'clock round to 12 o'clock
        assert_eq!((first.width, first.color), (2.0, crate::paint::ua_token(true, "progress").unwrap()));
        assert_eq!(first.from, Point::new(15.0, 8.0));
        assert!((last.to.x - 8.0).abs() < 1e-4 && (last.to.y - 1.0).abs() < 1e-4, "{:?}", last.to);
        let mut stopped = Spinner::default();
        stopped.props.spinning = false;
        assert!(testkit::record(&stopped, Size::new(16.0, 16.0), StateBits::NONE).0.is_empty());
    }

    #[test]
    fn busy_while_spinning() {
        let mut d = Driver::new(Spinner::default(), Size::new(16.0, 16.0));
        d.send(Event::Hover(true));
        assert!(d.state.contains(StateBits::BUSY));
        d.set("spinning", Value::Bool(false));
        d.send(Event::Hover(false));
        assert!(!d.state.contains(StateBits::BUSY));
        assert!(d.signals.is_empty());
        let n = testkit::a11y(&Spinner::default(), StateBits::NONE, Size::new(16.0, 16.0));
        assert_eq!(n.role(), accesskit::Role::ProgressIndicator);
        assert!(n.is_busy());
    }
}
