//! Springs as closed forms (`round2/animation-scroll.md` §3, architecture §12.1): the damped harmonic
//! oscillator `ẍ = −ω₀²(x − T) − 2ζω₀ẋ` evaluated at any time from its two poles (distinct real,
//! repeated, or a complex pair), so 60 Hz, 120 Hz and VRR give the same curve and the UI thread and
//! the compositor agree.
//!
//! Three parameterisations map onto the poles:
//! - `{duration, bounce}` (SwiftUI `Spring`, Flutter `SpringDescription.withDurationAndBounce`):
//!   `k = (2π/duration)²`, `ζ = 1 − bounce` for `bounce ≥ 0`, else `1/(1 + bounce)`, mass 1;
//! - physical `{mass, stiffness, damping}`;
//! - Svelte's `{stiffness, damping}` (svelte@5.57.2 `motion/spring.js` `tick_spring`): at 60 Hz its
//!   update is the recurrence `e[n+1] = (2 − c − k)·e[n] − (1 − c)·e[n−1]`, whose roots `r` become
//!   poles `s = 60·ln r`. The closed form then equals Svelte's integrator exactly at 60 Hz, given the
//!   initial velocity [`Spring::svelte_velocity`] derives from Svelte's last two values.

use std::f64::consts::TAU;

use skelter_base::anim::SpringParams;

/// The smallest recurrence root kept (a root of 0, Svelte `damping: 1`, is an infinitely fast pole).
const MIN_ROOT: f64 = 1e-9;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Poles {
    Real(f64, f64),
    Repeated(f64),
    /// `σ ± iω`.
    Complex { sigma: f64, omega: f64 },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spring {
    poles: Poles,
}

impl Spring {
    /// `m·ẍ + c·ẋ + k·x = 0`.
    pub fn physical(mass: f64, stiffness: f64, damping: f64) -> Self {
        let m = mass.max(1e-9);
        let disc = damping * damping - 4.0 * m * stiffness;
        let poles = if disc.abs() < 1e-12 * (damping * damping).max(1.0) {
            Poles::Repeated(-damping / (2.0 * m))
        } else if disc > 0.0 {
            let r = disc.sqrt();
            Poles::Real((-damping + r) / (2.0 * m), (-damping - r) / (2.0 * m))
        } else {
            Poles::Complex { sigma: -damping / (2.0 * m), omega: (-disc).sqrt() / (2.0 * m) }
        };
        Spring { poles }
    }

    pub fn omega_zeta(omega0: f64, zeta: f64) -> Self {
        Spring::physical(1.0, omega0 * omega0, 2.0 * zeta * omega0)
    }

    pub fn duration_bounce(duration_s: f64, bounce: f64) -> Self {
        let k = (TAU / duration_s.max(1e-6)).powi(2);
        let zeta = if bounce >= 0.0 { 1.0 - bounce } else { 1.0 / (1.0 + bounce).max(1e-6) };
        Spring::physical(1.0, k, 2.0 * zeta * k.sqrt())
    }

    /// Svelte's per-frame `stiffness` and `damping` (each clamped to [0, 1] as Svelte does), mapped
    /// through the 60 Hz recurrence roots.
    pub fn svelte(stiffness: f64, damping: f64) -> Self {
        let k = stiffness.clamp(0.0, 1.0);
        let c = damping.clamp(0.0, 1.0);
        let sum = 2.0 - c - k;
        let prod = 1.0 - c;
        let disc = sum * sum - 4.0 * prod;
        let poles = if disc.abs() < 1e-14 {
            Poles::Repeated(60.0 * (sum / 2.0).max(MIN_ROOT).ln())
        } else if disc > 0.0 {
            let r1 = (sum + disc.sqrt()) / 2.0;
            let r2 = (sum - disc.sqrt()) / 2.0;
            Poles::Real(60.0 * r1.max(MIN_ROOT).ln(), 60.0 * r2.max(MIN_ROOT).ln())
        } else {
            let re = sum / 2.0;
            let im = (-disc).sqrt() / 2.0;
            let rho = (re * re + im * im).sqrt().max(MIN_ROOT);
            Poles::Complex { sigma: 60.0 * rho.ln(), omega: 60.0 * im.atan2(re) }
        };
        Spring { poles }
    }

    pub fn from_params(p: &SpringParams) -> Self {
        match *p {
            SpringParams::DurationBounce { duration, bounce } => Spring::duration_bounce(duration.as_secs_f64(), bounce as f64),
            SpringParams::Physical { mass, stiffness, damping } => Spring::physical(mass as f64, stiffness as f64, damping as f64),
            SpringParams::Svelte { stiffness, damping, .. } => Spring::svelte(stiffness as f64, damping as f64),
        }
    }

    /// The natural frequency ω₀ (rad/s): `√(s₁·s₂)`.
    pub fn omega0(&self) -> f64 {
        match self.poles {
            Poles::Real(a, b) => (a * b).max(0.0).sqrt(),
            Poles::Repeated(s) => s.abs(),
            Poles::Complex { sigma, omega } => (sigma * sigma + omega * omega).sqrt(),
        }
    }

    /// The damping ratio ζ: `−(s₁ + s₂) / 2ω₀`.
    pub fn zeta(&self) -> f64 {
        let w = self.omega0();
        if w == 0.0 {
            return f64::INFINITY;
        }
        match self.poles {
            Poles::Real(a, b) => -(a + b) / (2.0 * w),
            Poles::Repeated(_) => 1.0,
            Poles::Complex { sigma, .. } => -sigma / w,
        }
    }

    /// Displacement from the target and velocity at `t` seconds, from `(e0, v0)` at 0.
    pub fn eval(&self, e0: f64, v0: f64, t: f64) -> (f64, f64) {
        match self.poles {
            Poles::Real(s1, s2) => {
                // e = A·e^{s1 t} + B·e^{s2 t}
                let b = (v0 - s1 * e0) / (s2 - s1);
                let a = e0 - b;
                let (x1, x2) = ((s1 * t).exp(), (s2 * t).exp());
                (a * x1 + b * x2, a * s1 * x1 + b * s2 * x2)
            }
            Poles::Repeated(s) => {
                // e = (A + B·t)·e^{s t}
                let a = e0;
                let b = v0 - s * e0;
                let x = (s * t).exp();
                ((a + b * t) * x, (b + s * (a + b * t)) * x)
            }
            Poles::Complex { sigma, omega } => {
                // e = e^{σt}(A cos ωt + B sin ωt)
                let a = e0;
                let b = (v0 - sigma * e0) / omega;
                let x = (sigma * t).exp();
                let (sn, cs) = (omega * t).sin_cos();
                let e = x * (a * cs + b * sn);
                let v = x * ((sigma * a + omega * b) * cs + (sigma * b - omega * a) * sn);
                (e, v)
            }
        }
    }

    /// The continuous initial velocity that makes [`Spring::eval`] pass through Svelte's values at
    /// every 60 Hz frame, given its current displacement `e0` and the previous frame's `e_prev`
    /// (equal when Svelte starts from rest). Only meaningful for [`Spring::svelte`].
    pub fn svelte_velocity(&self, e0: f64, e_prev: f64) -> f64 {
        match self.poles {
            Poles::Real(s1, s2) => {
                let (q1, q2) = ((-s1 / 60.0).exp(), (-s2 / 60.0).exp());
                // A + B = e0, A/r1 + B/r2 = e_prev
                let a = (e_prev - e0 * q2) / (q1 - q2);
                let b = e0 - a;
                a * s1 + b * s2
            }
            Poles::Repeated(s) => {
                // e_n = (A + B·n)·r^n: A = e0, (A − B)/r = e_prev.
                let r = (s / 60.0).exp();
                let a = e0;
                let b = a - r * e_prev;
                a * s + 60.0 * b
            }
            Poles::Complex { sigma, omega } => {
                // e_n = Re((p + iq)·r^n), r = u + iw: p = e0, (p·u + q·w)/|r|² = e_prev.
                let rho = (sigma / 60.0).exp();
                let (w, u) = ((omega / 60.0).sin() * rho, (omega / 60.0).cos() * rho);
                let p = e0;
                let q = (e_prev * rho * rho - p * u) / w;
                // e(t) = e^{σt}(p cos ωt − q sin ωt)
                sigma * p - omega * q
            }
        }
    }

    /// Svelte's settle rule on a spring of `span` value units: `|e| < precision` and the next
    /// 60 Hz step `|v/60| < precision`.
    pub fn settled(e: f64, v: f64, span: f64, precision: f64) -> bool {
        (e * span).abs() < precision && (v / 60.0 * span).abs() < precision
    }

    /// The first time (to 1 ms, at most 60 s) from which the spring stays settled.
    pub fn settle_time(&self, e0: f64, v0: f64, span: f64, precision: f64) -> f64 {
        const STEP: f64 = 0.001;
        let mut last_unsettled = 0.0;
        let mut t = 0.0;
        while t < 60.0 {
            let (e, v) = self.eval(e0, v0, t);
            if !Spring::settled(e, v, span, precision) {
                last_unsettled = t;
            } else if t - last_unsettled > 1.0 && self.decays() {
                break;
            }
            t += STEP;
        }
        if last_unsettled == 0.0 && Spring::settled(e0, v0, span, precision) { 0.0 } else { last_unsettled + STEP }
    }

    fn decays(&self) -> bool {
        match self.poles {
            Poles::Real(a, b) => a < 0.0 && b < 0.0,
            Poles::Repeated(s) => s < 0.0,
            Poles::Complex { sigma, .. } => sigma < 0.0,
        }
    }
}

/// The settle precision of a parameterisation, in value units (Svelte's own; 0.001 of the span for
/// the others).
pub fn precision(p: &SpringParams, span: f64) -> f64 {
    match *p {
        SpringParams::Svelte { precision, .. } => precision as f64,
        _ => 1e-3 * span.max(1e-9),
    }
}

/// Svelte's `tick_spring` at a fixed frame step for one number, for tests and comparisons:
/// `(last, current, target, dt in 60 Hz frames) → next` (None when settled).
pub fn svelte_tick(stiffness: f64, damping: f64, precision: f64, last: f64, current: f64, target: f64, dt: f64) -> Option<f64> {
    let delta = target - current;
    let velocity = (current - last) / if dt == 0.0 { 1.0 / 60.0 } else { dt };
    let spring = stiffness * delta;
    let damper = damping * velocity;
    let acceleration = spring - damper;
    let d = (velocity + acceleration) * dt;
    if d.abs() < precision && delta.abs() < precision { None } else { Some(current + d) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_svelte_spring_poles() {
        // round2/animation-scroll.md §3.1: {0.15, 0.8} → ω₀ = 33.4 rad/s, ζ = 1.45.
        let s = Spring::svelte(0.15, 0.8);
        assert!((s.omega0() - 33.37).abs() < 0.05, "{}", s.omega0());
        assert!((s.zeta() - 1.447).abs() < 0.005, "{}", s.zeta());
        // {0.1, 0.25} → ω₀ = 20.5, ζ = 0.42.
        let u = Spring::svelte(0.1, 0.25);
        assert!((u.omega0() - 20.5).abs() < 0.1, "{}", u.omega0());
        assert!((u.zeta() - 0.42).abs() < 0.01, "{}", u.zeta());
    }

    fn matches_svelte_at_60hz(k: f64, c: f64) {
        let s = Spring::svelte(k, c);
        let (from, to) = (0.0, 100.0);
        let e0 = from - to;
        let v0 = s.svelte_velocity(e0, e0);
        let (mut last, mut cur) = (from, from);
        for n in 1..=120 {
            match svelte_tick(k, c, 1e-9, last, cur, to, 1.0) {
                Some(next) => {
                    last = cur;
                    cur = next;
                }
                None => break,
            }
            let (e, _) = s.eval(e0, v0, n as f64 / 60.0);
            assert!((to + e - cur).abs() < 1e-6, "k={k} c={c} n={n}: closed {} svelte {cur}", to + e);
        }
    }

    #[test]
    fn closed_form_equals_svelte_integrator_at_60hz() {
        matches_svelte_at_60hz(0.15, 0.8);
        matches_svelte_at_60hz(0.1, 0.25);
        matches_svelte_at_60hz(0.5, 0.5);
        matches_svelte_at_60hz(0.2, 0.1);
    }

    #[test]
    fn duration_bounce_zero_is_critical() {
        let s = Spring::duration_bounce(0.5, 0.0);
        assert!((s.zeta() - 1.0).abs() < 1e-9);
        assert!((s.omega0() - TAU / 0.5).abs() < 1e-9);
        // "duration is approximately the settling duration": within 2% after it
        let (e, _) = s.eval(-1.0, 0.0, 0.5);
        assert!(e.abs() < 0.02);
    }

    #[test]
    fn retarget_keeps_velocity() {
        let s = Spring::omega_zeta(20.0, 0.5);
        let (e, v) = s.eval(-1.0, 0.0, 0.1);
        // the same physical state against a new target: displacement shifts, velocity stays
        let (_, v2) = s.eval(e - 0.5, v, 0.0);
        assert!((v2 - v).abs() < 1e-9);
    }
}
