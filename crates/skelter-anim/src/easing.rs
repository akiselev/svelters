//! Timing functions (CSS Easing 1 and 2): `cubic-bezier()` solved with Newton's method and a
//! bisection fallback, `steps()` with the before flag, and `linear()` point lists; plus Svelte's
//! `svelte/easing` functions, which Svelte bakes into 60 Hz keyframes.

use skelter_base::anim::{Easing, StepPosition, SvelteEasing};

pub const EASE: Easing = Easing::CubicBezier(0.25, 0.1, 0.25, 1.0);
pub const EASE_IN: Easing = Easing::CubicBezier(0.42, 0.0, 1.0, 1.0);
pub const EASE_OUT: Easing = Easing::CubicBezier(0.0, 0.0, 0.58, 1.0);
pub const EASE_IN_OUT: Easing = Easing::CubicBezier(0.42, 0.0, 0.58, 1.0);

/// The output progress of `e` at input progress `t`.
pub fn eval(e: &Easing, t: f32) -> f32 {
    eval_with(e, t, false)
}

/// As [`eval`], with Web Animations' before flag (it moves `steps()` jump points down a step).
pub fn eval_with(e: &Easing, t: f32, before: bool) -> f32 {
    match e {
        Easing::Linear => t,
        Easing::CubicBezier(x1, y1, x2, y2) => cubic_bezier(*x1 as f64, *y1 as f64, *x2 as f64, *y2 as f64, t as f64) as f32,
        Easing::Steps { count, position } => steps(*count, *position, t, before),
        Easing::Points(p) => linear_points(p, t),
        Easing::Svelte(c) => svelte::eval(*c, t as f64) as f32,
    }
}

/// `cubic-bezier(x1, y1, x2, y2)` at `x`. Outside [0, 1] the curve continues along its end tangent
/// (CSS Easing 1 §2.2).
pub fn cubic_bezier(x1: f64, y1: f64, x2: f64, y2: f64, x: f64) -> f64 {
    if x < 0.0 {
        let slope = if x1 > 0.0 {
            y1 / x1
        } else if y1 == 0.0 && x2 > 0.0 {
            y2 / x2
        } else {
            0.0
        };
        return slope * x;
    }
    if x > 1.0 {
        let slope = if x2 < 1.0 {
            (y2 - 1.0) / (x2 - 1.0)
        } else if x2 == 1.0 && x1 < 1.0 {
            (y1 - 1.0) / (x1 - 1.0)
        } else {
            0.0
        };
        return 1.0 + slope * (x - 1.0);
    }
    if x1 == y1 && x2 == y2 {
        return x;
    }
    let t = solve_bezier_x(x1, x2, x);
    bezier(y1, y2, t)
}

/// dy/dx of `cubic-bezier(x1, y1, x2, y2)` at `x` ∈ [0, 1] (0 where both derivatives vanish).
pub fn cubic_bezier_slope(x1: f64, y1: f64, x2: f64, y2: f64, x: f64) -> f64 {
    let t = solve_bezier_x(x1, x2, x.clamp(0.0, 1.0));
    let dx = bezier_slope(x1, x2, t);
    let dy = bezier_slope(y1, y2, t);
    if dx == 0.0 && dy == 0.0 {
        return 0.0;
    }
    dy / dx
}

/// One coordinate of the curve with end points 0 and 1: `3(1−t)²t·p1 + 3(1−t)t²·p2 + t³`.
fn bezier(p1: f64, p2: f64, t: f64) -> f64 {
    let a = 1.0 - 3.0 * p2 + 3.0 * p1;
    let b = 3.0 * p2 - 6.0 * p1;
    let c = 3.0 * p1;
    ((a * t + b) * t + c) * t
}

fn bezier_slope(p1: f64, p2: f64, t: f64) -> f64 {
    let a = 1.0 - 3.0 * p2 + 3.0 * p1;
    let b = 3.0 * p2 - 6.0 * p1;
    let c = 3.0 * p1;
    (3.0 * a * t + 2.0 * b) * t + c
}

/// The curve parameter whose x is `x` (x is monotonic on [0, 1] because x1, x2 ∈ [0, 1]).
fn solve_bezier_x(x1: f64, x2: f64, x: f64) -> f64 {
    const EPS: f64 = 1e-9;
    let mut t = x;
    for _ in 0..8 {
        let err = bezier(x1, x2, t) - x;
        if err.abs() < EPS {
            return t;
        }
        let d = bezier_slope(x1, x2, t);
        if d.abs() < 1e-6 {
            break;
        }
        t -= err / d;
    }
    let (mut lo, mut hi) = (0.0f64, 1.0f64);
    t = x;
    for _ in 0..64 {
        let v = bezier(x1, x2, t);
        if (v - x).abs() < EPS {
            break;
        }
        if v < x {
            lo = t;
        } else {
            hi = t;
        }
        t = 0.5 * (lo + hi);
    }
    t
}

/// `steps(count, position)` (CSS Easing 1 §2.3).
pub fn steps(count: u32, position: StepPosition, x: f32, before: bool) -> f32 {
    let steps = count.max(1) as f32;
    let jumps = match position {
        StepPosition::Start | StepPosition::End => steps,
        StepPosition::JumpBoth => steps + 1.0,
        StepPosition::JumpNone => (steps - 1.0).max(1.0),
    };
    let scaled = x * steps;
    let mut step = scaled.floor();
    if matches!(position, StepPosition::Start | StepPosition::JumpBoth) {
        step += 1.0;
    }
    if before && scaled.fract() == 0.0 {
        step -= 1.0;
    }
    if x >= 0.0 && step < 0.0 {
        step = 0.0;
    }
    if x <= 1.0 && step > jumps {
        step = jumps;
    }
    step / jumps
}

/// `linear()` over (input, output) points with inputs ascending (CSS Easing 2 §2.1): piecewise
/// linear, extrapolated from the first or last segment.
pub fn linear_points(points: &[(f32, f32)], x: f32) -> f32 {
    match points.len() {
        0 => return x,
        1 => return points[0].1,
        _ => {}
    }
    let n = points.len();
    let (a, b) = if x < points[0].0 {
        (points[0], points[1])
    } else if x >= points[n - 1].0 {
        (points[n - 2], points[n - 1])
    } else {
        let i = points.iter().rposition(|p| p.0 <= x).unwrap_or(0).min(n - 2);
        (points[i], points[i + 1])
    };
    if a.0 == b.0 {
        return b.1;
    }
    a.1 + (b.1 - a.1) * (x - a.0) / (b.0 - a.0)
}

/// `linear()` arguments with optional inputs, canonicalised (CSS Easing 2 §2.1.1): the first input
/// defaults to 0, the last to 1, each input is at least the largest before it, and runs of missing
/// inputs are spread evenly between their neighbours.
pub fn canonical_linear(points: &[(Option<f32>, f32)]) -> Vec<(f32, f32)> {
    let n = points.len();
    let mut inputs: Vec<Option<f32>> = points.iter().map(|p| p.0).collect();
    if n == 0 {
        return Vec::new();
    }
    if inputs[0].is_none() {
        inputs[0] = Some(0.0);
    }
    if inputs[n - 1].is_none() {
        inputs[n - 1] = Some(if n == 1 { 0.0 } else { 1.0 });
    }
    let mut max = f32::NEG_INFINITY;
    for v in inputs.iter_mut().flatten() {
        if *v < max {
            *v = max;
        }
        max = max.max(*v);
    }
    let mut i = 0;
    while i < n {
        if inputs[i].is_some() {
            i += 1;
            continue;
        }
        let start = i - 1;
        let mut end = i;
        while inputs[end].is_none() {
            end += 1;
        }
        let (a, b) = (inputs[start].unwrap_or(0.0), inputs[end].unwrap_or(1.0));
        for (k, slot) in inputs.iter_mut().enumerate().take(end).skip(start + 1) {
            *slot = Some(a + (b - a) * (k - start) as f32 / (end - start) as f32);
        }
        i = end;
    }
    inputs.into_iter().zip(points).map(|(i, p)| (i.unwrap_or(0.0), p.1)).collect()
}

/// `svelte/easing` (svelte@5.57.2 `src/easing/index.js`, adapted there from mattdesl/eases, MIT).
/// Computed in `f64` as JS does.
pub mod svelte {
    use std::f64::consts::PI;

    pub fn linear(t: f64) -> f64 {
        t
    }
    pub fn back_in_out(t: f64) -> f64 {
        let s = 1.70158 * 1.525;
        let t = t * 2.0;
        if t < 1.0 {
            return 0.5 * (t * t * ((s + 1.0) * t - s));
        }
        let t = t - 2.0;
        0.5 * (t * t * ((s + 1.0) * t + s) + 2.0)
    }
    pub fn back_in(t: f64) -> f64 {
        let s = 1.70158;
        t * t * ((s + 1.0) * t - s)
    }
    pub fn back_out(t: f64) -> f64 {
        let s = 1.70158;
        let t = t - 1.0;
        t * t * ((s + 1.0) * t + s) + 1.0
    }
    pub fn bounce_out(t: f64) -> f64 {
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
    pub fn bounce_in_out(t: f64) -> f64 {
        if t < 0.5 { 0.5 * (1.0 - bounce_out(1.0 - t * 2.0)) } else { 0.5 * bounce_out(t * 2.0 - 1.0) + 0.5 }
    }
    pub fn bounce_in(t: f64) -> f64 {
        1.0 - bounce_out(1.0 - t)
    }
    pub fn circ_in_out(t: f64) -> f64 {
        let t = t * 2.0;
        if t < 1.0 {
            return -0.5 * ((1.0 - t * t).sqrt() - 1.0);
        }
        let t = t - 2.0;
        0.5 * ((1.0 - t * t).sqrt() + 1.0)
    }
    pub fn circ_in(t: f64) -> f64 {
        1.0 - (1.0 - t * t).sqrt()
    }
    pub fn circ_out(t: f64) -> f64 {
        let t = t - 1.0;
        (1.0 - t * t).sqrt()
    }
    pub fn cubic_in_out(t: f64) -> f64 {
        if t < 0.5 { 4.0 * t * t * t } else { 0.5 * (2.0 * t - 2.0).powf(3.0) + 1.0 }
    }
    pub fn cubic_in(t: f64) -> f64 {
        t * t * t
    }
    pub fn cubic_out(t: f64) -> f64 {
        let f = t - 1.0;
        f * f * f + 1.0
    }
    pub fn elastic_in_out(t: f64) -> f64 {
        if t < 0.5 {
            0.5 * ((13.0 * PI / 2.0) * 2.0 * t).sin() * 2f64.powf(10.0 * (2.0 * t - 1.0))
        } else {
            0.5 * ((-13.0 * PI / 2.0) * (2.0 * t - 1.0 + 1.0)).sin() * 2f64.powf(-10.0 * (2.0 * t - 1.0)) + 1.0
        }
    }
    pub fn elastic_in(t: f64) -> f64 {
        (13.0 * t * PI / 2.0).sin() * 2f64.powf(10.0 * (t - 1.0))
    }
    pub fn elastic_out(t: f64) -> f64 {
        (-13.0 * (t + 1.0) * PI / 2.0).sin() * 2f64.powf(-10.0 * t) + 1.0
    }
    pub fn expo_in_out(t: f64) -> f64 {
        if t == 0.0 || t == 1.0 {
            t
        } else if t < 0.5 {
            0.5 * 2f64.powf(20.0 * t - 10.0)
        } else {
            -0.5 * 2f64.powf(10.0 - t * 20.0) + 1.0
        }
    }
    pub fn expo_in(t: f64) -> f64 {
        if t == 0.0 { t } else { 2f64.powf(10.0 * (t - 1.0)) }
    }
    pub fn expo_out(t: f64) -> f64 {
        if t == 1.0 { t } else { 1.0 - 2f64.powf(-10.0 * t) }
    }
    pub fn quad_in_out(t: f64) -> f64 {
        let t = t / 0.5;
        if t < 1.0 {
            return 0.5 * t * t;
        }
        let t = t - 1.0;
        -0.5 * (t * (t - 2.0) - 1.0)
    }
    pub fn quad_in(t: f64) -> f64 {
        t * t
    }
    pub fn quad_out(t: f64) -> f64 {
        -t * (t - 2.0)
    }
    pub fn quart_in_out(t: f64) -> f64 {
        if t < 0.5 { 8.0 * t.powf(4.0) } else { -8.0 * (t - 1.0).powf(4.0) + 1.0 }
    }
    pub fn quart_in(t: f64) -> f64 {
        t.powf(4.0)
    }
    pub fn quart_out(t: f64) -> f64 {
        (t - 1.0).powf(3.0) * (1.0 - t) + 1.0
    }
    pub fn quint_in_out(t: f64) -> f64 {
        let t = t * 2.0;
        if t < 1.0 {
            return 0.5 * t * t * t * t * t;
        }
        let t = t - 2.0;
        0.5 * (t * t * t * t * t + 2.0)
    }
    pub fn quint_in(t: f64) -> f64 {
        t * t * t * t * t
    }
    pub fn quint_out(t: f64) -> f64 {
        let t = t - 1.0;
        t * t * t * t * t + 1.0
    }
    pub fn sine_in_out(t: f64) -> f64 {
        -0.5 * ((PI * t).cos() - 1.0)
    }
    pub fn sine_in(t: f64) -> f64 {
        let v = (t * PI * 0.5).cos();
        if v.abs() < 1e-14 { 1.0 } else { 1.0 - v }
    }
    pub fn sine_out(t: f64) -> f64 {
        (t * PI / 2.0).sin()
    }

    /// `c` at `t`.
    pub fn eval(c: super::SvelteEasing, t: f64) -> f64 {
        use super::SvelteEasing as S;
        let f: fn(f64) -> f64 = match c {
            S::Linear => linear,
            S::BackInOut => back_in_out,
            S::BackIn => back_in,
            S::BackOut => back_out,
            S::BounceOut => bounce_out,
            S::BounceInOut => bounce_in_out,
            S::BounceIn => bounce_in,
            S::CircInOut => circ_in_out,
            S::CircIn => circ_in,
            S::CircOut => circ_out,
            S::CubicInOut => cubic_in_out,
            S::CubicIn => cubic_in,
            S::CubicOut => cubic_out,
            S::ElasticInOut => elastic_in_out,
            S::ElasticIn => elastic_in,
            S::ElasticOut => elastic_out,
            S::ExpoInOut => expo_in_out,
            S::ExpoIn => expo_in,
            S::ExpoOut => expo_out,
            S::QuadInOut => quad_in_out,
            S::QuadIn => quad_in,
            S::QuadOut => quad_out,
            S::QuartInOut => quart_in_out,
            S::QuartIn => quart_in,
            S::QuartOut => quart_out,
            S::QuintInOut => quint_in_out,
            S::QuintIn => quint_in,
            S::QuintOut => quint_out,
            S::SineInOut => sine_in_out,
            S::SineIn => sine_in,
            S::SineOut => sine_out,
        };
        f(t)
    }

    /// Looks a function up by its `svelte/easing` export name.
    pub fn by_name(name: &str) -> Option<fn(f64) -> f64> {
        Some(match name {
            "linear" => linear,
            "backInOut" => back_in_out,
            "backIn" => back_in,
            "backOut" => back_out,
            "bounceOut" => bounce_out,
            "bounceInOut" => bounce_in_out,
            "bounceIn" => bounce_in,
            "circInOut" => circ_in_out,
            "circIn" => circ_in,
            "circOut" => circ_out,
            "cubicInOut" => cubic_in_out,
            "cubicIn" => cubic_in,
            "cubicOut" => cubic_out,
            "elasticInOut" => elastic_in_out,
            "elasticIn" => elastic_in,
            "elasticOut" => elastic_out,
            "expoInOut" => expo_in_out,
            "expoIn" => expo_in,
            "expoOut" => expo_out,
            "quadInOut" => quad_in_out,
            "quadIn" => quad_in,
            "quadOut" => quad_out,
            "quartInOut" => quart_in_out,
            "quartIn" => quart_in,
            "quartOut" => quart_out,
            "quintInOut" => quint_in_out,
            "quintIn" => quint_in,
            "quintOut" => quint_out,
            "sineInOut" => sine_in_out,
            "sineIn" => sine_in,
            "sineOut" => sine_out,
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bezier_end_points_and_symmetry() {
        for e in [EASE, EASE_IN, EASE_OUT, EASE_IN_OUT] {
            assert!(eval(&e, 0.0).abs() < 1e-6);
            assert!((eval(&e, 1.0) - 1.0).abs() < 1e-6);
        }
        let a = eval(&EASE_IN_OUT, 0.3);
        let b = eval(&EASE_IN_OUT, 0.7);
        assert!((a + b - 1.0).abs() < 1e-5);
        assert!((eval(&EASE_IN_OUT, 0.5) - 0.5).abs() < 1e-5);
    }

    #[test]
    fn bezier_known_value() {
        // ease at 0.5; the reference is a 200-step bisection of the same polynomial.
        assert!((cubic_bezier(0.25, 0.1, 0.25, 1.0, 0.5) - 0.802_403_387_584_857).abs() < 1e-7);
    }

    #[test]
    fn bezier_extrapolates() {
        // ease-out starts with slope 0/0 → uses y2/x2 = 1/0.58.
        let v = cubic_bezier(0.0, 0.0, 0.58, 1.0, -0.1);
        assert!((v - (-0.1 / 0.58)).abs() < 1e-9);
        let w = cubic_bezier(0.42, 0.0, 1.0, 1.0, 1.5);
        assert!((w - (1.0 + 0.5 * (-1.0 / -0.58))).abs() < 1e-9);
    }

    #[test]
    fn steps_positions() {
        assert_eq!(steps(4, StepPosition::End, 0.0, false), 0.0);
        assert_eq!(steps(4, StepPosition::End, 0.3, false), 0.25);
        assert_eq!(steps(4, StepPosition::End, 1.0, false), 1.0);
        assert_eq!(steps(4, StepPosition::Start, 0.0, false), 0.25);
        assert_eq!(steps(4, StepPosition::Start, 0.0, true), 0.0);
        assert_eq!(steps(3, StepPosition::JumpBoth, 0.0, false), 0.25);
        assert_eq!(steps(3, StepPosition::JumpBoth, 0.99, false), 0.75);
        assert_eq!(steps(3, StepPosition::JumpBoth, 1.0, false), 1.0);
        assert_eq!(steps(3, StepPosition::JumpNone, 0.0, false), 0.0);
        assert_eq!(steps(3, StepPosition::JumpNone, 0.5, false), 0.5);
        assert_eq!(steps(3, StepPosition::JumpNone, 1.0, false), 1.0);
    }

    #[test]
    fn linear_function() {
        let p = canonical_linear(&[(None, 0.0), (None, 0.25), (Some(0.75), 0.5), (None, 1.0)]);
        assert_eq!(p, vec![(0.0, 0.0), (0.375, 0.25), (0.75, 0.5), (1.0, 1.0)]);
        assert!((linear_points(&p, 0.875) - 0.75).abs() < 1e-6);
        assert!((linear_points(&p, 1.5) - 2.0).abs() < 1e-6);
    }

    #[test]
    fn svelte_easings_hit_their_ends() {
        for name in ["cubicOut", "cubicInOut", "quintOut", "backOut", "elasticOut", "bounceOut", "sineIn", "expoInOut"] {
            let f = svelte::by_name(name).unwrap();
            assert!(f(0.0).abs() < 1e-3, "{name}(0)");
            assert!((f(1.0) - 1.0).abs() < 1e-3, "{name}(1)");
        }
    }

    #[test]
    fn svelte_variant_evaluates_every_named_curve() {
        for (c, name) in skelter_base::anim::SVELTE_EASINGS {
            let f = svelte::by_name(name).unwrap_or_else(|| panic!("{name}"));
            assert_eq!(skelter_base::anim::SvelteEasing::from_name(name), Some(c));
            for i in 0..=20 {
                let t = i as f32 / 20.0;
                assert_eq!(eval(&Easing::Svelte(c), t), f(t as f64) as f32, "{name}({t})");
            }
        }
    }
}
