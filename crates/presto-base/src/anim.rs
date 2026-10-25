//! Animation specs and events (architecture §12). Sampling (curves, interpolation, springs, scroll
//! physics) lives in `presto-anim`, which both the UI and the compositor thread call with the same
//! pure functions of time.

use crate::color::Color;
use crate::geom::{Affine, Rect, Vec2};
use crate::ids::{AnimationId, NodeId, PropNodeId};
use crate::time::{Span, Time};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum StepPosition {
    Start,
    #[default]
    End,
    JumpNone,
    JumpBoth,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub enum Easing {
    #[default]
    Linear,
    CubicBezier(f32, f32, f32, f32),
    Steps { count: u32, position: StepPosition },
    /// CSS `linear()`: (input, output) points.
    Points(Vec<(f32, f32)>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum FillMode {
    #[default]
    None,
    Forwards,
    Backwards,
    Both,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PlaybackDirection {
    #[default]
    Normal,
    Reverse,
    Alternate,
    AlternateReverse,
}

/// Web Animations `EffectTiming`.
#[derive(Clone, Debug, PartialEq)]
pub struct Timing {
    pub delay: Span,
    pub end_delay: Span,
    pub duration: Span,
    /// `f32::INFINITY` for infinite.
    pub iterations: f32,
    pub iteration_start: f32,
    pub direction: PlaybackDirection,
    pub fill: FillMode,
    pub easing: Easing,
}

/// One transform function of a CSS transform list.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TransformFn {
    Translate(f32, f32),
    Scale(f32, f32),
    Rotate(f32),
    Skew(f32, f32),
    Matrix(Affine),
}

#[derive(Clone, Debug, PartialEq)]
pub enum AnimValue {
    Number(f32),
    Length(f32),
    Color(Color),
    Transform(Vec<TransformFn>),
    Vec2(Vec2),
    Rect(Rect),
}

/// What an animation drives. The first group runs on the compositor; the rest on the UI thread
/// (paint: re-record without layout; layout: style + layout per frame).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AnimProperty {
    // compositor
    Opacity,
    Transform,
    Clip,
    ScrollOffset,
    // paint
    Color,
    BackgroundColor,
    BorderColor,
    Filter,
    StrokeDashOffset,
    // layout
    Width,
    Height,
    /// Any other CSS property, by svelters-style's property id.
    Css(u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Placement {
    Compositor,
    Paint,
    Layout,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Keyframe {
    pub offset: f32,
    pub value: AnimValue,
    /// Per-keyframe easing (to the next keyframe); `None` = linear.
    pub easing: Option<Easing>,
}

/// A spring as a closed form (architecture §12.1): canonical `{duration, bounce}` or physical params.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SpringParams {
    DurationBounce { duration: Span, bounce: f32 },
    Physical { mass: f32, stiffness: f32, damping: f32 },
    /// svelte/motion's per-frame parameters, mapped through the 60 Hz recurrence.
    Svelte { stiffness: f32, damping: f32, precision: f32 },
}

#[derive(Clone, Debug, PartialEq)]
pub enum Generator {
    Keyframes { keyframes: Vec<Keyframe>, timing: Timing },
    Spring { params: SpringParams, from: AnimValue, to: AnimValue, velocity: f32 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum StartTime {
    /// The compositor picks the start (the next frame's present time) and reports it.
    #[default]
    NextFrame,
    At(Time),
    /// Start together with the other animations of this group (aligned start times).
    Group(u64),
}

#[derive(Clone, Debug, PartialEq)]
pub struct AnimationSpec {
    pub id: AnimationId,
    pub node: NodeId,
    /// The property node a compositor animation drives (transform/effect/clip/scroll node).
    pub target: Option<PropNodeId>,
    pub property: AnimProperty,
    pub generator: Generator,
    pub start: StartTime,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AnimationCmd {
    Start(AnimationSpec),
    Cancel(AnimationId),
    /// Keep position and velocity, head for a new end value (springs, smooth scroll).
    Retarget { id: AnimationId, to: AnimValue },
    Pause { id: AnimationId, at: Time },
    Resume { id: AnimationId, at: Time },
}

/// Reported by the compositor to the UI thread (cc's event types).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AnimationEvent {
    Started { id: AnimationId, at: Time },
    Finished { id: AnimationId, at: Time },
    Aborted { id: AnimationId },
}

/// Scroll-physics curves per OS (architecture §12.2; constants in `presto-anim`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FlingCurve {
    /// Chromium's `PhysicsBasedFlingCurve`.
    Chromium,
    /// iOS-style exponential decay, rate per millisecond (0.998 normal, 0.99 fast).
    Exponential { rate_per_ms: f32 },
    /// Android `OverScroller` spline.
    AndroidSpline,
}
