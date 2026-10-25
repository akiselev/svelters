//! Stand-ins for APIs the views need that their owning crates do not have yet; the consolidation
//! replaces each and deletes this module.
//!
//! - [`scroll_physics`]: `presto_anim::scroll` (architecture §12.2), still unwritten in presto-anim.
//! - [`fast`]: a kind's fast commits of native-owned properties (architecture §7.3). The crate's kind
//!   interface ([`crate::shim::core::EventCx`]) and presto-core's `EventCx` have no way to send one;
//!   the views queue them per thread and the app runner drains them for the compositor.

pub mod fast {
    //! The per-thread queue of fast commits the views made while handling input.

    use presto_base::commit::FastCommit;
    use std::cell::RefCell;

    thread_local! {
        static FAST: RefCell<Vec<FastCommit>> = const { RefCell::new(Vec::new()) };
    }

    /// Queues `f` for the compositor.
    pub fn push(f: FastCommit) {
        FAST.with(|q| q.borrow_mut().push(f));
    }

    /// The fast commits queued on this thread since the last call, in order.
    pub fn take() -> Vec<FastCommit> {
        FAST.with(|q| std::mem::take(&mut *q.borrow_mut()))
    }
}

/// Stand-in for `presto_anim::scroll` (architecture §12.2; constants and sources in
/// `plans/research/presto/notes/round2/animation-scroll.md` §4): wheel deltas to points per OS, the
/// Chromium wheel smooth-scroll curve with its velocity-preserving retarget, the exponential fling,
/// and the release-velocity estimate for an own fling. Pure functions of time, as the compositor
/// will sample them.
pub mod scroll_physics {
    use crate::interact::Platform as Os;
    use presto_base::event::ScrollDelta;
    use presto_base::geom::{Size, Vec2};
    use presto_base::time::{Span, Time};

    /// Points one wheel detent scrolls: Windows 3 lines × 100/3 px (Chromium
    /// `web_input_event_builders_win.cc`), Linux 120 (Chromium `MouseWheelEvent::kWheelDelta`),
    /// macOS 40 per Cocoa tick (`kScrollbarPixelsPerCocoaTick`, the OS already accelerated it).
    pub fn points_per_detent(os: Os) -> f32 {
        match os {
            Os::Windows => 100.0,
            Os::Linux => 120.0,
            Os::Macos => 40.0,
        }
    }

    /// A page step along one axis of a `len`-point viewport: `max(0.875·len, len − 40)` (Chromium
    /// `cc/input/scroll_utils.h`).
    pub fn page_step(len: f32) -> f32 {
        (0.875 * len).max(len - 40.0).max(1.0)
    }

    /// The offset change (content points, positive towards the end) a wheel delta asks for. The
    /// event's positive `y` scrolls the content up (the wheel away from the user), so it is negated.
    pub fn wheel_delta_points(os: Os, delta: ScrollDelta, viewport: Size) -> Vec2 {
        let v = match delta {
            ScrollDelta::Lines(v) => v * points_per_detent(os),
            ScrollDelta::Pixels(v) => v,
            ScrollDelta::Pages(v) => Vec2::new(v.x * page_step(viewport.w), v.y * page_step(viewport.h)),
        };
        Vec2::new(-v.x, -v.y)
    }

    /// Chromium's `kInverseDelta` wheel duration: 200 ms for deltas up to 120 px, falling linearly to
    /// 100 ms at 480 px and more (`cc/animation/scroll_offset_animation_curve.cc`).
    pub fn wheel_duration(delta: f32) -> Span {
        let d = delta.abs().clamp(120.0, 480.0);
        let ms = 200.0 - (d - 120.0) / (480.0 - 120.0) * 100.0;
        Span::from_secs_f64(ms as f64 / 1000.0)
    }

    /// A unit cubic Bézier `(0,0) (x1,y1) (x2,y2) (1,1)` as a timing function.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct Bezier {
        pub x1: f32,
        pub y1: f32,
        pub x2: f32,
        pub y2: f32,
    }

    impl Bezier {
        fn coord(a: f32, b: f32, s: f32) -> f32 {
            let u = 1.0 - s;
            3.0 * u * u * s * a + 3.0 * u * s * s * b + s * s * s
        }
        fn dcoord(a: f32, b: f32, s: f32) -> f32 {
            let u = 1.0 - s;
            3.0 * u * u * a + 6.0 * u * s * (b - a) + 3.0 * s * s * (1.0 - b)
        }
        /// The curve's parameter at time `x` (Newton, then bisection when the slope is flat).
        fn solve(&self, x: f32) -> f32 {
            let mut s = x;
            for _ in 0..8 {
                let e = Self::coord(self.x1, self.x2, s) - x;
                if e.abs() < 1e-6 {
                    return s;
                }
                let d = Self::dcoord(self.x1, self.x2, s);
                if d.abs() < 1e-6 {
                    break;
                }
                s = (s - e / d).clamp(0.0, 1.0);
            }
            let (mut lo, mut hi) = (0.0f32, 1.0f32);
            s = x;
            for _ in 0..32 {
                let v = Self::coord(self.x1, self.x2, s);
                if (v - x).abs() < 1e-6 {
                    break;
                }
                if v < x { lo = s } else { hi = s }
                s = (lo + hi) / 2.0;
            }
            s
        }
        /// The progress at time fraction `x` in 0..=1.
        pub fn eval(&self, x: f32) -> f32 {
            if x <= 0.0 {
                return 0.0;
            }
            if x >= 1.0 {
                return 1.0;
            }
            Self::coord(self.y1, self.y2, self.solve(x))
        }
        /// d(progress)/d(time fraction) at `x`.
        pub fn slope(&self, x: f32) -> f32 {
            let s = self.solve(x.clamp(0.0, 1.0));
            let dx = Self::dcoord(self.x1, self.x2, s);
            if dx.abs() < 1e-6 { 0.0 } else { Self::dcoord(self.y1, self.y2, s) / dx }
        }
    }

    /// Chromium's `EaseInOutWithInitialSlope`: ease-in-out (0.42, 0, 0.58, 1) whose first control
    /// point is raised so the curve starts at `slope` (progress per time fraction).
    pub fn ease_in_out_with_slope(slope: f32) -> Bezier {
        let slope = slope.clamp(-1000.0, 1000.0);
        Bezier { x1: 0.42, y1: 0.42 * slope, x2: 0.58, y2: 1.0 }
    }

    /// A smooth scroll from `from` to `to` (Chromium's `ScrollOffsetAnimationCurve`, wheel flavour).
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct SmoothScroll {
        pub from: Vec2,
        pub to: Vec2,
        pub start: Time,
        pub duration: Span,
        pub curve: Bezier,
    }

    impl SmoothScroll {
        /// A wheel step starting at rest.
        pub fn wheel(now: Time, from: Vec2, to: Vec2) -> Self {
            let d = (to - from).length();
            SmoothScroll { from, to, start: now, duration: wheel_duration(d), curve: ease_in_out_with_slope(0.0) }
        }
        fn t(&self, now: Time) -> f32 {
            let total = self.duration.as_secs_f64();
            if total <= 0.0 {
                return 1.0;
            }
            ((now - self.start).as_secs_f64() / total).clamp(0.0, 1.0) as f32
        }
        pub fn offset_at(&self, now: Time) -> Vec2 {
            let p = self.curve.eval(self.t(now));
            self.from + (self.to - self.from) * p
        }
        /// Points per second at `now`.
        pub fn velocity_at(&self, now: Time) -> Vec2 {
            let total = self.duration.as_secs_f64() as f32;
            if total <= 0.0 || self.done_at(now) {
                return Vec2::ZERO;
            }
            (self.to - self.from) * (self.curve.slope(self.t(now)) / total)
        }
        pub fn done_at(&self, now: Time) -> bool {
            now >= self.start + self.duration
        }
        /// Heads for `to` from where the curve is at `now`, keeping its velocity (Chromium
        /// `UpdateTarget`): the new segment's duration is the wheel duration for its delta, bounded by
        /// `2.5 × remaining / velocity` (`VelocityBasedDurationBound`), and its curve starts at the
        /// current velocity.
        pub fn retarget(&mut self, now: Time, to: Vec2) {
            let at = self.offset_at(now);
            let v = self.velocity_at(now);
            let delta = to - at;
            let dist = delta.length();
            let mut dur = wheel_duration(dist).as_secs_f64() as f32;
            // the velocity along the new direction
            let along = if dist > 0.0 { (v.x * delta.x + v.y * delta.y) / dist } else { 0.0 };
            if along > 1.0 {
                dur = dur.min(2.5 * dist / along);
            }
            let slope = if dist > 0.0 && dur > 0.0 { along * dur / dist } else { 0.0 };
            *self = SmoothScroll { from: at, to, start: now, duration: Span::from_secs_f64(dur as f64), curve: ease_in_out_with_slope(slope) };
        }
    }

    /// The iOS-style exponential fling `v(t) = v₀·d^ms`, `x(t) = x₀ + v₀·(d^ms − 1)/ln d`, with the
    /// travel capped at three viewports (Chromium's `kDefaultBoundsMultiplier`) and the end where the
    /// speed drops below 0.5 pt per 16 ms frame.
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub struct Fling {
        pub from: Vec2,
        pub start: Time,
        /// Points per second at the start.
        pub velocity: Vec2,
        /// Velocity kept per millisecond (0.998 normal, 0.99 fast: `UIScrollView.decelerationRate`).
        pub rate_per_ms: f32,
        /// The longest travel per axis.
        pub max_travel: Vec2,
    }

    /// Below this speed (points per second) a fling stops: 0.5 pt per 16 ms frame.
    pub const FLING_STOP_SPEED: f32 = 0.5 / 0.016;

    impl Fling {
        pub fn new(now: Time, from: Vec2, velocity: Vec2, viewport: Size) -> Self {
            Fling { from, start: now, velocity, rate_per_ms: 0.998, max_travel: Vec2::new(viewport.w * 3.0, viewport.h * 3.0) }
        }
        fn ms(&self, now: Time) -> f32 {
            ((now - self.start).as_secs_f64() * 1000.0).max(0.0) as f32
        }
        pub fn velocity_at(&self, now: Time) -> Vec2 {
            self.velocity * self.rate_per_ms.powf(self.ms(now))
        }
        pub fn offset_at(&self, now: Time) -> Vec2 {
            let ln = self.rate_per_ms.ln();
            // v in points per ms for the integral
            let k = (self.rate_per_ms.powf(self.ms(now)) - 1.0) / ln / 1000.0;
            let mut d = self.velocity * k;
            d.x = d.x.clamp(-self.max_travel.x, self.max_travel.x);
            d.y = d.y.clamp(-self.max_travel.y, self.max_travel.y);
            self.from + d
        }
        pub fn done_at(&self, now: Time) -> bool {
            self.velocity_at(now).length() < FLING_STOP_SPEED
        }
        /// A fling does not retarget: a new wheel step replaces it with a smooth scroll from where it
        /// is, keeping its velocity.
        pub fn to_smooth(&self, now: Time, to: Vec2) -> SmoothScroll {
            let at = self.offset_at(now);
            let mut s = SmoothScroll { from: at, to: at, start: now, duration: Span::ZERO, curve: ease_in_out_with_slope(0.0) };
            // seed the velocity through a zero-length segment's retarget
            let v = self.velocity_at(now);
            let delta = to - at;
            let dist = delta.length();
            let mut dur = wheel_duration(dist).as_secs_f64() as f32;
            let along = if dist > 0.0 { (v.x * delta.x + v.y * delta.y) / dist } else { 0.0 };
            if along > 1.0 {
                dur = dur.min(2.5 * dist / along);
            }
            let slope = if dist > 0.0 && dur > 0.0 { along * dur / dist } else { 0.0 };
            s.to = to;
            s.duration = Span::from_secs_f64(dur as f64);
            s.curve = ease_in_out_with_slope(slope);
            s
        }
    }

    /// A running scroll animation (presto-anim's `ScrollAnimation`).
    #[derive(Clone, Copy, Debug, PartialEq)]
    pub enum ScrollAnimation {
        Smooth(SmoothScroll),
        Fling(Fling),
    }

    impl ScrollAnimation {
        pub fn offset_at(&self, now: Time) -> Vec2 {
            match self {
                ScrollAnimation::Smooth(s) => s.offset_at(now),
                ScrollAnimation::Fling(f) => f.offset_at(now),
            }
        }
        pub fn done_at(&self, now: Time) -> bool {
            match self {
                ScrollAnimation::Smooth(s) => s.done_at(now),
                ScrollAnimation::Fling(f) => f.done_at(now),
            }
        }
        /// Heads for `to` keeping the velocity (a fling becomes a smooth scroll).
        pub fn retarget(&mut self, now: Time, to: Vec2) {
            match self {
                ScrollAnimation::Smooth(s) => s.retarget(now, to),
                ScrollAnimation::Fling(f) => *self = ScrollAnimation::Smooth(f.to_smooth(now, to)),
            }
        }
        /// Where it is heading (a fling's end where its speed runs out).
        pub fn target(&self) -> Vec2 {
            match self {
                ScrollAnimation::Smooth(s) => s.to,
                ScrollAnimation::Fling(f) => {
                    let ln = f.rate_per_ms.ln();
                    let mut d = f.velocity * (-1.0 / ln / 1000.0);
                    d.x = d.x.clamp(-f.max_travel.x, f.max_travel.x);
                    d.y = d.y.clamp(-f.max_travel.y, f.max_travel.y);
                    f.from + d
                }
            }
        }
    }

    /// The window of samples a release velocity is fitted over (GTK `SCROLL_CAPTURE_THRESHOLD_MS`).
    pub const VELOCITY_WINDOW: Span = Span::from_millis(150);
    /// No fling when the last sample is older than this at release (Chromium
    /// `kFlingStartTimeoutMs`).
    pub const FLING_START_TIMEOUT: Span = Span::from_millis(200);

    /// Touchpad deltas of one gesture, for the release velocity (least squares of the cumulative
    /// offset over the last [`VELOCITY_WINDOW`], as Chromium's Wayland `RegressScrollVelocity`).
    #[derive(Clone, Debug, Default)]
    pub struct VelocityTracker {
        samples: Vec<(Time, Vec2)>,
        total: Vec2,
    }

    impl VelocityTracker {
        pub fn reset(&mut self) {
            self.samples.clear();
            self.total = Vec2::ZERO;
        }
        /// Adds a delta (content points) taken at `t`.
        pub fn push(&mut self, t: Time, delta: Vec2) {
            self.total = self.total + delta;
            self.samples.push((t, self.total));
            let keep_from = t - VELOCITY_WINDOW;
            self.samples.retain(|(s, _)| *s >= keep_from);
        }
        /// Points per second at release time `now`; zero when the last sample is too old or there
        /// are fewer than two samples.
        pub fn velocity(&self, now: Time) -> Vec2 {
            let Some(&(last, _)) = self.samples.last() else { return Vec2::ZERO };
            if now - last > FLING_START_TIMEOUT || self.samples.len() < 2 {
                return Vec2::ZERO;
            }
            let t0 = self.samples[0].0;
            let n = self.samples.len() as f64;
            let (mut st, mut stt) = (0.0f64, 0.0f64);
            let (mut sx, mut sy, mut stx, mut sty) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
            for (t, p) in &self.samples {
                let t = (*t - t0).as_secs_f64();
                st += t;
                stt += t * t;
                sx += p.x as f64;
                sy += p.y as f64;
                stx += t * p.x as f64;
                sty += t * p.y as f64;
            }
            let den = n * stt - st * st;
            if den.abs() < 1e-12 {
                return Vec2::ZERO;
            }
            Vec2::new(((n * stx - st * sx) / den) as f32, ((n * sty - st * sy) / den) as f32)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn wheel_points_per_os_and_sign() {
            let v = Size::new(400.0, 600.0);
            assert_eq!(wheel_delta_points(Os::Windows, ScrollDelta::Lines(Vec2::new(0.0, 1.0)), v), Vec2::new(-0.0, -100.0));
            assert_eq!(wheel_delta_points(Os::Linux, ScrollDelta::Lines(Vec2::new(0.0, -1.0)), v), Vec2::new(-0.0, 120.0));
            assert_eq!(wheel_delta_points(Os::Macos, ScrollDelta::Pixels(Vec2::new(3.0, -7.0)), v), Vec2::new(-3.0, 7.0));
            assert_eq!(wheel_delta_points(Os::Windows, ScrollDelta::Pages(Vec2::new(0.0, -1.0)), v).y, 560.0);
        }

        #[test]
        fn wheel_duration_falls_with_the_delta() {
            assert_eq!(wheel_duration(100.0), Span::from_millis(200));
            assert_eq!(wheel_duration(1000.0), Span::from_millis(100));
            let mid = wheel_duration(300.0).as_millis_f64();
            assert!((mid - 150.0).abs() < 0.01, "{mid}");
        }

        #[test]
        fn smooth_scroll_ends_at_its_target_and_retargets_with_velocity() {
            let t0 = Time(1_000_000_000);
            let mut s = SmoothScroll::wheel(t0, Vec2::ZERO, Vec2::new(0.0, 100.0));
            assert_eq!(s.offset_at(t0), Vec2::ZERO);
            assert!(s.done_at(t0 + Span::from_millis(200)));
            assert_eq!(s.offset_at(t0 + Span::from_millis(200)), Vec2::new(0.0, 100.0));
            let mid = t0 + Span::from_millis(100);
            let (at, v) = (s.offset_at(mid), s.velocity_at(mid));
            assert!(v.y > 0.0);
            s.retarget(mid, Vec2::new(0.0, 200.0));
            assert_eq!(s.offset_at(mid), at, "continuous position");
            let v2 = s.velocity_at(mid + Span::from_micros(1));
            assert!((v2.y - v.y).abs() / v.y < 0.05, "continuous velocity: {v:?} → {v2:?}");
            assert_eq!(s.offset_at(mid + s.duration), Vec2::new(0.0, 200.0));
        }

        #[test]
        fn fling_decays_and_stops() {
            let t0 = Time(0);
            let f = Fling::new(t0, Vec2::ZERO, Vec2::new(0.0, 1000.0), Size::new(100.0, 1000.0));
            // travel to the end is v0·0.4995 s for 0.998 per ms
            let end = ScrollAnimation::Fling(f).target();
            assert!((end.y - 499.5).abs() < 1.0, "{end:?}");
            assert!(!f.done_at(t0 + Span::from_millis(100)));
            assert!(f.done_at(t0 + Span::from_millis(5000)));
            let a = f.offset_at(t0 + Span::from_millis(100)).y;
            let b = f.offset_at(t0 + Span::from_millis(200)).y;
            assert!(a > 0.0 && b > a && b - a < a, "decelerates: {a} {b}");
        }

        #[test]
        fn velocity_is_a_regression_over_recent_samples() {
            let mut v = VelocityTracker::default();
            for i in 0..10 {
                v.push(Time(i * 10_000_000), Vec2::new(0.0, 5.0));
            }
            let now = Time(90_000_000);
            assert!((v.velocity(now).y - 500.0).abs() < 1.0, "{:?}", v.velocity(now));
            assert_eq!(v.velocity(now + Span::from_millis(250)), Vec2::ZERO, "released too late");
        }
    }
}
