//! Sampling a [`Generator`] at a time (Web Animations 1 §4 "timing model" and §5.3 "keyframe effects";
//! playback rate 1, replace composite): phases, active time with fill, iterations with
//! `iteration_start`, directions, the effect easing with the before flag, then the keyframe interval
//! with its own easing. Springs evaluate their closed form.
//!
//! A sample can have no value: outside the active interval without fill, or for an effect with no
//! keyframes (Svelte's delay animation); the underlying value then shows.

use presto_base::anim::{AnimValue, FillMode, Generator, Keyframe, PlaybackDirection, Timing};
use presto_base::time::{Span, Time};

use crate::easing;
use crate::interp::{lerp, magnitude};
use crate::spring::{self, Spring};

/// Web Animations' animation-effect phases.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Before,
    Active,
    After,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Sample {
    /// The effect's value, or `None` when it contributes nothing.
    pub value: Option<AnimValue>,
    pub phase: Phase,
    /// The animation finished (its end time passed, or a spring settled).
    pub done: bool,
    /// d(progress)/dt per second: the eased progress for keyframes, the spring's 0→1 progress.
    pub velocity: f32,
    /// The transformed (eased) progress, when resolved.
    pub progress: Option<f64>,
    /// The current iteration, when resolved.
    pub iteration: Option<f64>,
}

/// The computed timing of an effect at a local time (all in seconds).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Computed {
    pub phase: Phase,
    pub active_time: Option<f64>,
    pub progress: Option<f64>,
    pub iteration: Option<f64>,
    pub end_time: f64,
}

fn secs(s: Span) -> f64 {
    s.as_secs_f64()
}

/// The active duration: `duration × iterations`, 0 when either is 0.
pub fn active_duration(t: &Timing) -> f64 {
    let d = secs(t.duration).max(0.0);
    let n = (t.iterations as f64).max(0.0);
    if d == 0.0 || n == 0.0 { 0.0 } else { d * n }
}

/// `max(delay + active duration + end delay, 0)`.
pub fn end_time(t: &Timing) -> f64 {
    (secs(t.delay) + active_duration(t) + secs(t.end_delay)).max(0.0)
}

/// Web Animations 1 §4.6–§4.10 at local time `local` (seconds since the start time).
pub fn compute(t: &Timing, local: f64) -> Computed {
    let delay = secs(t.delay);
    let duration = secs(t.duration).max(0.0);
    let iterations = (t.iterations as f64).max(0.0);
    let iteration_start = (t.iteration_start as f64).max(0.0);
    let active = active_duration(t);
    let end = end_time(t);
    let before_boundary = delay.min(end).max(0.0);
    let after_boundary = (delay + active).min(end).max(0.0);
    let phase = if local < before_boundary {
        Phase::Before
    } else if local >= after_boundary {
        Phase::After
    } else {
        Phase::Active
    };
    let active_time = match phase {
        Phase::Before => matches!(t.fill, FillMode::Backwards | FillMode::Both).then(|| (local - delay).max(0.0)),
        Phase::Active => Some(local - delay),
        Phase::After => matches!(t.fill, FillMode::Forwards | FillMode::Both).then(|| (local - delay).min(active).max(0.0)),
    };
    let Some(at) = active_time else {
        return Computed { phase, active_time: None, progress: None, iteration: None, end_time: end };
    };
    // §4.8.3 overall progress
    let overall = if duration == 0.0 {
        if phase == Phase::Before { iteration_start } else { iteration_start + iterations }
    } else {
        at / duration + iteration_start
    };
    // §4.8.4 simple iteration progress
    let mut simple = if overall.is_infinite() { iteration_start % 1.0 } else { overall % 1.0 };
    if simple == 0.0 && matches!(phase, Phase::Active | Phase::After) && at == active && iterations != 0.0 {
        simple = 1.0;
    }
    // §4.8.5 current iteration
    let iteration = if phase == Phase::After && iterations.is_infinite() {
        f64::INFINITY
    } else if simple == 1.0 {
        overall.floor() - 1.0
    } else {
        overall.floor()
    };
    // §4.9.1 directed progress
    let forwards = match t.direction {
        PlaybackDirection::Normal => true,
        PlaybackDirection::Reverse => false,
        PlaybackDirection::Alternate | PlaybackDirection::AlternateReverse => {
            let d = if t.direction == PlaybackDirection::AlternateReverse { iteration + 1.0 } else { iteration };
            d % 2.0 == 0.0
        }
    };
    let directed = if forwards { simple } else { 1.0 - simple };
    // §4.10 transformed progress, with the before flag
    let before_flag = (phase == Phase::Before && forwards) || (phase == Phase::After && !forwards);
    let progress = easing::eval_with(&t.easing, directed as f32, before_flag) as f64;
    Computed { phase, active_time: Some(at), progress: Some(progress), iteration: Some(iteration), end_time: end }
}

/// Fills in missing offsets (NaN) as Web Animations "compute missing keyframe offsets" does: the
/// first defaults to 0, the last to 1, runs between known offsets spread evenly.
pub fn computed_offsets(frames: &[Keyframe]) -> Vec<f32> {
    let pts: Vec<(Option<f32>, f32)> = frames.iter().map(|k| ((!k.offset.is_nan()).then_some(k.offset), 0.0)).collect();
    easing::canonical_linear(&pts).into_iter().map(|p| p.0).collect()
}

/// The keyframe effect's value at transformed progress `p` (WA-1 §5.3.4), replace composite. A
/// missing 0 or 1 keyframe holds the nearest keyframe's value (there is no underlying value here).
pub fn keyframe_value(frames: &[Keyframe], p: f64) -> Option<AnimValue> {
    if frames.is_empty() {
        return None;
    }
    if frames.len() == 1 {
        return Some(frames[0].value.clone());
    }
    let offs = computed_offsets(frames);
    let n = frames.len();
    let p32 = p as f32;
    if p < 0.0 && offs.iter().filter(|&&o| o == 0.0).count() > 1 {
        return Some(frames[0].value.clone());
    }
    if p >= 1.0 && offs.iter().filter(|&&o| o == 1.0).count() > 1 {
        return Some(frames[n - 1].value.clone());
    }
    if p32 < offs[0] && offs[0] > 0.0 {
        return Some(frames[0].value.clone());
    }
    if p32 > offs[n - 1] && offs[n - 1] < 1.0 {
        return Some(frames[n - 1].value.clone());
    }
    let (i, j) = if p < 0.0 {
        (0, 1)
    } else if p >= 1.0 {
        (n - 2, n - 1)
    } else {
        let i = (0..n - 1).rev().find(|&i| offs[i] <= p32 && offs[i] < 1.0).unwrap_or(0);
        (i, i + 1)
    };
    let (a, b) = (offs[i], offs[j]);
    let local = if b == a { 0.0 } else { (p32 - a) / (b - a) };
    let eased = match &frames[i].easing {
        Some(e) => easing::eval(e, local),
        None => local,
    };
    Some(lerp(&frames[i].value, &frames[j].value, eased))
}

/// Samples `generator`, started at `start`, at `now` (the predicted present time).
pub fn sample(generator: &Generator, start: Time, now: Time) -> Sample {
    let local = (now - start).as_secs_f64();
    match generator {
        Generator::Keyframes { keyframes, timing } => {
            let c = compute(timing, local);
            let value = c.progress.and_then(|p| keyframe_value(keyframes, p));
            // eased progress rate over ±0.5 ms, for hand-off to springs and scroll curves
            let h = 0.000_5;
            let pa = compute(timing, (local - h).max(0.0)).progress;
            let pb = compute(timing, local + h).progress;
            let velocity = match (pa, pb) {
                (Some(a), Some(b)) if c.phase == Phase::Active => ((b - a) / (local + h - (local - h).max(0.0))) as f32,
                _ => 0.0,
            };
            Sample { value, phase: c.phase, done: local >= c.end_time, velocity, progress: c.progress, iteration: c.iteration }
        }
        Generator::Spring { params, from, to, velocity } => {
            let s = Spring::from_params(params);
            let span = magnitude(from, to) as f64;
            let e0 = -1.0;
            let v0 = match params {
                presto_base::anim::SpringParams::Svelte { .. } => s.svelte_velocity(e0, e0 - *velocity as f64 / 60.0),
                _ => *velocity as f64,
            };
            if local < 0.0 {
                return Sample { value: Some(from.clone()), phase: Phase::Before, done: false, velocity: 0.0, progress: Some(0.0), iteration: Some(0.0) };
            }
            let (e, v) = s.eval(e0, v0, local);
            let precision = spring::precision(params, span);
            let done = Spring::settled(e, v, span.max(1e-9), precision);
            let progress = if done { 1.0 } else { 1.0 + e };
            let value = if done { to.clone() } else { lerp(from, to, progress as f32) };
            Sample {
                value: Some(value),
                phase: if done { Phase::After } else { Phase::Active },
                done,
                velocity: if done { 0.0 } else { v as f32 },
                progress: Some(progress),
                iteration: Some(0.0),
            }
        }
    }
}

/// Retargets a running spring at `now` toward `to`, keeping its value and velocity: the new
/// generator starts at `now`. `None` for keyframe generators (Web Animations has no retarget; the
/// caller starts a new animation from the current value).
pub fn retarget(generator: &Generator, start: Time, now: Time, to: AnimValue) -> Option<Generator> {
    let Generator::Spring { params, from, to: old_to, .. } = generator else { return None };
    let cur = sample(generator, start, now);
    let value = cur.value.unwrap_or_else(|| from.clone());
    let old_span = magnitude(from, old_to);
    let new_span = magnitude(&value, &to);
    let velocity = if new_span > 0.0 { cur.velocity * old_span / new_span } else { 0.0 };
    // Svelte's parameters keep their continuous velocity through the generic form (the 60 Hz
    // exactness holds from rest only).
    let params = match *params {
        presto_base::anim::SpringParams::Svelte { stiffness, damping, .. } => {
            let s = Spring::svelte(stiffness as f64, damping as f64);
            let (w, z) = (s.omega0(), s.zeta());
            presto_base::anim::SpringParams::Physical { mass: 1.0, stiffness: (w * w) as f32, damping: (2.0 * z * w) as f32 }
        }
        p => p,
    };
    Some(Generator::Spring { params, from: value, to, velocity })
}

#[cfg(test)]
mod tests {
    use super::*;
    use presto_base::anim::Easing;

    fn timing(duration_ms: i64) -> Timing {
        Timing {
            delay: Span::ZERO,
            end_delay: Span::ZERO,
            duration: Span::from_millis(duration_ms),
            iterations: 1.0,
            iteration_start: 0.0,
            direction: PlaybackDirection::Normal,
            fill: FillMode::None,
            easing: Easing::Linear,
        }
    }

    #[test]
    fn zero_duration_is_after_at_once() {
        let c = compute(&Timing { fill: FillMode::Forwards, ..timing(0) }, 0.0);
        assert_eq!(c.phase, Phase::After);
        assert_eq!(c.progress, Some(1.0));
        let c = compute(&timing(0), 0.0);
        assert_eq!(c.progress, None);
    }

    #[test]
    fn delay_and_fill() {
        let t = Timing { delay: Span::from_millis(100), fill: FillMode::Backwards, ..timing(200) };
        let c = compute(&t, 0.05);
        assert_eq!(c.phase, Phase::Before);
        assert_eq!(c.progress, Some(0.0));
        let c = compute(&t, 0.4);
        assert_eq!(c.phase, Phase::After);
        assert_eq!(c.progress, None);
        let f = Timing { fill: FillMode::Forwards, ..t };
        assert_eq!(compute(&f, 0.4).progress, Some(1.0));
        assert_eq!(compute(&f, 0.05).progress, None);
    }

    #[test]
    fn iterations_and_directions() {
        let t = Timing { iterations: 3.0, direction: PlaybackDirection::Alternate, fill: FillMode::Both, ..timing(100) };
        let at = |s: f64| compute(&t, s).progress.unwrap();
        assert!((at(0.025) - 0.25).abs() < 1e-9);
        assert!((at(0.125) - 0.75).abs() < 1e-9, "second iteration runs backwards");
        assert!((at(0.225) - 0.25).abs() < 1e-9);
        // end of an odd count of alternate iterations: forwards, progress 1
        assert!((at(0.3) - 1.0).abs() < 1e-9);
        assert_eq!(compute(&t, 0.3).iteration, Some(2.0));
        let r = Timing { direction: PlaybackDirection::Reverse, ..t.clone() };
        assert!((compute(&r, 0.025).progress.unwrap() - 0.75).abs() < 1e-9);
    }

    #[test]
    fn iteration_start_shifts_progress() {
        let t = Timing { iteration_start: 0.5, iterations: 1.0, fill: FillMode::Both, ..timing(100) };
        assert!((compute(&t, 0.0).progress.unwrap() - 0.5).abs() < 1e-9);
        assert!((compute(&t, 0.06).progress.unwrap() - 0.1).abs() < 1e-6);
        assert_eq!(compute(&t, 0.06).iteration, Some(1.0));
        assert!((compute(&t, 0.1).progress.unwrap() - 0.5).abs() < 1e-9);
    }

    #[test]
    fn steps_before_flag_in_backwards_fill() {
        let t = Timing {
            delay: Span::from_millis(100),
            fill: FillMode::Both,
            easing: Easing::Steps { count: 2, position: presto_base::anim::StepPosition::Start },
            ..timing(100)
        };
        // during the delay the before flag holds jump-start's first step back
        assert_eq!(compute(&t, 0.05).progress, Some(0.0));
        assert_eq!(compute(&t, 0.1).progress, Some(0.5));
    }

    #[test]
    fn keyframes_evenly_spaced_with_per_keyframe_easing() {
        let frames = vec![
            Keyframe { offset: f32::NAN, value: AnimValue::Number(0.0), easing: Some(Easing::Steps { count: 1, position: presto_base::anim::StepPosition::End }) },
            Keyframe { offset: f32::NAN, value: AnimValue::Number(10.0), easing: None },
            Keyframe { offset: f32::NAN, value: AnimValue::Number(20.0), easing: None },
        ];
        assert_eq!(keyframe_value(&frames, 0.25), Some(AnimValue::Number(0.0)));
        assert_eq!(keyframe_value(&frames, 0.75), Some(AnimValue::Number(15.0)));
        assert_eq!(keyframe_value(&frames, 1.0), Some(AnimValue::Number(20.0)));
    }

    #[test]
    fn empty_keyframes_have_no_value_but_finish() {
        let g = Generator::Keyframes { keyframes: vec![], timing: Timing { fill: FillMode::Forwards, ..timing(100) } };
        let s = sample(&g, Time(0), Time(0) + Span::from_millis(150));
        assert_eq!(s.value, None);
        assert!(s.done);
    }

    #[test]
    fn spring_sample_settles_on_target() {
        let g = Generator::Spring {
            params: presto_base::anim::SpringParams::Svelte { stiffness: 0.15, damping: 0.8, precision: 0.01 },
            from: AnimValue::Number(0.0),
            to: AnimValue::Number(100.0),
            velocity: 0.0,
        };
        let s = sample(&g, Time(0), Time(0) + Span::from_millis(2000));
        assert!(s.done);
        assert_eq!(s.value, Some(AnimValue::Number(100.0)));
        let mid = sample(&g, Time(0), Time(0) + Span::from_nanos(16_666_667));
        let AnimValue::Number(x) = mid.value.unwrap() else { panic!() };
        // Svelte's first 60 Hz tick from rest: d = 0.15 × 100 = 15
        assert!((x - 15.0).abs() < 1e-3, "{x}");
    }
}
