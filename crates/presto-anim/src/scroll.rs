//! Scroll physics per OS (`round2/animation-scroll.md` §4, architecture §12.2), as closed-form
//! curves of the time since the gesture or animation started:
//! - wheel, keyboard and programmatic smooth scrolling: Chromium's `ScrollOffsetAnimationCurve`
//!   (ease-in-out, inverse-delta durations 200 → 100 ms, retargeting that keeps the velocity);
//! - flings: Chromium's `PhysicsBasedFlingCurve`, iOS-style exponential decay, Android's
//!   `OverScroller` spline;
//! - the macOS rubber band (resistance curve, critically damped return);
//! - the fling velocity estimate (least squares over the last 150 ms), per-gesture latching, and
//!   the per-OS defaults.
//!
//! Ported from Chromium (`cc/animation/scroll_offset_animation_curve.cc`,
//! `ui/events/gestures/physics_based_fling_curve.cc`, chromium/src@52144186, BSD-3-Clause) and AOSP
//! (`core/java/android/widget/OverScroller.java`, platform/frameworks/base@1cdfff55, Apache-2.0).
//! Modifications by the svelters authors.

use std::collections::VecDeque;

use presto_base::anim::FlingCurve;
use presto_base::geom::{Size, Vec2};
use presto_base::ids::NodeId;
use presto_base::time::{Span, Time};

use crate::easing::{cubic_bezier, cubic_bezier_slope};
use crate::spring::Spring;

/// A scroll-offset curve. `t` is the time since the curve started.
pub trait ScrollAnimation {
    fn offset_at(&self, t: Span) -> Vec2;
    fn velocity_at(&self, t: Span) -> Vec2;
    fn done_at(&self, t: Span) -> bool;
    /// Heads for `target` from the value at `t`, keeping the velocity. `false` when the curve
    /// cannot retarget (flings: start a smooth scroll from the fling's current state instead).
    fn retarget(&mut self, t: Span, target: Vec2) -> bool;
}

fn secs(t: Span) -> f64 {
    t.as_secs_f64()
}

/// Seconds as a span, rounded up, so `done_at(duration())` holds.
fn span_ceil(s: f64) -> Span {
    Span((s * 1e9).ceil() as i64)
}

/// Chromium's `MaximumDimension`: the component with the larger magnitude, signed.
fn max_dim(v: Vec2) -> f64 {
    if v.x.abs() > v.y.abs() { v.x as f64 } else { v.y as f64 }
}

fn lerp_vec(a: Vec2, b: Vec2, p: f64) -> Vec2 {
    Vec2::new(a.x + (b.x - a.x) * p as f32, a.y + (b.y - a.y) * p as f32)
}

// --- smooth scrolling ---------------------------------------------------------------------------

/// Chromium: line step 40 px; page step `max(0.875·len, len − 40)` (`cc/input/scroll_utils.h`).
pub const LINE_STEP_PX: f32 = 40.0;

pub fn page_step(len: f32) -> f32 {
    (0.875 * len).max(len - 40.0)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DurationBehavior {
    /// 9 frames (150 ms): keyboard.
    Constant,
    /// `√|Δ|` frames, at most [`PROGRAMMATIC_MAX`]: `scrollTo({behavior: 'smooth'})`.
    DeltaBased,
    /// 12 frames at ≤ 120 px falling to 6 frames at ≥ 480 px: the mouse wheel.
    InverseDelta,
}

const CONSTANT_FRAMES: f64 = 9.0;
const DURATION_DIVISOR: f64 = 60.0;
const INVERSE_RAMP_START_PX: f64 = 120.0;
const INVERSE_RAMP_END_PX: f64 = 480.0;
const INVERSE_MIN_FRAMES: f64 = 6.0;
const INVERSE_MAX_FRAMES: f64 = 12.0;
const EPSILON: f64 = 0.01;
/// Chromium's ease-in-out for user scrolls.
const EASE_IN_OUT: [f64; 4] = [0.42, 0.0, 0.58, 1.0];
/// Chromium desktop's programmatic curve and cap (`cc/base/features.cc`
/// `PROGRAMMATIC_SCROLL_ANIMATION_CURVE(0.4, 0, 0, 1, 1500)`).
const PROGRAMMATIC: [f64; 4] = [0.4, 0.0, 0.0, 1.0];
pub const PROGRAMMATIC_MAX: Span = Span::from_millis(1500);

/// The duration of a segment covering `delta`, in seconds.
pub fn segment_duration(delta: Vec2, behavior: DurationBehavior) -> f64 {
    let frames = match behavior {
        DurationBehavior::Constant => CONSTANT_FRAMES,
        DurationBehavior::DeltaBased => max_dim(delta).abs().sqrt().min(secs(PROGRAMMATIC_MAX) * DURATION_DIVISOR),
        DurationBehavior::InverseDelta => {
            let slope = (INVERSE_MIN_FRAMES - INVERSE_MAX_FRAMES) / (INVERSE_RAMP_END_PX - INVERSE_RAMP_START_PX);
            let offset = INVERSE_MAX_FRAMES - INVERSE_RAMP_START_PX * slope;
            (offset + max_dim(delta).abs() * slope).clamp(INVERSE_MIN_FRAMES, INVERSE_MAX_FRAMES)
        }
    };
    frames / DURATION_DIVISOR
}

/// Chromium's `ScrollOffsetAnimationCurve` (ease-in-out type).
#[derive(Clone, Debug, PartialEq)]
pub struct SmoothScroll {
    initial: Vec2,
    target: Vec2,
    behavior: DurationBehavior,
    control: [f64; 4],
    base: [f64; 4],
    /// End of the current segment, seconds since the curve started.
    total: f64,
    last_retarget: f64,
}

impl SmoothScroll {
    pub fn new(from: Vec2, to: Vec2, behavior: DurationBehavior) -> Self {
        let base = if behavior == DurationBehavior::DeltaBased { PROGRAMMATIC } else { EASE_IN_OUT };
        SmoothScroll { initial: from, target: to, behavior, control: base, base, total: segment_duration(to - from, behavior), last_retarget: 0.0 }
    }

    pub fn target(&self) -> Vec2 {
        self.target
    }

    /// When the curve reaches its target, since it started.
    pub fn duration(&self) -> Span {
        span_ceil(self.total)
    }

    fn velocity_scalar(&self, t: f64) -> f64 {
        let d = self.total - self.last_retarget;
        if d <= 0.0 {
            return 0.0;
        }
        let x = ((t - self.last_retarget) / d).clamp(0.0, 1.0);
        let [x1, y1, x2, y2] = self.control;
        cubic_bezier_slope(x1, y1, x2, y2, x) * (max_dim(self.target - self.initial) / d)
    }
}

impl ScrollAnimation for SmoothScroll {
    fn offset_at(&self, t: Span) -> Vec2 {
        let t = secs(t) - self.last_retarget;
        let d = self.total - self.last_retarget;
        if d <= 0.0 || t >= d {
            return self.target;
        }
        if t <= 0.0 {
            return self.initial;
        }
        let [x1, y1, x2, y2] = self.control;
        lerp_vec(self.initial, self.target, cubic_bezier(x1, y1, x2, y2, t / d))
    }

    fn velocity_at(&self, t: Span) -> Vec2 {
        let t = secs(t);
        if t >= self.total {
            return Vec2::ZERO;
        }
        let d = self.total - self.last_retarget;
        if d <= 0.0 {
            return Vec2::ZERO;
        }
        let x = ((t - self.last_retarget) / d).clamp(0.0, 1.0);
        let [x1, y1, x2, y2] = self.control;
        let slope = cubic_bezier_slope(x1, y1, x2, y2, x) / d;
        (self.target - self.initial) * slope as f32
    }

    fn done_at(&self, t: Span) -> bool {
        secs(t) >= self.total
    }

    /// Chromium `UpdateTarget` (without input-delay compensation).
    fn retarget(&mut self, t: Span, new_target: Vec2) -> bool {
        let t = secs(t).max(self.last_retarget);
        if max_dim(self.target - new_target).abs() < EPSILON {
            self.target = new_target;
            return true;
        }
        let current = self.offset_at(Span::from_secs_f64(t));
        let new_delta = new_target - current;
        if max_dim(new_delta).abs() < EPSILON {
            self.last_retarget = t;
            self.total = t;
            self.target = new_target;
            return true;
        }
        if self.total - self.last_retarget <= 0.0 {
            self.total = segment_duration(new_delta, self.behavior);
            self.target = new_target;
            return true;
        }
        let velocity = self.velocity_scalar(t);
        // VelocityBasedDurationBound: the time to the target at this velocity, ×2.5 for the ease out.
        let bound = if velocity.abs() < EPSILON {
            f64::INFINITY
        } else {
            let b = max_dim(new_delta) / velocity * 2.5;
            if b < 0.0 { f64::INFINITY } else { b }
        };
        let new_duration = segment_duration(new_delta, self.behavior).min(bound);
        if new_duration < EPSILON {
            self.target = new_target;
            self.total = t;
            return true;
        }
        let slope = (velocity * (new_duration / max_dim(new_delta))).clamp(-1000.0, 1000.0);
        let [x1, _, x2, y2] = self.base;
        self.control = [x1, x1 * slope, x2, y2];
        self.initial = current;
        self.target = new_target;
        self.total = t + new_duration;
        self.last_retarget = t;
        true
    }
}

// --- flings ---------------------------------------------------------------------------------------

/// Chromium's `PhysicsBasedFlingCurve` constants.
const PHYS_P1: (f64, f64) = (0.2, 1.0);
const PHYS_P2: (f64, f64) = (0.55, 1.0);
const PHYS_PIXEL_DECELERATION: f64 = 2300.0;
const PHYS_MAX_DURATION: f64 = 2.0;
/// inch/ms².
const PHYS_DECELERATION: f64 = 2.7559e-5;
pub const PHYS_BOUNDS_MULTIPLIER: f32 = 3.0;

/// Chromium's `PhysicsBasedFlingCurve`: a cubic bezier from (0.2, 1) to (0.55, 1) over at most 2 s,
/// its first control point fitted to the release velocity, the distance from `v²/2a` with a
/// physical deceleration, clamped to 3× the viewport (× the boost).
#[derive(Clone, Debug, PartialEq)]
pub struct PhysicsFling {
    distance: Vec2,
    duration: f64,
    p1: (f64, f64),
    p2: (f64, f64),
}

impl PhysicsFling {
    /// `velocity` in px/s, `ppi` the display's pixels per inch per axis, `viewport` the scroller's
    /// size.
    pub fn new(velocity: Vec2, ppi: Vec2, boost: f32, viewport: Size) -> Self {
        let bound = Size::new((viewport.w * boost * PHYS_BOUNDS_MULTIPLIER).floor(), (viewport.h * boost * PHYS_BOUNDS_MULTIPLIER).floor());
        let v_ms = (velocity.x as f64 / 1000.0, velocity.y as f64 / 1000.0);
        let axis = |v: f64, ppi: f32, bound: f32| -> f32 {
            let decel = ppi as f64 * PHYS_DECELERATION;
            if decel <= 0.0 || v == 0.0 {
                return 0.0;
            }
            let dur = (v / decel).abs();
            let pos = ((v.abs() - decel * dur * 0.5) * dur).copysign(v) as f32;
            if pos.abs() > bound { bound.copysign(pos) } else { pos }
        };
        let distance = Vec2::new(axis(v_ms.0, ppi.x, bound.w), axis(v_ms.1, ppi.y, bound.h));
        let fling_v = (velocity.x.abs().max(velocity.y.abs())) as f64;
        let duration = PHYS_MAX_DURATION.min(fling_v / PHYS_PIXEL_DECELERATION);
        let slope = if velocity.y.abs() as f64 == fling_v {
            (velocity.y as f64 * duration / distance.y as f64).abs()
        } else {
            (velocity.x as f64 * duration / distance.x as f64).abs()
        };
        let mut p1 = PHYS_P1;
        if slope.is_finite() {
            if slope * p1.0 < 1.0 {
                p1.1 = p1.0 * slope;
            } else {
                p1.0 = p1.1 / slope;
            }
        }
        PhysicsFling { distance, duration, p1, p2: PHYS_P2 }
    }

    pub fn distance(&self) -> Vec2 {
        self.distance
    }

    pub fn duration(&self) -> Span {
        span_ceil(self.duration)
    }
}

impl ScrollAnimation for PhysicsFling {
    fn offset_at(&self, t: Span) -> Vec2 {
        let t = secs(t);
        if t < 0.0 {
            return Vec2::ZERO;
        }
        if self.duration <= 0.0 || t >= self.duration {
            return self.distance;
        }
        let p = cubic_bezier(self.p1.0, self.p1.1, self.p2.0, self.p2.1, t / self.duration);
        self.distance * p as f32
    }

    fn velocity_at(&self, t: Span) -> Vec2 {
        let t = secs(t);
        if t < 0.0 || self.duration <= 0.0 || t >= self.duration {
            return Vec2::ZERO;
        }
        let s = cubic_bezier_slope(self.p1.0, self.p1.1, self.p2.0, self.p2.1, t / self.duration) / self.duration;
        self.distance * s as f32
    }

    fn done_at(&self, t: Span) -> bool {
        secs(t) >= self.duration
    }

    fn retarget(&mut self, _t: Span, _target: Vec2) -> bool {
        false
    }
}

/// iOS-style exponential decay: `v(t) = v₀·d^{t_ms}`, `x(t) = v₀·(d^{t_ms} − 1)/(1000·ln d)`;
/// `d` = 0.998 (normal) or 0.99 (fast) per millisecond. Ends below `stop` px/s; the travel is
/// clamped per axis to `bound`.
#[derive(Clone, Debug, PartialEq)]
pub struct ExponentialFling {
    v0: Vec2,
    ln_d: f64,
    stop: f32,
    bound: Size,
}

/// Flings stop below 0.5 px per 60 Hz frame (svelters' rule, `round2/animation-scroll.md` §4.6).
pub const FLING_STOP_PX_PER_S: f32 = 30.0;

impl ExponentialFling {
    pub fn new(velocity: Vec2, rate_per_ms: f32, viewport: Size) -> Self {
        let bound = Size::new(viewport.w * PHYS_BOUNDS_MULTIPLIER, viewport.h * PHYS_BOUNDS_MULTIPLIER);
        ExponentialFling { v0: velocity, ln_d: (rate_per_ms.clamp(1e-6, 0.999_999) as f64).ln(), stop: FLING_STOP_PX_PER_S, bound }
    }

    /// When the speed falls below the stop threshold.
    pub fn end(&self) -> f64 {
        let v = self.v0.length();
        if v <= self.stop {
            return 0.0;
        }
        ((self.stop / v) as f64).ln() / (1000.0 * self.ln_d)
    }

    fn raw(&self, t: f64) -> Vec2 {
        let k = (((1000.0 * t * self.ln_d).exp() - 1.0) / (1000.0 * self.ln_d)) as f32;
        let clamp = |x: f32, b: f32| if b > 0.0 { x.clamp(-b, b) } else { x };
        Vec2::new(clamp(self.v0.x * k, self.bound.w), clamp(self.v0.y * k, self.bound.h))
    }
}

impl ScrollAnimation for ExponentialFling {
    fn offset_at(&self, t: Span) -> Vec2 {
        self.raw(secs(t).clamp(0.0, self.end()))
    }

    fn velocity_at(&self, t: Span) -> Vec2 {
        let t = secs(t);
        if t < 0.0 || t >= self.end() {
            return Vec2::ZERO;
        }
        self.v0 * (1000.0 * t * self.ln_d).exp() as f32
    }

    fn done_at(&self, t: Span) -> bool {
        secs(t) >= self.end()
    }

    fn retarget(&mut self, _t: Span, _target: Vec2) -> bool {
        false
    }
}

/// Android `OverScroller`'s spline (`SplineOverScroller`).
pub mod android {
    use std::sync::OnceLock;

    pub const DECELERATION_RATE: f64 = 2.358_201_815_840_154_6; // ln(0.78) / ln(0.9)
    pub const INFLEXION: f32 = 0.35;
    pub const START_TENSION: f32 = 0.5;
    pub const END_TENSION: f32 = 1.0;
    pub const SCROLL_FRICTION: f32 = 0.015;
    pub const GRAVITY_EARTH: f32 = 9.80665;
    const P1: f32 = START_TENSION * INFLEXION;
    const P2: f32 = 1.0 - END_TENSION * (1.0 - INFLEXION);
    pub const NB_SAMPLES: usize = 100;

    /// `SPLINE_POSITION`, built exactly as the Java static initialiser does (in `f32`).
    pub fn spline_position() -> &'static [f32; NB_SAMPLES + 1] {
        static TABLE: OnceLock<[f32; NB_SAMPLES + 1]> = OnceLock::new();
        TABLE.get_or_init(|| {
            let mut pos = [0f32; NB_SAMPLES + 1];
            let mut x_min = 0.0f32;
            for (i, slot) in pos.iter_mut().enumerate().take(NB_SAMPLES) {
                let alpha = i as f32 / NB_SAMPLES as f32;
                let mut x_max = 1.0f32;
                let (mut x, mut coef);
                loop {
                    x = x_min + (x_max - x_min) / 2.0;
                    coef = 3.0 * x * (1.0 - x);
                    let tx = coef * ((1.0 - x) * P1 + x * P2) + x * x * x;
                    if (tx - alpha).abs() < 1e-5 {
                        break;
                    }
                    if tx > alpha {
                        x_max = x;
                    } else {
                        x_min = x;
                    }
                }
                *slot = coef * ((1.0 - x) * START_TENSION + x) + x * x * x;
            }
            pos[NB_SAMPLES] = 1.0;
            pos
        })
    }

    /// `mPhysicalCoeff`: g · 39.37 inch/m · ppi · 0.84, with `ppi = density · 160`.
    pub fn physical_coeff(ppi: f32) -> f64 {
        GRAVITY_EARTH as f64 * 39.37 * ppi as f64 * 0.84
    }

    fn deceleration(velocity: f64, coeff: f64) -> f64 {
        (INFLEXION as f64 * velocity.abs() / (SCROLL_FRICTION as f64 * coeff)).ln()
    }

    /// Fling duration in ms for `velocity` px/s.
    pub fn duration_ms(velocity: f64, ppi: f32) -> f64 {
        let l = deceleration(velocity, physical_coeff(ppi));
        1000.0 * (l / (DECELERATION_RATE - 1.0)).exp()
    }

    /// Fling distance in px for `velocity` px/s.
    pub fn distance(velocity: f64, ppi: f32) -> f64 {
        let c = physical_coeff(ppi);
        let l = deceleration(velocity, c);
        SCROLL_FRICTION as f64 * c * (DECELERATION_RATE / (DECELERATION_RATE - 1.0) * l).exp()
    }

    /// (distance coefficient, velocity coefficient) at `t` ∈ [0, 1] of the duration.
    pub fn spline_at(t: f32) -> (f32, f32) {
        let pos = spline_position();
        let index = (NB_SAMPLES as f32 * t) as usize;
        if index < NB_SAMPLES {
            let t_inf = index as f32 / NB_SAMPLES as f32;
            let t_sup = (index + 1) as f32 / NB_SAMPLES as f32;
            let (d_inf, d_sup) = (pos[index], pos[index + 1]);
            let v = (d_sup - d_inf) / (t_sup - t_inf);
            (d_inf + (t - t_inf) * v, v)
        } else {
            (1.0, 0.0)
        }
    }
}

/// Android's spline fling, per axis.
#[derive(Clone, Debug, PartialEq)]
pub struct SplineFling {
    distance: Vec2,
    duration: Vec2,
}

impl SplineFling {
    pub fn new(velocity: Vec2, ppi: f32) -> Self {
        let axis = |v: f32| -> (f32, f32) {
            if v == 0.0 {
                return (0.0, 0.0);
            }
            let d = android::distance(v as f64, ppi) as f32;
            (d.copysign(v), (android::duration_ms(v as f64, ppi) / 1000.0) as f32)
        };
        let (dx, tx) = axis(velocity.x);
        let (dy, ty) = axis(velocity.y);
        SplineFling { distance: Vec2::new(dx, dy), duration: Vec2::new(tx, ty) }
    }

    fn axis(d: f32, dur: f32, t: f64) -> (f32, f32) {
        if dur <= 0.0 {
            return (d, 0.0);
        }
        let x = (t as f32 / dur).clamp(0.0, 1.0);
        let (dc, vc) = android::spline_at(x);
        (dc * d, vc * d / dur)
    }
}

impl ScrollAnimation for SplineFling {
    fn offset_at(&self, t: Span) -> Vec2 {
        let t = secs(t).max(0.0);
        Vec2::new(Self::axis(self.distance.x, self.duration.x, t).0, Self::axis(self.distance.y, self.duration.y, t).0)
    }

    fn velocity_at(&self, t: Span) -> Vec2 {
        let t = secs(t).max(0.0);
        Vec2::new(Self::axis(self.distance.x, self.duration.x, t).1, Self::axis(self.distance.y, self.duration.y, t).1)
    }

    fn done_at(&self, t: Span) -> bool {
        secs(t) >= self.duration.x.max(self.duration.y) as f64
    }

    fn retarget(&mut self, _t: Span, _target: Vec2) -> bool {
        false
    }
}

/// A fling of any curve.
#[derive(Clone, Debug, PartialEq)]
pub enum Fling {
    Physics(PhysicsFling),
    Exponential(ExponentialFling),
    Spline(SplineFling),
}

impl Fling {
    /// `velocity` px/s at release; `ppi` the display's pixels per inch.
    pub fn new(curve: FlingCurve, velocity: Vec2, ppi: f32, viewport: Size) -> Self {
        match curve {
            FlingCurve::Chromium => Fling::Physics(PhysicsFling::new(velocity, Vec2::new(ppi, ppi), 1.0, viewport)),
            FlingCurve::Exponential { rate_per_ms } => Fling::Exponential(ExponentialFling::new(velocity, rate_per_ms, viewport)),
            FlingCurve::AndroidSpline => Fling::Spline(SplineFling::new(velocity, ppi)),
        }
    }

    fn inner(&self) -> &dyn ScrollAnimation {
        match self {
            Fling::Physics(f) => f,
            Fling::Exponential(f) => f,
            Fling::Spline(f) => f,
        }
    }
}

impl ScrollAnimation for Fling {
    fn offset_at(&self, t: Span) -> Vec2 {
        self.inner().offset_at(t)
    }
    fn velocity_at(&self, t: Span) -> Vec2 {
        self.inner().velocity_at(t)
    }
    fn done_at(&self, t: Span) -> bool {
        self.inner().done_at(t)
    }
    fn retarget(&mut self, _t: Span, _target: Vec2) -> bool {
        false
    }
}

// --- overscroll -------------------------------------------------------------------------------

/// The iOS-style resistance curve: `(1 − 1/(x·c/d + 1))·d`, `c = 0.55`, `d` the viewport dimension.
pub fn rubber_band(overscroll: f32, dimension: f32) -> f32 {
    const C: f32 = 0.55;
    if dimension <= 0.0 {
        return 0.0;
    }
    let x = overscroll.abs();
    ((1.0 - 1.0 / (x * C / dimension + 1.0)) * dimension).copysign(overscroll)
}

/// Velocity carried into the rubber band when momentum hits the edge (Flutter `BouncingScrollPhysics`).
pub const RUBBER_BAND_MAX_VELOCITY: f32 = 5000.0;
/// The return spring's ω₀ (GTK `OVERSHOOT_FRICTION 20` is `2ω₀`).
pub const RUBBER_BAND_OMEGA: f64 = 10.0;

/// The critically damped return of an overscroll to 0 (per axis).
#[derive(Clone, Debug, PartialEq)]
pub struct RubberBandReturn {
    spring: Spring,
    e0: Vec2,
    v0: Vec2,
}

impl RubberBandReturn {
    pub fn new(overscroll: Vec2, velocity: Vec2) -> Self {
        let cap = |v: f32| v.clamp(-RUBBER_BAND_MAX_VELOCITY, RUBBER_BAND_MAX_VELOCITY);
        RubberBandReturn { spring: Spring::omega_zeta(RUBBER_BAND_OMEGA, 1.0), e0: overscroll, v0: Vec2::new(cap(velocity.x), cap(velocity.y)) }
    }
}

impl ScrollAnimation for RubberBandReturn {
    fn offset_at(&self, t: Span) -> Vec2 {
        let t = secs(t).max(0.0);
        let x = self.spring.eval(self.e0.x as f64, self.v0.x as f64, t).0;
        let y = self.spring.eval(self.e0.y as f64, self.v0.y as f64, t).0;
        Vec2::new(x as f32, y as f32)
    }

    fn velocity_at(&self, t: Span) -> Vec2 {
        let t = secs(t).max(0.0);
        let x = self.spring.eval(self.e0.x as f64, self.v0.x as f64, t).1;
        let y = self.spring.eval(self.e0.y as f64, self.v0.y as f64, t).1;
        Vec2::new(x as f32, y as f32)
    }

    /// Within half a pixel and 1 px/s of rest.
    fn done_at(&self, t: Span) -> bool {
        let o = self.offset_at(t);
        let v = self.velocity_at(t);
        o.x.abs() < 0.5 && o.y.abs() < 0.5 && v.length() < 1.0
    }

    fn retarget(&mut self, _t: Span, _target: Vec2) -> bool {
        false
    }
}

// --- velocity, latching, defaults -----------------------------------------------------------------

/// The window the fling velocity is fitted over (GTK `SCROLL_CAPTURE_THRESHOLD_MS`).
pub const VELOCITY_WINDOW: Span = Span::from_millis(150);
/// No fling when the last sample is older than this (Chromium `kFlingStartTimeoutMs`).
pub const FLING_START_TIMEOUT: Span = Span::from_millis(200);

/// Least-squares velocity over the recent scroll deltas (Chromium's Wayland fling, after
/// libgestures `RegressScrollVelocity`).
#[derive(Clone, Debug, Default)]
pub struct VelocityTracker {
    samples: VecDeque<(Time, Vec2)>,
    pos: Vec2,
}

impl VelocityTracker {
    pub fn new() -> Self {
        VelocityTracker::default()
    }

    pub fn reset(&mut self) {
        self.samples.clear();
        self.pos = Vec2::ZERO;
    }

    pub fn add(&mut self, at: Time, delta: Vec2) {
        self.pos = self.pos + delta;
        self.samples.push_back((at, self.pos));
        while self.samples.front().is_some_and(|&(t, _)| at - t > VELOCITY_WINDOW) {
            self.samples.pop_front();
        }
    }

    /// px/s, or `None` with fewer than two samples or a stale stream.
    pub fn velocity(&self, now: Time) -> Option<Vec2> {
        let &(last, _) = self.samples.back()?;
        if now - last > FLING_START_TIMEOUT || self.samples.len() < 2 {
            return None;
        }
        let n = self.samples.len() as f64;
        let t0 = self.samples[0].0;
        let ts: Vec<f64> = self.samples.iter().map(|&(t, _)| (t - t0).as_secs_f64()).collect();
        let mt = ts.iter().sum::<f64>() / n;
        let var: f64 = ts.iter().map(|t| (t - mt) * (t - mt)).sum();
        if var <= 0.0 {
            return None;
        }
        let fit = |f: fn(&Vec2) -> f32| {
            let mx = self.samples.iter().map(|(_, p)| f(p) as f64).sum::<f64>() / n;
            let cov: f64 = self.samples.iter().zip(&ts).map(|((_, p), t)| (t - mt) * (f(p) as f64 - mx)).sum();
            (cov / var) as f32
        };
        Some(Vec2::new(fit(|p| p.x), fit(|p| p.y)))
    }
}

/// Where a scroll event is in its gesture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GesturePhase {
    Began,
    Changed,
    Ended,
    MomentumBegan,
    Momentum,
    MomentumEnded,
    /// A wheel tick without phase information.
    Wheel,
}

/// How long discrete wheel ticks stay latched to one scroller. No source states the browsers'
/// number; 300 ms is the research's choice (`round2/animation-scroll.md` §6 q4).
pub const WHEEL_LATCH_TIMEOUT: Span = Span::from_millis(300);

/// Per-gesture scroll latching: a gesture (including its momentum) scrolls the scroller chosen at
/// its start, never chaining mid-gesture.
#[derive(Clone, Debug, Default)]
pub struct ScrollLatch {
    latched: Option<NodeId>,
    last: Option<Time>,
}

impl ScrollLatch {
    pub fn new() -> Self {
        ScrollLatch::default()
    }

    pub fn latched(&self) -> Option<NodeId> {
        self.latched
    }

    /// The scroller this event goes to. `choose` picks the innermost scroller that can move in the
    /// event's direction; it is called only when a new latch starts.
    pub fn route(&mut self, now: Time, phase: GesturePhase, choose: impl FnOnce() -> Option<NodeId>) -> Option<NodeId> {
        let fresh = match phase {
            GesturePhase::Began => true,
            GesturePhase::Wheel => self.last.is_none_or(|l| now - l > WHEEL_LATCH_TIMEOUT) || self.latched.is_none(),
            _ => self.latched.is_none() && phase == GesturePhase::MomentumBegan,
        };
        if fresh {
            self.latched = choose();
        }
        self.last = Some(now);
        let target = self.latched;
        if phase == GesturePhase::MomentumEnded {
            self.latched = None;
        }
        target
    }

    /// The fingers lifted with no momentum to follow (the platform knows; call it then).
    pub fn release(&mut self) {
        self.latched = None;
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Os {
    Windows,
    MacOs,
    Linux,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Overscroll {
    None,
    RubberBand,
}

/// The per-OS defaults of `round2/animation-scroll.md` §4.6.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollDefaults {
    /// Pixels per wheel detent (macOS: per non-precise tick, before the OS acceleration).
    pub wheel_px_per_detent: f32,
    /// Wheel steps animate with [`DurationBehavior::InverseDelta`].
    pub wheel_animated: bool,
    /// Our own fling for touchpads; `None` where the OS sends momentum.
    pub touchpad_fling: Option<FlingCurve>,
    pub touch_fling: Option<FlingCurve>,
    pub overscroll: Overscroll,
}

/// `lines_per_detent` is Windows' `SPI_GETWHEELSCROLLLINES` (default 3).
pub fn defaults(os: Os, lines_per_detent: u32) -> ScrollDefaults {
    match os {
        Os::Windows => ScrollDefaults {
            wheel_px_per_detent: lines_per_detent as f32 * 100.0 / 3.0,
            wheel_animated: true,
            touchpad_fling: None,
            touch_fling: None,
            overscroll: Overscroll::None,
        },
        Os::MacOs => ScrollDefaults {
            wheel_px_per_detent: 40.0,
            wheel_animated: false,
            touchpad_fling: None,
            touch_fling: None,
            overscroll: Overscroll::RubberBand,
        },
        Os::Linux => ScrollDefaults {
            wheel_px_per_detent: 120.0,
            wheel_animated: true,
            touchpad_fling: Some(FlingCurve::Chromium),
            touch_fling: Some(FlingCurve::AndroidSpline),
            overscroll: Overscroll::None,
        },
    }
}

/// With reduced motion, smooth curves are instant; flings stay (they are direct manipulation).
pub fn animate_wheel(defaults: &ScrollDefaults, reduced_motion: bool) -> bool {
    defaults.wheel_animated && !reduced_motion
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inverse_delta_durations() {
        let ms = |px: f32| (segment_duration(Vec2::new(0.0, px), DurationBehavior::InverseDelta) * 1000.0).round();
        assert_eq!(ms(100.0), 200.0);
        assert_eq!(ms(120.0), 200.0);
        assert_eq!(ms(300.0), 150.0);
        assert_eq!(ms(480.0), 100.0);
        assert_eq!(ms(2000.0), 100.0);
        assert_eq!((segment_duration(Vec2::ZERO, DurationBehavior::Constant) * 1000.0).round(), 150.0);
        // √900 = 30 frames = 0.5 s
        assert!((segment_duration(Vec2::new(900.0, 0.0), DurationBehavior::DeltaBased) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn smooth_scroll_ends_on_target_and_retargets_smoothly() {
        let mut s = SmoothScroll::new(Vec2::ZERO, Vec2::new(0.0, 100.0), DurationBehavior::InverseDelta);
        assert_eq!(s.offset_at(Span::ZERO), Vec2::ZERO);
        assert_eq!(s.offset_at(Span::from_millis(200)), Vec2::new(0.0, 100.0));
        let t = Span::from_millis(100);
        let before = s.offset_at(t);
        let v_before = s.velocity_at(t);
        assert!(s.retarget(t, Vec2::new(0.0, 200.0)));
        let after = s.offset_at(t);
        assert!((after.y - before.y).abs() < 1e-3, "position continuous");
        let v_after = s.velocity_at(t);
        assert!((v_after.y - v_before.y).abs() / v_before.y.abs() < 0.02, "velocity kept: {v_before:?} → {v_after:?}");
        assert!(s.done_at(s.duration()));
        assert_eq!(s.offset_at(s.duration()), Vec2::new(0.0, 200.0));
    }

    #[test]
    fn physics_fling_fits_release_velocity_and_bounds() {
        let f = PhysicsFling::new(Vec2::new(0.0, 2000.0), Vec2::new(96.0, 96.0), 1.0, Size::new(800.0, 600.0));
        assert!((f.duration().as_secs_f64() - 2000.0 / 2300.0).abs() < 1e-6);
        assert!(f.distance().y > 0.0 && f.distance().y <= 1800.0);
        let v0 = f.velocity_at(Span::ZERO).y;
        assert!((v0 - 2000.0).abs() / 2000.0 < 0.05, "initial slope fitted: {v0}");
        assert_eq!(f.offset_at(f.duration()), f.distance());
        let big = PhysicsFling::new(Vec2::new(0.0, 50_000.0), Vec2::new(96.0, 96.0), 1.0, Size::new(800.0, 600.0));
        assert_eq!(big.distance().y, 1800.0);
    }

    #[test]
    fn exponential_fling_travel() {
        // travel → v₀ · (−1/ln 0.998)/1000 ≈ v₀ · 0.4995 s, before the stop threshold cuts it
        let f = ExponentialFling::new(Vec2::new(1000.0, 0.0), 0.998, Size::new(10_000.0, 10_000.0));
        let end = f.offset_at(Span::from_millis(10_000)).x;
        assert!(end > 480.0 && end < 500.0, "{end}");
        assert!(f.done_at(Span::from_secs_f64(f.end() + 1e-3)));
    }

    #[test]
    fn android_spline_table_and_fling() {
        let t = android::spline_position();
        // the Java table bisects to 1e-5, so its first entry is near 0, not 0
        assert!(t[0] < 1e-4);
        assert_eq!(t[android::NB_SAMPLES], 1.0);
        assert!(t.windows(2).all(|w| w[1] >= w[0]));
        let f = SplineFling::new(Vec2::new(0.0, 4000.0), 160.0 * 2.0);
        assert!(f.offset_at(Span::from_secs_f64(100.0)).y > 0.0);
        assert!(f.velocity_at(Span::ZERO).y > 0.0);
    }

    #[test]
    fn rubber_band_resists_and_returns() {
        assert_eq!(rubber_band(0.0, 600.0), 0.0);
        let a = rubber_band(100.0, 600.0);
        assert!(a > 0.0 && a < 100.0);
        assert!(rubber_band(10_000.0, 600.0) < 600.0);
        assert_eq!(rubber_band(-100.0, 600.0), -a);
        let r = RubberBandReturn::new(Vec2::new(0.0, 80.0), Vec2::ZERO);
        assert!(!r.done_at(Span::ZERO));
        assert!(r.offset_at(Span::from_millis(300)).y < 80.0);
        assert!(r.done_at(Span::from_millis(1500)));
        // critically damped from rest: never crosses 0
        assert!((0..150).all(|ms| r.offset_at(Span::from_millis(ms * 10)).y >= -1e-3));
    }

    #[test]
    fn velocity_regression() {
        let mut v = VelocityTracker::new();
        for i in 0..10 {
            v.add(Time(0) + Span::from_millis(10 * i), Vec2::new(0.0, 5.0));
        }
        let vel = v.velocity(Time(0) + Span::from_millis(95)).unwrap();
        assert!((vel.y - 500.0).abs() < 1.0, "{vel:?}");
        assert!(v.velocity(Time(0) + Span::from_millis(400)).is_none());
    }

    #[test]
    fn latching_per_gesture() {
        let a = NodeId(1);
        let b = NodeId(2);
        let mut l = ScrollLatch::new();
        let t = |ms: i64| Time(0) + Span::from_millis(ms);
        assert_eq!(l.route(t(0), GesturePhase::Began, || Some(a)), Some(a));
        assert_eq!(l.route(t(10), GesturePhase::Changed, || Some(b)), Some(a));
        assert_eq!(l.route(t(20), GesturePhase::Ended, || Some(b)), Some(a));
        assert_eq!(l.route(t(30), GesturePhase::MomentumBegan, || Some(b)), Some(a), "momentum stays latched");
        assert_eq!(l.route(t(40), GesturePhase::MomentumEnded, || Some(b)), Some(a));
        assert_eq!(l.latched(), None);
        assert_eq!(l.route(t(50), GesturePhase::Wheel, || Some(b)), Some(b));
        assert_eq!(l.route(t(300), GesturePhase::Wheel, || Some(a)), Some(b));
        assert_eq!(l.route(t(700), GesturePhase::Wheel, || Some(a)), Some(a));
    }

    #[test]
    fn per_os_defaults() {
        assert_eq!(defaults(Os::Windows, 3).wheel_px_per_detent, 100.0);
        assert!(!defaults(Os::MacOs, 3).wheel_animated);
        assert!(!animate_wheel(&defaults(Os::Linux, 3), true));
    }
}
