//! The compositor's seams to the crates other lanes write: the renderer (`presto-render`), the frame
//! clock (`presto-frame`) and animation sampling (`presto-anim`).
//!
//! Each seam is a trait with the signature the batch brief gives the real crate, so the compositor
//! compiles and is testable before those crates exist; when they land, their types implement these
//! traits (or replace them) without changing the compositor's logic. [`Signal`] and [`Action`] mirror
//! `presto_frame::{Signal, Action}`; [`Sample`] mirrors `presto_anim::Sample`.

use presto_base::anim::{AnimValue, FlingCurve, Generator, TransformFn};
use presto_base::geom::{Affine, DeviceSize, Vec2};
use presto_base::ids::{FrameId, Revision, TextureId};
use presto_base::scene::FrameScene;
use presto_base::text::GlyphRasterizer;
use presto_base::time::{Span, Time};
use smallvec::SmallVec;
use std::sync::Arc;

/// What the compositor needs from `presto_render::Renderer`.
pub trait SceneRenderer: Send {
    fn register_texture(&mut self, id: TextureId, view: wgpu::TextureView, size: DeviceSize);
    fn unregister_texture(&mut self, id: TextureId);
    /// Encode one frame into `target`: only `scene.damage`, scissored; the whole target when empty.
    fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        scene: &FrameScene<'_>,
    );
    /// Atlas LRU generation, instance-ring advance.
    fn end_frame(&mut self);
}

/// Builds the renderer on the compositor thread, at startup and again after device loss.
pub type RendererFactory = Box<
    dyn FnMut(&wgpu::Device, &wgpu::Queue, wgpu::TextureFormat, Arc<dyn GlyphRasterizer>) -> Box<dyn SceneRenderer>
        + Send,
>;

/// An input to a window's frame clock (`presto_frame::Signal`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Signal {
    Vsync { target: Time, interval: Span },
    CommitArrived { revision: Revision, frame: Option<FrameId> },
    /// The UI thread has dirty state and wants a `BeginFrame` (not in the brief's list; requested from
    /// presto-frame).
    NeedsUiFrame,
    CompositorDamage,
    AnimationsActive(bool),
    Presented { frame: FrameId, at: Time, estimated: bool },
    Visible(bool),
    HighRateInput,
    Timer,
    GpuDone { frame: FrameId, at: Time },
    UiCost { frame: FrameId, cost: Span },
    /// A `Draw` found no damage and presented nothing: the frame is not in flight.
    DidNotDraw { frame: FrameId },
}

/// A decision of a window's frame clock (`presto_frame::Action`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    SubscribeVsync(bool),
    BeginUiFrame { frame: FrameId, target: Time, start_at: Time, deadline: Time },
    Draw { frame: FrameId, target: Time },
    SetTimer(Time),
}

/// One window's frame clock (`presto_frame::FrameClock`): a pure state machine fed signals with the
/// time they happened at.
pub trait FramePacer: Send {
    fn on_signal(&mut self, now: Time, sig: Signal) -> SmallVec<[Action; 4]>;
    fn next_wakeup(&self) -> Option<Time>;
}

/// Builds a window's frame clock for a display refresh interval.
pub type PacerFactory = Box<dyn Fn(Span) -> Box<dyn FramePacer> + Send>;

/// One sample of an animation at a time (`presto_anim::Sample`).
#[derive(Clone, Debug, PartialEq)]
pub struct Sample {
    pub value: AnimValue,
    pub done: bool,
    /// Rate of change of the animated value per second, along its primary axis (springs retarget with it).
    pub velocity: f32,
}

/// Animation math (`presto_anim::{sample, interp::compose}`).
pub trait AnimSampler: Send + Sync {
    fn sample(&self, generator: &Generator, start: Time, now: Time) -> Sample;
    fn compose(&self, fns: &[TransformFn]) -> Affine;
}

/// A compositor-owned scroll motion (fling, smooth scroll): absolute offsets at absolute times
/// (`presto_anim::scroll::ScrollAnimation`).
pub trait ScrollMotion: Send {
    fn offset_at(&self, t: Time) -> Vec2;
    fn done_at(&self, t: Time) -> bool;
}

/// Starts a fling: `curve`, initial `velocity` (points per second), from `offset` at `start`.
pub type FlingFactory = Box<dyn Fn(FlingCurve, Vec2, Vec2, Time) -> Box<dyn ScrollMotion> + Send>;
