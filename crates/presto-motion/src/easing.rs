//! svelte/easing's 31 curves (svelte@5.57.2 `src/easing/index.js`, adapted there from
//! mattdesl/eases, MIT), evaluated in `f64` with the same operations in the same order, so a value
//! equals the JS one to the last bit or within an ulp of `Math.pow`/`Math.sin`.
//!
//! The JS side (`packages/native-runtime/src/motion.js`) names a curve by its export name when the
//! function is svelte/easing's own; any other easing function stays in JS (the 60 Hz keyframes).

use presto_base::anim::Easing;
use std::f64::consts::PI;

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

fn bounce_out(t: f64) -> f64 {
    let a = 4.0 / 11.0;
    let b = 8.0 / 11.0;
    let c = 9.0 / 10.0;
    let ca = 4356.0 / 361.0;
    let cb = 35442.0 / 1805.0;
    let cc = 16061.0 / 1805.0;
    let t2 = t * t;
    if t < a {
        7.5625 * t2
    } else if t < b {
        9.075 * t2 - 9.9 * t + 3.4
    } else if t < c {
        ca * t2 - cb * t + cc
    } else {
        10.8 * t * t - 20.52 * t + 10.72
    }
}

impl SvelteEasing {
    /// The curve exported as `name` from svelte/easing.
    pub fn from_name(name: &str) -> Option<SvelteEasing> {
        ALL.iter().find(|(_, n)| *n == name).map(|(e, _)| *e)
    }

    pub fn name(self) -> &'static str {
        ALL.iter().find(|(e, _)| *e == self).map(|(_, n)| *n).expect("every curve is in ALL")
    }

    /// The curve at `t`, as svelte/easing computes it.
    pub fn eval(self, t: f64) -> f64 {
        match self {
            Linear => t,
            BackInOut => {
                let s = 1.70158 * 1.525;
                let t = t * 2.0;
                if t < 1.0 {
                    0.5 * (t * t * ((s + 1.0) * t - s))
                } else {
                    let t = t - 2.0;
                    0.5 * (t * t * ((s + 1.0) * t + s) + 2.0)
                }
            }
            BackIn => {
                let s = 1.70158;
                t * t * ((s + 1.0) * t - s)
            }
            BackOut => {
                let s = 1.70158;
                let t = t - 1.0;
                t * t * ((s + 1.0) * t + s) + 1.0
            }
            BounceOut => bounce_out(t),
            BounceInOut => {
                if t < 0.5 {
                    0.5 * (1.0 - bounce_out(1.0 - t * 2.0))
                } else {
                    0.5 * bounce_out(t * 2.0 - 1.0) + 0.5
                }
            }
            BounceIn => 1.0 - bounce_out(1.0 - t),
            CircInOut => {
                let t = t * 2.0;
                if t < 1.0 {
                    -0.5 * ((1.0 - t * t).sqrt() - 1.0)
                } else {
                    let t = t - 2.0;
                    0.5 * ((1.0 - t * t).sqrt() + 1.0)
                }
            }
            CircIn => 1.0 - (1.0 - t * t).sqrt(),
            CircOut => {
                let t = t - 1.0;
                (1.0 - t * t).sqrt()
            }
            CubicInOut => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    0.5 * (2.0 * t - 2.0).powf(3.0) + 1.0
                }
            }
            CubicIn => t * t * t,
            CubicOut => {
                let f = t - 1.0;
                f * f * f + 1.0
            }
            ElasticInOut => {
                if t < 0.5 {
                    0.5 * (((13.0 * PI) / 2.0) * 2.0 * t).sin() * 2f64.powf(10.0 * (2.0 * t - 1.0))
                } else {
                    0.5 * (((-13.0 * PI) / 2.0) * (2.0 * t - 1.0 + 1.0)).sin() * 2f64.powf(-10.0 * (2.0 * t - 1.0)) + 1.0
                }
            }
            ElasticIn => ((13.0 * t * PI) / 2.0).sin() * 2f64.powf(10.0 * (t - 1.0)),
            ElasticOut => ((-13.0 * (t + 1.0) * PI) / 2.0).sin() * 2f64.powf(-10.0 * t) + 1.0,
            ExpoInOut => {
                if t == 0.0 || t == 1.0 {
                    t
                } else if t < 0.5 {
                    0.5 * 2f64.powf(20.0 * t - 10.0)
                } else {
                    -0.5 * 2f64.powf(10.0 - t * 20.0) + 1.0
                }
            }
            ExpoIn => {
                if t == 0.0 {
                    t
                } else {
                    2f64.powf(10.0 * (t - 1.0))
                }
            }
            ExpoOut => {
                if t == 1.0 {
                    t
                } else {
                    1.0 - 2f64.powf(-10.0 * t)
                }
            }
            QuadInOut => {
                let t = t / 0.5;
                if t < 1.0 {
                    0.5 * t * t
                } else {
                    let t = t - 1.0;
                    -0.5 * (t * (t - 2.0) - 1.0)
                }
            }
            QuadIn => t * t,
            QuadOut => -t * (t - 2.0),
            QuartInOut => {
                if t < 0.5 {
                    8.0 * t.powf(4.0)
                } else {
                    -8.0 * (t - 1.0).powf(4.0) + 1.0
                }
            }
            QuartIn => t.powf(4.0),
            QuartOut => (t - 1.0).powf(3.0) * (1.0 - t) + 1.0,
            QuintInOut => {
                let t = t * 2.0;
                if t < 1.0 {
                    0.5 * t * t * t * t * t
                } else {
                    let t = t - 2.0;
                    0.5 * (t * t * t * t * t + 2.0)
                }
            }
            QuintIn => t * t * t * t * t,
            QuintOut => {
                let t = t - 1.0;
                t * t * t * t * t + 1.0
            }
            SineInOut => -0.5 * ((PI * t).cos() - 1.0),
            SineIn => {
                let v = (t * PI * 0.5).cos();
                if v.abs() < 1e-14 { 1.0 } else { 1.0 - v }
            }
            SineOut => ((t * PI) / 2.0).sin(),
        }
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
