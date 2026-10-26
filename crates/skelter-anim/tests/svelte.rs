//! Svelte's built-in transitions through skelter's engine, against Svelte's own formulas.
//!
//! Svelte 5.57.2 (`src/internal/client/dom/elements/transitions.js`, `animate()`) bakes the easing
//! into `n = ceil(duration / (1000/60)) + 1` evenly spaced keyframes, `t = t1 + delta·easing(i/n)`,
//! each `css_to_keyframe(css(t, 1 − t))`, and plays them with `{duration, fill: 'forwards'}`
//! (linear effect easing). The `css` functions below are copied from `src/transition/index.js`
//! with a computed `transform: none` and `opacity: 1`. Sampled at 60 Hz, skelter's value must equal
//! that formula; between keyframes, Web Animations' linear interpolation of them.

use skelter_anim::css::{ResolveCx, parse_declarations, parse_value, resolve};
use skelter_anim::easing::svelte as ease;
use skelter_anim::interp::compose;
use skelter_anim::{sample, spring};
use skelter_base::anim::{AnimValue, Easing, FillMode, Generator, Keyframe, PlaybackDirection, SpringParams, Timing};
use skelter_base::time::{Span, Time};

const FRAME_MS: f64 = 1000.0 / 60.0;

type Css = dyn Fn(f64, f64) -> String;

/// Svelte's keyframes for one property, intro (`t1 = 0, delta = 1`) or outro (`t1 = 1, delta = −1`).
fn svelte_track(duration_ms: f64, easing: fn(f64) -> f64, css: &Css, prop: &str, intro: bool) -> Vec<Keyframe> {
    let (t1, delta) = if intro { (0.0, 1.0) } else { (1.0, -1.0) };
    let n = (duration_ms / FRAME_MS).ceil() as usize;
    (0..=n)
        .map(|i| {
            let t = t1 + delta * easing(i as f64 / n as f64);
            let decls = parse_declarations(&css(t, 1.0 - t));
            let (_, v) = decls.iter().find(|(p, _)| p == prop).unwrap_or_else(|| panic!("no {prop} in {decls:?}"));
            let parsed = parse_value(prop, v).unwrap_or_else(|e| panic!("{prop}: {v:?}: {e:?}"));
            Keyframe { offset: f32::NAN, value: resolve(&parsed, &ResolveCx::default()).unwrap(), easing: None }
        })
        .collect()
}

fn generator(frames: Vec<Keyframe>, duration_ms: f64) -> Generator {
    Generator::Keyframes {
        keyframes: frames,
        timing: Timing {
            delay: Span::ZERO,
            end_delay: Span::ZERO,
            duration: Span::from_secs_f64(duration_ms / 1000.0),
            iterations: 1.0,
            iteration_start: 0.0,
            direction: PlaybackDirection::Normal,
            fill: FillMode::Forwards,
            easing: Easing::Linear,
        },
    }
}

/// The value the formula gives at `ms` into the animation: keyframe `i` at `i·D/n`, linear between.
fn expected(duration_ms: f64, easing: fn(f64) -> f64, value: &dyn Fn(f64) -> f64, intro: bool, ms: f64) -> f64 {
    let (t1, delta) = if intro { (0.0, 1.0) } else { (1.0, -1.0) };
    let n = (duration_ms / FRAME_MS).ceil() as usize;
    let kf = |i: usize| value(t1 + delta * easing(i as f64 / n as f64));
    let p = (ms / duration_ms).clamp(0.0, 1.0) * n as f64;
    let i = (p.floor() as usize).min(n - 1);
    let f = p - i as f64;
    kf(i) + (kf(i + 1) - kf(i)) * f
}

fn number(v: &AnimValue) -> f64 {
    match v {
        AnimValue::Number(x) | AnimValue::Length(x) => *x as f64,
        other => panic!("not a number: {other:?}"),
    }
}

fn at_60hz(g: &Generator, k: u32) -> AnimValue {
    sample(g, Time(0), Time((k as f64 * FRAME_MS * 1e6).round() as u64)).value.expect("filled")
}

fn check_number(duration_ms: f64, easing: fn(f64) -> f64, css: &Css, prop: &str, value: &dyn Fn(f64) -> f64, intro: bool) {
    let g = generator(svelte_track(duration_ms, easing, css, prop, intro), duration_ms);
    let frames = (duration_ms / FRAME_MS).ceil() as u32 + 3;
    for k in 0..=frames {
        let got = number(&at_60hz(&g, k));
        let want = expected(duration_ms, easing, value, intro, k as f64 * FRAME_MS);
        assert!((got - want).abs() < 1e-3 * want.abs().max(1.0), "{prop} frame {k}: skelter {got}, svelte {want}");
    }
}

#[test]
fn fade_in_and_out() {
    // fade: `css: (t) => opacity: ${t * o}`, 400 ms, linear
    let css = |t: f64, _u: f64| format!("opacity: {}", t * 1.0);
    check_number(400.0, ease::linear, &css, "opacity", &|t| t, true);
    check_number(400.0, ease::linear, &css, "opacity", &|t| t, false);
    // a duration that is not a whole number of frames: samples fall between keyframes
    check_number(333.0, ease::linear, &css, "opacity", &|t| t, true);
}

#[test]
fn fly_translate_and_opacity() {
    // fly({x: 100}): transform: ${transform} translate(${(1-t)*x}${unit}, ${(1-t)*y}${unit}); opacity: ${o - od*u}
    let css = |t: f64, u: f64| format!("\n\t\t\ttransform:  translate({}px, {}px);\n\t\t\topacity: {}", (1.0 - t) * 100.0, (1.0 - t) * 0.0, 1.0 - 1.0 * u);
    check_number(400.0, ease::cubic_out, &css, "opacity", &|t| t, true);
    let g = generator(svelte_track(400.0, ease::cubic_out, &css, "transform", true), 400.0);
    for k in 0..=27 {
        let AnimValue::Transform(fns) = at_60hz(&g, k) else { panic!() };
        let x = compose(&fns).e as f64;
        let want = expected(400.0, ease::cubic_out, &|t| (1.0 - t) * 100.0, true, k as f64 * FRAME_MS);
        assert!((x - want).abs() < 1e-3, "frame {k}: {x} vs {want}");
    }
}

#[test]
fn fly_over_a_computed_matrix() {
    // getComputedStyle().transform is `matrix(…)`; every keyframe starts with it
    let css = |t: f64, _u: f64| format!("transform: matrix(2, 0, 0, 2, 10, 0) translate({}px, 0px)", (1.0 - t) * 50.0);
    let g = generator(svelte_track(250.0, ease::cubic_out, &css, "transform", true), 250.0);
    for k in 0..=16 {
        let AnimValue::Transform(fns) = at_60hz(&g, k) else { panic!() };
        let m = compose(&fns);
        let tx = expected(250.0, ease::cubic_out, &|t| (1.0 - t) * 50.0, true, k as f64 * FRAME_MS);
        assert!((m.a - 2.0).abs() < 1e-5 && (m.e as f64 - (10.0 + 2.0 * tx)).abs() < 1e-3, "frame {k}: {m:?}");
    }
}

#[test]
fn scale_transition() {
    // scale({start: 0}): transform: ${transform} scale(${1 - sd*u}); opacity: ${o - od*u}
    let css = |_t: f64, u: f64| format!("\n\t\t\ttransform:  scale({});\n\t\t\topacity: {}\n\t\t", 1.0 - 1.0 * u, 1.0 - 1.0 * u);
    let g = generator(svelte_track(400.0, ease::cubic_out, &css, "transform", true), 400.0);
    for k in 0..=26 {
        let AnimValue::Transform(fns) = at_60hz(&g, k) else { panic!() };
        let s = compose(&fns).a as f64;
        let want = expected(400.0, ease::cubic_out, &|t| t, true, k as f64 * FRAME_MS);
        assert!((s - want).abs() < 1e-3, "frame {k}: {s} vs {want}");
    }
}

#[test]
fn slide_layout_properties() {
    // slide on a 120 px box with 8 px padding: height: ${t*h}px; padding-top: ${t*p}px; opacity: ${min(t*20,1)*o}
    let css = |t: f64, _u: f64| {
        format!(
            "overflow: hidden;opacity: {};height: {}px;padding-top: {}px;padding-bottom: {}px;min-height: 0",
            (t * 20.0).min(1.0) * 1.0,
            t * 120.0,
            t * 8.0,
            t * 8.0
        )
    };
    check_number(400.0, ease::cubic_out, &css, "height", &|t| t * 120.0, true);
    check_number(400.0, ease::cubic_out, &css, "padding-top", &|t| t * 8.0, true);
    check_number(400.0, ease::cubic_out, &css, "opacity", &|t| (t * 20.0).min(1.0), true);
}

#[test]
fn blur_opacity_and_draw_dashes() {
    // blur: opacity: ${o - od*u}; filter: ${f} blur(${u*amount}${unit}) (filters resolve to no value yet)
    let css = |_t: f64, u: f64| format!("opacity: {}; filter:  blur({}px);", 1.0 - u, u * 5.0);
    check_number(400.0, ease::cubic_in_out, &css, "opacity", &|t| t, true);
    // draw on a 300 px path: stroke-dasharray: ${len}; stroke-dashoffset: ${u*len}
    let len = 300.0;
    let css = move |_t: f64, u: f64| format!("\n\t\t\tstroke-dasharray: {len};\n\t\t\tstroke-dashoffset: {};\n\t\t", u * len);
    check_number(800.0, ease::cubic_in_out, &css, "stroke-dashoffset", &|t| (1.0 - t) * 300.0, true);
}

#[test]
fn delay_animation_holds_without_value_and_finishes() {
    // Svelte's delay timer: element.animate([], {duration: delay, fill: 'forwards'})
    let g = generator(Vec::new(), 100.0);
    let s = sample(&g, Time(0), Time(50_000_000));
    assert_eq!(s.value, None);
    assert!(!s.done);
    assert!(sample(&g, Time(0), Time(100_000_000)).done);
    // and the zero-delay case finishes at once
    assert!(sample(&generator(Vec::new(), 0.0), Time(0), Time(0)).done);
}

#[test]
fn spring_matches_svelte_at_60hz() {
    for (k, c) in [(0.15, 0.8), (0.1, 0.25)] {
        let g = Generator::Spring {
            params: SpringParams::Svelte { stiffness: k as f32, damping: c as f32, precision: 0.01 },
            from: AnimValue::Number(0.0),
            to: AnimValue::Number(100.0),
            velocity: 0.0,
        };
        let (mut last, mut cur) = (0.0, 0.0);
        for n in 1..=40u32 {
            let Some(next) = spring::svelte_tick(k, c, 0.01, last, cur, 100.0, 1.0) else { break };
            last = cur;
            cur = next;
            let got = number(&at_60hz(&g, n));
            assert!((got - cur).abs() < 2e-3, "{{{k}, {c}}} frame {n}: skelter {got}, svelte {cur}");
        }
    }
}

#[test]
fn web_animations_timing_edge_cases() {
    use skelter_anim::timing::{Phase, compute};
    let base = Timing {
        delay: Span::ZERO,
        end_delay: Span::ZERO,
        duration: Span::from_millis(100),
        iterations: 1.0,
        iteration_start: 0.0,
        direction: PlaybackDirection::Normal,
        fill: FillMode::Forwards,
        easing: Easing::Linear,
    };
    // a negative end delay ends the effect early; WA-1 §4.7.2 keeps the after-phase active time at
    // max(min(local − delay, active duration), 0)
    let t = Timing { end_delay: Span::from_millis(-50), ..base.clone() };
    let c = compute(&t, 0.06);
    assert_eq!(c.phase, Phase::After);
    assert!((c.end_time - 0.05).abs() < 1e-9, "{c:?}");
    assert!((c.progress.unwrap() - 0.6).abs() < 1e-6, "{c:?}");
    // zero iterations: progress is the iteration start
    let t = Timing { iterations: 0.0, iteration_start: 0.25, ..base.clone() };
    assert!((compute(&t, 0.0).progress.unwrap() - 0.25).abs() < 1e-9);
    // infinite iterations never reach the after phase
    let t = Timing { iterations: f32::INFINITY, ..base.clone() };
    let c = compute(&t, 1000.05);
    assert_eq!(c.phase, Phase::Active);
    assert!((c.progress.unwrap() - 0.5).abs() < 1e-6);
    // a fractional iteration count ends mid-iteration
    let t = Timing { iterations: 1.5, ..base.clone() };
    assert!((compute(&t, 1.0).progress.unwrap() - 0.5).abs() < 1e-9);
    assert_eq!(compute(&t, 1.0).iteration, Some(1.0));
    // the effect easing applies after direction
    let t = Timing { direction: PlaybackDirection::Reverse, easing: skelter_anim::easing::EASE_IN, ..base.clone() };
    let p = compute(&t, 0.025).progress.unwrap();
    let want = skelter_anim::easing::cubic_bezier(0.42, 0.0, 1.0, 1.0, 0.75);
    assert!((p - want).abs() < 1e-6);
}
