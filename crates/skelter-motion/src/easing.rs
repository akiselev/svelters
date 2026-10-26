//! svelte/easing's curves as skelter-motion uses them: [`SvelteEasing`] (skelter-base's) evaluated
//! with `skelter_anim::easing::svelte`, Svelte's formulas in `f64`, and as the skelter [`Easing`] a
//! compositor track carries ([`Easing::Svelte`], exact).
//!
//! The JS side (`packages/native-runtime/src/motion.js`) names a curve by its export name when the
//! function is svelte/easing's own; any other easing function stays in JS (the 60 Hz keyframes).

use skelter_base::anim::Easing;
pub use skelter_base::anim::{SVELTE_EASINGS as ALL, SvelteEasing};

/// Evaluating a [`SvelteEasing`] and handing it to skelter.
pub trait SvelteEasingExt {
    /// The curve at `t`, as svelte/easing computes it.
    fn eval(self, t: f64) -> f64;
    /// The curve as skelter's easing for a compositor track.
    fn to_skelter(self) -> Easing;
}

impl SvelteEasingExt for SvelteEasing {
    fn eval(self, t: f64) -> f64 {
        skelter_anim::easing::svelte::eval(self, t)
    }

    fn to_skelter(self) -> Easing {
        match self {
            SvelteEasing::Linear => Easing::Linear,
            e => Easing::Svelte(e),
        }
    }
}
