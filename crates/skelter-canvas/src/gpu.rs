//! The canvas renderer: draws [`Placement`]s of canvases inside a host renderer's pass.
//!
//! Per frame, [`CanvasRenderer::prepare`] (before the host's main pass) plans each placement's planes
//! ([`crate::cull`]), uploads the buffers of chunks it has not seen (keyed by chunk version, kept while
//! drawn), uploads glyphs new to the glyph table, writes one uniform per chunk draw, and renders the
//! static plane into the placement's cache texture when its content or view changed. Then
//! [`CanvasRenderer::draw`] (inside the host's pass, under its scissor) blits the cache, or draws the
//! static chunks directly while the view is moving, and draws the overlay chunks on top. Call
//! [`CanvasRenderer::end_frame`] once per submitted frame.
//!
//! A pan or zoom is one camera change: chunk buffers stay as they are; only the uniforms change.

use crate::canvas::{Canvas, Plane, Snapshot};
use crate::chunk::{GlyphPlacement, PolyVertex, ShapeInstance};
use crate::cull::{plan, ChunkDraw};
use crate::slug::{GlyphRecord, GlyphTable};
use crate::text::GlyphKey;
use crate::world::Projection;
use crate::CanvasView;
use skelter_base::geom::{Affine, DeviceSize, Rect};
use skelter_base::ids::CanvasId;
use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;

/// Frames a chunk's buffers (or a cache texture) survive without being drawn.
const KEEP_FRAMES: u64 = 120;
/// Antialiasing width in device pixels.
const AA_PX: f32 = 1.0;

/// One canvas drawn in the host's frame.
#[derive(Clone, Debug)]
pub struct Placement {
    pub canvas: Arc<Canvas>,
    pub view: CanvasView,
    /// The canvas rectangle's size in points.
    pub size: (f32, f32),
    /// Canvas points (`(0, 0)` = the rectangle's top-left) → device pixels of the target, before the
    /// view transform (which this renderer applies).
    pub to_device: Affine,
    /// Where it is visible, in device pixels of the target.
    pub clip: Rect,
    pub opacity: f32,
}

/// What [`CanvasRenderer::prepare`] did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CanvasStats {
    pub placements: u32,
    pub chunks_visible: u32,
    pub chunk_draws: u32,
    pub shapes: u64,
    pub glyphs: u64,
    pub upload_bytes: u64,
    /// Static planes drawn into their cache this frame.
    pub cache_renders: u32,
    /// Static planes shown from an unchanged cache.
    pub cache_hits: u32,
    /// Static planes drawn directly (the view is moving).
    pub direct: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
struct DrawUniform {
    m: [f32; 4],
    t: [f32; 4],
    target: [f32; 4],
    origin: [f32; 4],
}

/// The GPU form of one glyph placement (an instance-rate vertex).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
struct GlyphVertex {
    anchor: [f32; 2],
    pen: [f32; 2],
    rot: [f32; 2],
    size: f32,
    glyph: u32,
    color: u32,
}

struct GpuChunk {
    shapes: Option<wgpu::Buffer>,
    greek: Option<wgpu::Buffer>,
    polys: Option<(wgpu::Buffer, wgpu::Buffer)>,
    glyphs: Option<wgpu::Buffer>,
    last_used: u64,
}

/// What draws a static plane this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
enum StaticMode {
    None,
    /// Blit the canvas's cache texture with the uniform at this offset.
    Cached { uniform: u32 },
    /// Draw `draws[first..first + count]` directly.
    Direct,
}

/// A prepared chunk draw: its uniform and what to draw of it.
struct Prepared {
    uniform: u32,
    version: u64,
    draw: ChunkDraw,
}

struct PreparedPlacement {
    canvas: CanvasId,
    statics: StaticMode,
    direct: std::ops::Range<usize>,
    overlay: std::ops::Range<usize>,
}

/// Everything that decides a static cache's pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
struct CacheKey {
    revision: u64,
    projection: Projection,
    lod: crate::LodParams,
    rect: [i32; 4],
}

struct StaticCache {
    /// The key of the last frame that drew this canvas.
    key: Option<CacheKey>,
    /// The texture holds the plane as `key` describes it.
    valid: bool,
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    size: DeviceSize,
    /// The key changed on the previous frame too: the view is moving, so draw directly.
    changed_last_frame: bool,
    last_used: u64,
}

pub struct CanvasRenderer {
    device: wgpu::Device,
    format: wgpu::TextureFormat,
    uniform_layout: wgpu::BindGroupLayout,
    glyph_layout: wgpu::BindGroupLayout,
    texture_layout: wgpu::BindGroupLayout,
    shape: wgpu::RenderPipeline,
    poly: wgpu::RenderPipeline,
    glyph: wgpu::RenderPipeline,
    blit: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
    align: u32,
    uniforms: wgpu::Buffer,
    uniform_capacity: u64,
    uniform_bg: wgpu::BindGroup,
    staged: Vec<u8>,
    chunks: HashMap<u64, GpuChunk>,
    table: GlyphTable,
    glyph_index: HashMap<GlyphKey, u32>,
    glyph_gpu: Option<(wgpu::Buffer, wgpu::Buffer, wgpu::Buffer, wgpu::BindGroup, [u64; 3])>,
    caches: HashMap<CanvasId, StaticCache>,
    prepared: Vec<Prepared>,
    placements: Vec<PreparedPlacement>,
    frame: u64,
}

fn align_up(v: u64, a: u64) -> u64 {
    v.div_ceil(a) * a
}

impl CanvasRenderer {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("skelter-canvas shaders"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("shaders.wgsl"))),
        });
        let vf = wgpu::ShaderStages::VERTEX_FRAGMENT;
        let uniform_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("skelter-canvas draw"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: vf,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: true,
                    min_binding_size: wgpu::BufferSize::new(size_of::<DrawUniform>() as u64),
                },
                count: None,
            }],
        });
        let storage = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: vf,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only: true },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let glyph_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("skelter-canvas glyphs"),
            entries: &[storage(0), storage(1), storage(2)],
        });
        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("skelter-canvas cache"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: vf,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: vf,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let layout = |name: &str, groups: &[Option<&wgpu::BindGroupLayout>]| {
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(name),
                bind_group_layouts: groups,
                immediate_size: 0,
            })
        };
        let plain = layout("skelter-canvas plain", &[Some(&uniform_layout)]);
        let glyphs = layout("skelter-canvas glyph", &[Some(&uniform_layout), Some(&glyph_layout)]);
        let textured = layout("skelter-canvas blit", &[Some(&uniform_layout), Some(&texture_layout)]);
        let strip = wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleStrip, ..Default::default() };
        let list = wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, ..Default::default() };
        let make = |name: &str,
                    layout: &wgpu::PipelineLayout,
                    vs: &str,
                    fs: &str,
                    primitive: wgpu::PrimitiveState,
                    buffers: &[Option<wgpu::VertexBufferLayout<'_>>]| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(name),
                layout: Some(layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some(vs),
                    compilation_options: Default::default(),
                    buffers,
                },
                primitive,
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some(fs),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        use wgpu::VertexFormat as F;
        let attr = |format, offset, shader_location| wgpu::VertexAttribute { format, offset, shader_location };
        let shape_attrs = [
            attr(F::Float32x2, 0, 0),
            attr(F::Float32x2, 8, 1),
            attr(F::Float32x4, 16, 2),
            attr(F::Uint32, 32, 3),
            attr(F::Uint32, 36, 4),
        ];
        let shape_buf = wgpu::VertexBufferLayout {
            array_stride: size_of::<ShapeInstance>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &shape_attrs,
        };
        let poly_attrs = [attr(F::Float32x2, 0, 0), attr(F::Uint32, 8, 1)];
        let poly_buf = wgpu::VertexBufferLayout {
            array_stride: size_of::<PolyVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &poly_attrs,
        };
        let glyph_attrs = [
            attr(F::Float32x2, 0, 0),
            attr(F::Float32x2, 8, 1),
            attr(F::Float32x2, 16, 2),
            attr(F::Float32, 24, 3),
            attr(F::Uint32, 28, 4),
            attr(F::Uint32, 32, 5),
        ];
        let glyph_buf = wgpu::VertexBufferLayout {
            array_stride: size_of::<GlyphVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &glyph_attrs,
        };
        let shape = make("skelter-canvas shape", &plain, "vs_shape", "fs_shape", strip, &[Some(shape_buf)]);
        let poly = make("skelter-canvas poly", &plain, "vs_poly", "fs_poly", list, &[Some(poly_buf)]);
        let glyph = make("skelter-canvas glyph", &glyphs, "vs_glyph", "fs_glyph", strip, &[Some(glyph_buf)]);
        let blit = make("skelter-canvas blit", &textured, "vs_blit", "fs_blit", strip, &[]);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("skelter-canvas cache"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let align = device.limits().min_uniform_buffer_offset_alignment.max(size_of::<DrawUniform>() as u32);
        let (uniforms, uniform_bg) = Self::uniform_buffer(device, &uniform_layout, 64 * align as u64);
        CanvasRenderer {
            device: device.clone(),
            format,
            uniform_layout,
            glyph_layout,
            texture_layout,
            shape,
            poly,
            glyph,
            blit,
            sampler,
            align,
            uniforms,
            uniform_capacity: 64 * align as u64,
            uniform_bg,
            staged: Vec::new(),
            chunks: HashMap::new(),
            table: GlyphTable::default(),
            glyph_index: HashMap::new(),
            glyph_gpu: None,
            caches: HashMap::new(),
            prepared: Vec::new(),
            placements: Vec::new(),
            frame: 0,
        }
    }

    fn uniform_buffer(device: &wgpu::Device, layout: &wgpu::BindGroupLayout, size: u64) -> (wgpu::Buffer, wgpu::BindGroup) {
        let buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("skelter-canvas draws"),
            size,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("skelter-canvas draws"),
            layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &buf,
                    offset: 0,
                    size: wgpu::BufferSize::new(size_of::<DrawUniform>() as u64),
                }),
            }],
        });
        (buf, bg)
    }

    fn push_uniform(&mut self, u: DrawUniform) -> u32 {
        let offset = align_up(self.staged.len() as u64, self.align as u64);
        self.staged.resize(offset as usize, 0);
        self.staged.extend_from_slice(bytemuck::bytes_of(&u));
        offset as u32
    }

    fn chunk_uniform(&self, d: &ChunkDraw, target: DeviceSize, origin: [f32; 2], min_stroke: f32, opacity: f32) -> DrawUniform {
        let x = &d.xform;
        DrawUniform {
            m: x.m,
            t: [x.t[0], x.t[1], x.px_per_unit, x.y_sign],
            target: [target.w as f32, target.h as f32, min_stroke, opacity],
            origin: [origin[0], origin[1], AA_PX, 0.0],
        }
    }

    fn buffer(&self, label: &str, data: &[u8], usage: wgpu::BufferUsages) -> wgpu::Buffer {
        use wgpu::util::DeviceExt;
        let mut padded;
        let contents = if data.len() % wgpu::COPY_BUFFER_ALIGNMENT as usize == 0 && !data.is_empty() {
            data
        } else {
            padded = data.to_vec();
            padded.resize(align_up(data.len().max(4) as u64, wgpu::COPY_BUFFER_ALIGNMENT) as usize, 0);
            &padded[..]
        };
        self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some(label), contents, usage })
    }

    /// The table index of a glyph, adding its outline (from the canvas fonts) on first use.
    fn glyph_slot(&mut self, canvas: &Canvas, key: GlyphKey) -> u32 {
        if let Some(i) = self.glyph_index.get(&key) {
            return *i;
        }
        let g = canvas.fonts().glyph(key).unwrap_or_default();
        let i = self.table.push(&g);
        self.glyph_index.insert(key, i);
        i
    }

    /// Creates the buffers of a chunk drawn for the first time; returns the bytes uploaded.
    fn upload_chunk(&mut self, canvas: &Canvas, d: &ChunkDraw) -> u64 {
        let c = &d.chunk;
        if let Some(g) = self.chunks.get_mut(&c.version) {
            g.last_used = self.frame;
            return 0;
        }
        let mut bytes = 0;
        let vertex = wgpu::BufferUsages::VERTEX;
        let shapes = (!c.shapes.is_empty()).then(|| {
            let data: &[u8] = bytemuck::cast_slice(&c.shapes);
            bytes += data.len() as u64;
            self.buffer("skelter-canvas shapes", data, vertex)
        });
        let greek = (!c.greek.is_empty()).then(|| {
            let data: &[u8] = bytemuck::cast_slice(&c.greek);
            bytes += data.len() as u64;
            self.buffer("skelter-canvas greek", data, vertex)
        });
        let polys = (!c.poly_indices.is_empty()).then(|| {
            let v: &[u8] = bytemuck::cast_slice(&c.poly_vertices);
            let i: &[u8] = bytemuck::cast_slice(&c.poly_indices);
            bytes += (v.len() + i.len()) as u64;
            (self.buffer("skelter-canvas poly vertices", v, vertex), self.buffer("skelter-canvas poly indices", i, wgpu::BufferUsages::INDEX))
        });
        let glyphs = if c.glyphs.is_empty() {
            None
        } else {
            let verts: Vec<GlyphVertex> = c
                .glyphs
                .iter()
                .map(|g: &GlyphPlacement| GlyphVertex {
                    anchor: g.anchor,
                    pen: g.pen,
                    rot: g.rot,
                    size: g.size,
                    glyph: self.glyph_slot(canvas, g.glyph),
                    color: g.color,
                })
                .collect();
            let data: &[u8] = bytemuck::cast_slice(&verts);
            bytes += data.len() as u64;
            Some(self.buffer("skelter-canvas glyphs", data, vertex))
        };
        self.chunks.insert(c.version, GpuChunk { shapes, greek, polys, glyphs, last_used: self.frame });
        bytes
    }

    /// Mirrors the glyph table on the GPU (recreated when it outgrows its buffers).
    fn sync_glyphs(&mut self, queue: &wgpu::Queue) -> u64 {
        if self.table.records.is_empty() {
            return 0;
        }
        let records: &[u8] = bytemuck::cast_slice(&self.table.records);
        let curves: &[u8] = bytemuck::cast_slice(&self.table.curves);
        let bands: &[u8] = bytemuck::cast_slice(&self.table.bands);
        let sizes = [records.len() as u64, curves.len().max(16) as u64, bands.len().max(4) as u64];
        if let Some((r, c, b, _, uploaded)) = &mut self.glyph_gpu {
            if *uploaded == sizes {
                return 0;
            }
            if r.size() >= sizes[0] && c.size() >= sizes[1] && b.size() >= sizes[2] {
                queue.write_buffer(r, 0, records);
                if !curves.is_empty() {
                    queue.write_buffer(c, 0, curves);
                }
                if !bands.is_empty() {
                    queue.write_buffer(b, 0, bands);
                }
                *uploaded = sizes;
                return sizes.iter().sum();
            }
        }
        let make = |label, data: &[u8], len: u64| {
            let size = align_up((len * 2).next_power_of_two().max(256), wgpu::COPY_BUFFER_ALIGNMENT);
            let buf = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            if !data.is_empty() {
                queue.write_buffer(&buf, 0, data);
            }
            buf
        };
        let r = make("skelter-canvas glyph records", records, sizes[0]);
        let c = make("skelter-canvas glyph curves", curves, sizes[1]);
        let b = make("skelter-canvas glyph bands", bands, sizes[2]);
        let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("skelter-canvas glyphs"),
            layout: &self.glyph_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: r.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: c.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: b.as_entire_binding() },
            ],
        });
        self.glyph_gpu = Some((r, c, b, bg, sizes));
        sizes.iter().sum()
    }

    fn cache_for(&mut self, id: CanvasId, size: DeviceSize) -> &mut StaticCache {
        let fits = self.caches.get(&id).is_some_and(|c| c.size == size);
        if !fits {
            let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("skelter-canvas static cache"),
                size: wgpu::Extent3d { width: size.w, height: size.h, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let view = texture.create_view(&Default::default());
            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("skelter-canvas static cache"),
                layout: &self.texture_layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) },
                ],
            });
            let changed_last_frame = self.caches.get(&id).is_some_and(|c| c.changed_last_frame);
            self.caches.insert(id, StaticCache { key: None, valid: false, texture, bind_group, size, changed_last_frame, last_used: self.frame });
        }
        let c = self.caches.get_mut(&id).expect("inserted");
        c.last_used = self.frame;
        c
    }

    /// Plans, uploads and renders static caches for this frame's placements (`target` = the size of
    /// the texture the host draws into). Must precede [`CanvasRenderer::draw`] and the host's pass.
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: DeviceSize,
        placements: &[Placement],
    ) -> CanvasStats {
        let _ = device;
        let mut stats = CanvasStats { placements: placements.len() as u32, ..Default::default() };
        self.staged.clear();
        self.prepared.clear();
        self.placements.clear();
        let full = Rect::new(0.0, 0.0, target.w as f32, target.h as f32);
        // Cache passes to encode once the uniforms are written: (canvas, draws range).
        let mut cache_passes: Vec<(CanvasId, std::ops::Range<usize>)> = Vec::new();
        for p in placements {
            let snap: Arc<Snapshot> = p.canvas.snapshot();
            let to_device = p.to_device.then(&p.view.view);
            let size = (p.size.0 as f64, p.size.1 as f64);
            let proj = Projection::from_affine(p.view.camera, size, &to_device);
            let clip = p.clip.intersect(&full);
            let lod = p.view.lod;
            let id = p.canvas.id();
            let mut out = PreparedPlacement { canvas: id, statics: StaticMode::None, direct: 0..0, overlay: 0..0 };

            // The static plane: into the cache when its key is stable, else directly.
            let mut draws = Vec::new();
            let s = plan(snap.plane(Plane::Static), &proj, &clip, &lod, &mut draws);
            stats.chunks_visible += s.visible;
            stats.shapes += s.shapes;
            stats.glyphs += s.glyphs;
            let rect = [clip.min.x.floor() as i32, clip.min.y.floor() as i32, clip.max.x.ceil() as i32, clip.max.y.ceil() as i32];
            let cache_size = DeviceSize { w: (rect[2] - rect[0]).max(0) as u32, h: (rect[3] - rect[1]).max(0) as u32 };
            if !draws.is_empty() && cache_size.w > 0 && cache_size.h > 0 {
                let key = CacheKey { revision: snap.statics.revision, projection: proj, lod, rect };
                let (render, moving) = {
                    let cache = self.cache_for(id, cache_size);
                    let changed = cache.key != Some(key);
                    let moving = changed && cache.changed_last_frame;
                    cache.changed_last_frame = changed;
                    cache.key = Some(key);
                    let render = !moving && !cache.valid || !moving && changed;
                    cache.valid = !moving;
                    (render, moving)
                };
                if moving {
                    stats.direct += 1;
                    let first = self.prepared.len();
                    for d in draws {
                        let u = self.chunk_uniform(&d, target, [0.0, 0.0], lod.min_stroke_px, p.opacity);
                        let uniform = self.push_uniform(u);
                        stats.upload_bytes += self.upload_chunk(&p.canvas, &d);
                        self.prepared.push(Prepared { uniform, version: d.chunk.version, draw: d });
                    }
                    out.statics = StaticMode::Direct;
                    out.direct = first..self.prepared.len();
                } else {
                    if render {
                        stats.cache_renders += 1;
                        let first = self.prepared.len();
                        for d in draws {
                            let origin = [rect[0] as f32, rect[1] as f32];
                            let u = self.chunk_uniform(&d, cache_size, origin, lod.min_stroke_px, 1.0);
                            let uniform = self.push_uniform(u);
                            stats.upload_bytes += self.upload_chunk(&p.canvas, &d);
                            self.prepared.push(Prepared { uniform, version: d.chunk.version, draw: d });
                        }
                        cache_passes.push((id, first..self.prepared.len()));
                    } else {
                        stats.cache_hits += 1;
                        // Keep the chunks' buffers alive while the cache shows them.
                        for (_, c) in &snap.statics.chunks {
                            if let Some(g) = self.chunks.get_mut(&c.version) {
                                g.last_used = self.frame;
                            }
                        }
                    }
                    let blit = DrawUniform {
                        m: [cache_size.w as f32, 0.0, 0.0, cache_size.h as f32],
                        t: [rect[0] as f32, rect[1] as f32, 1.0, 1.0],
                        target: [target.w as f32, target.h as f32, 0.0, p.opacity],
                        origin: [0.0, 0.0, AA_PX, 0.0],
                    };
                    out.statics = StaticMode::Cached { uniform: self.push_uniform(blit) };
                }
            }

            // The overlay plane: always directly.
            let mut draws = Vec::new();
            let s = plan(snap.plane(Plane::Overlay), &proj, &clip, &lod, &mut draws);
            stats.chunks_visible += s.visible;
            stats.shapes += s.shapes;
            stats.glyphs += s.glyphs;
            let first = self.prepared.len();
            for d in draws {
                let u = self.chunk_uniform(&d, target, [0.0, 0.0], lod.min_stroke_px, p.opacity);
                let uniform = self.push_uniform(u);
                stats.upload_bytes += self.upload_chunk(&p.canvas, &d);
                self.prepared.push(Prepared { uniform, version: d.chunk.version, draw: d });
            }
            out.overlay = first..self.prepared.len();
            self.placements.push(out);
        }
        stats.chunk_draws = self.prepared.len() as u32;
        stats.upload_bytes += self.sync_glyphs(queue);

        if self.staged.is_empty() {
            return stats;
        }
        let needed = align_up(self.staged.len() as u64, wgpu::COPY_BUFFER_ALIGNMENT);
        if needed > self.uniform_capacity {
            let cap = needed.next_power_of_two();
            let (buf, bg) = Self::uniform_buffer(&self.device, &self.uniform_layout, cap);
            self.uniforms = buf;
            self.uniform_bg = bg;
            self.uniform_capacity = cap;
        }
        self.staged.resize(needed as usize, 0);
        queue.write_buffer(&self.uniforms, 0, &self.staged);
        stats.upload_bytes += needed;

        for (id, range) in cache_passes {
            let Some(cache) = self.caches.get(&id) else { continue };
            let view = cache.texture.create_view(&Default::default());
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("skelter-canvas static cache"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            for i in range {
                self.draw_chunk(&mut pass, i);
            }
        }
        stats
    }

    fn draw_chunk(&self, pass: &mut wgpu::RenderPass<'_>, i: usize) {
        let p = &self.prepared[i];
        let Some(g) = self.chunks.get(&p.version) else { return };
        let d = &p.draw;
        pass.set_bind_group(0, &self.uniform_bg, &[p.uniform]);
        if let (Some((v, ix)), n) = (&g.polys, d.lod.poly_indices) {
            if n > 0 {
                pass.set_pipeline(&self.poly);
                pass.set_vertex_buffer(0, v.slice(..));
                pass.set_index_buffer(ix.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..n, 0, 0..1);
            }
        }
        if let (Some(b), n) = (&g.shapes, d.lod.shapes) {
            if n > 0 {
                pass.set_pipeline(&self.shape);
                pass.set_vertex_buffer(0, b.slice(..));
                pass.draw(0..4, 0..n);
            }
        }
        if let Some(b) = &g.greek {
            let l = d.labels;
            if l.greek_end > l.greek_start {
                pass.set_pipeline(&self.shape);
                pass.set_vertex_buffer(0, b.slice(..));
                pass.draw(0..4, l.greek_start..l.greek_end);
            }
        }
        if let (Some(b), Some((_, _, _, bg, _))) = (&g.glyphs, &self.glyph_gpu) {
            if d.labels.glyphs > 0 {
                pass.set_pipeline(&self.glyph);
                pass.set_bind_group(1, bg, &[]);
                pass.set_vertex_buffer(0, b.slice(..));
                pass.draw(0..4, 0..d.labels.glyphs);
            }
        }
    }

    /// Draws prepared placement `i` into the host's pass (under the scissor the host set). It changes
    /// the pass's pipeline and bind groups 0 and 1: the host sets its own again afterwards.
    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>, i: usize) {
        let Some(p) = self.placements.get(i) else { return };
        match p.statics {
            StaticMode::None => {}
            StaticMode::Cached { uniform } => {
                if let Some(cache) = self.caches.get(&p.canvas) {
                    pass.set_pipeline(&self.blit);
                    pass.set_bind_group(0, &self.uniform_bg, &[uniform]);
                    pass.set_bind_group(1, &cache.bind_group, &[]);
                    pass.draw(0..4, 0..1);
                }
            }
            StaticMode::Direct => {
                for j in p.direct.clone() {
                    self.draw_chunk(pass, j);
                }
            }
        }
        for j in p.overlay.clone() {
            self.draw_chunk(pass, j);
        }
    }

    /// Once per submitted frame: drops the buffers of chunks and the caches of canvases not drawn for a
    /// while.
    pub fn end_frame(&mut self) {
        self.frame += 1;
        let frame = self.frame;
        self.chunks.retain(|_, c| frame - c.last_used <= KEEP_FRAMES);
        self.caches.retain(|_, c| frame - c.last_used <= KEEP_FRAMES);
    }

    /// GPU chunk buffers alive (for tests and the latency overlay).
    pub fn resident_chunks(&self) -> usize {
        self.chunks.len()
    }

    /// Glyphs in the GPU table.
    pub fn glyph_count(&self) -> usize {
        self.table.records.len()
    }

    /// The record of glyph `i` in the table (tests).
    pub fn glyph_record(&self, i: usize) -> Option<GlyphRecord> {
        self.table.records.get(i).copied()
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn shaders_parse_and_validate() {
        let src = include_str!("shaders.wgsl");
        let module = naga::front::wgsl::parse_str(src).unwrap_or_else(|e| panic!("{}", e.emit_to_string(src)));
        naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::empty())
            .validate(&module)
            .unwrap_or_else(|e| panic!("{}", e.emit_to_string(src)));
    }

    #[test]
    fn record_layouts_match_the_vertex_attributes() {
        assert_eq!(size_of::<crate::chunk::ShapeInstance>(), 40);
        assert_eq!(size_of::<crate::chunk::PolyVertex>(), 12);
        assert_eq!(size_of::<super::GlyphVertex>(), 36);
        assert_eq!(size_of::<super::DrawUniform>(), 64);
    }
}
