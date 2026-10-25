//! presto's wgpu renderer (architecture §8): turns a compositor [`FrameScene`] into GPU draws.
//!
//! The renderer is target-agnostic: [`Renderer::render`] records into the caller's encoder and view;
//! the compositor owns surfaces, acquire, submit and present. [`headless`] renders into an offscreen
//! texture and reads it back, for tests.
//!
//! A frame: flatten the scene into instance records sorted into batches ([`flatten`]), upload new
//! atlas tiles and the records (one storage buffer per frame in flight, written through a
//! `StagingBelt`), rasterise path batches into MSAA intermediates, then draw every batch under one
//! scissor per damage region. Call [`Renderer::end_frame`] once per submitted frame, after
//! `queue.submit`: it recalls the staging belt and ages the atlases and caches.
//!
//! Targets must be non-sRGB formats (`Bgra8Unorm`, `Rgba8Unorm`): colours are sRGB-encoded and blend
//! in that space (architecture §8.3).

mod atlas;
mod bounds_tree;
mod damage;
mod flatten;
mod gamma;
mod gpu;
pub mod headless;
pub use headless::render_to_image;
mod paths;
mod pipelines;

use std::collections::HashMap;
use std::num::NonZeroU64;
use std::sync::Arc;
use std::time::Instant;

use presto_base::geom::{DeviceRect, DeviceSize, Rect};
use presto_base::ids::{ImageId, PathId, TextureId};
use presto_base::scene::FrameScene;
use presto_base::text::{GlyphKey, GlyphRasterizer};

use crate::atlas::{Atlas, AtlasKey, AtlasKind};
use crate::bounds_tree::BoundsTree;
use crate::flatten::{Ctx, Frame, GlyphEntry, Kind, Tex};
use crate::gpu::Globals;
use crate::paths::PathCache;
use crate::pipelines::{PATH_SAMPLES, Pipelines};

pub use crate::damage::plan as damage_regions;
pub use crate::gamma::{TextParams, gamma_correction_ratios};

/// Frames in flight: instance buffers in the ring.
pub const FRAMES_IN_FLIGHT: usize = 3;
/// Atlas page budget per kind (mask, colour); oversized single-tile pages don't count.
pub const MAX_ATLAS_PAGES: usize = 8;
/// Glyph-index entries kept before the index is reset (rasters stay in the atlas).
const MAX_GLYPH_INDEX: usize = 1 << 16;

/// What one [`Renderer::render`] call did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RenderStats {
    /// Draw calls recorded (main pass, path pre-passes and clears).
    pub draws: u32,
    /// Instances drawn across all pipelines (a path counts once).
    pub instances: u32,
    /// Batches after sorting (one per draw in a single-region frame).
    pub batches: u32,
    /// Bytes uploaded: instance records, atlas tiles, path meshes.
    pub upload_bytes: u64,
    /// Atlas pages alive after the frame (mask + colour).
    pub atlas_pages: u32,
    /// Path pre-passes (one per path batch).
    pub path_passes: u32,
    /// Scissor regions the frame was drawn in.
    pub regions: u32,
    /// Primitives not drawn: missing resources, unregistered textures, atlas overflow, canvas content.
    pub skipped: u32,
    /// CPU time spent in `render`, in microseconds.
    pub cpu_us: u32,
}

struct RingSlot {
    instances: wgpu::Buffer,
    capacity: u64,
    globals: wgpu::Buffer,
    globals_bg: wgpu::BindGroup,
    instances_bg: wgpu::BindGroup,
}

struct GpuPage {
    generation: u64,
    texture: wgpu::Texture,
    linear: wgpu::BindGroup,
    nearest: wgpu::BindGroup,
}

struct Registered {
    linear: wgpu::BindGroup,
    nearest: wgpu::BindGroup,
}

/// A path batch's MSAA target and its resolve texture, reused across frames.
struct PathScratch {
    size: (u32, u32),
    msaa: wgpu::TextureView,
    resolve: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
}

pub struct Renderer {
    device: wgpu::Device,
    format: wgpu::TextureFormat,
    glyphs: Arc<dyn GlyphRasterizer>,
    pipelines: Pipelines,
    linear: wgpu::Sampler,
    nearest: wgpu::Sampler,
    ring: Vec<RingSlot>,
    ring_index: usize,
    belt: wgpu::util::StagingBelt,
    mask: Atlas,
    color: Atlas,
    mask_pages: HashMap<u32, GpuPage>,
    color_pages: HashMap<u32, GpuPage>,
    textures: HashMap<TextureId, Registered>,
    texture_sizes: HashMap<TextureId, DeviceSize>,
    glyph_index: HashMap<GlyphKey, GlyphEntry>,
    paths: PathCache,
    path_scratch: Vec<PathScratch>,
    tree: BoundsTree,
    text: TextParams,
}

fn align_up(v: u64, a: u64) -> u64 {
    v.div_ceil(a) * a
}

/// Appends `items` at an offset that is a multiple of the record size; returns the base index.
fn section<T: bytemuck::Pod>(bytes: &mut Vec<u8>, items: &[T]) -> u32 {
    let size = size_of::<T>() as u64;
    let start = align_up(bytes.len() as u64, size);
    bytes.resize(start as usize, 0);
    bytes.extend_from_slice(bytemuck::cast_slice(items));
    (start / size) as u32
}

impl Renderer {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target_format: wgpu::TextureFormat,
        glyphs: Arc<dyn GlyphRasterizer>,
    ) -> Self {
        let _ = queue;
        let pipelines = Pipelines::new(device, target_format);
        let sampler = |filter: wgpu::FilterMode, label: &str| {
            device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some(label),
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                mag_filter: filter,
                min_filter: filter,
                ..Default::default()
            })
        };
        let max_dim = device.limits().max_texture_dimension_2d;
        let mut r = Renderer {
            device: device.clone(),
            format: target_format,
            glyphs,
            linear: sampler(wgpu::FilterMode::Linear, "presto linear"),
            nearest: sampler(wgpu::FilterMode::Nearest, "presto nearest"),
            ring: Vec::new(),
            ring_index: 0,
            belt: wgpu::util::StagingBelt::new(device.clone(), 1 << 20),
            mask: Atlas::new(AtlasKind::Mask, MAX_ATLAS_PAGES, max_dim),
            color: Atlas::new(AtlasKind::Color, MAX_ATLAS_PAGES, max_dim),
            mask_pages: HashMap::new(),
            color_pages: HashMap::new(),
            textures: HashMap::new(),
            texture_sizes: HashMap::new(),
            glyph_index: HashMap::new(),
            paths: PathCache::default(),
            path_scratch: Vec::new(),
            tree: BoundsTree::default(),
            text: TextParams::default(),
            pipelines,
        };
        for _ in 0..FRAMES_IN_FLIGHT {
            let slot = r.new_slot(device, 64 * 1024);
            r.ring.push(slot);
        }
        r
    }

    /// The target format the pipelines were built for.
    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    /// Text gamma and contrast (read from the OS by the platform layer).
    pub fn set_text_params(&mut self, params: TextParams) {
        self.text = params;
    }

    /// Makes `view` drawable by [`presto_base::display::SurfacePrim`]s naming `id`. The view must be a
    /// filterable float format holding premultiplied colour.
    pub fn register_texture(&mut self, id: TextureId, view: wgpu::TextureView, size: DeviceSize) {
        let device = self.device.clone();
        let linear = self.texture_bind_group(&device, &view, &self.linear);
        let nearest = self.texture_bind_group(&device, &view, &self.nearest);
        self.textures.insert(id, Registered { linear, nearest });
        self.texture_sizes.insert(id, size);
    }

    pub fn unregister_texture(&mut self, id: TextureId) {
        self.textures.remove(&id);
        self.texture_sizes.remove(&id);
    }

    /// Drops the atlas tiles of an image the compositor removed.
    pub fn forget_image(&mut self, id: ImageId) {
        self.mask.remove(&AtlasKey::Image { id, mask: true });
        self.color.remove(&AtlasKey::Image { id, mask: false });
    }

    /// Drops every atlas tile, glyph raster and path mesh: after a text-parameter change that alters
    /// rasters, or to release memory. Everything is re-created lazily by the next frames.
    pub fn clear_caches(&mut self) {
        self.mask.clear();
        self.color.clear();
        self.mask_pages.clear();
        self.color_pages.clear();
        self.glyph_index.clear();
        self.paths.clear_gpu();
        self.path_scratch.clear();
    }

    /// Drops the cached meshes of a path the compositor removed.
    pub fn forget_path(&mut self, id: PathId) {
        self.paths.forget(id);
    }

    fn texture_bind_group(&self, device: &wgpu::Device, view: &wgpu::TextureView, sampler: &wgpu::Sampler) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("presto texture"),
            layout: &self.pipelines.texture_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(sampler) },
            ],
        })
    }

    fn new_slot(&self, device: &wgpu::Device, capacity: u64) -> RingSlot {
        let instances = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("presto instances"),
            size: capacity,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("presto globals"),
            size: size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let globals_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("presto globals"),
            layout: &self.pipelines.globals_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() }],
        });
        let instances_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("presto instances"),
            layout: &self.pipelines.instances_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: instances.as_entire_binding() }],
        });
        RingSlot { instances, capacity, globals, globals_bg, instances_bg }
    }

    /// Mirrors the atlases' pages in GPU textures and queues their pending uploads.
    fn sync_atlas(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, kind: AtlasKind) -> u64 {
        let (format, atlas) = match kind {
            AtlasKind::Mask => (wgpu::TextureFormat::R8Unorm, &mut self.mask),
            AtlasKind::Color => (wgpu::TextureFormat::Rgba8Unorm, &mut self.color),
        };
        let pages: Vec<_> = atlas.pages().collect();
        let uploads = atlas.take_uploads();
        let gpu = match kind {
            AtlasKind::Mask => &mut self.mask_pages,
            AtlasKind::Color => &mut self.color_pages,
        };
        gpu.retain(|i, _| pages.iter().any(|p| p.index == *i));
        for p in &pages {
            if gpu.get(&p.index).is_some_and(|g| g.generation == p.generation) {
                continue;
            }
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("presto atlas page"),
                size: wgpu::Extent3d { width: p.size, height: p.size, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let view = texture.create_view(&Default::default());
            let layout = &self.pipelines.texture_layout;
            let bg = |sampler: &wgpu::Sampler| {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("presto atlas page"),
                    layout,
                    entries: &[
                        wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
                        wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(sampler) },
                    ],
                })
            };
            let (linear, nearest) = (bg(&self.linear), bg(&self.nearest));
            gpu.insert(p.index, GpuPage { generation: p.generation, texture, linear, nearest });
        }
        let bpp = kind.bytes_per_pixel() as u32;
        let mut bytes = 0;
        for u in uploads {
            let Some(page) = gpu.get(&u.page) else { continue };
            bytes += u.data.len() as u64;
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &page.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d { x: u.x, y: u.y, z: 0 },
                    aspect: wgpu::TextureAspect::All,
                },
                &u.data,
                wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(u.w * bpp), rows_per_image: Some(u.h) },
                wgpu::Extent3d { width: u.w, height: u.h, depth_or_array_layers: 1 },
            );
        }
        bytes
    }

    /// Creates the GPU buffers of every mesh this frame draws; returns the bytes uploaded.
    fn upload_meshes(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, frame: &Frame) -> u64 {
        let mut bytes = 0;
        for draw in &frame.paths {
            let Some(mesh) = self.paths.mesh_mut(&draw.mesh) else { continue };
            if mesh.gpu.is_some() {
                continue;
            }
            let make = |data: &[u8], usage: wgpu::BufferUsages| {
                let buf = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("presto path mesh"),
                    size: align_up(data.len() as u64, wgpu::COPY_BUFFER_ALIGNMENT),
                    usage: usage | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                let mut padded = data.to_vec();
                padded.resize(align_up(data.len() as u64, wgpu::COPY_BUFFER_ALIGNMENT) as usize, 0);
                queue.write_buffer(&buf, 0, &padded);
                buf
            };
            let v: &[u8] = bytemuck::cast_slice(&mesh.vertices);
            let i: &[u8] = bytemuck::cast_slice(&mesh.indices);
            bytes += (v.len() + i.len()) as u64;
            mesh.gpu = Some(paths::GpuMesh {
                vertices: make(v, wgpu::BufferUsages::VERTEX),
                indices: make(i, wgpu::BufferUsages::INDEX),
            });
        }
        bytes
    }

    /// A scratch target of at least `w`×`h` not used yet this frame (`used` counts the ones taken).
    fn scratch(&mut self, device: &wgpu::Device, w: u32, h: u32, used: usize) -> usize {
        if let Some(i) =
            (used..self.path_scratch.len()).find(|&i| self.path_scratch[i].size.0 >= w && self.path_scratch[i].size.1 >= h)
        {
            self.path_scratch.swap(used, i);
            return used;
        }
        // Round up so a slowly growing path reuses its target.
        let size = (align_up(w as u64, 256) as u32, align_up(h as u64, 256) as u32);
        let make = |samples: u32, usage: wgpu::TextureUsages| {
            device
                .create_texture(&wgpu::TextureDescriptor {
                    label: Some("presto path intermediate"),
                    size: wgpu::Extent3d { width: size.0, height: size.1, depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: samples,
                    dimension: wgpu::TextureDimension::D2,
                    format: self.format,
                    usage,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        let msaa = make(PATH_SAMPLES, wgpu::TextureUsages::RENDER_ATTACHMENT);
        let resolve = make(1, wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING);
        let bind_group = self.texture_bind_group(device, &resolve, &self.nearest);
        self.path_scratch.push(PathScratch { size, msaa, resolve, bind_group });
        let last = self.path_scratch.len() - 1;
        self.path_scratch.swap(used, last);
        used
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
        let started = Instant::now();
        let mut stats = RenderStats::default();
        let regions = damage::plan(scene.damage, scene.target);
        stats.regions = regions.len() as u32;
        if regions.is_empty() {
            return stats;
        }
        let full = scene.damage.is_empty();
        let cull = regions.iter().skip(1).fold(regions[0], |u, r| u.union(r));
        let cull = Rect::new(cull.x as f32, cull.y as f32, (cull.x + cull.w as i32) as f32, (cull.y + cull.h as i32) as f32);

        if self.glyph_index.len() > MAX_GLYPH_INDEX {
            self.glyph_index.clear();
        }
        let frame = flatten::flatten(
            scene,
            cull,
            &mut Ctx {
                mask: &mut self.mask,
                color: &mut self.color,
                paths: &mut self.paths,
                glyph_index: &mut self.glyph_index,
                glyphs: &*self.glyphs,
                surfaces: &self.texture_sizes,
                tree: &mut self.tree,
            },
        );
        stats.skipped = frame.skipped;
        stats.batches = frame.batches.len() as u32;
        stats.upload_bytes += self.sync_atlas(device, queue, AtlasKind::Mask);
        stats.upload_bytes += self.sync_atlas(device, queue, AtlasKind::Color);
        stats.upload_bytes += self.upload_meshes(device, queue, &frame);

        // Instance records, one section per kind.
        let mut bytes = Vec::new();
        let base_quads = section(&mut bytes, &frame.quads);
        let base_shadows = section(&mut bytes, &frame.shadows);
        let base_lines = section(&mut bytes, &frame.lines);
        let base_masks = section(&mut bytes, &frame.masks);
        let base_images = section(&mut bytes, &frame.images);
        let path_instances: Vec<gpu::PathInstance> = frame.paths.iter().map(|p| p.instance).collect();
        let base_paths = section(&mut bytes, &path_instances);
        let base_sprites = section(&mut bytes, &frame.path_sprites);
        bytes.resize(align_up(bytes.len().max(16) as u64, wgpu::COPY_BUFFER_ALIGNMENT) as usize, 0);

        let slot_idx = self.ring_index;
        self.ring_index = (self.ring_index + 1) % self.ring.len();
        if self.ring[slot_idx].capacity < bytes.len() as u64 {
            let cap = (bytes.len() as u64).next_power_of_two().min(device.limits().max_storage_buffer_binding_size as u64);
            if cap < bytes.len() as u64 {
                // More records than one storage binding holds: draw nothing rather than garbage.
                stats.skipped += (frame.quads.len() + frame.masks.len() + frame.images.len()) as u32;
                return stats;
            }
            self.ring[slot_idx] = self.new_slot(device, cap);
        }
        let globals = Globals {
            viewport: [scene.target.w as f32, scene.target.h as f32, 0.0, 0.0],
            clear: scene.clear.map(|c| c.premultiplied()).unwrap_or([0.0; 4]),
            gamma_ratios: self.text.ratios(),
            text: [self.text.grayscale_contrast, 0.0, 0.0, 0.0],
        };
        {
            let slot = &self.ring[slot_idx];
            let size = NonZeroU64::new(bytes.len() as u64).expect("at least 16 bytes");
            self.belt.write_buffer(encoder, &slot.instances, 0, size).copy_from_slice(&bytes);
            let gsize = NonZeroU64::new(size_of::<Globals>() as u64).expect("non-empty");
            self.belt.write_buffer(encoder, &slot.globals, 0, gsize).copy_from_slice(bytemuck::bytes_of(&globals));
        }
        stats.upload_bytes += bytes.len() as u64 + size_of::<Globals>() as u64;

        // Path pre-passes: each path batch into its own MSAA intermediate, resolved for compositing.
        let mut scratch_for_target = Vec::with_capacity(frame.path_targets.len());
        for (i, t) in frame.path_targets.iter().enumerate() {
            let s = self.scratch(device, t.rect[2].max(1), t.rect[3].max(1), i);
            scratch_for_target.push(s);
        }
        let slot = &self.ring[slot_idx];
        for (t, &s) in frame.path_targets.iter().zip(&scratch_for_target) {
            let scratch = &self.path_scratch[s];
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("presto path"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &scratch.msaa,
                    depth_slice: None,
                    resolve_target: Some(&scratch.resolve),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Discard,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_viewport(0.0, 0.0, t.rect[2].max(1) as f32, t.rect[3].max(1) as f32, 0.0, 1.0);
            pass.set_pipeline(&self.pipelines.path_raster);
            pass.set_bind_group(0, &slot.globals_bg, &[]);
            pass.set_bind_group(1, &slot.instances_bg, &[]);
            for p in t.first..t.first + t.count {
                let draw = &frame.paths[p as usize];
                let Some(mesh) = self.paths.mesh(&draw.mesh) else { continue };
                let Some(gpu) = &mesh.gpu else { continue };
                pass.set_vertex_buffer(0, gpu.vertices.slice(..));
                pass.set_index_buffer(gpu.indices.slice(..), wgpu::IndexFormat::Uint32);
                let inst = base_paths + p;
                pass.draw_indexed(0..mesh.indices.len() as u32, 0, inst..inst + 1);
                stats.draws += 1;
                stats.instances += 1;
            }
            stats.path_passes += 1;
        }

        // The main pass.
        let load = match (full, scene.clear) {
            (true, Some(c)) => {
                let p = c.premultiplied();
                wgpu::LoadOp::Clear(wgpu::Color { r: p[0] as f64, g: p[1] as f64, b: p[2] as f64, a: p[3] as f64 })
            }
            _ => wgpu::LoadOp::Load,
        };
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("presto main"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load, store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_bind_group(0, &slot.globals_bg, &[]);
        pass.set_bind_group(1, &slot.instances_bg, &[]);
        for region in &regions {
            pass.set_scissor_rect(region.x as u32, region.y as u32, region.w, region.h);
            let region_rect = device_rect_to_rect(region);
            if !full && scene.clear.is_some() {
                pass.set_pipeline(&self.pipelines.clear);
                pass.draw(0..3, 0..1);
                stats.draws += 1;
            }
            for batch in &frame.batches {
                if !batch.bounds.intersects(&region_rect) {
                    continue;
                }
                let (pipeline, base) = match batch.kind {
                    Kind::Quad => (&self.pipelines.quad, base_quads),
                    Kind::Shadow => (&self.pipelines.shadow, base_shadows),
                    Kind::Line => (&self.pipelines.line, base_lines),
                    Kind::Mask => (&self.pipelines.mask, base_masks),
                    Kind::Image => (&self.pipelines.image, base_images),
                    Kind::Path => (&self.pipelines.path_sprite, base_sprites),
                };
                let texture = match batch.tex {
                    Tex::None if batch.kind == Kind::Path => {
                        Some(&self.path_scratch[scratch_for_target[batch.first as usize]].bind_group)
                    }
                    Tex::None => None,
                    Tex::MaskPage(p) => match self.mask_pages.get(&p) {
                        Some(page) => Some(&page.linear),
                        None => continue,
                    },
                    Tex::ColorPage { page, nearest } => match self.color_pages.get(&page) {
                        Some(page) => Some(if nearest { &page.nearest } else { &page.linear }),
                        None => continue,
                    },
                    Tex::Surface { id, nearest } => match self.textures.get(&id) {
                        Some(t) => Some(if nearest { &t.nearest } else { &t.linear }),
                        None => continue,
                    },
                };
                pass.set_pipeline(pipeline);
                if let Some(bg) = texture {
                    pass.set_bind_group(2, bg, &[]);
                }
                let first = base + batch.first;
                pass.draw(0..4, first..first + batch.count);
                stats.draws += 1;
                if batch.kind != Kind::Path {
                    stats.instances += batch.count;
                }
            }
        }
        drop(pass);
        self.belt.finish();

        stats.atlas_pages = (self.mask.page_count() + self.color.page_count()) as u32;
        stats.cpu_us = started.elapsed().as_micros().min(u32::MAX as u128) as u32;
        stats
    }

    /// Call once per submitted frame, after `queue.submit`: recalls the staging belt, advances the
    /// atlas LRU generation and ages the path cache.
    pub fn end_frame(&mut self) {
        self.belt.recall();
        self.mask.end_frame();
        self.color.end_frame();
        self.paths.end_frame();
    }
}

fn device_rect_to_rect(r: &DeviceRect) -> Rect {
    Rect::new(r.x as f32, r.y as f32, (r.x + r.w as i32) as f32, (r.y + r.h as i32) as f32)
}
