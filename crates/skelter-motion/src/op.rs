//! The `OP_ANIMATE` payload (`host.style(11, handle, json)`, ABI v2.9, `packages/native-runtime/src/motion.js`):
//! an animation's channel keyframes and timing, sent once, plus (MOTION-CSS) the `native`
//! descriptor of a built-in transition or `flip`, from which skelter evaluates the formulas itself.
//!
//! `{ "id": 7, "duration": 400, "delay": 0, "frames": [{ "opacity": 0, "transform": [tx, ty, sx, sy, ox, oy], "clip": [w, h] }, ...],
//!    "native": { "kind": "fly", "params": {...}, "computed": {...}, "t1": 0, "t2": 1 } }`
//! or `{ "id": 7, "cancel": true }`.
//!
//! [`AnimateOp::plan`] prefers the descriptor and falls back to the frames (evenly spaced, linear)
//! when there is none or skelter cannot resolve it ([`NotNative`]).

use crate::easing::SvelteEasing;
use crate::flip::{DomRect, Flip, FlipInput, FlipParams};
use crate::transition::{parse_transform, Channel, Computed, NotNative, Params, Plan, Run, Track, Transition, TransitionKind};
use skelter_base::anim::{AnimValue, Easing, FillMode, Keyframe, PlaybackDirection, Timing, TransformFn};
use skelter_base::geom::Vec2;
use skelter_base::time::Span;
use serde::Deserialize;

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct AnimateOp {
    pub id: u64,
    pub cancel: bool,
    /// ms
    pub duration: f64,
    /// ms
    pub delay: f64,
    pub frames: Vec<ChannelFrame>,
    pub native: Option<NativeDescriptor>,
}

/// One keyframe as motion.js's `parseFrame` gives it.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct ChannelFrame {
    pub opacity: Option<f64>,
    /// `[tx, ty, sx, sy, ox, oy]`: px, factors, origin as fractions.
    pub transform: Option<[f64; 6]>,
    /// `[w, h]` px, `-1` = that axis unclipped.
    pub clip: Option<[f64; 2]>,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct NativeDescriptor {
    /// A svelte/transition built-in (`fade`, ...) or `flip`.
    pub kind: String,
    pub params: Params,
    pub computed: Computed,
    /// The run: `t` at the first and the last keyframe.
    pub t1: f64,
    pub t2: f64,
    /// `flip` only.
    pub flip: Option<FlipDescriptor>,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct FlipDescriptor {
    pub from: DomRect,
    pub to: DomRect,
    pub client_width: f64,
    pub client_height: f64,
    /// `transform-origin` in px.
    pub origin: [f64; 2],
    pub zoom: Option<f64>,
}

impl AnimateOp {
    pub fn parse(json: &str) -> Result<AnimateOp, String> {
        serde_json::from_str(json).map_err(|e| format!("OP_ANIMATE payload: {e}"))
    }

    /// The tracks skelter plays: from the descriptor when it resolves, else from the frames.
    pub fn plan(&self) -> Plan {
        match self.native_plan() {
            Some(Ok(plan)) => plan,
            _ => self.frames_plan(),
        }
    }

    /// The descriptor's plan; `None` without one.
    pub fn native_plan(&self) -> Option<Result<Plan, NotNative>> {
        let d = self.native.as_ref()?;
        Some(self.resolve(d))
    }

    fn resolve(&self, d: &NativeDescriptor) -> Result<Plan, NotNative> {
        let easing = match &d.params.easing {
            Some(n) => Some(SvelteEasing::from_name(n).ok_or_else(|| NotNative::UnknownEasing(n.clone()))?),
            None => None,
        };
        if d.kind == "flip" {
            let f = d.flip.clone().unwrap_or_default();
            let transform = if d.computed.transform == "none" {
                Vec::new()
            } else {
                parse_transform(&d.computed.transform).ok_or_else(|| NotNative::Style("transform", d.computed.transform.clone()))?
            };
            let input = FlipInput {
                from: f.from,
                to: f.to,
                client_width: f.client_width,
                client_height: f.client_height,
                origin: (f.origin[0], f.origin[1]),
                transform,
                zoom: f.zoom.unwrap_or(1.0),
            };
            // the run's own duration and delay are the ones the JS animation got
            let params = FlipParams { delay: self.delay, duration: Some(self.duration), easing: easing.unwrap_or(SvelteEasing::CubicOut) };
            return Ok(Flip::new(&input, &params).plan());
        }
        let kind = TransitionKind::from_name(&d.kind).ok_or_else(|| NotNative::Style("kind", d.kind.clone()))?;
        let t = Transition::new(kind, &d.params, &d.computed)?;
        let run = Run { t1: d.t1, t2: d.t2, duration: self.duration, delay: self.delay };
        Ok(t.plan(&run))
    }

    /// The channel keyframes as tracks, evenly spaced, linear (Svelte's easing is baked into them).
    pub fn frames_plan(&self) -> Plan {
        let n = self.frames.len();
        let offset = |i: usize| if n <= 1 { 0.0 } else { i as f32 / (n - 1) as f32 };
        let mut tracks = Vec::new();
        let mut origin = None;
        let opacity: Vec<Keyframe> = self
            .frames
            .iter()
            .enumerate()
            .filter_map(|(i, f)| f.opacity.map(|o| Keyframe { offset: offset(i), value: AnimValue::Number(o as f32), easing: None }))
            .collect();
        if !opacity.is_empty() {
            tracks.push(Track { channel: Channel::Opacity, keyframes: opacity });
        }
        let transform: Vec<Keyframe> = self
            .frames
            .iter()
            .enumerate()
            .filter_map(|(i, f)| {
                let [tx, ty, sx, sy, ox, oy] = f.transform?;
                origin.get_or_insert((ox, oy));
                Some(Keyframe {
                    offset: offset(i),
                    value: AnimValue::Transform(vec![TransformFn::Translate(tx as f32, ty as f32), TransformFn::Scale(sx as f32, sy as f32)]),
                    easing: None,
                })
            })
            .collect();
        if !transform.is_empty() {
            tracks.push(Track { channel: Channel::Transform, keyframes: transform });
        }
        let clip: Vec<Keyframe> = self
            .frames
            .iter()
            .enumerate()
            .filter_map(|(i, f)| f.clip.map(|[w, h]| Keyframe { offset: offset(i), value: AnimValue::Vec2(Vec2 { x: w as f32, y: h as f32 }), easing: None }))
            .collect();
        if !clip.is_empty() {
            tracks.push(Track { channel: Channel::ClipSize, keyframes: clip });
        }
        Plan {
            tracks,
            statics: Vec::new(),
            overflow_hidden: false,
            origin,
            easing: SvelteEasing::Linear,
            timing: Timing {
                delay: Span::from_secs_f64(self.delay / 1000.0),
                end_delay: Span::ZERO,
                duration: Span::from_secs_f64(self.duration / 1000.0),
                iterations: 1.0,
                iteration_start: 0.0,
                direction: PlaybackDirection::Normal,
                fill: FillMode::Forwards,
                easing: Easing::Linear,
            },
        }
    }
}
