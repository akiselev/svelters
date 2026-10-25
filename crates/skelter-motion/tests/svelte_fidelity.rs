//! skelter-motion against svelte@5.57.2's own JS: `fixtures/svelte-goldens.json` is what Svelte's
//! transitions, easings, `flip` and spring integrator compute (`gen-svelte-goldens.mjs` writes it).
//! The native formulas must equal it within 1e-6 at Svelte's 60 Hz keyframe times.

use skelter_base::anim::{AnimValue, TransformFn};
use skelter_motion::easing::{SvelteEasing, SvelteEasingExt};
use skelter_motion::flip::{DomRect, Flip, FlipInput, FlipParams};
use skelter_motion::spring::{SpringMotion, SvelteSpring};
use skelter_motion::transition::{parse_transform, Channel, Computed, Params, Run, Transition, TransitionKind};
use serde::Deserialize;

const GOLDENS: &str = include_str!("fixtures/svelte-goldens.json");

#[derive(Deserialize)]
struct Goldens {
    transitions: Vec<TransitionCase>,
    easings: std::collections::BTreeMap<String, Vec<f64>>,
    flips: Vec<FlipCase>,
    springs: Vec<SpringCase>,
}

#[derive(Deserialize)]
struct Frame {
    t: f64,
    numbers: Vec<f64>,
}

#[derive(Deserialize)]
struct Config {
    delay: f64,
    duration: f64,
}

#[derive(Deserialize)]
struct TransitionCase {
    kind: String,
    params: Params,
    computed: Computed,
    t1: f64,
    t2: f64,
    duration: f64,
    config: Config,
    frames: Vec<Frame>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FlipCase {
    from: DomRect,
    to: DomRect,
    client_width: f64,
    client_height: f64,
    origin: String,
    transform: String,
    params: Params,
    delay: f64,
    duration: f64,
    frames: Vec<Frame>,
}

#[derive(Deserialize)]
struct SpringParamsJson {
    stiffness: f64,
    damping: f64,
    precision: f64,
}

#[derive(Deserialize)]
struct SpringCase {
    params: SpringParamsJson,
    from: f64,
    to: f64,
    frames: Vec<f64>,
}

fn goldens() -> Goldens {
    serde_json::from_str(GOLDENS).expect("fixtures/svelte-goldens.json parses")
}

/// `|a - b| <= tol * max(1, |b|)`.
fn close(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol * b.abs().max(1.0)
}

fn assert_numbers(what: &str, ours: &[f64], svelte: &[f64]) {
    assert_eq!(ours.len(), svelte.len(), "{what}: ours {ours:?}, svelte {svelte:?}");
    for (i, (a, b)) in ours.iter().zip(svelte).enumerate() {
        assert!(close(*a, *b, 1e-6), "{what}: number {i}: ours {a}, svelte {b} (ours {ours:?}, svelte {svelte:?})");
    }
}

#[test]
fn every_easing_equals_svelte() {
    let g = goldens();
    assert_eq!(g.easings.len(), skelter_motion::easing::ALL.len(), "svelte/easing exports a curve skelter-motion lacks (or the other way)");
    for (name, samples) in &g.easings {
        let e = SvelteEasing::from_name(name).unwrap_or_else(|| panic!("no curve {name}"));
        assert_eq!(e.name(), name);
        for (i, want) in samples.iter().enumerate() {
            let got = e.eval(i as f64 / 200.0);
            assert!(close(got, *want, 1e-12), "{name}({}) = {got}, svelte {want}", i as f64 / 200.0);
        }
    }
}

#[test]
fn compositor_easings_are_exact() {
    // a compositor track's easing is the curve itself: skelter-anim's sampler gives Svelte's values
    use skelter_base::anim::Easing;
    for (e, name) in skelter_motion::easing::ALL {
        let skelter = e.to_skelter();
        assert!(matches!(skelter, Easing::Svelte(_)) || e == SvelteEasing::Linear, "{name}");
        for i in 0..=1000 {
            let x = i as f32 / 1000.0;
            let got = skelter_anim::easing::eval(&skelter, x) as f64;
            let want = e.eval(x as f64);
            assert!((got - want).abs() <= 1e-6 * want.abs().max(1.0), "{name} at {x}: skelter {got}, svelte {want}");
        }
    }
}

#[test]
fn transitions_equal_svelte_at_60hz() {
    for case in goldens().transitions {
        let kind = TransitionKind::from_name(&case.kind).unwrap();
        let what = format!("{} {:?} {}→{}", case.kind, case.params, case.t1, case.t2);
        let tr = Transition::new(kind, &case.params, &case.computed).unwrap_or_else(|e| panic!("{what}: {e}"));
        assert!(close(tr.duration, case.config.duration, 1e-12), "{what}: duration {} vs {}", tr.duration, case.config.duration);
        assert_eq!(tr.delay, case.config.delay, "{what}: delay");
        let run = Run { t1: case.t1, t2: case.t2, duration: case.duration, delay: 0.0 };
        assert_eq!(run, Run { delay: 0.0, ..Run::new(&tr, case.t1, case.t2) }, "{what}: the run's duration is the transition's scaled by |t2 - t1|");
        let ts = run.keyframe_ts(tr.easing);
        assert_eq!(ts.len(), case.frames.len(), "{what}: keyframe count");
        for (i, f) in case.frames.iter().enumerate() {
            assert!(close(ts[i], f.t, 1e-12), "{what}: t[{i}] {} vs {}", ts[i], f.t);
            assert_numbers(&format!("{what} frame {i}"), &tr.css(f.t, 1.0 - f.t).numbers(kind), &f.numbers);
        }
    }
}

/// The value of a two-keyframe transform track at eased progress `p`, function by function.
fn lerp_transform(a: &[TransformFn], b: &[TransformFn], p: f64) -> Vec<f64> {
    let nums = |f: &TransformFn| -> Vec<f64> {
        match *f {
            TransformFn::Translate(x, y) | TransformFn::Scale(x, y) | TransformFn::Skew(x, y) => vec![x as f64, y as f64],
            TransformFn::Rotate(r) => vec![r as f64],
            TransformFn::Matrix(m) => [m.a, m.b, m.c, m.d, m.e, m.f].iter().map(|v| *v as f64).collect(),
        }
    };
    a.iter().zip(b).flat_map(|(x, y)| nums(x).into_iter().zip(nums(y)).map(|(x, y)| x + (y - x) * p).collect::<Vec<_>>()).collect()
}

#[test]
fn transition_tracks_equal_svelte_at_60hz() {
    for case in goldens().transitions {
        let kind = TransitionKind::from_name(&case.kind).unwrap();
        let what = format!("{} {:?} {}→{}", case.kind, case.params, case.t1, case.t2);
        let tr = Transition::new(kind, &case.params, &case.computed).unwrap();
        let run = Run { t1: case.t1, t2: case.t2, duration: case.duration, delay: 0.0 };
        let plan = tr.plan(&run);
        let n = case.frames.len() - 1;
        for (i, f) in case.frames.iter().enumerate() {
            let progress = if n == 0 { 1.0 } else { i as f64 / n as f64 };
            let want = tr.css(f.t, 1.0 - f.t);
            for track in &plan.tracks {
                match track.channel {
                    Channel::Transform => {
                        let (AnimValue::Transform(a), AnimValue::Transform(b)) = (&track.keyframes[0].value, &track.keyframes[1].value) else { panic!("{what}: transform keyframes") };
                        let got = lerp_transform(a, b, plan.easing.eval(progress));
                        let mut exp = Vec::new();
                        for t in want.transform.as_ref().unwrap() {
                            t.numbers(&mut exp);
                        }
                        for (g, e) in got.iter().zip(&exp) {
                            // f32 keyframe values
                            assert!(close(*g, *e, 1e-6), "{what} frame {i}: transform {got:?} vs {exp:?}");
                        }
                    }
                    channel => {
                        let got = plan.sample_number(track, progress).unwrap();
                        let exp = match channel {
                            Channel::Opacity => want.opacity.unwrap(),
                            Channel::Blur => want.blur.unwrap(),
                            Channel::StrokeDashOffset => want.dash.unwrap().1,
                            Channel::Size(_) => want.dims[0].1,
                            Channel::Css(name) => want.dims.iter().find(|d| d.0 == name).unwrap().1,
                            other => panic!("{what}: unexpected track {other:?}"),
                        };
                        assert!(close(got, exp, 1e-6), "{what} frame {i}: {channel:?} {got} vs {exp}");
                    }
                }
            }
        }
    }
}

fn origin_px(text: &str) -> (f64, f64) {
    let mut it = text.split_whitespace().map(skelter_motion::transition::parse_float);
    (it.next().unwrap(), it.next().unwrap())
}

#[test]
fn flip_equals_svelte_at_60hz() {
    for case in goldens().flips {
        let input = FlipInput {
            from: case.from,
            to: case.to,
            client_width: case.client_width,
            client_height: case.client_height,
            origin: origin_px(&case.origin),
            transform: if case.transform == "none" { Vec::new() } else { parse_transform(&case.transform).unwrap() },
            zoom: 1.0,
        };
        let params = FlipParams {
            delay: case.params.delay.unwrap_or(0.0),
            duration: case.params.duration,
            easing: case.params.easing.as_deref().map_or(SvelteEasing::CubicOut, |n| SvelteEasing::from_name(n).unwrap()),
        };
        let flip = Flip::new(&input, &params);
        assert!(close(flip.duration, case.duration, 1e-12), "flip duration {} vs {}", flip.duration, case.duration);
        assert_eq!(flip.delay, case.delay);
        for (i, f) in case.frames.iter().enumerate() {
            let mut ours = Vec::new();
            for t in flip.css(f.t, 1.0 - f.t) {
                t.numbers(&mut ours);
            }
            assert_numbers(&format!("flip {:?}→{:?} frame {i}", case.from, case.to), &ours, &f.numbers);
        }
    }
}

#[test]
fn flip_scales_by_the_layout_size() {
    // the measured rect is twice the layout size (a 2x ancestor): the move halves, as in Svelte
    let input = FlipInput {
        from: DomRect { left: 0.0, top: 0.0, width: 200.0, height: 80.0 },
        to: DomRect { left: 0.0, top: 120.0, width: 200.0, height: 80.0 },
        client_width: 100.0,
        client_height: 40.0,
        origin: (0.0, 0.0),
        transform: Vec::new(),
        zoom: 1.0,
    };
    let flip = Flip::new(&input, &FlipParams::default());
    assert_eq!((flip.dx, flip.dy), (0.0, -60.0));
}

#[test]
fn spring_closed_form_equals_svelte_at_60hz() {
    for case in goldens().springs {
        let params = SvelteSpring { stiffness: case.params.stiffness, damping: case.params.damping, precision: case.params.precision };
        let motion = SpringMotion::new(params, case.from, case.from, case.to);
        let range = (case.to - case.from).abs();
        for (n, want) in case.frames.iter().enumerate() {
            let got = motion.value(n as f64);
            assert!((got - want).abs() <= 1e-6 * range.max(1.0), "{params:?} frame {n}: closed form {got}, svelte {want}");
        }
        // Svelte writes the target at the frame after the settle check passes, and stops
        assert_eq!(motion.settle_frame(), Some(case.frames.len() as u64 - 2), "{params:?}: settle frame");
    }
}

#[test]
fn spring_oracle_matches_the_js_goldens() {
    for case in goldens().springs {
        let params = SvelteSpring { stiffness: case.params.stiffness, damping: case.params.damping, precision: case.params.precision };
        let ours = skelter_motion::spring::svelte_frames(params, case.from, case.to, 10_000);
        assert_eq!(ours.len(), case.frames.len(), "{params:?}");
        for (a, b) in ours.iter().zip(&case.frames) {
            assert!(close(*a, *b, 1e-9), "{params:?}: {a} vs {b}");
        }
    }
}
