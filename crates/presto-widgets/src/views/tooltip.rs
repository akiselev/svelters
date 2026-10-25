//! Tooltips: when a host's tooltip shows and hides ([`TooltipTimer`]) and the `tooltip` surface kind
//! ([`Tooltip`]).
//!
//! Timing (svelters-egui `interact/tooltip.rs`): the tip shows after the pointer rested on its host
//! for the platform's delay ([`Policy::tooltip_delay`], or the host's own), at once while another
//! tip hid within the follow-on window ([`Policy::tooltip_follow_on`], Qt's fall-asleep timer, kept
//! per window in [`TooltipWarmth`]); also on a disabled host. A press, a scroll, Escape or the pointer
//! leaving hides it, and after a press, scroll or Escape it stays hidden until the pointer leaves;
//! `duration` hides it after a time.
//!
//! The surface goes above its host, centred (`top-center`), flipping below when there is no room
//! (svelters-egui `widgets/tooltip.rs`); it is a click-through overlay entry.

use super::draw;
use super::placement::{AnchorParams, Placed, Placement, place};
use super::legacy::{Event, EventCx, Kind, KindSpec, Labels, Palette, Policy, PropError, PropType, RecordCx, Value, prop};
use presto_base::accesskit;
use presto_base::change::ChangeClass;
use presto_base::geom::{Point, Rect, Size};
use presto_base::time::{Span, Time};

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

static SPEC: KindSpec = KindSpec {
    name: "tooltip",
    props: &[
        prop("text", PropType::Str, ChangeClass::Layout),
        prop("placement", PropType::Enum(super::popup::PLACEMENTS), ChangeClass::Property),
        prop("delay", PropType::F64, ChangeClass::Property),
        prop("duration", PropType::F64, ChangeClass::Property),
    ],
    signals: &[],
    slots: &[""],
    role: accesskit::Role::Tooltip,
};

/// The widest a tooltip's text runs before it wraps.
pub const MAX_WIDTH: f32 = 360.0;
/// Text inset and the gap to the host.
const PAD: Size = Size::new(8.0, 4.0);
const GAP: f32 = 6.0;

/// A `tooltip` surface.
#[derive(Clone, Debug, PartialEq)]
pub struct Tooltip {
    pub text: String,
    pub placement: Placement,
    /// `None`: the policy's delay.
    pub delay: Option<Span>,
    /// Zero: while hovered.
    pub duration: Span,
    pub timer: TooltipTimer,
}

impl Default for Tooltip {
    fn default() -> Self {
        Tooltip { text: String::new(), placement: Placement::TOP_CENTER, delay: None, duration: Span::ZERO, timer: TooltipTimer::default() }
    }
}

impl Tooltip {
    /// Steps the timer for the host's input with the policy's timings.
    pub fn step(&mut self, input: TipInput, now: Time, policy: &Policy, warmth: &mut TooltipWarmth) -> Option<Tip> {
        let input = if self.text.is_empty() { TipInput { over: false, ..input } } else { input };
        self.timer.step(input, now, self.delay.unwrap_or(policy.tooltip_delay), self.duration, policy.tooltip_follow_on, warmth)
    }

    /// The surface's size for its text.
    pub fn size(&self, labels: &mut dyn Labels, palette: &Palette) -> Size {
        let t = labels.measure(&self.text, palette.font_size, Some(MAX_WIDTH));
        Size::new(t.w + 2.0 * PAD.w, t.h + 2.0 * PAD.h)
    }

    /// Where it goes for a host at `host` (window points) inside `bounds`.
    pub fn place(&self, host: Rect, bounds: Rect, size: Size) -> Placed {
        place(&AnchorParams { anchor: host, size, bounds, placement: self.placement, gap: GAP, flip: true, shift: true, ..AnchorParams::default() })
    }
}

impl Kind for Tooltip {
    fn spec() -> &'static KindSpec {
        &SPEC
    }

    fn set_prop(&mut self, name: &str, v: &Value) -> Result<ChangeClass, PropError> {
        let secs = |v: &Value| v.as_f64().map(|s| Span::from_secs_f64(s.max(0.0))).ok_or(PropError::Invalid("expected seconds"));
        match name {
            "text" => self.text = v.as_str().ok_or(PropError::Invalid("expected a string"))?.to_string(),
            "placement" => self.placement = v.as_str().and_then(Placement::parse).ok_or(PropError::Invalid("unknown placement"))?,
            "delay" => self.delay = if *v == Value::Null { None } else { Some(secs(v)?) },
            "duration" => self.duration = if *v == Value::Null { Span::ZERO } else { secs(v)? },
            _ => return Err(PropError::Unknown),
        }
        Ok(SPEC.prop(name).map_or(ChangeClass::Paint, |p| p.change))
    }

    fn record(&self, cx: &mut RecordCx<'_>) {
        let p = cx.palette;
        let rect = Rect::from_origin_size(Point::ZERO, cx.size);
        draw::shadow(cx.list, rect, p.radius, p.shadow, 4.0, presto_base::geom::Vec2::new(0.0, 1.0));
        draw::boxed(cx.list, rect, p.radius, p.surface_raised, 1.0, p.border);
        let max = (cx.size.w - 2.0 * PAD.w).max(0.0);
        cx.labels.record(cx.list, &self.text, Point::new(PAD.w, PAD.h), p.font_size, p.text, Some(max));
    }

    /// Tooltips take no input (click-through).
    fn event(&mut self, _cx: &mut EventCx<'_>, _ev: &Event) -> super::legacy::Handled {
        super::legacy::Handled::No
    }

    fn a11y(&self, node: &mut accesskit::Node) {
        node.set_role(accesskit::Role::Tooltip);
        node.set_label(self.text.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::views::placement::Side;
    use crate::views::legacy::{FixedLabels, Os};

    fn ms(n: i64) -> Time {
        Time(n as u64 * 1_000_000)
    }
    const OVER: TipInput = TipInput { over: true, pressed: false, scrolled: false, escape: false };
    const AWAY: TipInput = TipInput { over: false, pressed: false, scrolled: false, escape: false };

    #[test]
    fn shows_after_the_delay_and_hides_on_press_until_left() {
        let pol = Policy::for_os(Os::Linux); // 500 ms delay, 1000 ms follow-on
        let mut warm = TooltipWarmth::default();
        let mut t = Tooltip { text: "Save".into(), ..Tooltip::default() };
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
        let pol = Policy::for_os(Os::Linux);
        let mut warm = TooltipWarmth::default();
        let mut a = Tooltip { text: "A".into(), ..Tooltip::default() };
        let mut b = Tooltip { text: "B".into(), ..Tooltip::default() };
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
        let pol = Policy::for_os(Os::Windows); // 400 ms
        let mut warm = TooltipWarmth::default();
        let mut t = Tooltip { text: "x".into(), duration: Span::from_millis(1000), ..Tooltip::default() };
        t.step(OVER, ms(0), &pol, &mut warm);
        assert_eq!(t.step(OVER, ms(400), &pol, &mut warm), Some(Tip::Show));
        assert_eq!(t.timer.wake(), Some(ms(1400)));
        assert_eq!(t.step(OVER, ms(1400), &pol, &mut warm), Some(Tip::Hide));
        let mut e = Tooltip::default();
        e.step(OVER, ms(0), &pol, &mut warm);
        assert_eq!(e.step(OVER, ms(5000), &pol, &mut warm), None);
    }

    #[test]
    fn goes_above_the_host_and_flips_below_at_the_top() {
        let t = Tooltip { text: "Save all".into(), ..Tooltip::default() };
        let size = t.size(&mut FixedLabels, &Palette::default());
        let bounds = Rect::new(0.0, 0.0, 800.0, 600.0);
        let host = Rect::new(100.0, 100.0, 140.0, 120.0);
        let placed = t.place(host, bounds, size);
        assert_eq!(placed.side, Side::Top);
        assert_eq!(placed.rect.max.y, 100.0 - GAP);
        assert!((placed.rect.min.x + placed.rect.max.x - 240.0).abs() < 0.01, "centred on the host");
        let top = Rect::new(100.0, 0.0, 140.0, 20.0);
        assert_eq!(t.place(top, bounds, size).side, Side::Bottom);
    }
}
