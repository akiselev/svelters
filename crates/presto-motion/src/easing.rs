//! svelte/easing's 31 curves (svelte@5.57.2 `src/easing/index.js`) by name, evaluated with
//! `presto_anim::easing::svelte` (the same operations as the JS, in `f64`), and as presto easings.
//!
//! The JS side (`packages/native-runtime/src/motion.js`) names a curve by its export name when the
//! function is svelte/easing's own; any other easing function stays in JS (the 60 Hz keyframes).

use presto_base::anim::Easing;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SvelteEasing {
    Linear,
    BackInOut,
    BackIn,
    BackOut,
    BounceOut,
    BounceInOut,
    BounceIn,
    CircInOut,
    CircIn,
    CircOut,
    CubicInOut,
    CubicIn,
    CubicOut,
    ElasticInOut,
    ElasticIn,
    ElasticOut,
    ExpoInOut,
    ExpoIn,
    ExpoOut,
    QuadInOut,
    QuadIn,
    QuadOut,
    QuartInOut,
    QuartIn,
    QuartOut,
    QuintInOut,
    QuintIn,
    QuintOut,
    SineInOut,
    SineIn,
    SineOut,
}

use SvelteEasing::*;

/// Every curve with its svelte/easing export name.
pub const ALL: [(SvelteEasing, &str); 31] = [
    (Linear, "linear"),
    (BackInOut, "backInOut"),
    (BackIn, "backIn"),
    (BackOut, "backOut"),
    (BounceOut, "bounceOut"),
    (BounceInOut, "bounceInOut"),
    (BounceIn, "bounceIn"),
    (CircInOut, "circInOut"),
    (CircIn, "circIn"),
    (CircOut, "circOut"),
    (CubicInOut, "cubicInOut"),
    (CubicIn, "cubicIn"),
    (CubicOut, "cubicOut"),
    (ElasticInOut, "elasticInOut"),
    (ElasticIn, "elasticIn"),
    (ElasticOut, "elasticOut"),
    (ExpoInOut, "expoInOut"),
    (ExpoIn, "expoIn"),
    (ExpoOut, "expoOut"),
    (QuadInOut, "quadInOut"),
    (QuadIn, "quadIn"),
    (QuadOut, "quadOut"),
    (QuartInOut, "quartInOut"),
    (QuartIn, "quartIn"),
    (QuartOut, "quartOut"),
    (QuintInOut, "quintInOut"),
    (QuintIn, "quintIn"),
    (QuintOut, "quintOut"),
    (SineInOut, "sineInOut"),
    (SineIn, "sineIn"),
    (SineOut, "sineOut"),
];

impl SvelteEasing {
    /// The curve exported as `name` from svelte/easing.
    pub fn from_name(name: &str) -> Option<SvelteEasing> {
        ALL.iter().find(|(_, n)| *n == name).map(|(e, _)| *e)
    }

    pub fn name(self) -> &'static str {
        ALL.iter().find(|(e, _)| *e == self).map(|(_, n)| *n).expect("every curve is in ALL")
    }

    /// The curve at `t`, as svelte/easing computes it (`presto_anim::easing::svelte`).
    pub fn eval(self, t: f64) -> f64 {
        let f = presto_anim::easing::svelte::by_name(self.name()).expect("presto-anim has every svelte/easing curve");
        f(t)
    }

    /// The curve as presto's [`Easing`] for a compositor track: `Linear` exactly; every other curve
    /// as CSS `linear()` points placed so the piecewise-linear curve stays within `tolerance` of it
    /// (presto-base's `Easing` has no variant for these curves; see the lane record).
    pub fn to_presto(self, tolerance: f64) -> Easing {
        if self == Linear {
            return Easing::Linear;
        }
        // 64 even segments first, so no oscillation (elastic, bounce) hides between two samples
        const SEED: usize = 64;
        let mut pts: Vec<(f64, f64)> = vec![(0.0, self.eval(0.0))];
        for i in 0..SEED {
            let a = i as f64 / SEED as f64;
            let b = (i + 1) as f64 / SEED as f64;
            self.subdivide(a, self.eval(a), b, self.eval(b), tolerance, 0, &mut pts);
        }
        Easing::Points(pts.into_iter().map(|(x, y)| (x as f32, y as f32)).collect())
    }

    /// Pushes the points after `a` up to and including `b`.
    #[allow(clippy::too_many_arguments)]
    fn subdivide(self, a: f64, fa: f64, b: f64, fb: f64, tol: f64, depth: u32, out: &mut Vec<(f64, f64)>) {
        let m = 0.5 * (a + b);
        let fm = self.eval(m);
        // check the midpoint and the quarter points against the chord
        let q1 = 0.5 * (a + m);
        let q3 = 0.5 * (m + b);
        let chord = |x: f64| fa + (fb - fa) * (x - a) / (b - a);
        let err = [(m, fm), (q1, self.eval(q1)), (q3, self.eval(q3))].iter().map(|&(x, y)| (y - chord(x)).abs()).fold(0.0, f64::max);
        if err <= tol || depth >= 12 {
            out.push((b, fb));
            return;
        }
        self.subdivide(a, fa, m, fm, tol, depth + 1, out);
        self.subdivide(m, fm, b, fb, tol, depth + 1, out);
    }
}
