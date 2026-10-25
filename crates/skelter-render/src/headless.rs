//! Headless rendering: an offscreen `Rgba8Unorm` target and its readback, for tests and tools.

use skelter_base::geom::DeviceSize;
use skelter_base::scene::FrameScene;

use crate::{RenderStats, Renderer};

/// The format of [`HeadlessTarget`]s (create the [`Renderer`] with it).
pub const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// A persistent offscreen target: successive renders draw over its previous content, as a window's
/// surface does with damage.
pub struct HeadlessTarget {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    size: DeviceSize,
}

impl HeadlessTarget {
    pub fn new(device: &wgpu::Device, size: DeviceSize) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("skelter headless target"),
            size: wgpu::Extent3d { width: size.w.max(1), height: size.h.max(1), depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        HeadlessTarget { texture, view, size }
    }

    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    pub fn size(&self) -> DeviceSize {
        self.size
    }

    /// Renders `scene`, submits and ends the renderer's frame.
    pub fn render(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        renderer: &mut Renderer,
        scene: &FrameScene<'_>,
    ) -> RenderStats {
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("skelter headless") });
        let stats = renderer.render(device, queue, &mut encoder, &self.view, scene);
        queue.submit([encoder.finish()]);
        renderer.end_frame();
        stats
    }

    /// The target's pixels: `w * h * 4` bytes, rows top to bottom, premultiplied RGBA8.
    pub fn read_rgba(&self, device: &wgpu::Device, queue: &wgpu::Queue) -> Result<Vec<u8>, ReadbackError> {
        let (w, h) = (self.size.w.max(1), self.size.h.max(1));
        let row = w * 4;
        let padded = row.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("skelter readback"),
            size: (padded * h) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("skelter readback") });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(padded), rows_per_image: Some(h) },
            },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        queue.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer.slice(..).map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        device.poll(wgpu::PollType::wait_indefinitely()).map_err(|e| ReadbackError(e.to_string()))?;
        rx.recv().map_err(|e| ReadbackError(e.to_string()))?.map_err(|e| ReadbackError(e.to_string()))?;
        let mut out = Vec::with_capacity((row * h) as usize);
        {
            let data = buffer.slice(..).get_mapped_range().map_err(|e| ReadbackError(format!("{e:?}")))?;
            for y in 0..h as usize {
                out.extend_from_slice(&data[y * padded as usize..][..row as usize]);
            }
        }
        buffer.unmap();
        Ok(out)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadbackError(pub String);

impl std::fmt::Display for ReadbackError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "readback failed: {}", self.0)
    }
}

impl std::error::Error for ReadbackError {}

/// Renders `scene` into a fresh target of `scene.target` and returns its RGBA8 pixels. `renderer`
/// must have been created with [`FORMAT`].
pub fn render_to_image(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    renderer: &mut Renderer,
    scene: &FrameScene<'_>,
) -> Result<Vec<u8>, ReadbackError> {
    let target = HeadlessTarget::new(device, scene.target);
    target.render(device, queue, renderer, scene);
    target.read_rgba(device, queue)
}
