//! TR2: the native animator (SV13, SV37). JS sends an animation's keyframes and timing once
//! (`SVL_STYLE_ANIMATE`, JSON); the scene samples them every frame into the node's visual channels
//! (`Visual`: opacity, transform, clip) and tells JS when one has finished (`animationend`, the
//! `outro_done` of the Svelte transition). No JS runs per frame.
//!
//! - **Clock.** The animator reads egui's input time (`ctx.input(|i| i.time)`), which kittest
//!   advances by its step (a virtual clock: `Harness::with_step_dt`). A `slow` factor scales the
//!   time that passes for animations (SV37: 0.1 plays at a tenth of the speed).
//! - **Reduced motion.** Every animation jumps to its last keyframe and finishes at the next step.
//! - **Composition.** The node's static visual (what `visual_op` set) is the base. The newest
//!   animation owning a channel wins it, as the JS player does. A finished animation keeps its last
//!   frame (fill forwards) until it is cancelled.
//! - **Demand.** `running_roots` lists the roots with an unfinished animation: only their
//!   viewports are asked for frames (SV37), not every viewport.
//! - **Exiting nodes.** JS marks an outroing node inert (`SVL_STYLE_INERT`) as Svelte does; the
//!   scene drops its events, hit tests skip it and its accessibility node is hidden.

use crate::scene::{Handle, Visual};
use serde_json::Value as Json;
use std::collections::HashMap;

/// One keyframe: the channels it sets (Svelte's keyframes are evenly spaced).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Keyframe {
    pub opacity: Option<f32>,
    /// tx, ty (points), sx, sy, origin x, y (fractions of the size).
    pub transform: Option<[f32; 6]>,
    /// width, height the node occupies; < 0: that axis is free.
    pub clip: Option<[f32; 2]>,
}

/// An animation as JS sends it.
#[derive(Clone, Debug, PartialEq)]
pub struct Spec {
    pub id: u64,
    /// ms
    pub duration: f64,
    /// ms
    pub delay: f64,
    pub frames: Vec<Keyframe>,
}

/// The two messages of `SVL_STYLE_ANIMATE`.
#[derive(Clone, Debug, PartialEq)]
pub enum Cmd {
    Start(Spec),
    Cancel(u64),
}

fn nums<const N: usize>(v: Option<&Json>, what: &str) -> Result<Option<[f32; N]>, String> {
    let Some(v) = v.filter(|v| !v.is_null()) else { return Ok(None) };
    let a = v.as_array().filter(|a| a.len() == N).ok_or_else(|| format!("animate: {what} is not {N} numbers"))?;
    let mut o = [0.0; N];
    for (i, x) in a.iter().enumerate() {
        o[i] = x.as_f64().ok_or_else(|| format!("animate: {what} is not {N} numbers"))? as f32;
    }
    Ok(Some(o))
}

/// Parses the `SVL_STYLE_ANIMATE` text: `{"id":1,"duration":120,"delay":0,"frames":[{"opacity":1},
/// {"opacity":0}]}` to start, `{"id":1,"cancel":true}` to cancel.
pub fn parse(text: &str) -> Result<Cmd, String> {
    let j: Json = serde_json::from_str(text).map_err(|e| format!("animate: {e}"))?;
    let id = j.get("id").and_then(Json::as_u64).ok_or("animate: missing id")?;
    if j.get("cancel").and_then(Json::as_bool) == Some(true) {
        return Ok(Cmd::Cancel(id));
    }
    let duration = j.get("duration").and_then(Json::as_f64).unwrap_or(0.0);
    let delay = j.get("delay").and_then(Json::as_f64).unwrap_or(0.0);
    if !(duration.is_finite() && duration >= 0.0 && delay.is_finite()) {
        return Err("animate: duration and delay must be finite, duration >= 0".into());
    }
    let mut frames = Vec::new();
    for f in j.get("frames").and_then(Json::as_array).map(Vec::as_slice).unwrap_or(&[]) {
        frames.push(Keyframe {
            opacity: f.get("opacity").and_then(Json::as_f64).map(|o| o as f32),
            transform: nums::<6>(f.get("transform"), "transform")?,
            clip: nums::<2>(f.get("clip"), "clip")?,
        });
    }
    if frames.is_empty() {
        return Err("animate: no keyframes".into());
    }
    Ok(Cmd::Start(Spec { id, duration, delay, frames }))
}

fn lerp(a: f32, b: f32, p: f32) -> f32 {
    a + (b - a) * p
}

/// Between two keyframes' values of one channel, as the JS player mixes them: a channel only one
/// of them sets holds `a` (or switches at the middle when only `b` has it); a negative clip axis
/// (free) steps instead of interpolating.
fn mix<const N: usize>(a: Option<[f32; N]>, b: Option<[f32; N]>, p: f32, step_negative: bool) -> Option<[f32; N]> {
    match (a, b) {
        (a, _) if p == 0.0 => a,
        (a, None) => a,
        (None, b) => (p >= 0.5).then_some(b).flatten(),
        (Some(a), Some(b)) => {
            let mut o = [0.0; N];
            for i in 0..N {
                o[i] = if step_negative && (a[i] < 0.0 || b[i] < 0.0) {
                    if p < 0.5 { a[i] } else { b[i] }
                } else {
                    lerp(a[i], b[i], p)
                };
            }
            Some(o)
        }
    }
}

impl Spec {
    /// The channels at progress `p` (0..=1) over the keyframes.
    pub fn sample(&self, p: f32) -> Keyframe {
        let f = &self.frames;
        let n = p.clamp(0.0, 1.0) * (f.len() - 1) as f32;
        let lo = f[n.floor() as usize];
        let hi = f[n.ceil() as usize];
        let q = n.fract();
        let one = |a: Option<f32>, b: Option<f32>| mix(a.map(|x| [x]), b.map(|x| [x]), q, false).map(|x| x[0]);
        Keyframe { opacity: one(lo.opacity, hi.opacity), transform: mix(lo.transform, hi.transform, q, false), clip: mix(lo.clip, hi.clip, q, true) }
    }
}

struct Running {
    spec: Spec,
    /// The animation clock (seconds) when it started.
    start: f64,
    finished: bool,
    current: Keyframe,
}

/// What a step changed.
#[derive(Default, Debug)]
pub struct Step {
    /// Nodes whose composed visual changed.
    pub visuals: Vec<(Handle, Visual)>,
    /// Animations that finished in this step.
    pub done: Vec<(Handle, u64)>,
}

/// Motion settings of the backend (SV37, reduced motion).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MotionConfig {
    /// Time scale for animations: 1.0 normal, 0.1 ten times slower. Must be > 0.
    pub slow: f32,
    /// Prefers reduced motion: animations jump to their end.
    pub reduced: bool,
}

impl Default for MotionConfig {
    fn default() -> Self {
        MotionConfig { slow: 1.0, reduced: false }
    }
}

#[derive(Default)]
pub(crate) struct Animator {
    anims: HashMap<Handle, Vec<Running>>,
    /// The static visual (what `visual_op` set) of every animated node.
    base: HashMap<Handle, Visual>,
    pub config: MotionConfig,
    /// Animation clock (seconds): real time passed times `slow`.
    clock: f64,
    last_real: Option<f64>,
}

impl Animator {
    pub fn is_empty(&self) -> bool {
        self.anims.is_empty()
    }

    /// Handles with an animation that has not finished (they need frames).
    pub fn running(&self) -> impl Iterator<Item = Handle> + '_ {
        self.anims.iter().filter(|(_, v)| v.iter().any(|r| !r.finished)).map(|(&h, _)| h)
    }

    pub fn running_count(&self) -> usize {
        self.anims.values().flatten().filter(|r| !r.finished).count()
    }

    /// Starts `spec` on `h` over the static visual `base`; the first frame applies at once.
    pub fn start(&mut self, h: Handle, base: Visual, spec: Spec) -> Visual {
        if self.running().next().is_none() {
            // idle until now: the next step only sets the baseline, it does not jump over the gap
            self.last_real = None;
        }
        self.base.entry(h).or_insert(base);
        let first = spec.sample(0.0);
        let list = self.anims.entry(h).or_default();
        list.retain(|r| r.spec.id != spec.id);
        list.push(Running { spec, start: self.clock, finished: false, current: first });
        self.compose(h)
    }

    /// Cancels animation `id` of `h`; the visual falls back to the static one (or older animations).
    /// None if `h` has no such animation.
    pub fn cancel(&mut self, h: Handle, id: u64) -> Option<Visual> {
        let list = self.anims.get_mut(&h)?;
        let n = list.len();
        list.retain(|r| r.spec.id != id);
        if list.len() == n {
            return None;
        }
        let v = self.compose(h);
        if self.anims.get(&h).is_some_and(Vec::is_empty) {
            self.anims.remove(&h);
            self.base.remove(&h);
        }
        Some(v)
    }

    /// `visual_op` wrote the static visual of an animated node: the animations stay on top of it.
    /// Returns the composed visual, or None if `h` is not animated.
    pub fn set_base(&mut self, h: Handle, base: Visual) -> Option<Visual> {
        if !self.anims.contains_key(&h) {
            return None;
        }
        self.base.insert(h, base);
        Some(self.compose(h))
    }

    /// The node died: forget its animations.
    pub fn forget(&mut self, h: Handle) {
        self.anims.remove(&h);
        self.base.remove(&h);
    }

    pub fn static_of(&self, h: Handle) -> Option<Visual> {
        self.base.get(&h).copied()
    }

    fn compose(&self, h: Handle) -> Visual {
        let mut v = self.base.get(&h).copied().unwrap_or(Visual::IDENTITY);
        if let Some(list) = self.anims.get(&h) {
            for r in list {
                v.opacity = r.current.opacity.map(|o| (o < 1.0).then_some(o.max(0.0))).unwrap_or(v.opacity);
                v.transform = r.current.transform.or(v.transform);
                v.clip = r.current.clip.or(v.clip);
            }
        }
        v
    }

    /// Advances the clock to egui time `real` (seconds) and samples every animation.
    pub fn step(&mut self, real: f64) -> Step {
        let dt = self.last_real.map_or(0.0, |l| (real - l).max(0.0));
        self.last_real = Some(real);
        self.clock += dt * f64::from(self.config.slow.max(f32::MIN_POSITIVE));
        let mut out = Step::default();
        let handles: Vec<Handle> = self.anims.keys().copied().collect();
        for h in handles {
            let mut changed = false;
            for r in self.anims.get_mut(&h).into_iter().flatten() {
                if r.finished {
                    continue;
                }
                let t_ms = (self.clock - r.start) * 1000.0 - r.spec.delay;
                if t_ms < 0.0 && !self.config.reduced {
                    continue; // still in the delay: the first frame holds
                }
                let end = self.config.reduced || t_ms >= r.spec.duration;
                let p = if end { 1.0 } else { (t_ms / r.spec.duration) as f32 };
                let now = r.spec.sample(p);
                if now != r.current {
                    r.current = now;
                    changed = true;
                }
                if end {
                    r.finished = true;
                    out.done.push((h, r.spec.id));
                }
            }
            if changed {
                out.visuals.push((h, self.compose(h)));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fade(id: u64, duration: f64) -> Spec {
        Spec {
            id,
            duration,
            delay: 0.0,
            frames: vec![Keyframe { opacity: Some(1.0), ..Default::default() }, Keyframe { opacity: Some(0.0), ..Default::default() }],
        }
    }

    #[test]
    fn parse_start_and_cancel() {
        let c = parse(r#"{"id":4,"duration":100,"delay":20,"frames":[{"opacity":1},{"opacity":0,"clip":[-1,0]}]}"#).unwrap();
        let Cmd::Start(s) = c else { panic!() };
        assert_eq!((s.id, s.duration, s.delay, s.frames.len()), (4, 100.0, 20.0, 2));
        assert_eq!(s.frames[1].clip, Some([-1.0, 0.0]));
        assert_eq!(parse(r#"{"id":4,"cancel":true}"#), Ok(Cmd::Cancel(4)));
        assert!(parse(r#"{"id":4,"duration":1,"frames":[]}"#).is_err());
        assert!(parse("nope").is_err());
    }

    #[test]
    fn sampling_interpolates_and_steps_free_axes() {
        let s = fade(1, 100.0);
        assert_eq!(s.sample(0.5).opacity, Some(0.5));
        let c = Spec {
            id: 2,
            duration: 10.0,
            delay: 0.0,
            frames: vec![Keyframe { clip: Some([-1.0, 10.0]), ..Default::default() }, Keyframe { clip: Some([-1.0, 0.0]), ..Default::default() }],
        };
        assert_eq!(c.sample(0.5).clip, Some([-1.0, 5.0]));
    }

    #[test]
    fn plays_delays_finishes_once_and_keeps_the_last_frame() {
        let mut a = Animator::default();
        let h = 7;
        let mut sp = fade(1, 100.0);
        sp.delay = 50.0;
        assert_eq!(a.start(h, Visual::IDENTITY, sp).opacity, None, "frame 0 is opacity 1: identity");
        assert_eq!(a.step(10.0).visuals.len(), 0, "first step sets the clock");
        assert!(a.step(10.03).visuals.is_empty(), "inside the delay");
        let s = a.step(10.1); // 100 ms in: 50 ms into the animation
        assert!((s.visuals[0].1.opacity.unwrap() - 0.5).abs() < 1e-3);
        assert!(a.running().next().is_some());
        let s = a.step(10.3);
        assert_eq!(s.done, vec![(h, 1)]);
        assert_eq!(s.visuals[0].1.opacity, Some(0.0));
        assert!(a.running().next().is_none(), "finished animations ask for no frames");
        assert!(a.step(11.0).done.is_empty());
        assert_eq!(a.cancel(h, 1).unwrap().opacity, None, "cancel restores the static visual");
        assert!(a.is_empty());
    }

    #[test]
    fn slow_motion_and_reduced_motion() {
        let mut a = Animator::default();
        a.config.slow = 0.1;
        a.start(1, Visual::IDENTITY, fade(1, 100.0));
        a.step(0.0);
        assert!(a.step(0.1).done.is_empty(), "100 ms of real time is 10 ms of animation");
        assert!(!a.step(1.2).done.is_empty());

        let mut r = Animator::default();
        r.config.reduced = true;
        r.start(1, Visual::IDENTITY, fade(1, 10_000.0));
        let s = r.step(0.0);
        assert_eq!(s.done, vec![(1, 1)]);
        assert_eq!(s.visuals[0].1.opacity, Some(0.0), "jumps to the end");
    }

    #[test]
    fn static_visual_is_the_base_under_animations() {
        let mut a = Animator::default();
        let base = Visual { inert: true, ..Visual::IDENTITY };
        assert!(a.start(1, base, fade(1, 100.0)).inert);
        let v = a.set_base(1, Visual::IDENTITY).unwrap();
        assert!(!v.inert);
        assert!(a.set_base(2, base).is_none());
    }
}
