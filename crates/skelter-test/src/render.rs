//! Pixels for the harness (architecture §16.1 `render`): a wgpu device on whatever adapter the
//! environment gives (lavapipe on Linux CI with `WGPU_BACKEND=vulkan`, WARP on Windows, Metal on
//! macOS) and a `skelter_render::Renderer` drawing the headless compositor's committed tree into
//! `skelter_render`'s offscreen target.

use skelter_base::geom::DeviceSize;
use skelter_base::text::GlyphRasterizer;
use skelter_render::Renderer;
use skelter_render::headless::{FORMAT, HeadlessTarget};
use std::sync::Arc;

/// Why `render` produced no image.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RenderError(pub String);

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "render failed: {}", self.0)
    }
}

impl std::error::Error for RenderError {}

/// A rendered frame: premultiplied RGBA8, rows top to bottom.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl Image {
    /// The pixel at (`x`, `y`) in device pixels.
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * self.width + x) * 4) as usize;
        [
            self.rgba[i],
            self.rgba[i + 1],
            self.rgba[i + 2],
            self.rgba[i + 3],
        ]
    }
}

/// The harness's GPU: one device, one renderer on the headless target format.
pub struct GpuRender {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub renderer: Renderer,
}

impl GpuRender {
    /// Opens an adapter (the default one, then the fallback adapter) and a device.
    pub fn new(glyphs: Arc<dyn GlyphRasterizer>) -> Result<Self, RenderError> {
        let instance =
            wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
        let adapter =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
                .or_else(|_| {
                    pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                        force_fallback_adapter: true,
                        ..Default::default()
                    }))
                })
                .map_err(|e| RenderError(format!("no wgpu adapter: {e}")))?;
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
                .map_err(|e| RenderError(format!("no wgpu device: {e}")))?;
        let renderer = Renderer::new(&device, &queue, FORMAT, glyphs);
        Ok(Self {
            device,
            queue,
            renderer,
        })
    }

    /// Draws `scene` into a fresh target and reads it back.
    pub fn draw(
        &mut self,
        scene: &skelter_base::scene::FrameScene<'_>,
    ) -> Result<Image, RenderError> {
        let size: DeviceSize = scene.target;
        let target = HeadlessTarget::new(&self.device, size);
        target.render(&self.device, &self.queue, &mut self.renderer, scene);
        let rgba = target
            .read_rgba(&self.device, &self.queue)
            .map_err(|e| RenderError(e.to_string()))?;
        Ok(Image {
            width: size.w.max(1),
            height: size.h.max(1),
            rgba,
        })
    }
}
