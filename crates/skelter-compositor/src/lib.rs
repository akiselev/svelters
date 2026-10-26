//! skelter's compositor (`plans/research/skelter/architecture.md` §7).
//!
//! One compositor thread per process owns the wgpu `Instance`, `Device` and `Queue`, every surface,
//! each window's committed layer tree, property trees, display lists and resources, the compositor
//! animations and scroll motions, damage and present pacing. The UI thread talks to it through a
//! [`CompositorHandle`] (commits, fast commits, surfaces, the resize handshake) and hears back through
//! [`ToUi`] messages. It never calls winit or the OS window API.
//!
//! | Module | What |
//! |---|---|
//! | [`tree`] | a window's committed state: commits and fast commits applied, flattening per surface |
//! | [`damage`] | damage by diffing flattened frames, device rects |
//! | [`caret`] | the caret blink rule and its per-OS defaults |
//! | [`core`] | the engine: messages, frame clocks, animations, scroll motions, caret, drawing |
//! | [`gpu`] | adapter, device, surfaces, persistent targets, late acquire, device loss |
//! | [`vsync`] | a thread per platform frame source, posting ticks, occlusion and feedback |
//! | [`seams`] | the renderer, frame-clock and scroll-motion interfaces (real crates behind traits) |
//! | [`viewport`] | UI-thread viewports: promotion to an OS layer or the `Prim::Surface` fallback, resize |

pub mod caret;
pub mod core;
pub mod damage;
pub mod gpu;
pub mod seams;
pub mod tree;
pub mod viewport;
pub mod vsync;

pub use crate::caret::CaretBlink;
pub use crate::core::Stats;
pub use crate::gpu::GpuContext;
pub use skelter_base::commit::ToUi;

use crate::core::{Engine, EngineConfig, Msg};
use crate::gpu::{GpuBackend, SharedGpu};
use crate::seams::{PacerFactory, RendererFactory};
use skelter_anim::scroll::DurationBehavior;
use skelter_base::anim::FlingCurve;
use skelter_base::color::Color;
use skelter_base::commit::{Commit, FastCommit};
use skelter_base::geom::{DeviceSize, Scale, Vec2};
use skelter_base::ids::{CanvasId, FrameId, InputSeq, PropNodeId, SurfaceId, TextureId, WindowId};
use skelter_base::text::GlyphRasterizer;
use skelter_base::time::{Clock, Span, Time};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, RwLock};
use std::thread::JoinHandle;
use std::time::Duration;

/// How the compositor is set up.
pub struct CompositorConfig {
    /// The process clock every thread shares (the platform's).
    pub clock: Arc<dyn Clock>,
    /// Builds the renderer on the compositor thread (again after device loss); defaults to
    /// [`skelter_render::Renderer`].
    pub renderer: RendererFactory,
    /// Display pixels per inch, for fling physics (the platform's monitor value).
    pub ppi: f32,
    /// Builds each window's frame clock; defaults to [`skelter_frame::FrameClock`].
    pub pacer: PacerFactory,
    /// The refresh interval of new windows until the platform reports theirs.
    pub refresh: Span,
    /// Passed to the instance for GL/Wayland (winit's `OwnedDisplayHandle`).
    pub display: Option<Box<dyn wgpu::wgt::WgpuHasDisplayHandle>>,
    pub power: wgpu::PowerPreference,
    /// Chooses the adapter from the enumerated ones (cadatomic's `adapter::select`: drop adapters
    /// without what the app needs, rank backends); `None` = by `power`, software last. Also used to
    /// rebuild after device loss.
    pub adapter: Option<gpu::AdapterSelect>,
    /// The device's `required_limits` for the chosen adapter; `None` = `adapter.limits()`.
    pub limits: Option<gpu::DeviceLimits>,
    /// Clear colour of damaged areas; `None` = draw over the previous frame.
    pub clear: Option<Color>,
    /// The caret blink of new windows (the OS setting; [`CaretBlink::platform_default`]); `None` = a
    /// steady caret. Per window: [`CompositorHandle::set_caret_blink`].
    pub caret_blink: Option<CaretBlink>,
}

impl CompositorConfig {
    pub fn new(clock: Arc<dyn Clock>, refresh: Span) -> Self {
        CompositorConfig {
            clock,
            renderer: seams::skelter_renderer(),
            ppi: 96.0,
            pacer: Box::new(seams::frame_clock),
            refresh,
            display: None,
            power: wgpu::PowerPreference::HighPerformance,
            adapter: None,
            limits: None,
            clear: Some(Color::TRANSPARENT),
            caret_blink: Some(CaretBlink::platform_default()),
        }
    }
}

/// The compositor thread's constructor.
pub struct Compositor;

impl Compositor {
    /// Starts the compositor thread. The `Instance` is created here, on the calling thread, so the
    /// main thread can create surfaces with [`CompositorHandle::instance`] right away; the device is
    /// created on the compositor thread with the first surface.
    pub fn spawn(
        cfg: CompositorConfig,
        glyphs: Arc<dyn GlyphRasterizer>,
        to_ui: Box<dyn Fn(ToUi) + Send>,
        wake_ui: Box<dyn Fn() + Send>,
    ) -> CompositorHandle {
        let instance = gpu::create_instance(cfg.display);
        let (tx, rx) = mpsc::channel();
        let shared: SharedGpu = Arc::new(RwLock::new(None));
        let mut backend = GpuBackend::new(
            instance.clone(),
            cfg.power,
            shared.clone(),
            cfg.renderer,
            glyphs,
            tx.clone(),
            cfg.clock.clone(),
        );
        backend.set_selection(cfg.adapter, cfg.limits);
        let engine = Engine::new(
            EngineConfig {
                refresh: cfg.refresh,
                clear: cfg.clear,
                caret_blink: cfg.caret_blink,
                ppi: cfg.ppi,
            },
            cfg.pacer,
            to_ui,
            wake_ui,
        );
        let frame_args: core::SharedFrameArgs = Arc::default();
        let mut engine = engine;
        engine.publish_frame_args(frame_args.clone());
        let clock = cfg.clock;
        let thread = std::thread::Builder::new()
            .name("skelter-compositor".into())
            .spawn(move || run(engine, backend, rx, clock))
            .expect("spawn the compositor thread");
        CompositorHandle {
            tx,
            instance: Arc::new(instance),
            gpu: shared,
            frame_args,
            thread: Arc::new(Mutex::new(Some(thread))),
        }
    }
}

/// The compositor thread: one blocking wait on the message channel with the engine's next deadline
/// as its timeout, then every queued message, then what is due.
fn run(mut engine: Engine, mut backend: GpuBackend, rx: Receiver<Msg>, clock: Arc<dyn Clock>) {
    loop {
        let now = clock.now();
        let first = match engine.next_wakeup() {
            Some(t) if t <= now => rx.try_recv().ok(),
            Some(t) => match rx.recv_timeout(Duration::from_nanos((t - now).0.max(0) as u64)) {
                Ok(m) => Some(m),
                Err(RecvTimeoutError::Timeout) => None,
                Err(RecvTimeoutError::Disconnected) => return,
            },
            None => match rx.recv() {
                Ok(m) => Some(m),
                Err(_) => return,
            },
        };
        let mut next = first;
        while let Some(m) = next {
            if !engine.handle(m, clock.now(), &mut backend) {
                engine.flush();
                return;
            }
            next = rx.try_recv().ok();
        }
        engine.advance(clock.now(), &mut backend);
    }
}

/// The UI thread's side of the compositor. `Send + Clone`; every method posts a message and returns.
#[derive(Clone)]
pub struct CompositorHandle {
    tx: Sender<Msg>,
    instance: Arc<wgpu::Instance>,
    gpu: SharedGpu,
    frame_args: core::SharedFrameArgs,
    thread: Arc<Mutex<Option<JoinHandle<()>>>>,
}

impl CompositorHandle {
    fn post(&self, m: Msg) {
        if self.tx.send(m).is_err() {
            log::warn!("skelter-compositor: the compositor thread has exited");
        }
    }

    /// The instance to create surfaces with, on the main thread.
    pub fn instance(&self) -> &wgpu::Instance {
        &self.instance
    }

    /// The window's frame-clock snapshot (next target, `D_ui`, `est_ui_input`, idle, visible), as of
    /// the clock's last update: what the UI thread's urgent-frame rule reads without a round trip.
    pub fn frame_args(&self, window: WindowId) -> Option<skelter_frame::FrameArgs> {
        self.frame_args.read().unwrap_or_else(|e| e.into_inner()).get(&window).copied()
    }

    /// The device and queue, once the first surface created them (`None` before that and while a lost
    /// device is rebuilt).
    pub fn gpu(&self) -> Option<GpuContext> {
        self.gpu.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn add_window(&self, window: WindowId, surface: wgpu::Surface<'static>, size: DeviceSize, scale: Scale) {
        self.post(Msg::AddWindow { window, surface, size, scale });
    }

    /// An OS compositor layer of `window` (DComp visual, CAMetalLayer, subsurface) for the
    /// `LayerKind::OsLayer(surface_id)` layer's subtree.
    pub fn add_os_layer(&self, window: WindowId, surface_id: SurfaceId, surface: wgpu::Surface<'static>, size: DeviceSize) {
        self.post(Msg::AddOsLayer { window, surface_id, surface, size });
    }

    pub fn resize_os_layer(&self, window: WindowId, surface_id: SurfaceId, size: DeviceSize) {
        self.post(Msg::ResizeOsLayer { window, surface_id, size, done: None });
    }

    /// The OS layer resize handshake (architecture §7.6): the compositor reconfigures the layer's
    /// surface, draws and presents the new size with every commit that has arrived, then answers.
    /// Waits up to `wait` (about one refresh); `false` when it timed out. [`viewport::Viewport`]
    /// orders the platform's layer geometry around it so no frame is shown stretched.
    pub fn resize_os_layer_sync(&self, window: WindowId, surface_id: SurfaceId, size: DeviceSize, wait: Duration) -> bool {
        let (tx, rx) = mpsc::channel();
        self.post(Msg::ResizeOsLayer { window, surface_id, size, done: Some(tx) });
        rx.recv_timeout(wait).is_ok()
    }

    /// The producer of a registered texture (an offscreen viewport) drew a new image into it: what
    /// shows the texture is redrawn on the next frame, with no UI commit. On an OS layer only that
    /// layer's surface redraws.
    pub fn texture_updated(&self, window: WindowId, texture: TextureId) {
        self.post(Msg::TextureUpdated { window, texture });
    }

    /// Makes a canvas drawable by `Prim::Canvas` and `LayerKind::Canvas` (architecture §9.5).
    pub fn register_canvas(&self, canvas: Arc<skelter_canvas::Canvas>) {
        self.post(Msg::RegisterCanvas(canvas));
    }

    pub fn unregister_canvas(&self, id: CanvasId) {
        self.post(Msg::UnregisterCanvas(id));
    }

    /// Pans or zooms a canvas of `window`: a compositor property, so the next frame redraws the canvas
    /// with no UI commit and no re-upload. `lod`, when given, replaces its level-of-detail settings.
    pub fn set_canvas_camera(
        &self,
        window: WindowId,
        canvas: CanvasId,
        camera: skelter_canvas::Camera,
        lod: Option<skelter_canvas::LodParams>,
        input_seq: InputSeq,
    ) {
        self.post(Msg::CanvasCamera { window, canvas, camera, lod, input_seq });
    }

    /// A canvas's content changed (after `Canvas::edit`): its placements in `window` are redrawn.
    pub fn canvas_changed(&self, window: WindowId, canvas: CanvasId) {
        self.post(Msg::CanvasChanged { window, canvas });
    }

    pub fn remove_os_layer(&self, window: WindowId, surface_id: SurfaceId) {
        self.post(Msg::RemoveOsLayer { window, surface_id, done: None });
    }

    /// Drops the OS layer's surface and waits (up to `wait`) until it is gone, so the platform can
    /// destroy the layer under it. `false` when it timed out.
    pub fn remove_os_layer_sync(&self, window: WindowId, surface_id: SurfaceId, wait: Duration) -> bool {
        let (tx, rx) = mpsc::channel();
        self.post(Msg::RemoveOsLayer { window, surface_id, done: Some(tx) });
        rx.recv_timeout(wait).is_ok()
    }

    pub fn remove_window(&self, window: WindowId) {
        self.post(Msg::RemoveWindow(window));
    }

    pub fn commit(&self, c: Commit) {
        self.post(Msg::Commit(Box::new(c)));
    }

    pub fn fast(&self, f: FastCommit) {
        self.post(Msg::Fast(f));
    }

    /// The resize handshake: the compositor reconfigures, renders and presents the new size, then
    /// replies `ToUi::ResizeDone { seq }`.
    pub fn resize(&self, window: WindowId, size: DeviceSize, scale: Scale, seq: u64) {
        self.post(Msg::Resize { window, size, scale, seq });
    }

    pub fn set_visible(&self, window: WindowId, visible: bool) {
        self.post(Msg::SetVisible { window, visible });
    }

    /// The window's caret blink (the user's OS setting); `None` = a steady caret.
    pub fn set_caret_blink(&self, window: WindowId, blink: Option<CaretBlink>) {
        self.post(Msg::SetCaretBlink { window, blink });
    }

    /// The display refresh interval of the window's monitor.
    pub fn set_refresh(&self, window: WindowId, interval: Span) {
        self.post(Msg::SetRefresh { window, interval });
    }

    /// The pixel density of the window's monitor (fling distances), replacing `CompositorConfig::ppi`
    /// for that window.
    pub fn set_ppi(&self, window: WindowId, ppi: f32) {
        self.post(Msg::SetPpi { window, ppi });
    }

    /// A texture produced elsewhere (an egui island, an offscreen viewport), created on [`Self::gpu`]'s
    /// device, drawn by `Prim::Surface` and `LayerKind::Texture`.
    pub fn register_texture(&self, id: TextureId, texture: wgpu::Texture) {
        self.post(Msg::RegisterTexture { id, texture });
    }

    pub fn unregister_texture(&self, id: TextureId) {
        self.post(Msg::UnregisterTexture(id));
    }

    /// The UI thread's scene is dirty and wants a `ToUi::BeginFrame` (`true`), or it has nothing
    /// more to commit (`false`). A level, not a counter.
    pub fn needs_ui_frame(&self, window: WindowId, on: bool) {
        self.post(Msg::NeedsUiFrame { window, on });
    }

    /// The UI answered `BeginFrame { frame }` without a commit (nothing changed).
    pub fn ui_frame_skipped(&self, window: WindowId, frame: FrameId) {
        self.post(Msg::UiFrameSkipped { window, frame });
    }

    /// Continuous input arrived (pointer moves, touchpad scroll): keeps variable-refresh displays up.
    pub fn high_rate_input(&self, window: WindowId) {
        self.post(Msg::HighRateInput(window));
    }

    /// How long the UI phases of `frame` took (the frame clock's estimator).
    pub fn ui_cost(&self, window: WindowId, frame: FrameId, cost: Span) {
        self.post(Msg::UiCost { window, frame, cost });
    }

    /// Like [`Self::ui_cost`], for a UI frame that handled discrete input.
    pub fn ui_input_cost(&self, window: WindowId, frame: FrameId, cost: Span) {
        self.post(Msg::UiInputCost { window, frame, cost });
    }

    /// Starts a compositor-owned fling on a scroll node (skelter-anim's curves; `velocity` in points
    /// per second at release); offsets come back as `ToUi::ScrollMoved`. A scroll-offset fast commit
    /// on the same node stops it.
    pub fn fling(&self, window: WindowId, node: PropNodeId, velocity: Vec2, curve: FlingCurve, at: Time) {
        self.post(Msg::Fling { window, node, velocity, curve, at });
    }

    /// Animates a scroll node to `to` on the compositor (wheel steps, keyboard, `scrollTo` smooth); a
    /// smooth scroll already running heads for the new target keeping its velocity.
    pub fn smooth_scroll(&self, window: WindowId, node: PropNodeId, to: Vec2, behavior: DurationBehavior, at: Time) {
        self.post(Msg::SmoothScroll { window, node, to, behavior, at });
    }

    /// Paces `window` by the platform's frame source (`PlatformCx::frame_source`) instead of a timer
    /// at the refresh interval; its presentation feedback replaces the estimate from GPU completion,
    /// and its occlusion reports become `ToUi::Occluded`.
    pub fn set_frame_source(&self, window: WindowId, source: Arc<dyn skelter_platform::FrameSource>) {
        let link = vsync::spawn(window, source, self.tx.clone());
        self.post(Msg::SetFrameSource { window, link });
    }

    /// The platform's committer for `window`'s OS compositor tree (`PlatformCx::os_layer_committer`):
    /// called after each configure of a DirectComposition surface, which wgpu does not commit. Set it
    /// before `add_window`.
    pub fn set_os_layer_committer(&self, window: WindowId, committer: skelter_platform::OsLayerCommitter) {
        self.post(Msg::SetCommitter { window, committer });
    }

    /// Whether windows should present through the OS compositor tree (`WindowDesc::os_compositing`):
    /// only with wgpu's DX12 backend, which the compositor uses on Windows and nowhere else.
    pub fn os_compositing(&self) -> bool {
        gpu::backend_order() == [wgpu::Backends::DX12]
    }

    /// Stops the thread and waits for it.
    pub fn shutdown(self) {
        self.post(Msg::Shutdown);
        let thread = self.thread.lock().unwrap_or_else(|e| e.into_inner()).take();
        if let Some(t) = thread {
            if t.join().is_err() {
                log::error!("skelter-compositor: the compositor thread panicked");
            }
        }
    }
}
