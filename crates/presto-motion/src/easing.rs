//! svelte/easing's curves as presto-motion uses them: [`SvelteEasing`] (presto-base's) evaluated
//! with `presto_anim::easing::svelte`, Svelte's formulas in `f64`, and as the presto [`Easing`] a
//! compositor track carries ([`Easing::Svelte`], exact).
//!
//! The JS side (`packages/native-runtime/src/motion.js`) names a curve by its export name when the
//! function is svelte/easing's own; any other easing function stays in JS (the 60 Hz keyframes).

use presto_base::anim::Easing;
pub use presto_base::anim::{SVELTE_EASINGS as ALL, SvelteEasing};

/// Evaluating a [`SvelteEasing`] and handing it to presto.
pub trait SvelteEasingExt {
    /// The curve at `t`, as svelte/easing computes it.
    fn eval(self, t: f64) -> f64;
    /// The curve as presto's easing for a compositor track.
    fn to_presto(self) -> Easing;
}

impl SvelteEasingExt for SvelteEasing {
    fn eval(self, t: f64) -> f64 {
        presto_anim::easing::svelte::eval(self, t)
    }

    fn to_presto(self) -> Easing {
        match self {
            SvelteEasing::Linear => Easing::Linear,
            e => Easing::Svelte(e),
        }
    }
}
