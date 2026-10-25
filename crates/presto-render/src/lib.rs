//! presto's wgpu renderer (architecture §8): turns a compositor [`FrameScene`] into GPU draws.
//!
//! The renderer is target-agnostic: [`Renderer::render`] records into a caller's encoder and view; the
//! compositor owns surfaces, acquire and present. [`render_to_image`] is the headless path tests use.

use std::sync::Arc;

use presto_base::geom::DeviceSize;
use presto_base::ids::TextureId;
use presto_base::scene::FrameScene;
use presto_base::text::GlyphRasterizer;

/// What one [`Renderer::render`] call did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RenderStats {
    /// Draw calls recorded.
    pub draws: u32,
    /// Instances drawn across all pipelines.
    pub instances: u32,
    /// Bytes uploaded (instances, atlas tiles, path vertices).
    pub upload_bytes: u64,
    /// Atlas pages alive after the frame (mask + colour).
    pub atlas_pages: u32,
    /// CPU time spent in `render`, in microseconds.
    pub cpu_us: u32,
}

pub struct Renderer {
    target_format: wgpu::TextureFormat,
    glyphs: Arc<dyn GlyphRasterizer>,
}

impl Renderer {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target_format: wgpu::TextureFormat,
        glyphs: Arc<dyn GlyphRasterizer>,
    ) -> Self {
        let _ = (device, queue);
        Renderer { target_format, glyphs }
    }

    pub fn register_texture(&mut self, id: TextureId, view: wgpu::TextureView, size: DeviceSize) {
        let _ = (id, view, size);
    }

    pub fn unregister_texture(&mut self, id: TextureId) {
        let _ = id;
    }

    /// Encode one frame into `target` (only `scene.damage`, scissored; full when empty).
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        scene: &FrameScene<'_>,
    ) -> RenderStats {
        let _ = (device, queue, encoder, target, scene, &self.glyphs, self.target_format);
        RenderStats::default()
    }

    /// Advance the atlas LRU generation and the instance ring.
    pub fn end_frame(&mut self) {}
}
