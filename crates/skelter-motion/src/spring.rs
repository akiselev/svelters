//! svelte/motion's spring as a motion with Svelte's settle rule and retargeting, on
//! `skelter_anim::spring::Spring::svelte`'s closed form.
//!
//! Svelte (`src/motion/spring.js`, `tick_spring`) steps, per frame of `dt` sixtieths of a second,
//! `v = (x - x_prev) / dt`, `a = (k (T - x) - c v) * inv_mass`, `x += (v + a) dt`. At 60 Hz
//! (`dt = 1`, `inv_mass = 1`) the error `e = x - T` follows the linear recurrence
//! `e[n+1] = (2 - k - c) e[n] - (1 - c) e[n-1]`, so `e[n] = A r1^n + B r2^n` for the roots of
//! `r² - (2 - k - c) r + (1 - c)`. Evaluating that at real `n = 60 * seconds` gives a spring that
//! equals Svelte's at every 60 Hz frame and is frame-rate independent between them (Svelte's own
//! integrator changes with the frame rate). It settles where Svelte does: at the first frame whose
//! error and next step are both under `precision`.
//!
//! The momentum options (`preserveMomentum`, `soft`) ramp the inverse mass per frame, which has no
//! closed form; the JS module runs Svelte's integrator for those.

use skelter_anim::spring::Spring;

/// svelte/motion's spring options, per 60 Hz frame (`Spring` clamps stiffness and damping to 0..=1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SvelteSpring {
    pub stiffness: f64,
    pub damping: f64,
    pub precision: f64,
}

impl Default for SvelteSpring {
    fn default() -> Self {
        SvelteSpring { stiffness: 0.15, damping: 0.8, precision: 0.01 }
    }
}

/// One spring motion from a start state toward `target`, in frames (`n = 60 * seconds`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpringMotion {
    pub params: SvelteSpring,
    pub target: f64,
    spring: Spring,
    e0: f64,
    /// The continuous start velocity (per second) through Svelte's last two values.
    v0: f64,
}

impl SpringMotion {
    /// A motion from `x0` with Svelte's previous value `x_prev` (the frame before; `x_prev = x0` from rest).
    pub fn new(params: SvelteSpring, x0: f64, x_prev: f64, target: f64) -> SpringMotion {
        let spring = Spring::svelte(params.stiffness, params.damping);
        let e0 = x0 - target;
        let v0 = spring.svelte_velocity(e0, x_prev - target);
        SpringMotion { params, target, spring, e0, v0 }
    }

    /// The error `x - target` at frame `n` (real, ≥ 0).
    pub fn error(&self, n: f64) -> f64 {
        self.spring.eval(self.e0, self.v0, n.max(0.0) / 60.0).0
    }

    /// The value at frame `n`, before settling ([`SpringMotion::value`] snaps to the target).
    pub fn raw(&self, n: f64) -> f64 {
        self.target + self.error(n)
    }

    /// Whether Svelte has settled by frame `n`: some frame `m < n` had `|e[m]|` and `|e[m+1] - e[m]|`
    /// under `precision` (Svelte then writes the target at frame `m + 1`).
    pub fn settled(&self, n: f64) -> bool {
        // `n` comes from a clock: allow its rounding (41.9999... is frame 42)
        self.settle_frame().is_some_and(|m| n >= (m + 1) as f64 - 1e-6)
    }

    /// The value at frame `n`: the closed form until the settle frame, then the target.
    pub fn value(&self, n: f64) -> f64 {
        if self.settled(n) { self.target } else { self.raw(n) }
    }

    /// The first integer frame at which Svelte's settle check passes (it writes the target one frame
    /// later); `None` if not within 100 000 frames (a spring with no damping).
    pub fn settle_frame(&self) -> Option<u64> {
        let p = self.params.precision;
        let mut e = self.error(0.0);
        for m in 0..100_000u64 {
            let next = self.error((m + 1) as f64);
            if (next - e).abs() < p && e.abs() < p {
                return Some(m);
            }
            e = next;
        }
        None
    }

    /// Svelte's `set` while running: keep the position and the last frame's step, head for `target`
    /// (the returned motion's frame 0 is this one's frame `n`).
    pub fn retarget(&self, n: f64, target: f64) -> SpringMotion {
        let x = self.value(n);
        let x_prev = if self.settled(n) { x } else { self.raw(n - 1.0) };
        SpringMotion::new(self.params, x, x_prev, target)
    }

    /// The equivalent continuous spring per second (unit mass): `(stiffness, damping)` with
    /// `x'' = -stiffness e - damping x'`.
    pub fn physical(&self) -> (f64, f64) {
        let w = self.spring.omega0();
        (w * w, 2.0 * self.spring.zeta() * w)
    }

    /// The continuous velocity (units per second) at frame `n`: with [`SpringMotion::physical`], the
    /// initial velocity that makes a physical spring follow this motion.
    pub fn velocity(&self, n: f64) -> f64 {
        self.spring.eval(self.e0, self.v0, n.max(0.0) / 60.0).1
    }
}

/// Svelte's 60 Hz integrator (`tick_spring` with `dt = 1`, `inv_mass = 1`) from rest at `x0`, frame
/// by frame until it settles or `max` frames: the oracle the closed form is tested against.
pub fn svelte_frames(params: SvelteSpring, x0: f64, target: f64, max: usize) -> Vec<f64> {
    let mut out = vec![x0];
    let (mut last, mut cur) = (x0, x0);
    for _ in 0..max {
        let delta = target - cur;
        let velocity = cur - last;
        let spring = params.stiffness * delta;
        let damper = params.damping * velocity;
        let acceleration = spring - damper;
        let d = velocity + acceleration;
        // settled: Svelte writes the target and stops the loop
        if d.abs() < params.precision && delta.abs() < params.precision {
            out.push(target);
            break;
        }
        last = cur;
        cur += d;
        out.push(cur);
    }
    out
}
