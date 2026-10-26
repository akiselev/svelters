//! The wgpu side of the compositor (architecture §7.1, §7.6, §8.5; `round2/wgpu-integration.md` §1, §3,
//! §5, §8): adapter selection, the one `Device`/`Queue`, the surfaces the UI thread created and moved
//! here, a persistent render target per surface, late acquire, GPU-completion timing and rebuilding
//! after device loss.
//!
//! Every surface draws its damage into a persistent target, then the whole target is copied into the
//! acquired swapchain image and presented. The copy is one blit; what the damage saves is rendering.
//! It does not depend on buffer age: a WSI may cycle through more images than the latency asks for
//! (Mesa's X11 and Wayland WSIs do) or hand them out in any order, and an image that missed frames
//! would show stale or empty pixels outside the damage. A surface whose images cannot be copied into
//! (no `COPY_DST` usage) is redrawn whole into the acquired image instead. wgpu 30 has no
//! present-with-damage (patch P1 of the notes), so presents are full presents.

use crate::core::{Backend, DrawOutcome, Msg};
use crate::seams::{RendererFactory, SceneRenderer};
use crate::tree::SurfaceKey;
use skelter_base::geom::DeviceSize;
use skelter_base::ids::{CanvasId, FrameId, ImageId, PathId, TextureId, WindowId};
use skelter_base::scene::FrameScene;
use skelter_base::text::GlyphRasterizer;
use skelter_base::time::Clock;
use std::collections::HashMap;
use std::future::Future;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, RwLock};
use std::task::{Context, Poll, Waker};
use std::time::Duration;

/// The device and queue as other threads may use them (egui islands, offscreen viewports create
/// their textures with it); replaced after device loss.
#[derive(Clone, Debug)]
pub struct GpuContext {
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    /// The format of every surface and persistent target.
    pub format: wgpu::TextureFormat,
}

/// Filters and ranks the enumerated adapters (all backends the compositor uses, able to present to
/// the first surface when there is one) and picks one.
pub type AdapterSelect = Arc<dyn Fn(&[wgpu::Adapter]) -> Result<wgpu::Adapter, String> + Send + Sync>;
/// The `required_limits` of the device for an adapter (default: `adapter.limits()`).
pub type DeviceLimits = Arc<dyn Fn(&wgpu::Adapter) -> wgpu::Limits + Send + Sync>;

/// Where the published [`GpuContext`] lives.
pub type SharedGpu = Arc<RwLock<Option<GpuContext>>>;

/// The backends tried in order: DX12 only on Windows, Metal on Apple, Vulkan then GL elsewhere
/// (`windows.md` §3.2, architecture §7.1).
pub fn backend_order() -> &'static [wgpu::Backends] {
    if cfg!(windows) {
        &[wgpu::Backends::DX12]
    } else if cfg!(any(target_os = "macos", target_os = "ios")) {
        &[wgpu::Backends::METAL]
    } else {
        &[wgpu::Backends::VULKAN, wgpu::Backends::GL]
    }
}

/// The instance the compositor and the main thread share (the main thread creates surfaces with it).
pub fn create_instance(display: Option<Box<dyn wgpu::wgt::WgpuHasDisplayHandle>>) -> wgpu::Instance {
    let mut desc = match display {
        Some(d) => wgpu::InstanceDescriptor::new_with_display_handle(d),
        None => wgpu::InstanceDescriptor::new_without_display_handle(),
    };
    desc.backends = backend_order().iter().fold(wgpu::Backends::empty(), |a, b| a | *b);
    desc.flags = wgpu::InstanceFlags::from_build_config();
    wgpu::Instance::new(desc)
}

/// Polls a future to completion on this thread. wgpu's native adapter and device futures are ready
/// on first poll; the loop only guards against a backend that is not.
pub fn block_on<F: Future>(f: F) -> F::Output {
    let mut f = std::pin::pin!(f);
    let mut cx = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(v) = f.as_mut().poll(&mut cx) {
            return v;
        }
        std::thread::yield_now();
    }
}

/// Picks an adapter: the first backend in [`backend_order`] that has one able to present to
/// `surface` (any adapter when headless), preferring by `power` the discrete or the integrated GPU,
/// then any other hardware adapter, and software (lavapipe, WARP, llvmpipe) last.
pub fn select_adapter(
    instance: &wgpu::Instance,
    power: wgpu::PowerPreference,
    surface: Option<&wgpu::Surface<'_>>,
) -> Option<wgpu::Adapter> {
    for backends in backend_order() {
        let mut adapters: Vec<wgpu::Adapter> = block_on(instance.enumerate_adapters(*backends))
            .into_iter()
            .filter(|a| surface.is_none_or(|s| a.is_surface_supported(s)))
            .collect();
        adapters.sort_by_key(|a| rank(a.get_info().device_type, power));
        if let Some(a) = adapters.into_iter().next() {
            return Some(a);
        }
    }
    None
}

fn rank(t: wgpu::DeviceType, power: wgpu::PowerPreference) -> u8 {
    use wgpu::DeviceType::*;
    match (t, power) {
        (DiscreteGpu, wgpu::PowerPreference::HighPerformance) | (IntegratedGpu, wgpu::PowerPreference::LowPower) => 0,
        (DiscreteGpu, _) | (IntegratedGpu, _) => 1,
        (VirtualGpu, _) | (Other, _) => 2,
        (Cpu, _) => 3,
    }
}

/// A non-sRGB format (skelter blends sRGB-encoded values, architecture §8.3), BGRA or RGBA 8-bit
/// preferred.
pub fn pick_format(formats: &[wgpu::TextureFormat]) -> Option<wgpu::TextureFormat> {
    use wgpu::TextureFormat::*;
    [Bgra8Unorm, Rgba8Unorm]
        .into_iter()
        .find(|f| formats.contains(f))
        .or_else(|| formats.iter().copied().find(|f| !f.is_srgb()))
}

struct Gpu {
    ctx: GpuContext,
    renderer: Box<dyn SceneRenderer>,
    /// Sends submissions to the thread that waits for them (GPU completion times).
    waiter: Sender<(wgpu::Device, wgpu::SubmissionIndex)>,
}

struct Surf {
    /// `None`: an offscreen target (headless tests, screenshots) that is rendered and never presented.
    surface: Option<wgpu::Surface<'static>>,
    size: DeviceSize,
    configured: bool,
    /// Swapchain images accept copies; otherwise the frame renders straight into the image.
    copy: bool,
    target: Option<(wgpu::Texture, wgpu::TextureView, DeviceSize)>,
    /// Offscreen only: images standing in for a swapchain, presented in turn (tests of the copy path).
    chain: Option<Chain>,
}

struct Chain {
    images: Vec<wgpu::Texture>,
    next: usize,
    last: Option<usize>,
}

pub struct GpuBackend {
    instance: wgpu::Instance,
    power: wgpu::PowerPreference,
    gpu: Option<Gpu>,
    shared: SharedGpu,
    surfaces: HashMap<(WindowId, SurfaceKey), Surf>,
    textures: HashMap<TextureId, wgpu::Texture>,
    committers: HashMap<WindowId, skelter_platform::OsLayerCommitter>,
    /// Canvases are CPU data: re-registered with the renderer rebuilt after device loss.
    canvases: HashMap<CanvasId, Arc<skelter_canvas::Canvas>>,
    new_renderer: RendererFactory,
    glyphs: Arc<dyn GlyphRasterizer>,
    notify: Sender<Msg>,
    clock: Arc<dyn Clock>,
    select: Option<AdapterSelect>,
    limits: Option<DeviceLimits>,
    /// Set when no adapter could be created, so every draw does not retry the enumeration.
    unavailable: bool,
}

impl GpuBackend {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        instance: wgpu::Instance,
        power: wgpu::PowerPreference,
        shared: SharedGpu,
        new_renderer: RendererFactory,
        glyphs: Arc<dyn GlyphRasterizer>,
        notify: Sender<Msg>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        GpuBackend {
            instance,
            power,
            gpu: None,
            shared,
            surfaces: HashMap::new(),
            textures: HashMap::new(),
            committers: HashMap::new(),
            canvases: HashMap::new(),
            new_renderer,
            glyphs,
            notify,
            clock,
            select: None,
            limits: None,
            unavailable: false,
        }
    }

    /// Replaces the default adapter choice and device limits (used again after device loss).
    pub fn set_selection(&mut self, select: Option<AdapterSelect>, limits: Option<DeviceLimits>) {
        self.select = select;
        self.limits = limits;
    }

    fn pick_adapter(&self, surface: Option<&wgpu::Surface<'_>>) -> Option<wgpu::Adapter> {
        let Some(select) = &self.select else { return select_adapter(&self.instance, self.power, surface) };
        let adapters: Vec<wgpu::Adapter> = backend_order()
            .iter()
            .flat_map(|b| block_on(self.instance.enumerate_adapters(*b)))
            .filter(|a| surface.is_none_or(|s| a.is_surface_supported(s)))
            .collect();
        match select(&adapters) {
            Ok(a) => Some(a),
            Err(e) => {
                log::error!("skelter-compositor: adapter selection failed: {e}");
                None
            }
        }
    }

    /// Creates the device on first need, compatible with `surface` when one is given.
    fn ensure_gpu(&mut self, surface: Option<&wgpu::Surface<'_>>) -> bool {
        if self.gpu.is_some() {
            return true;
        }
        if self.unavailable && surface.is_none() {
            return false;
        }
        let Some(adapter) = self.pick_adapter(surface) else {
            log::error!("skelter-compositor: no GPU adapter for backends {:?}", backend_order());
            self.unavailable = true;
            return false;
        };
        let desc = wgpu::DeviceDescriptor {
            label: Some("skelter-compositor"),
            required_limits: self.limits.as_ref().map_or_else(|| adapter.limits(), |f| f(&adapter)),
            ..Default::default()
        };
        let (device, queue) = match block_on(adapter.request_device(&desc)) {
            Ok(dq) => dq,
            Err(e) => {
                log::error!("skelter-compositor: request_device failed: {e}");
                self.unavailable = true;
                return false;
            }
        };
        let format = surface
            .and_then(|s| pick_format(&s.get_capabilities(&adapter).formats))
            .unwrap_or(wgpu::TextureFormat::Rgba8Unorm);
        let tx = self.notify.clone();
        device.set_device_lost_callback(move |reason, msg| {
            if reason != wgpu::DeviceLostReason::Destroyed {
                log::error!("skelter-compositor: device lost: {msg}");
                let _ = tx.send(Msg::DeviceLost);
            }
        });
        device.on_uncaptured_error(Arc::new(|e| log::error!("skelter-compositor: wgpu error: {e}")));
        let mut renderer = (self.new_renderer)(&device, &queue, format, self.glyphs.clone());
        for c in self.canvases.values() {
            renderer.register_canvas(c.clone());
        }
        let info = adapter.get_info();
        log::info!("skelter-compositor: {} on {:?} ({:?})", info.name, info.backend, info.device_type);
        let ctx = GpuContext { adapter, device, queue, format };
        *self.shared.write().unwrap_or_else(|e| e.into_inner()) = Some(ctx.clone());
        self.gpu = Some(Gpu { ctx, renderer, waiter: spawn_waiter() });
        self.unavailable = false;
        true
    }

    fn configure(gpu: &Gpu, s: &mut Surf) {
        s.configured = false;
        s.target = None;
        if s.size.w == 0 || s.size.h == 0 {
            return;
        }
        let Some(surface) = &s.surface else {
            s.copy = true;
            s.configured = true;
            return;
        };
        let caps = surface.get_capabilities(&gpu.ctx.adapter);
        if !caps.formats.contains(&gpu.ctx.format) {
            log::error!("skelter-compositor: surface does not support {:?} ({:?})", gpu.ctx.format, caps.formats);
            return;
        }
        let Some(mut config) = surface.get_default_config(&gpu.ctx.adapter, s.size.w, s.size.h) else {
            log::error!("skelter-compositor: surface is not supported by the adapter");
            return;
        };
        s.copy = caps.usages.contains(wgpu::TextureUsages::COPY_DST);
        config.format = gpu.ctx.format;
        config.usage = wgpu::TextureUsages::RENDER_ATTACHMENT
            | if s.copy { wgpu::TextureUsages::COPY_DST } else { wgpu::TextureUsages::empty() };
        config.present_mode = wgpu::PresentMode::Fifo;
        config.desired_maximum_frame_latency = 1;
        if let Some(a) = caps.alpha_modes.first() {
            config.alpha_mode = *a;
        }
        config.view_formats = Vec::new();
        surface.configure(&gpu.ctx.device, &config);
        s.configured = true;
    }

    /// DirectComposition shows a configured swap chain only after a `Commit` (wgpu does not call it).
    fn commit_os(&self, window: WindowId) {
        if let Some(c) = self.committers.get(&window) {
            c.commit();
        }
    }

    /// Posts `GpuDone` for `frame` when the GPU finishes submission `index`.
    fn report_done(
        notify: &Sender<Msg>,
        clock: &Arc<dyn Clock>,
        gpu: &Gpu,
        window: WindowId,
        frame: FrameId,
        index: wgpu::SubmissionIndex,
    ) {
        let tx = notify.clone();
        let clock = clock.clone();
        gpu.ctx.queue.on_submitted_work_done(move || {
            let _ = tx.send(Msg::GpuDone { window, frame, at: clock.now() });
        });
        let _ = gpu.waiter.send((gpu.ctx.device.clone(), index));
    }

    /// Adds an offscreen target for `key` of `window`: frames render into it and are never presented
    /// (headless runs, screenshots). Creates the device if needed; `false` without a GPU.
    pub fn attach_offscreen(&mut self, window: WindowId, key: SurfaceKey, size: DeviceSize) -> bool {
        if !self.ensure_gpu(None) {
            return false;
        }
        let mut s = Surf { surface: None, size, configured: false, copy: true, target: None, chain: None };
        if let Some(gpu) = &self.gpu {
            Self::configure(gpu, &mut s);
        }
        self.surfaces.insert((window, key), s);
        true
    }

    /// Like [`Self::attach_offscreen`], with `images` textures standing in for a swapchain: each frame
    /// goes through the same copy into the next image as a real surface, and [`Self::read_pixels`]
    /// reads the last one presented.
    pub fn attach_offscreen_chain(&mut self, window: WindowId, key: SurfaceKey, size: DeviceSize, images: usize) -> bool {
        if !self.attach_offscreen(window, key, size) {
            return false;
        }
        let Some(gpu) = &self.gpu else { return false };
        let images = (0..images.max(1))
            .map(|_| {
                gpu.ctx.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("skelter-compositor emulated swapchain image"),
                    size: wgpu::Extent3d { width: size.w, height: size.h, depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: gpu.ctx.format,
                    usage: wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::COPY_SRC,
                    view_formats: &[],
                })
            })
            .collect();
        if let Some(s) = self.surfaces.get_mut(&(window, key)) {
            s.chain = Some(Chain { images, next: 0, last: None });
        }
        true
    }

    /// Reads back a surface's persistent target as tightly packed rows of its format's texels (BGRA or
    /// RGBA 8-bit). Blocks until the GPU is done.
    pub fn read_pixels(&self, window: WindowId, key: SurfaceKey) -> Option<(DeviceSize, wgpu::TextureFormat, Vec<u8>)> {
        let gpu = self.gpu.as_ref()?;
        let s = self.surfaces.get(&(window, key))?;
        let (target, _, size) = s.target.as_ref()?;
        let tex = match &s.chain {
            Some(c) => &c.images[c.last?],
            None => target,
        };
        let out = Self::read_texture(gpu, tex, *size)?;
        Some((*size, gpu.ctx.format, out))
    }

    /// Copies `tex` (`size`, the device's format) into a buffer and reads it back as tightly packed
    /// rows. Blocks until the GPU is done.
    fn read_texture(gpu: &Gpu, tex: &wgpu::Texture, size: DeviceSize) -> Option<Vec<u8>> {
        let bpp = gpu.ctx.format.block_copy_size(None)?;
        let row = size.w * bpp;
        let padded = row.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let buf = gpu.ctx.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("skelter-compositor readback"),
            size: padded as u64 * size.h as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = gpu.ctx.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        enc.copy_texture_to_buffer(
            tex.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buf,
                layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(padded), rows_per_image: Some(size.h) },
            },
            wgpu::Extent3d { width: size.w, height: size.h, depth_or_array_layers: 1 },
        );
        let index = gpu.ctx.queue.submit([enc.finish()]);
        let slice = buf.slice(..);
        let (tx, rx) = mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        let _ = gpu.ctx.device.poll(wgpu::PollType::Wait { submission_index: Some(index), timeout: Some(Duration::from_secs(10)) });
        rx.recv().ok()?.ok()?;
        let data = slice.get_mapped_range().ok()?;
        let mut out = Vec::with_capacity((row * size.h) as usize);
        for y in 0..size.h as usize {
            let start = y * padded as usize;
            out.extend_from_slice(&data[start..start + row as usize]);
        }
        drop(data);
        buf.unmap();
        Some(out)
    }

    fn persistent_target(gpu: &Gpu, s: &mut Surf) -> bool {
        if matches!(&s.target, Some((_, _, size)) if *size == s.size) {
            return false;
        }
        let tex = gpu.ctx.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("skelter-compositor surface target"),
            size: wgpu::Extent3d { width: s.size.w, height: s.size.h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: gpu.ctx.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        s.target = Some((tex, view, s.size));
        true
    }
}

/// Copies the whole persistent target into a presentable image.
fn copy_whole(encoder: &mut wgpu::CommandEncoder, target: &wgpu::Texture, image: &wgpu::Texture, size: DeviceSize) {
    let w = size.w.min(image.width()).min(target.width());
    let h = size.h.min(image.height()).min(target.height());
    if w == 0 || h == 0 {
        return;
    }
    encoder.copy_texture_to_texture(
        target.as_image_copy(),
        image.as_image_copy(),
        wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
    );
}

/// A thread that blocks on each submission and so fires its `on_submitted_work_done` callback as soon
/// as the GPU finishes, without the compositor loop polling the device.
fn spawn_waiter() -> Sender<(wgpu::Device, wgpu::SubmissionIndex)> {
    let (tx, rx) = mpsc::channel::<(wgpu::Device, wgpu::SubmissionIndex)>();
    let spawned = std::thread::Builder::new().name("skelter-gpu-wait".into()).spawn(move || {
        for (device, index) in rx {
            // until the submission is done: one timed-out wait would drop its `GpuDone` (a slow
            // software adapter takes longer than one wait), and nothing else polls the device
            let mut waits = 0u32;
            loop {
                let poll = wgpu::PollType::Wait { submission_index: Some(index.clone()), timeout: Some(Duration::from_secs(2)) };
                match device.poll(poll) {
                    Ok(_) => break,
                    Err(wgpu::PollError::Timeout) => {
                        waits += 1;
                        if waits == 5 {
                            log::warn!("skelter-compositor: the GPU has not finished a submission after 10 s; still waiting");
                        }
                    }
                    Err(e) => {
                        log::warn!("skelter-compositor: waiting for the GPU failed: {e}");
                        break;
                    }
                }
            }
        }
    });
    if let Err(e) = spawned {
        log::error!("skelter-compositor: cannot spawn the GPU wait thread: {e}");
    }
    tx
}

impl Backend for GpuBackend {
    fn attach(&mut self, window: WindowId, key: SurfaceKey, surface: wgpu::Surface<'static>, size: DeviceSize) {
        self.ensure_gpu(Some(&surface));
        let mut s = Surf {
            surface: Some(surface),
            size,
            configured: false,
            copy: false,
            target: None,
            chain: None,
        };
        if let Some(gpu) = &self.gpu {
            Self::configure(gpu, &mut s);
        }
        self.surfaces.insert((window, key), s);
        self.commit_os(window);
    }

    fn detach(&mut self, window: WindowId, key: SurfaceKey) {
        self.surfaces.remove(&(window, key));
    }

    fn resize(&mut self, window: WindowId, key: SurfaceKey, size: DeviceSize) {
        if let Some(s) = self.surfaces.get_mut(&(window, key)) {
            s.size = size;
            if let Some(gpu) = &self.gpu {
                Self::configure(gpu, s);
            }
            self.commit_os(window);
        }
    }

    fn set_committer(&mut self, window: WindowId, committer: skelter_platform::OsLayerCommitter) {
        committer.commit();
        self.committers.insert(window, committer);
    }

    fn register_texture(&mut self, id: TextureId, texture: wgpu::Texture) {
        if let Some(gpu) = &mut self.gpu {
            let size = DeviceSize { w: texture.width(), h: texture.height() };
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            gpu.renderer.register_texture(id, view, size);
        }
        self.textures.insert(id, texture);
    }

    fn unregister_texture(&mut self, id: TextureId) {
        if let Some(gpu) = &mut self.gpu {
            gpu.renderer.unregister_texture(id);
        }
        self.textures.remove(&id);
    }

    fn register_canvas(&mut self, canvas: Arc<skelter_canvas::Canvas>) {
        if let Some(gpu) = &mut self.gpu {
            gpu.renderer.register_canvas(canvas.clone());
        }
        self.canvases.insert(canvas.id(), canvas);
    }

    fn unregister_canvas(&mut self, id: CanvasId) {
        if let Some(gpu) = &mut self.gpu {
            gpu.renderer.unregister_canvas(id);
        }
        self.canvases.remove(&id);
    }

    fn set_canvas_view(&mut self, id: CanvasId, view: skelter_canvas::CanvasView) {
        if let Some(gpu) = &mut self.gpu {
            gpu.renderer.set_canvas_view(id, view);
        }
    }

    fn draw(&mut self, window: WindowId, key: SurfaceKey, frame: Option<FrameId>, scene: &FrameScene<'_>) -> DrawOutcome {
        let Some(gpu) = &mut self.gpu else { return DrawOutcome::NoSurface };
        let Some(s) = self.surfaces.get_mut(&(window, key)) else { return DrawOutcome::NoSurface };
        if !s.configured {
            Self::configure(gpu, s);
            if !s.configured {
                return DrawOutcome::NoSurface;
            }
        }
        let device = &gpu.ctx.device;
        let queue = &gpu.ctx.queue;
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("skelter frame") });

        // Encode into the persistent target before acquiring: hold the swapchain image as briefly as
        // possible (acquire late).
        if s.copy {
            let fresh = Self::persistent_target(gpu, s);
            let whole;
            let scene = if fresh && !scene.damage.is_empty() {
                // A new target holds nothing: redraw all of it.
                whole = FrameScene { damage: &[], ..*scene };
                &whole
            } else {
                scene
            };
            let (_, view, _) = s.target.as_ref().expect("persistent target");
            gpu.renderer.render(device, queue, &mut encoder, view, scene);
        }

        let Some(surface) = &s.surface else {
            if let (Some(c), Some((tex, _, _))) = (&mut s.chain, &s.target) {
                copy_whole(&mut encoder, tex, &c.images[c.next], s.size);
                c.last = Some(c.next);
                c.next = (c.next + 1) % c.images.len();
            }
            let index = queue.submit([encoder.finish()]);
            if let Some(frame) = frame {
                Self::report_done(&self.notify, &self.clock, gpu, window, frame, index);
            }
            gpu.renderer.end_frame();
            return DrawOutcome::Presented { submitted: self.clock.now() };
        };
        let acquired = match surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(st) => Ok((st, false)),
            wgpu::CurrentSurfaceTexture::Suboptimal(st) => Ok((st, true)),
            wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded
            | wgpu::CurrentSurfaceTexture::Validation => Err((DrawOutcome::Skipped, false)),
            wgpu::CurrentSurfaceTexture::Outdated => Err((DrawOutcome::Skipped, true)),
            wgpu::CurrentSurfaceTexture::Lost => {
                log::warn!("skelter-compositor: surface of {window:?} {key:?} lost; it must be recreated");
                Err((DrawOutcome::NoSurface, false))
            }
        };
        let (st, suboptimal) = match acquired {
            Ok(a) => a,
            Err((outcome, reconfigure)) => {
                // The renderer's uploads are in this encoder: submit it (the persistent target keeps
                // the drawn damage; the engine redraws it next frame anyway) and close the frame.
                queue.submit([encoder.finish()]);
                gpu.renderer.end_frame();
                if reconfigure {
                    Self::configure(gpu, s);
                    if let Some(c) = self.committers.get(&window) {
                        c.commit();
                    }
                } else if outcome == DrawOutcome::NoSurface {
                    s.configured = false;
                }
                return outcome;
            }
        };

        if s.copy {
            let (tex, _, _) = s.target.as_ref().expect("persistent target");
            copy_whole(&mut encoder, tex, &st.texture, s.size);
        } else {
            let view = st.texture.create_view(&wgpu::TextureViewDescriptor::default());
            let whole = FrameScene { damage: &[], ..*scene };
            gpu.renderer.render(device, queue, &mut encoder, &view, &whole);
        }

        let index = queue.submit([encoder.finish()]);
        if let Some(frame) = frame {
            Self::report_done(&self.notify, &self.clock, gpu, window, frame, index);
        }
        queue.present(st);
        gpu.renderer.end_frame();
        if suboptimal {
            Self::configure(gpu, s);
            if let Some(c) = self.committers.get(&window) {
                c.commit();
            }
        }
        DrawOutcome::Presented { submitted: self.clock.now() }
    }

    fn forget(&mut self, images: &[ImageId], paths: &[PathId]) {
        if let Some(gpu) = &mut self.gpu {
            for id in images {
                gpu.renderer.forget_image(*id);
            }
            for id in paths {
                gpu.renderer.forget_path(*id);
            }
        }
    }

    fn read_back(&mut self, _window: WindowId, scene: &FrameScene<'_>) -> Option<crate::WindowImage> {
        let gpu = self.gpu.as_mut()?;
        let size = scene.target;
        let tex = gpu.ctx.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("skelter-compositor window readback"),
            size: wgpu::Extent3d { width: size.w, height: size.h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: gpu.ctx.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = gpu.ctx.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("skelter readback frame") });
        gpu.renderer.render(&gpu.ctx.device, &gpu.ctx.queue, &mut encoder, &view, &FrameScene { damage: &[], ..*scene });
        gpu.ctx.queue.submit([encoder.finish()]);
        gpu.renderer.end_frame();
        let mut rgba = Self::read_texture(gpu, &tex, size)?;
        match gpu.ctx.format {
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb => {
                for px in rgba.chunks_exact_mut(4) {
                    px.swap(0, 2);
                }
            }
            wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Rgba8UnormSrgb => {}
            f => {
                log::warn!("skelter-compositor: readback of {f:?} is not RGBA8");
                return None;
            }
        }
        Some(crate::WindowImage { size, scale: scene.scale, rgba })
    }

    fn recover(&mut self) -> bool {
        // Textures registered by other users belong to the lost device: they re-register after
        // `ToUi::DeviceReset`.
        self.textures.clear();
        self.gpu = None;
        *self.shared.write().unwrap_or_else(|e| e.into_inner()) = None;
        let first = self.surfaces.keys().next().copied();
        let first = self.surfaces.iter().find(|(_, s)| s.surface.is_some()).map(|(k, _)| *k).or(first);
        let ok = match first {
            Some(k) => {
                let s = self.surfaces.remove(&k).expect("surface");
                let ok = self.ensure_gpu(s.surface.as_ref());
                self.surfaces.insert(k, s);
                ok
            }
            None => self.ensure_gpu(None),
        };
        if let Some(gpu) = &self.gpu {
            for s in self.surfaces.values_mut() {
                Self::configure(gpu, s);
            }
        }
        for c in self.committers.values() {
            c.commit();
        }
        ok
    }
}
