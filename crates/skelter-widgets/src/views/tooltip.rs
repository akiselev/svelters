//! Tooltips: when a host's tooltip shows and hides ([`TooltipTimer`]) and the `tooltip` surface kind
//! ([`Tooltip`]).
//!
//! Timing (svelters-egui `interact/tooltip.rs`): the tip shows after the pointer rested on its host
//! for the platform's delay ([`Policy::tooltip_delay_ms`], or the host's own), at once while another
//! tip hid within the follow-on window ([`Policy::tooltip_follow_on_ms`], Qt's fall-asleep timer, kept
//! per window in [`TooltipWarmth`]); also on a disabled host. A press, a scroll, Escape or the pointer
//! leaving hides it, and after a press, scroll or Escape it stays hidden until the pointer leaves;
//! `duration` hides it after a time.
//!
//! The surface goes above its host, centred (`top-center`), flipping below when there is no room
//! (svelters-egui `widgets/tooltip.rs`); it is a click-through overlay entry. A `tooltip` element is
//! in the window's top layer (`position: fixed`), placed so next to its owner.
//!
//! [`TitleTooltips`] shows it for the common `title` prop (SV61): skelter-core's
//! [`TitleTips`](skelter_core::tips::TitleTips) seam reports the hovered host and the input, and each
//! step runs [`Tooltip::step`] with the document's [`Policy`] as it is then (`host::set_policy`
//! applies to the next hover), drawing the surface in the window's tooltip layer.

use super::common::ms;
use super::placement::{AnchorParams, Placed, Placement, place};
use crate::controls::common::host_view;
use crate::interact::Policy;
use crate::kind_props;
use crate::paint::{self, Align, PaintCx, StyleView, Ua, UaRule, UaValue::*};
use crate::shim::core::{A11yCx, Event, EventCx, Handled, Kind, KindSpec, MeasureCx, NameFrom, PropError, PropId, RecordCx, StyleCx, TopLayerCx, Value};
use crate::shim::text::TextShaper;
use skelter_base::accesskit;
use skelter_base::geom::{Rect, Size};
use skelter_base::time::{Span, Time};

/// What a host does this step, as the timer sees it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TipInput {
    /// The pointer is over the host (also when the host is disabled).
    pub over: bool,
    /// A pointer button went down.
    pub pressed: bool,
    /// A wheel or touchpad scroll arrived.
    pub scrolled: bool,
    /// Escape was pressed.
    pub escape: bool,
}

/// What [`TooltipTimer::step`] reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tip {
    Show,
    Hide,
}

/// When another tooltip in the same window hid last (one per window).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TooltipWarmth {
    last_hidden: Option<Time>,
}

impl TooltipWarmth {
    /// As if a tip hid at `t`.
    pub fn hidden_at(t: Time) -> TooltipWarmth {
        TooltipWarmth { last_hidden: Some(t) }
    }
    /// Another tip hid within `follow_on` of `now`.
    pub fn warm(&self, now: Time, follow_on: Span) -> bool {
        self.last_hidden.is_some_and(|t| now - t <= follow_on)
    }
}

/// One host's tooltip timing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TooltipTimer {
    /// When the pointer came to rest on the host.
    since: Option<Time>,
    shown: Option<Time>,
    /// Hidden by a press, scroll or Escape: not again until the pointer leaves.
    suppressed: bool,
    /// The next time the timer must step without input.
    wake: Option<Time>,
}

impl TooltipTimer {
    pub fn shown(&self) -> bool {
        self.shown.is_some()
    }
    /// When to step again without input.
    pub fn wake(&self) -> Option<Time> {
        self.wake
    }

    /// One step at `now`. `delay` and `duration` (zero: while hovered) come from the host or the
    /// policy.
    pub fn step(&mut self, input: TipInput, now: Time, delay: Span, duration: Span, follow_on: Span, warmth: &mut TooltipWarmth) -> Option<Tip> {
        self.wake = None;
        let was = self.shown.is_some();
        if !input.over {
            self.since = None;
            self.suppressed = false;
        } else if input.pressed || input.scrolled || input.escape {
            self.suppressed = true;
        } else if self.since.is_none() {
            self.since = Some(now);
        }
        let mut show = false;
        if let Some(since) = self.since
            && !self.suppressed
        {
            let due = if was || warmth.warm(since, follow_on) { since } else { since + delay };
            if now >= due {
                show = true;
            } else {
                self.wake = Some(due);
            }
        }
        if show
            && let Some(at) = self.shown
            && duration > Span::ZERO
        {
            if now >= at + duration {
                show = false;
                self.suppressed = true;
            } else {
                self.wake = Some(at + duration);
            }
        }
        match (was, show) {
            (false, true) => {
                self.shown = Some(now);
                if duration > Span::ZERO {
                    self.wake = Some(now + duration);
                }
                Some(Tip::Show)
            }
            (true, false) => {
                self.shown = None;
                warmth.last_hidden = Some(now);
                Some(Tip::Hide)
            }
            _ => None,
        }
    }
}

/// One window's `title` tooltip: the hovered host, its timer and the follow-on warmth.
#[derive(Default)]
struct WindowTip {
    host: Option<skelter_base::ids::NodeId>,
    tip: Tooltip,
    warmth: TooltipWarmth,
    /// What the window's tip layer shows: its text and its host's window rect.
    drawn: Option<(String, Rect)>,
}

/// The `title` tooltip provider ([`skelter_core::Document::set_title_tips`]); `host::install`
/// installs it.
#[derive(Default)]
pub struct TitleTooltips {
    windows: std::collections::HashMap<skelter_base::ids::WindowId, WindowTip>,
}

impl skelter_core::tips::TitleTips for TitleTooltips {
    fn step(&mut self, doc: &mut skelter_core::Document, window: skelter_base::ids::WindowId, input: &skelter_core::tips::TitleInput, now: Time) -> Option<Time> {
        // read on use: a policy set since the last step applies now (SV62)
        let policy = crate::host::policy_of(doc);
        let w = self.windows.entry(window).or_default();
        let host = input.host.as_ref().map(|h| h.0);
        if w.host != host {
            // the pointer left the old host: its tip hides (and warms the next one)
            if w.tip.step(TipInput::default(), now, &policy, &mut w.warmth) == Some(Tip::Hide) || w.drawn.is_some() {
                w.drawn = None;
                doc.set_window_tip(window, None);
            }
            w.tip = Tooltip::default();
            w.host = host;
        }
        let Some((node, text)) = &input.host else { return None };
        w.tip.props.text = text.to_string();
        let tin = TipInput { over: true, pressed: input.pressed, scrolled: input.scrolled, escape: input.escape };
        match w.tip.step(tin, now, &policy, &mut w.warmth) {
            Some(Tip::Hide) => {
                w.drawn = None;
                doc.set_window_tip(window, None);
            }
            _ if w.tip.timer.shown() => {
                let anchor = doc.world_rect(*node).unwrap_or(Rect::ZERO);
                let key = (w.tip.props.text.clone(), anchor);
                if w.drawn.as_ref() != Some(&key) {
                    w.drawn = Some(key);
                    let root = doc.window_root(window);
                    let bounds = root.and_then(|r| doc.world_rect(r)).unwrap_or(anchor);
                    let scale = root.and_then(|r| doc.view(r)).map_or(1.0, |v| v.scale());
                    let mut tip = w.tip.clone();
                    tip.room = Some(bounds.width());
                    let (size, mut list, runs) = crate::host::record_tooltip(doc, &tip, scale);
                    let rect = tip.place(anchor, bounds, size).rect;
                    list.translate(skelter_base::geom::Vec2::new(rect.min.x, rect.min.y));
                    doc.set_window_tip(window, Some((list, runs)));
                }
            }
            _ => {}
        }
        self.windows.get(&window).and_then(|w| w.tip.timer.wake())
    }
}

/// The surface's UA sheet: the tooltip fill, the widget border, 8/4 padding, `popup-shadow`.
pub const UA: Ua = Ua(&[UaRule::base(&[
    ("background-color", Token("tooltip-bg")),
    ("border-color", Token("border-widget")),
    ("border-width", Token("stroke")),
    ("border-radius", Token("radius")),
    ("box-shadow", Token("popup-shadow")),
    ("padding-left", Px(8.0)),
    ("padding-right", Px(8.0)),
    ("padding-top", Px(4.0)),
    ("padding-bottom", Px(4.0)),
    ("font-size", Token("font-size-sm")),
])]);

kind_props! {
    /// The tooltip's props.
    pub struct TooltipProps;
    text "text": String = Str("") ; Str, Layout;
    placement "placement": String = Enum("top-center") ; Enum(super::popup::PLACEMENTS), Property;
    /// Seconds before it shows; unset: the platform's delay.
    delay "delay": Option<f64> = None ; F64, Property;
    /// Seconds it stays; zero: while hovered.
    duration "duration": f64 = F64(0.0) ; F64, Property;
}

pub static SPEC: KindSpec = KindSpec {
    tag: "tooltip",
    props: TooltipProps::SPECS,
    signals: &[],
    slots: &[],
    children: false,
    role: accesskit::Role::Tooltip,
    name_from: NameFrom::Prop("text"),
    text_prop: Some("text"),
    focusable: false,
};

/// The widest a tooltip's text runs before it wraps.
pub const MAX_WIDTH: f32 = 360.0;
/// What a tooltip keeps clear of the window's edges when its text wraps to fit.
pub const EDGE: f32 = 8.0;
/// The gap to the host.
const GAP: f32 = 6.0;

/// A `tooltip` surface.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tooltip {
    pub props: TooltipProps,
    pub timer: TooltipTimer,
    /// The width it may take (the window's): its text wraps short of [`MAX_WIDTH`] to stay
    /// [`EDGE`] inside it. `None`: [`MAX_WIDTH`].
    pub room: Option<f32>,
    /// Its direction is `rtl` (at the last restyle): `placement`'s `-start`/`-end` are logical.
    pub rtl: std::cell::Cell<bool>,
}

impl Tooltip {
    /// A tooltip showing `text`, the rest default.
    pub fn with_text(text: &str) -> Tooltip {
        let mut t = Tooltip::default();
        t.props.text = text.to_owned();
        t
    }

    pub fn placement(&self) -> Placement {
        Placement::parse(&self.props.placement).unwrap_or(Placement::TOP_CENTER)
    }
    /// Zero: while hovered.
    pub fn duration(&self) -> Span {
        Span::from_secs_f64(self.props.duration.max(0.0))
    }

    /// Steps the timer for the host's input with the policy's timings (the tooltip's own `delay` first).
    pub fn step(&mut self, input: TipInput, now: Time, policy: &Policy, warmth: &mut TooltipWarmth) -> Option<Tip> {
        let input = if self.props.text.is_empty() { TipInput { over: false, ..input } } else { input };
        let delay = self.props.delay.map_or(ms(policy.tooltip_delay_ms), |s| Span::from_secs_f64(s.max(0.0)));
        let duration = self.duration();
        self.timer.step(input, now, delay, duration, ms(policy.tooltip_follow_on_ms), warmth)
    }

    /// The surface's size for its text under `s` (its style view): the text wrapped at [`MAX_WIDTH`]
    /// plus padding and border.
    pub fn size(&self, shaper: &mut dyn TextShaper, s: &StyleView) -> Size {
        let e = paint::box_extra(s);
        // a narrow window wraps it sooner, so it never runs off the window's side
        let wrap = self.room.map_or(MAX_WIDTH, |r| MAX_WIDTH.min(r - 2.0 * EDGE - e.x)).max(e.x.max(40.0));
        let t = paint::measure_text(shaper, s, &self.props.text, Some(wrap));
        Size::new(t.w + e.x, t.h + e.y)
    }

    /// Where it goes for a host at `host` (window points) inside `bounds`.
    pub fn place(&self, host: Rect, bounds: Rect, size: Size) -> Placed {
        // `rtl`: `-start` and `-end` are logical (start is the host's right edge)
        let mut placement = self.placement();
        if self.rtl.get() && placement.side.vertical() {
            use super::placement::Align as A;
            placement.align = match placement.align {
                A::Start => A::End,
                A::End => A::Start,
                a => a,
            };
        }
        place(&AnchorParams { anchor: host, size, bounds, placement, gap: GAP, flip: true, shift: true, ..AnchorParams::default() })
    }
}

impl Kind for Tooltip {
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

    /// In the window's top layer, out of its owner's flow and clip.
    fn layout_style(&self, cx: &StyleCx) -> taffy::Style {
        self.rtl.set(cx.is_rtl());
        let mut s = cx.default_layout();
        s.position = taffy::Position::Fixed;
        s
    }

    /// Next to its owner (`placement`, flipped and shifted into the window), click-through.
    fn top_layer(&self, cx: &TopLayerCx) -> Option<skelter_core::overlay::TopLayer> {
        use skelter_core::overlay::{OverlayClass, TopLayer};
        let bounds = Rect::from_origin_size(skelter_base::geom::Point::ZERO, cx.window);
        let at = self.place(cx.owner.unwrap_or(bounds), bounds, cx.size).rect.min;
        Some(TopLayer::new(OverlayClass::Tooltip).at(at))
    }

    fn measure(&self, cx: &mut MeasureCx, _known: taffy::Size<Option<f32>>, avail: taffy::Size<taffy::AvailableSpace>) -> taffy::Size<f32> {
        let host = host_view(&cx.style, UA);
        // laid out against the window: it wraps to fit it
        let room = match avail.width {
            taffy::AvailableSpace::Definite(w) => Some(w),
            _ => self.room,
        };
        let s = Tooltip { room, ..self.clone() }.size(cx.text, &host);
        taffy::Size { width: s.w, height: s.h }
    }

    fn record(&self, cx: &mut RecordCx) {
        let host = host_view(&cx.style, UA);
        let rect = cx.rect();
        let mut pc = PaintCx::of(cx);
        let geom = paint::paint_box(&mut pc, &host, rect);
        let layout = paint::text_layout(pc.text, &host, &self.props.text, Some(geom.content.width().max(0.0)));
        paint::paint_text(&mut pc, &host, &layout, geom.content, Align::Start);
    }

    /// Tooltips take no input (click-through).
    fn event(&mut self, _cx: &mut EventCx, _ev: &Event) -> Handled {
        Handled::No
    }

    fn a11y(&self, cx: &mut A11yCx) {
        cx.node.set_role(accesskit::Role::Tooltip);
        cx.node.set_label(self.props.text.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controls::testkit;
    use crate::interact::{Native, StateBits};
    use crate::shim::testing::MonoShaper;
    use crate::views::placement::Side;

    fn ms(n: i64) -> Time {
        Time(n as u64 * 1_000_000)
    }
    const OVER: TipInput = TipInput { over: true, pressed: false, scrolled: false, escape: false };
    const AWAY: TipInput = TipInput { over: false, pressed: false, scrolled: false, escape: false };

    #[test]
    fn shows_after_the_delay_and_hides_on_press_until_left() {
        let pol = *Native::Gtk.policy(); // 500 ms delay, 1000 ms follow-on
        let mut warm = TooltipWarmth::default();
        let mut t = Tooltip::with_text("Save");
        assert_eq!(t.step(OVER, ms(0), &pol, &mut warm), None);
        assert_eq!(t.timer.wake(), Some(ms(500)));
        assert_eq!(t.step(OVER, ms(499), &pol, &mut warm), None);
        assert_eq!(t.step(OVER, ms(500), &pol, &mut warm), Some(Tip::Show));
        assert_eq!(t.step(TipInput { pressed: true, ..OVER }, ms(600), &pol, &mut warm), Some(Tip::Hide));
        assert_eq!(t.step(OVER, ms(2000), &pol, &mut warm), None, "not again until the pointer leaves");
        assert_eq!(t.step(AWAY, ms(2100), &pol, &mut warm), None);
        // scroll and Escape suppress it too
        t.step(OVER, ms(5000), &pol, &mut warm);
        assert_eq!(t.step(OVER, ms(5500), &pol, &mut warm), Some(Tip::Show));
        assert_eq!(t.step(TipInput { scrolled: true, ..OVER }, ms(5600), &pol, &mut warm), Some(Tip::Hide));
        t.step(AWAY, ms(5700), &pol, &mut warm);
        t.step(OVER, ms(9000), &pol, &mut warm);
        assert_eq!(t.step(OVER, ms(9500), &pol, &mut warm), Some(Tip::Show));
        assert_eq!(t.step(TipInput { escape: true, ..OVER }, ms(9600), &pol, &mut warm), Some(Tip::Hide));
    }

    #[test]
    fn follow_on_shows_the_next_tip_at_once() {
        let pol = *Native::Gtk.policy();
        let mut warm = TooltipWarmth::default();
        let mut a = Tooltip::with_text("A");
        let mut b = Tooltip::with_text("B");
        a.step(OVER, ms(0), &pol, &mut warm);
        assert_eq!(a.step(OVER, ms(500), &pol, &mut warm), Some(Tip::Show));
        // the pointer moves from A to B
        assert_eq!(a.step(AWAY, ms(800), &pol, &mut warm), Some(Tip::Hide));
        assert_eq!(b.step(OVER, ms(850), &pol, &mut warm), Some(Tip::Show), "warm: no delay");
        assert_eq!(b.step(AWAY, ms(900), &pol, &mut warm), Some(Tip::Hide));
        // past the follow-on window: the full delay again
        assert_eq!(a.step(OVER, ms(2000), &pol, &mut warm), None);
        assert_eq!(a.step(OVER, ms(2500), &pol, &mut warm), Some(Tip::Show));
    }

    #[test]
    fn duration_hides_it_and_empty_text_never_shows() {
        let pol = *Native::Windows.policy(); // 400 ms
        let mut warm = TooltipWarmth::default();
        let mut t = Tooltip::with_text("x");
        t.props.duration = 1.0;
        t.step(OVER, ms(0), &pol, &mut warm);
        assert_eq!(t.step(OVER, ms(400), &pol, &mut warm), Some(Tip::Show));
        assert_eq!(t.timer.wake(), Some(ms(1400)));
        assert_eq!(t.step(OVER, ms(1400), &pol, &mut warm), Some(Tip::Hide));
        let mut e = Tooltip::default();
        e.step(OVER, ms(0), &pol, &mut warm);
        assert_eq!(e.step(OVER, ms(5000), &pol, &mut warm), None);
    }

    #[test]
    fn a_title_shows_its_tooltip_after_the_live_policy_delay() {
        // SV61 with SV62: hover, then the policy's delay, then the tooltip layer; a new policy
        // applies to the next hover
        use crate::testdoc::{W, child, doc, frame, pointer};
        use skelter_base::event::PointerPhase;
        let (mut d, root) = doc(Size::new(300.0, 200.0), |_| {});
        let mut pol = *Native::Gtk.policy();
        crate::host::install(&mut d, skelter_text::TextSystem::new(skelter_text::FontStore::new(skelter_text::FontConfig::testing())), pol, crate::theme::Theme::dark_modern());
        let host = child(&mut d, root, skelter_core::KindId::of("box"), "position: absolute; left: 100px; top: 100px; width: 40px; height: 20px; background-color: #ff0000");
        d.set_prop(host, skelter_core::common::TITLE, skelter_core::Value::from("Save all")).unwrap();
        frame(&mut d);
        d.dispatch(&pointer(PointerPhase::Move, 110.0, 110.0, ms(0).0, false));
        assert_eq!(d.next_timer(), Some(ms(500)), "GTK's delay");
        assert!(frame(&mut d).is_none_or(|c| c.layers.is_empty()) && d.window_tip_layer(W).is_none());
        d.run_timers(ms(500));
        frame(&mut d).expect("the tip");
        assert!(d.window_tip_layer(W).is_some(), "shown after the delay");
        // away: it hides; past the follow-on window a new policy's delay applies
        d.dispatch(&pointer(PointerPhase::Move, 10.0, 10.0, ms(600).0, false));
        frame(&mut d);
        assert!(d.window_tip_layer(W).is_none());
        pol.tooltip_delay_ms = 1200;
        crate::host::set_policy(&mut d, pol);
        d.dispatch(&pointer(PointerPhase::Move, 110.0, 110.0, ms(5000).0, false));
        assert_eq!(d.next_timer(), Some(ms(6200)), "the new delay");
        d.run_timers(ms(6200));
        frame(&mut d);
        assert!(d.window_tip_layer(W).is_some());
        // a press hides it
        d.dispatch(&pointer(PointerPhase::Down, 110.0, 110.0, ms(6300).0, false));
        frame(&mut d);
        assert!(d.window_tip_layer(W).is_none());
    }

    #[test]
    fn goes_above_the_host_and_flips_below_at_the_top() {
        let t = Tooltip::with_text("Save all");
        let size = t.size(&mut MonoShaper, &StyleView::ua_only(UA, StateBits::NONE, true));
        // 8 chars of 12px text, 8/4 padding, 1px border
        assert_eq!(size.w, 8.0 * MonoShaper::advance(12.0) + 18.0);
        let bounds = Rect::new(0.0, 0.0, 800.0, 600.0);
        let host = Rect::new(100.0, 100.0, 140.0, 120.0);
        let placed = t.place(host, bounds, size);
        assert_eq!(placed.side, Side::Top);
        assert_eq!(placed.rect.max.y, 100.0 - GAP);
        assert!((placed.rect.min.x + placed.rect.max.x - 240.0).abs() < 0.01, "centred on the host");
        let top = Rect::new(100.0, 0.0, 140.0, 20.0);
        assert_eq!(t.place(top, bounds, size).side, Side::Bottom);
    }

    #[test]
    fn delay_prop_overrides_the_policy_and_records_box_and_text() {
        let pol = *Native::Gtk.policy();
        let mut warm = TooltipWarmth::default();
        let mut t = Tooltip::with_text("Save");
        t.set_prop(SPEC.prop("delay").unwrap(), &Value::F64(0.1)).unwrap();
        t.step(OVER, ms(0), &pol, &mut warm);
        assert_eq!(t.step(OVER, ms(100), &pol, &mut warm), Some(Tip::Show));
        let (list, res) = testkit::record(&t, Size::new(60.0, 24.0), StateBits::NONE);
        assert_eq!(testkit::kinds(&list), ["shadow", "quad", "glyphs"]);
        assert_eq!(res.runs.len(), 1);
    }
}
