//! The compositor's seams: the renderer (`presto_render::Renderer` behind a trait, so tests can record
//! or replace it), the frame clock (`presto_frame::FrameClock` behind a trait, so tests can script
//! one) and compositor-owned scroll motions (presto-anim's scroll physics, not written yet).

use presto_base::anim::FlingCurve;
use presto_base::geom::{DeviceSize, Vec2};
use presto_base::ids::{ImageId, PathId, TextureId};
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
    /// Once per submitted frame, after `queue.submit`: atlas LRU generation, staging-belt recall.
    fn end_frame(&mut self);
    /// Drops the cached atlas tiles of an image a commit removed.
    fn forget_image(&mut self, id: ImageId);
    /// Drops the cached tessellation of a path a commit removed.
    fn forget_path(&mut self, id: PathId);
}

impl SceneRenderer for presto_render::Renderer {
    fn register_texture(&mut self, id: TextureId, view: wgpu::TextureView, size: DeviceSize) {
        presto_render::Renderer::register_texture(self, id, view, size)
    }
    fn unregister_texture(&mut self, id: TextureId) {
        presto_render::Renderer::unregister_texture(self, id)
    }
    fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        scene: &FrameScene<'_>,
    ) {
        presto_render::Renderer::render(self, device, queue, encoder, target, scene);
    }
    fn end_frame(&mut self) {
        presto_render::Renderer::end_frame(self)
    }
    fn forget_image(&mut self, id: ImageId) {
        presto_render::Renderer::forget_image(self, id)
    }
    fn forget_path(&mut self, id: PathId) {
        presto_render::Renderer::forget_path(self, id)
    }
}

/// The default renderer factory: [`presto_render::Renderer`].
pub fn presto_renderer() -> RendererFactory {
    Box::new(|device, queue, format, glyphs| Box::new(presto_render::Renderer::new(device, queue, format, glyphs)))
}

/// Builds the renderer on the compositor thread, at startup and again after device loss.
pub type RendererFactory = Box<
    dyn FnMut(&wgpu::Device, &wgpu::Queue, wgpu::TextureFormat, Arc<dyn GlyphRasterizer>) -> Box<dyn SceneRenderer>
        + Send,
>;

pub use presto_frame::{Action, Signal};

/// One window's frame clock: [`presto_frame::FrameClock`], or a scripted one in tests.
pub trait FramePacer: Send {
    fn on_signal(&mut self, now: Time, sig: Signal) -> SmallVec<[Action; 4]>;
    fn next_wakeup(&self) -> Option<Time>;
}

impl FramePacer for presto_frame::FrameClock {
    fn on_signal(&mut self, now: Time, sig: Signal) -> SmallVec<[Action; 4]> {
        presto_frame::FrameClock::on_signal(self, now, sig)
    }
    fn next_wakeup(&self) -> Option<Time> {
        presto_frame::FrameClock::next_wakeup(self)
    }
}

/// Builds a window's frame clock for a display refresh interval.
pub type PacerFactory = Box<dyn Fn(Span) -> Box<dyn FramePacer> + Send>;

/// The default: a [`presto_frame::FrameClock`] with the default latch for `interval`.
pub fn frame_clock(interval: Span) -> Box<dyn FramePacer> {
    Box::new(presto_frame::FrameClock::new(presto_frame::ClockConfig { interval, ..Default::default() }))
}

/// A compositor-owned scroll motion (fling, smooth scroll): absolute offsets at absolute times
/// (`presto_anim::scroll::ScrollAnimation`).
pub trait ScrollMotion: Send {
    fn offset_at(&self, t: Time) -> Vec2;
    fn done_at(&self, t: Time) -> bool;
}

/// Starts a fling: `curve`, initial `velocity` (points per second), from `offset` at `start`.
pub type FlingFactory = Box<dyn Fn(FlingCurve, Vec2, Vec2, Time) -> Box<dyn ScrollMotion> + Send>;
