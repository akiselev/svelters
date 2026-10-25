//! svelte/animate's `flip` (svelte@5.57.2 `src/animate/index.js`, MIT) from presto's layout: the
//! node's rect before (`from`) and after (`to`) a keyed reorder, its untransformed layout size and
//! its transform origin, played as one transform track.
//!
//! Svelte divides the move by `node.clientWidth / to.width / zoom` (the parent transforms and
//! zoom between the layout size and the measured rect). The port in
//! `packages/native-runtime/src/animate.js` computed `to.width / to.width`, always 1; here the
//! layout size is presto's own, so a scaled ancestor is accounted for.

use crate::easing::SvelteEasing;
use crate::transition::{Channel, Plan, Track, Transform64};
use crate::shim::css::TransformList;
use presto_base::anim::{AnimValue, FillMode, Keyframe, PlaybackDirection, Timing};
use presto_base::time::Span;

/// A rect as `getBoundingClientRect` gives it (CSS px, `f64` as in JS).
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Deserialize)]
pub struct DomRect {
    pub left: f64,
    pub top: f64,
    pub width: f64,
    pub height: f64,
}

impl DomRect {
    pub fn from_rect(r: presto_base::geom::Rect) -> DomRect {
        DomRect { left: r.min.x as f64, top: r.min.y as f64, width: r.width() as f64, height: r.height() as f64 }
    }
}

/// What `flip` reads from the node.
#[derive(Clone, Debug, PartialEq)]
pub struct FlipInput {
    pub from: DomRect,
    pub to: DomRect,
    /// `node.clientWidth`, `node.clientHeight`: the layout size without transforms.
    pub client_width: f64,
    pub client_height: f64,
    /// `transform-origin` in px from the node's top left (`getComputedStyle(node).transformOrigin`).
    pub origin: (f64, f64),
    /// The node's computed transform, prepended.
    pub transform: TransformList,
    /// The effective zoom of the node and its ancestors.
    pub zoom: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FlipParams {
    pub delay: f64,
    /// A fixed duration (ms), or `None` for Svelte's default `sqrt(d) * 120` of the distance `d`
    /// (a custom duration function is called in JS and arrives here as a number).
    pub duration: Option<f64>,
    pub easing: SvelteEasing,
}

impl Default for FlipParams {
    fn default() -> Self {
        FlipParams { delay: 0.0, duration: None, easing: SvelteEasing::CubicOut }
    }
}

/// A resolved `flip`: Svelte's `dx`, `dy`, `dsx`, `dsy` and duration.
#[derive(Clone, Debug, PartialEq)]
pub struct Flip {
    pub dx: f64,
    pub dy: f64,
    pub dsx: f64,
    pub dsy: f64,
    pub delay: f64,
    pub duration: f64,
    pub easing: SvelteEasing,
    pub transform: TransformList,
}

impl Flip {
    pub fn new(input: &FlipInput, params: &FlipParams) -> Flip {
        let FlipInput { from, to, client_width, client_height, origin, zoom, .. } = input;
        let ox = origin.0 / client_width;
        let oy = origin.1 / client_height;
        let sx = client_width / to.width / zoom;
        let sy = client_height / to.height / zoom;
        let fx = from.left + from.width * ox;
        let fy = from.top + from.height * oy;
        let tx = to.left + to.width * ox;
        let ty = to.top + to.height * oy;
        let dx = (fx - tx) * sx;
        let dy = (fy - ty) * sy;
        let dsx = from.width / to.width;
        let dsy = from.height / to.height;
        let duration = params.duration.unwrap_or_else(|| (dx * dx + dy * dy).sqrt().sqrt() * 120.0);
        Flip { dx, dy, dsx, dsy, delay: params.delay, duration, easing: params.easing, transform: input.transform.clone() }
    }

    /// Svelte's `css(t, u)`: the computed transform, `translate(u dx, u dy)`, `scale(t + u dsx, t + u dsy)`.
    pub fn css(&self, t: f64, u: f64) -> Vec<Transform64> {
        let mut list = self.transform.clone();
        list.push(Transform64::Translate(u * self.dx, u * self.dy));
        list.push(Transform64::Scale(t + u * self.dsx, t + u * self.dsy));
        list
    }

    /// The transform track from `t = 0` to `t = 1` (an animation always runs the full way).
    pub fn plan(&self) -> Plan {
        let list = |t: f64| AnimValue::Transform(self.css(t, 1.0 - t).iter().map(|f| f.to_presto()).collect());
        let keyframes = vec![Keyframe { offset: 0.0, value: list(0.0), easing: None }, Keyframe { offset: 1.0, value: list(1.0), easing: None }];
        Plan {
            tracks: vec![Track { channel: Channel::Transform, keyframes }],
            statics: Vec::new(),
            overflow_hidden: false,
            origin: None,
            easing: self.easing,
            timing: Timing {
                delay: Span::from_secs_f64(self.delay / 1000.0),
                end_delay: Span::ZERO,
                duration: Span::from_secs_f64(self.duration / 1000.0),
                iterations: 1.0,
                iteration_start: 0.0,
                direction: PlaybackDirection::Normal,
                fill: FillMode::Forwards,
                easing: self.easing.to_presto(crate::transition::EASING_TOLERANCE),
            },
        }
    }
}
