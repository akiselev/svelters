//! The compositor's seams: the renderer (`skelter_render::Renderer` behind a trait, so tests can record
//! or replace it) and the frame clock (`skelter_frame::FrameClock` behind a trait, so tests can
//! script one).

use skelter_base::geom::DeviceSize;
use skelter_base::ids::{CanvasId, ImageId, PathId, TextureId};
use skelter_base::scene::FrameScene;
use skelter_base::text::GlyphRasterizer;
use skelter_base::time::{Span, Time};
use smallvec::SmallVec;
use std::sync::Arc;

/// What the compositor needs from `skelter_render::Renderer`.
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
    /// Makes a canvas drawable by `Prim::Canvas` items naming its id.
    fn register_canvas(&mut self, canvas: Arc<skelter_canvas::Canvas>);
    fn unregister_canvas(&mut self, id: CanvasId);
    /// The camera, view transform and LOD settings a canvas is drawn with this frame.
    fn set_canvas_view(&mut self, id: CanvasId, view: skelter_canvas::CanvasView);
}

impl SceneRenderer for skelter_render::Renderer {
    fn register_texture(&mut self, id: TextureId, view: wgpu::TextureView, size: DeviceSize) {
        skelter_render::Renderer::register_texture(self, id, view, size)
    }
    fn unregister_texture(&mut self, id: TextureId) {
        skelter_render::Renderer::unregister_texture(self, id)
    }
    fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        scene: &FrameScene<'_>,
    ) {
        skelter_render::Renderer::render(self, device, queue, encoder, target, scene);
    }
    fn end_frame(&mut self) {
        skelter_render::Renderer::end_frame(self)
    }
    fn forget_image(&mut self, id: ImageId) {
        skelter_render::Renderer::forget_image(self, id)
    }
    fn forget_path(&mut self, id: PathId) {
        skelter_render::Renderer::forget_path(self, id)
    }
    fn register_canvas(&mut self, canvas: Arc<skelter_canvas::Canvas>) {
        skelter_render::Renderer::register_canvas(self, canvas)
    }
    fn unregister_canvas(&mut self, id: CanvasId) {
        skelter_render::Renderer::unregister_canvas(self, id)
    }
    fn set_canvas_view(&mut self, id: CanvasId, view: skelter_canvas::CanvasView) {
        skelter_render::Renderer::set_canvas_view(self, id, view)
    }
}

/// The default renderer factory: [`skelter_render::Renderer`].
pub fn skelter_renderer() -> RendererFactory {
    Box::new(|device, queue, format, glyphs| Box::new(skelter_render::Renderer::new(device, queue, format, glyphs)))
}

/// Builds the renderer on the compositor thread, at startup and again after device loss.
pub type RendererFactory = Box<
    dyn FnMut(&wgpu::Device, &wgpu::Queue, wgpu::TextureFormat, Arc<dyn GlyphRasterizer>) -> Box<dyn SceneRenderer>
        + Send,
>;

pub use skelter_frame::{Action, Signal};

/// One window's frame clock: [`skelter_frame::FrameClock`], or a scripted one in tests.
pub trait FramePacer: Send {
    fn on_signal(&mut self, now: Time, sig: Signal) -> SmallVec<[Action; 4]>;
    fn next_wakeup(&self) -> Option<Time>;
    /// The snapshot the UI thread reads for urgent frames; `None` for a clock that has none.
    fn frame_args(&self) -> Option<skelter_frame::FrameArgs> {
        None
    }
}

impl FramePacer for skelter_frame::FrameClock {
    fn on_signal(&mut self, now: Time, sig: Signal) -> SmallVec<[Action; 4]> {
        skelter_frame::FrameClock::on_signal(self, now, sig)
    }
    fn next_wakeup(&self) -> Option<Time> {
        skelter_frame::FrameClock::next_wakeup(self)
    }
    fn frame_args(&self) -> Option<skelter_frame::FrameArgs> {
        Some(skelter_frame::FrameClock::frame_args(self))
    }
}

/// Builds a window's frame clock for a display refresh interval.
pub type PacerFactory = Box<dyn Fn(Span) -> Box<dyn FramePacer> + Send>;

/// The default: a [`skelter_frame::FrameClock`] with the default latch for `interval`.
pub fn frame_clock(interval: Span) -> Box<dyn FramePacer> {
    Box::new(skelter_frame::FrameClock::new(skelter_frame::ClockConfig { interval, ..Default::default() }))
}
