//! The compositor's engine: every window's committed tree, frame clock, animations, scroll motions,
//! caret and damage, driven by messages and time (architecture §7.2, `round2/scheduler-latency.md`
//! §2.12). It never blocks and never reads a clock: the thread loop (`run` in the crate root) passes `now`
//! and does the one blocking wait, and the GPU sits behind [`Backend`], so tests drive the engine with
//! a virtual clock and a recording backend.

use crate::damage::{self, Pending, Snapshots};
use crate::seams::{Action, FramePacer, PacerFactory, Signal};
use skelter_anim::scroll::{DurationBehavior, Fling, ScrollAnimation, SmoothScroll};
use crate::tree::{self, FlatItem, SurfaceKey, WindowTree};
use crate::caret::CaretBlink;
use crate::vsync::VsyncLink;
use skelter_platform::OsLayerCommitter;
use skelter_base::anim::{
    Placement, AnimProperty, AnimValue, AnimationCmd, AnimationEvent, AnimationSpec, FillMode, FlingCurve, Generator, Keyframe,
    StartTime,
};
use skelter_base::color::{Color, Fill};
use skelter_base::commit::{Commit, FastCommit, ToUi};
use skelter_base::display::{DisplayList, Prim, Quad};
use skelter_base::geom::{Corners, DeviceSize, Insets, Rect, Scale, Vec2};
use skelter_base::ids::{CanvasId, FrameId, ImageId, InputSeq, PathId, LayerId, NodeId, PropNodeId, Revision, SurfaceId, TextureId, WindowId};
use skelter_canvas::{Camera, LodParams};
use skelter_base::scene::{DrawItem, FrameScene};
use skelter_base::time::{Span, Time};
use std::collections::{BTreeMap, HashMap};
use skelter_frame::FrameArgs;
use std::sync::{Arc, RwLock};

/// The GPU side of drawing, implemented by [`crate::gpu::GpuBackend`] (and by recording fakes in tests).
pub trait Backend {
    /// Takes ownership of a surface for `key` of `window` and configures it at `size`.
    fn attach(&mut self, window: WindowId, key: SurfaceKey, surface: wgpu::Surface<'static>, size: DeviceSize);
    fn detach(&mut self, window: WindowId, key: SurfaceKey);
    fn resize(&mut self, window: WindowId, key: SurfaceKey, size: DeviceSize);
    fn register_texture(&mut self, id: TextureId, texture: wgpu::Texture);
    fn unregister_texture(&mut self, id: TextureId);
    /// Renders `scene` into the surface and presents it. `frame` is reported back through
    /// [`Msg::GpuDone`] when the GPU finishes (`None`: a resize frame, not reported).
    fn draw(&mut self, window: WindowId, key: SurfaceKey, frame: Option<FrameId>, scene: &FrameScene<'_>) -> DrawOutcome;
    /// Drops GPU caches of images and paths that commits removed.
    fn forget(&mut self, images: &[ImageId], paths: &[PathId]);
    /// Makes OS-compositor changes visible after configuring a surface of `window` (DComp `Commit`).
    fn set_committer(&mut self, _window: WindowId, _committer: OsLayerCommitter) {}
    /// Makes a canvas drawable (kept across device loss).
    fn register_canvas(&mut self, _canvas: Arc<skelter_canvas::Canvas>) {}
    fn unregister_canvas(&mut self, _id: CanvasId) {}
    /// The view state a canvas is drawn with from now on.
    fn set_canvas_view(&mut self, _id: CanvasId, _view: skelter_canvas::CanvasView) {}
    /// Rebuilds the device, queue, renderer and surface configurations after device loss.
    fn recover(&mut self) -> bool;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DrawOutcome {
    /// Submitted and presented; `submitted` is when the CPU side finished.
    Presented { submitted: Time },
    /// Nothing reached the screen (timeout, occluded, outdated): keep the damage, try next frame.
    Skipped,
    /// No surface (not attached, or no device).
    NoSurface,
}

/// Messages into the compositor thread.
pub enum Msg {
    AddWindow { window: WindowId, surface: wgpu::Surface<'static>, size: DeviceSize, scale: Scale },
    AddOsLayer { window: WindowId, surface_id: SurfaceId, surface: wgpu::Surface<'static>, size: DeviceSize },
    /// `done`: the handshake (`CompositorHandle::resize_os_layer_sync`): draw and present the new
    /// size now, then answer.
    ResizeOsLayer { window: WindowId, surface_id: SurfaceId, size: DeviceSize, done: Option<std::sync::mpsc::Sender<()>> },
    /// `done`: answered once the surface is dropped (the platform may destroy the layer then).
    RemoveOsLayer { window: WindowId, surface_id: SurfaceId, done: Option<std::sync::mpsc::Sender<()>> },
    RemoveWindow(WindowId),
    Commit(Box<Commit>),
    Fast(FastCommit),
    Resize { window: WindowId, size: DeviceSize, scale: Scale, seq: u64 },
    SetVisible { window: WindowId, visible: bool },
    SetRefresh { window: WindowId, interval: Span },
    /// The pixel density of the window's monitor (fling physics); replaces `EngineConfig::ppi` for it.
    SetPpi { window: WindowId, ppi: f32 },
    SetCaretBlink { window: WindowId, blink: Option<CaretBlink> },
    RegisterTexture { id: TextureId, texture: wgpu::Texture },
    UnregisterTexture(TextureId),
    /// The texture's producer drew a new image into it (a viewport frame): damage what shows it.
    TextureUpdated { window: WindowId, texture: TextureId },
    RegisterCanvas(Arc<skelter_canvas::Canvas>),
    UnregisterCanvas(CanvasId),
    /// A canvas's camera (and LOD settings when given), a compositor property (architecture §9.5).
    CanvasCamera { window: WindowId, canvas: CanvasId, camera: Camera, lod: Option<LodParams>, input_seq: InputSeq },
    /// A canvas's content changed (`Canvas::edit`): damage its placements.
    CanvasChanged { window: WindowId, canvas: CanvasId },
    /// The UI thread's scene is dirty (`true`) or it has nothing more to commit (`false`).
    NeedsUiFrame { window: WindowId, on: bool },
    /// The UI answered a `BeginFrame` without a commit.
    UiFrameSkipped { window: WindowId, frame: FrameId },
    UiInputCost { window: WindowId, frame: FrameId, cost: Span },
    HighRateInput(WindowId),
    UiCost { window: WindowId, frame: FrameId, cost: Span },
    Fling { window: WindowId, node: PropNodeId, velocity: Vec2, curve: FlingCurve, at: Time },
    SmoothScroll { window: WindowId, node: PropNodeId, to: Vec2, behavior: DurationBehavior, at: Time },
    /// A display-clock tick (from the window's frame source, [`crate::vsync`]).
    Vsync { window: WindowId, target: Time, interval: Span },
    /// Presentation feedback from the platform.
    Presented { window: WindowId, frame: FrameId, at: Time, estimated: bool },
    Occluded { window: WindowId, occluded: bool },
    SetFrameSource { window: WindowId, link: VsyncLink },
    /// Makes DirectComposition changes visible after a surface of `window` is configured.
    SetCommitter { window: WindowId, committer: OsLayerCommitter },
    GpuDone { window: WindowId, frame: FrameId, at: Time },
    DeviceLost,
    Shutdown,
}

/// Engine settings (the thread-independent part of [`crate::CompositorConfig`]).
pub struct EngineConfig {
    pub refresh: Span,
    pub clear: Option<Color>,
    pub caret_color: Color,
    /// The caret blink of new windows; `None` = a steady caret.
    pub caret_blink: Option<CaretBlink>,
    /// Display pixels per inch, for fling physics.
    pub ppi: f32,
}

struct Anim {
    spec: AnimationSpec,
    start: Option<Time>,
    paused_at: Option<Time>,
    started: bool,
    /// Finished with a forwards fill: holds its last value until cancelled.
    holding: bool,
    last: Option<(AnimValue, f32)>,
}

struct Caret {
    rect: Option<Rect>,
    revision: skelter_base::ids::Revision,
    blink: Option<CaretBlink>,
    /// The last caret change (the blink restarts there).
    since: Time,
    on: bool,
    next_toggle: Option<Time>,
}

struct Win {
    tree: WindowTree,
    pacer: Box<dyn FramePacer>,
    interval: Span,
    /// The window's monitor's pixels per inch, once the UI said (else `EngineConfig::ppi`).
    ppi: Option<f32>,
    size: DeviceSize,
    scale: Scale,
    visible: bool,
    os_layers: HashMap<SurfaceId, DeviceSize>,
    commits: Vec<Commit>,
    /// The frame clock's `ApplyCommit`: pending commits up to this revision go into the next draw.
    apply_upto: Option<Revision>,
    pending: HashMap<SurfaceKey, Pending>,
    snapshots: Snapshots,
    anims: Vec<Anim>,
    motions: HashMap<PropNodeId, Motion>,
    animating: bool,
    caret: Caret,
    vsync_on: bool,
    /// The platform frame source; without one, a timer on the refresh interval.
    source: Option<VsyncLink>,
    /// The source reports real presentation times (otherwise they are estimated from GPU completion).
    feedback: bool,
    occluded: bool,
    next_vsync: Option<Time>,
    timer: Option<Time>,
    /// Inputs applied since the last frame that drew.
    inputs: Vec<InputSeq>,
    in_flight: HashMap<FrameId, InFlight>,
}

/// A presented frame waiting for its presentation time.
struct InFlight {
    inputs: Vec<InputSeq>,
    draw: Time,
    submit: Time,
    gpu_done: Option<Time>,
}

/// The frame clocks' published [`FrameArgs`] per window, for the UI thread's urgent-frame decision
/// (`round2/scheduler-latency.md` §2.4) without a round trip.
pub type SharedFrameArgs = Arc<RwLock<HashMap<WindowId, FrameArgs>>>;

/// Counters for tests and the latency overlay.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub frames_drawn: u64,
    pub frames_without_damage: u64,
    pub commits_applied: u64,
    pub fast_rejected: u64,
}

pub struct Engine {
    cfg: EngineConfig,
    windows: BTreeMap<WindowId, Win>,
    new_pacer: PacerFactory,
    to_ui: Box<dyn Fn(ToUi) + Send>,
    wake_ui: Box<dyn Fn() + Send>,
    sent: bool,
    scheduled: Vec<(Time, ToUi)>,
    frame_args: Option<SharedFrameArgs>,
    pub stats: Stats,
}

/// Paint-order keys of the caret's synthetic item (above everything in the window).
const CARET_LAYER: LayerId = LayerId(u32::MAX);
const CARET_NODE: NodeId = NodeId(u64::MAX);

impl Engine {
    pub fn new(
        cfg: EngineConfig,
        new_pacer: PacerFactory,
        to_ui: Box<dyn Fn(ToUi) + Send>,
        wake_ui: Box<dyn Fn() + Send>,
    ) -> Self {
        Engine {
            cfg,
            windows: BTreeMap::new(),
            new_pacer,
            to_ui,
            wake_ui,
            sent: false,
            scheduled: Vec::new(),
            frame_args: None,
            stats: Stats::default(),
        }
    }

    /// Publishes every window's [`FrameArgs`] into `shared` after each frame-clock update.
    pub fn publish_frame_args(&mut self, shared: SharedFrameArgs) {
        self.frame_args = Some(shared);
    }

    fn publish(&self, window: WindowId) {
        let (Some(shared), Some(w)) = (&self.frame_args, self.windows.get(&window)) else { return };
        let mut map = shared.write().unwrap_or_else(|e| e.into_inner());
        match w.pacer.frame_args() {
            Some(a) => map.insert(window, a),
            None => map.remove(&window),
        };
    }

    fn send(&mut self, m: ToUi) {
        (self.to_ui)(m);
        self.sent = true;
    }

    /// Wakes the UI thread once if anything was sent since the last flush.
    pub fn flush(&mut self) {
        if std::mem::take(&mut self.sent) {
            (self.wake_ui)();
        }
    }

    /// Registers a window without a surface (the backend attaches one separately). Its first frame
    /// is drawn when its first commit arrives.
    pub fn insert_window(&mut self, window: WindowId, size: DeviceSize, scale: Scale) {
        let interval = self.cfg.refresh;
        let mut pending = HashMap::new();
        pending.insert(SurfaceKey::Main, Pending { rects: Vec::new(), full: true });
        let w = Win {
            tree: WindowTree::default(),
            pacer: (self.new_pacer)(interval),
            interval,
            ppi: None,
            size,
            scale,
            visible: true,
            os_layers: HashMap::new(),
            commits: Vec::new(),
            apply_upto: None,
            pending,
            snapshots: Snapshots::default(),
            anims: Vec::new(),
            motions: HashMap::new(),
            animating: false,
            caret: Caret {
                rect: None,
                revision: Default::default(),
                blink: self.cfg.caret_blink,
                since: Time::ZERO,
                on: true,
                next_toggle: None,
            },
            vsync_on: false,
            source: None,
            feedback: false,
            occluded: false,
            next_vsync: None,
            timer: None,
            inputs: Vec::new(),
            in_flight: HashMap::new(),
        };
        self.windows.insert(window, w);
    }

    /// Registers an OS layer of `window` without a surface (the backend attaches one separately, or
    /// a test records draws to it).
    pub fn insert_os_layer(&mut self, window: WindowId, surface_id: SurfaceId, size: DeviceSize) {
        if let Some(w) = self.windows.get_mut(&window) {
            w.os_layers.insert(surface_id, size);
            w.pending.entry(SurfaceKey::Os(surface_id)).or_default().full = true;
        }
    }

    pub fn has_window(&self, window: WindowId) -> bool {
        self.windows.contains_key(&window)
    }

    pub fn tree(&self, window: WindowId) -> Option<&WindowTree> {
        self.windows.get(&window).map(|w| &w.tree)
    }

    /// Handles one message. Returns `false` on [`Msg::Shutdown`].
    pub fn handle(&mut self, msg: Msg, now: Time, gpu: &mut dyn Backend) -> bool {
        match msg {
            Msg::AddWindow { window, surface, size, scale } => {
                gpu.attach(window, SurfaceKey::Main, surface, size);
                self.insert_window(window, size, scale);
            }
            Msg::AddOsLayer { window, surface_id, surface, size } => {
                if let Some(w) = self.windows.get_mut(&window) {
                    gpu.attach(window, SurfaceKey::Os(surface_id), surface, size);
                    w.os_layers.insert(surface_id, size);
                    w.pending.entry(SurfaceKey::Os(surface_id)).or_default().full = true;
                    self.signal(window, now, Signal::CompositorDamage, gpu);
                }
            }
            Msg::ResizeOsLayer { window, surface_id, size, done } => {
                if let Some(w) = self.windows.get_mut(&window) {
                    if let Some(s) = w.os_layers.get_mut(&surface_id) {
                        *s = size;
                        gpu.resize(window, SurfaceKey::Os(surface_id), size);
                        w.snapshots.remove(&SurfaceKey::Os(surface_id));
                        w.pending.entry(SurfaceKey::Os(surface_id)).or_default().full = true;
                        if done.is_some() {
                            // The handshake: a new-size frame now, with every commit that arrived (the
                            // UI committed the layout the new size belongs to), before answering.
                            w.apply_upto = Some(Revision(u64::MAX));
                            self.draw(window, None, now, now, gpu);
                        } else {
                            self.signal(window, now, Signal::CompositorDamage, gpu);
                        }
                    }
                }
                if let Some(d) = done {
                    let _ = d.send(());
                }
            }
            Msg::RemoveOsLayer { window, surface_id, done } => {
                if let Some(w) = self.windows.get_mut(&window) {
                    w.os_layers.remove(&surface_id);
                    w.pending.remove(&SurfaceKey::Os(surface_id));
                    w.snapshots.remove(&SurfaceKey::Os(surface_id));
                    gpu.detach(window, SurfaceKey::Os(surface_id));
                }
                if let Some(d) = done {
                    let _ = d.send(());
                }
            }
            Msg::RemoveWindow(window) => {
                if let Some(shared) = &self.frame_args {
                    shared.write().unwrap_or_else(|e| e.into_inner()).remove(&window);
                }
                if let Some(w) = self.windows.remove(&window) {
                    for sid in w.os_layers.keys() {
                        gpu.detach(window, SurfaceKey::Os(*sid));
                    }
                    gpu.detach(window, SurfaceKey::Main);
                    if let Some(l) = &w.source {
                        l.gate.close();
                    }
                }
            }
            Msg::Commit(c) => self.commit(*c, now, gpu),
            Msg::Fast(f) => self.fast(f, now, gpu),
            Msg::Resize { window, size, scale, seq } => self.resize(window, size, scale, seq, now, gpu),
            Msg::SetVisible { window, visible } => {
                if let Some(w) = self.windows.get_mut(&window) {
                    w.visible = visible;
                    let visible = visible && !w.occluded;
                    if visible {
                        for p in w.pending.values_mut() {
                            p.full = true;
                        }
                    }
                    self.signal(window, now, Signal::Visible(visible), gpu);
                }
            }
            Msg::SetCaretBlink { window, blink } => {
                if let Some(w) = self.windows.get_mut(&window) {
                    let c = &mut w.caret;
                    c.blink = blink;
                    c.since = now;
                    c.next_toggle = c.rect.and(blink).and_then(|b| b.next_change(now, now));
                    if !c.on {
                        c.on = true;
                        self.signal(window, now, Signal::CompositorDamage, gpu);
                    }
                }
            }
            Msg::SetRefresh { window, interval } => {
                if let Some(w) = self.windows.get_mut(&window) {
                    w.interval = interval;
                }
            }
            Msg::SetPpi { window, ppi } => {
                if let Some(w) = self.windows.get_mut(&window) {
                    w.ppi = (ppi > 0.0).then_some(ppi);
                }
            }
            Msg::RegisterTexture { id, texture } => gpu.register_texture(id, texture),
            Msg::UnregisterTexture(id) => gpu.unregister_texture(id),
            Msg::TextureUpdated { window, texture } => {
                if let Some(w) = self.windows.get_mut(&window) {
                    w.tree.dirty_textures.insert(texture);
                    self.signal(window, now, Signal::CompositorDamage, gpu);
                }
            }
            Msg::RegisterCanvas(c) => gpu.register_canvas(c),
            Msg::UnregisterCanvas(id) => gpu.unregister_canvas(id),
            Msg::CanvasCamera { window, canvas, camera, lod, input_seq } => {
                if let Some(w) = self.windows.get_mut(&window) {
                    w.tree.canvas_cameras.insert(canvas, camera);
                    if let Some(l) = lod {
                        w.tree.canvas_lods.insert(canvas, l);
                    }
                    w.tree.dirty_canvases.insert(canvas);
                    w.inputs.push(input_seq);
                    self.signal(window, now, Signal::CompositorDamage, gpu);
                }
            }
            Msg::CanvasChanged { window, canvas } => {
                if let Some(w) = self.windows.get_mut(&window) {
                    w.tree.dirty_canvases.insert(canvas);
                    self.signal(window, now, Signal::CompositorDamage, gpu);
                }
            }
            Msg::NeedsUiFrame { window, on } => self.signal(window, now, Signal::NeedsUiFrame(on), gpu),
            Msg::UiFrameSkipped { window, frame } => self.signal(window, now, Signal::UiFrameSkipped { frame }, gpu),
            Msg::UiInputCost { window, frame, cost } => {
                self.signal(window, now, Signal::UiInputCost { frame, cost }, gpu)
            }
            Msg::HighRateInput(window) => self.signal(window, now, Signal::HighRateInput, gpu),
            Msg::UiCost { window, frame, cost } => self.signal(window, now, Signal::UiCost { frame, cost }, gpu),
            Msg::Fling { window, node, velocity, curve, at } => {
                let ppi = self.windows.get(&window).and_then(|w| w.ppi).unwrap_or(self.cfg.ppi);
                self.start_motion(window, node, now, gpu, |s| {
                    let curve = Fling::new(curve, velocity, ppi, s.viewport.size());
                    Motion { anim: MotionKind::Fling(curve), base: s.offset, start: at }
                })
            }
            Msg::SmoothScroll { window, node, to, behavior, at } => self.smooth_scroll(window, node, to, behavior, at, now, gpu),
            Msg::Vsync { window, target, interval } => {
                if let Some(w) = self.windows.get_mut(&window) {
                    w.interval = interval;
                    self.signal(window, now, Signal::Vsync { target, interval }, gpu);
                }
            }
            Msg::Presented { window, frame, at, estimated } => {
                if let Some(w) = self.windows.get_mut(&window) {
                    w.feedback |= !estimated;
                }
                self.presented(window, frame, at, estimated, now, gpu)
            }
            Msg::Occluded { window, occluded } => {
                if let Some(w) = self.windows.get_mut(&window) {
                    if w.occluded != occluded {
                        w.occluded = occluded;
                        let visible = w.visible && !occluded;
                        if visible {
                            for p in w.pending.values_mut() {
                                p.full = true;
                            }
                        }
                        self.send(ToUi::Occluded { window, occluded });
                        self.signal(window, now, Signal::Visible(visible), gpu);
                    }
                }
            }
            Msg::SetFrameSource { window, link } => {
                if let Some(w) = self.windows.get_mut(&window) {
                    if let Some(old) = w.source.replace(link.clone()) {
                        old.gate.close();
                    }
                    w.next_vsync = None;
                    if let Some(i) = link.source.interval() {
                        w.interval = i;
                    }
                    link.gate.set(w.vsync_on);
                } else {
                    link.gate.close();
                }
            }
            Msg::SetCommitter { window, committer } => gpu.set_committer(window, committer),
            Msg::GpuDone { window, frame, at } => {
                if let Some(f) = self.windows.get_mut(&window).and_then(|w| w.in_flight.get_mut(&frame)) {
                    f.gpu_done = Some(at);
                }
                self.signal(window, now, Signal::GpuDone { frame, at }, gpu);
                // Without presentation feedback the frame is shown at the first vblank after the GPU
                // finished, on the vsync grid.
                if let Some(w) = self.windows.get(&window).filter(|w| !w.feedback) {
                    let at = next_grid(at, w.interval);
                    self.presented(window, frame, at, true, now, gpu);
                }
            }
            Msg::DeviceLost => {
                let ok = gpu.recover();
                for w in self.windows.values_mut() {
                    w.snapshots.clear();
                    for p in w.pending.values_mut() {
                        p.full = true;
                    }
                    w.in_flight.clear();
                }
                if ok {
                    self.send(ToUi::DeviceReset);
                }
                let ids: Vec<_> = self.windows.keys().copied().collect();
                for id in ids {
                    self.signal(id, now, Signal::DeviceLost, gpu);
                    self.signal(id, now, Signal::CompositorDamage, gpu);
                }
            }
            Msg::Shutdown => return false,
        }
        true
    }

    fn commit(&mut self, c: Commit, now: Time, gpu: &mut dyn Backend) {
        let window = c.window;
        let Some(w) = self.windows.get_mut(&window) else {
            log::debug!("skelter-compositor: commit for unknown window {window:?}");
            return;
        };
        let (revision, frame) = (c.revision, c.frame);
        w.commits.push(c);
        self.signal(window, now, Signal::CommitArrived { revision, frame }, gpu);
    }

    fn fast(&mut self, f: FastCommit, now: Time, gpu: &mut dyn Backend) {
        let window = match f {
            FastCommit::ScrollOffset { window, .. }
            | FastCommit::Transform { window, .. }
            | FastCommit::Opacity { window, .. }
            | FastCommit::CanvasView { window, .. }
            | FastCommit::Caret { window, .. } => window,
        };
        let Some(w) = self.windows.get_mut(&window) else { return };
        match f {
            FastCommit::Caret { rect, revision, .. } => {
                if revision < w.caret.revision {
                    self.stats.fast_rejected += 1;
                    return;
                }
                w.caret.revision = revision;
                w.caret.rect = rect;
                w.caret.on = true;
                w.caret.since = now;
                w.caret.next_toggle = rect.and(w.caret.blink).and_then(|b| b.next_change(now, now));
            }
            FastCommit::ScrollOffset { node, input_seq, .. } => {
                // A user scroll takes over from a running fling on the same container.
                w.motions.remove(&node);
                w.tree.compositor_scrolling.remove(&node);
                if w.tree.apply_fast(&f).is_err() {
                    self.stats.fast_rejected += 1;
                    return;
                }
                w.inputs.push(input_seq);
            }
            FastCommit::Transform { input_seq, .. } | FastCommit::CanvasView { input_seq, .. } => {
                if w.tree.apply_fast(&f).is_err() {
                    self.stats.fast_rejected += 1;
                    return;
                }
                w.inputs.push(input_seq);
            }
            FastCommit::Opacity { .. } => {
                if w.tree.apply_fast(&f).is_err() {
                    self.stats.fast_rejected += 1;
                    return;
                }
            }
        }
        self.signal(window, now, Signal::CompositorDamage, gpu);
    }

    fn resize(&mut self, window: WindowId, size: DeviceSize, scale: Scale, seq: u64, now: Time, gpu: &mut dyn Backend) {
        let Some(w) = self.windows.get_mut(&window) else { return };
        w.size = size;
        w.scale = scale;
        gpu.resize(window, SurfaceKey::Main, size);
        w.snapshots.clear();
        for p in w.pending.values_mut() {
            p.full = true;
        }
        // The handshake draws now, outside the frame clock, with every commit that has arrived, and
        // answers once the frame is presented.
        w.apply_upto = Some(Revision(u64::MAX));
        let target = now;
        self.draw(window, None, target, now, gpu);
        self.send(ToUi::ResizeDone { window, seq, size, scale });
    }

    /// Starts a compositor-owned scroll motion on `node`, replacing any running one.
    fn start_motion(
        &mut self,
        window: WindowId,
        node: PropNodeId,
        now: Time,
        gpu: &mut dyn Backend,
        make: impl FnOnce(&skelter_base::props::ScrollNode) -> Motion,
    ) {
        let Some(w) = self.windows.get_mut(&window) else { return };
        let Some(s) = w.tree.props.scrolls.get(&node) else { return };
        let motion = make(s);
        w.motions.insert(node, motion);
        w.tree.compositor_scrolling.insert(node);
        self.update_animating(window, now, gpu);
    }

    /// Animates `node` to `to`: a running smooth scroll heads for the new target keeping its velocity
    /// (Chromium's retarget); otherwise a new curve starts from the current offset.
    #[allow(clippy::too_many_arguments)]
    fn smooth_scroll(
        &mut self,
        window: WindowId,
        node: PropNodeId,
        to: Vec2,
        behavior: DurationBehavior,
        at: Time,
        now: Time,
        gpu: &mut dyn Backend,
    ) {
        let Some(w) = self.windows.get_mut(&window) else { return };
        let Some(s) = w.tree.props.scrolls.get(&node) else { return };
        let to = tree::clamp_offset(s, to);
        if let Some(m) = w.motions.get_mut(&node) {
            if let MotionKind::Smooth(c) = &mut m.anim {
                if c.retarget(at - m.start, to) {
                    return;
                }
            }
        }
        self.start_motion(window, node, now, gpu, |s| Motion {
            anim: MotionKind::Smooth(SmoothScroll::new(s.offset, to, behavior)),
            base: Vec2::ZERO,
            start: at,
        });
    }

    fn presented(&mut self, window: WindowId, frame: FrameId, at: Time, estimated: bool, now: Time, gpu: &mut dyn Backend) {
        let Some(w) = self.windows.get_mut(&window) else { return };
        let f = w.in_flight.remove(&frame);
        let inputs = f.as_ref().map(|f| f.inputs.clone()).unwrap_or_default();
        self.send(ToUi::Presented { window, frame, at, estimated, inputs });
        if let Some(f) = f {
            self.send(ToUi::FrameTimings {
                window,
                frame,
                draw: f.draw,
                submit: f.submit,
                gpu_done: f.gpu_done,
                present: at,
                estimated,
            });
        }
        self.signal(window, now, Signal::Presented { frame, at, estimated }, gpu);
    }

    /// Feeds a signal to the window's pacer and carries out its actions.
    fn signal(&mut self, window: WindowId, now: Time, sig: Signal, gpu: &mut dyn Backend) {
        let Some(w) = self.windows.get_mut(&window) else { return };
        let actions = w.pacer.on_signal(now, sig);
        self.publish(window);
        for a in actions {
            match a {
                Action::SubscribeVsync(on) => {
                    let Some(w) = self.windows.get_mut(&window) else { return };
                    w.vsync_on = on;
                    match &w.source {
                        Some(l) => l.gate.set(on),
                        None => w.next_vsync = on.then(|| next_grid(now, w.interval)),
                    }
                }
                Action::ApplyCommit { revision } => {
                    if let Some(w) = self.windows.get_mut(&window) {
                        w.apply_upto = Some(w.apply_upto.map_or(revision, |r| r.max(revision)));
                    }
                }
                Action::ForceFullRedraw => {
                    if let Some(w) = self.windows.get_mut(&window) {
                        w.snapshots.clear();
                        for p in w.pending.values_mut() {
                            p.full = true;
                        }
                    }
                }
                // Re-presenting the last image needs a present without a new acquire, which wgpu does
                // not offer; the keep-alive is skipped (the panel may lower its rate).
                Action::Represent { .. } => {}
                Action::ClearTimer => {
                    if let Some(w) = self.windows.get_mut(&window) {
                        w.timer = None;
                    }
                }
                Action::BeginUiFrame { frame, target, start_at, deadline } => {
                    let m = ToUi::BeginFrame { window, frame, target, deadline };
                    if start_at <= now {
                        self.send(m);
                    } else {
                        self.scheduled.push((start_at, m));
                    }
                }
                Action::Draw { frame, target } => self.draw(window, Some(frame), target, now, gpu),
                Action::SetTimer(t) => {
                    if let Some(w) = self.windows.get_mut(&window) {
                        w.timer = Some(t);
                    }
                }
            }
        }
    }

    /// Runs one frame for `window` at `target`: apply pending commits, sample animations and scroll
    /// motions, flatten, compute damage, render and present each damaged surface.
    pub fn draw(&mut self, window: WindowId, frame: Option<FrameId>, target: Time, now: Time, gpu: &mut dyn Backend) {
        let Some(w) = self.windows.get_mut(&window) else { return };
        let mut cmds = Vec::new();
        let mut applied = None;
        if let Some(upto) = w.apply_upto.take() {
            let (now_, later): (Vec<Commit>, Vec<Commit>) =
                std::mem::take(&mut w.commits).into_iter().partition(|c| c.revision <= upto);
            w.commits = later;
            let mut images = Vec::new();
            let mut paths = Vec::new();
            for c in &now_ {
                images.extend_from_slice(&c.resources.removed_images);
                paths.extend_from_slice(&c.resources.removed_paths);
            }
            if !images.is_empty() || !paths.is_empty() {
                gpu.forget(&images, &paths);
            }
            for c in now_ {
                w.inputs.push(c.input_seq);
                applied = Some(c.revision);
                cmds.extend(w.tree.apply_commit(c));
                self.stats.commits_applied += 1;
            }
        }
        if let Some(revision) = applied {
            self.send(ToUi::CommitApplied { window, revision });
        }
        self.run_cmds(window, cmds, target);
        self.sample(window, target);
        let Some(w) = self.windows.get_mut(&window) else { return };

        let mut flat = w.tree.flatten();
        if let (Some(rect), true) = (w.caret.rect, w.caret.on) {
            flat.entry(SurfaceKey::Main).or_default().push(caret_item(rect, self.cfg.caret_color));
        }
        let mut extra: HashMap<SurfaceKey, Vec<Rect>> = HashMap::new();
        for (k, r) in w.tree.take_layer_damage() {
            extra.entry(k).or_default().push(r);
        }
        for (k, r) in w.tree.take_content_damage(&flat) {
            extra.entry(k).or_default().push(r);
        }
        for id in w.tree.canvas_ids() {
            gpu.set_canvas_view(id, w.tree.canvas_view(id));
        }
        let mut drew = None;
        let mut failed = false;
        let keys: Vec<SurfaceKey> = std::iter::once(SurfaceKey::Main).chain(w.os_layers.keys().map(|s| SurfaceKey::Os(*s))).collect();
        for key in keys {
            let size = match key {
                SurfaceKey::Main => w.size,
                SurfaceKey::Os(s) => w.os_layers[&s],
            };
            let items: &[FlatItem] = flat.get(&key).map(|v| &v[..]).unwrap_or(&[]);
            let mut rects = extra.remove(&key).unwrap_or_default();
            w.snapshots.entry(key).or_default().diff(items, &mut rects);
            let pending = w.pending.entry(key).or_default();
            pending.add(damage::to_device(&rects, w.scale, size));
            if !w.visible || w.occluded || size.w == 0 || size.h == 0 {
                continue;
            }
            let Some(dmg) = pending.take(size) else { continue };
            let draw_items: Vec<DrawItem<'_>> = items
                .iter()
                .filter(|it| it.opacity > 0.0 && !it.bounds.is_empty())
                .map(|it| DrawItem { transform: it.transform, clip: it.clip, opacity: it.opacity, list: &it.list })
                .collect();
            let full = damage::covers(&dmg, size);
            let scene = FrameScene {
                target: size,
                scale: w.scale,
                clear: self.cfg.clear,
                damage: if full { &[] } else { &dmg },
                items: &draw_items,
                resources: &w.tree.resources,
            };
            if let (Some(f), SurfaceKey::Main, Some(l)) = (frame, key, &w.source) {
                // Arms the next frame callback and presentation feedback (Wayland).
                l.source.before_present_sized(f, size);
            }
            match gpu.draw(window, key, frame, &scene) {
                DrawOutcome::Presented { submitted } => drew = Some(drew.map_or(submitted, |d: Time| d.max(submitted))),
                DrawOutcome::Skipped => {
                    pending.add(dmg);
                    failed = true;
                }
                DrawOutcome::NoSurface => {}
            }
        }
        if drew.is_some() {
            self.stats.frames_drawn += 1;
        } else {
            self.stats.frames_without_damage += 1;
        }
        let inputs = std::mem::take(&mut w.inputs);
        if let Some(frame) = frame {
            match drew {
                Some(at) => {
                    let submit = at.max(now);
                    w.in_flight.insert(frame, InFlight { inputs, draw: now, submit, gpu_done: None });
                    self.signal(window, now, Signal::Submitted { frame, at: submit }, gpu);
                }
                None => {
                    // Nothing reached the screen: the inputs ride on the next frame that draws.
                    w.inputs = inputs;
                    let sig = if failed { Signal::DrawFailed { frame } } else { Signal::Discarded { frame } };
                    self.signal(window, now, sig, gpu);
                }
            }
        } else {
            w.inputs = inputs;
        }
        self.update_animating(window, now, gpu);
    }

    fn update_animating(&mut self, window: WindowId, now: Time, gpu: &mut dyn Backend) {
        let Some(w) = self.windows.get_mut(&window) else { return };
        let active = w.anims.iter().any(|a| !a.holding && a.paused_at.is_none()) || !w.motions.is_empty();
        if active != w.animating {
            w.animating = active;
            self.signal(window, now, Signal::AnimationsActive(active), gpu);
        }
    }

    fn run_cmds(&mut self, window: WindowId, cmds: Vec<AnimationCmd>, now: Time) {
        let mut events = Vec::new();
        let Some(w) = self.windows.get_mut(&window) else { return };
        for cmd in cmds {
            match cmd {
                AnimationCmd::Start(spec) => {
                    let compositor = skelter_anim::placement(spec.property) == Placement::Compositor;
                    if !compositor || spec.target.is_none() {
                        // Not runnable here: a UI-thread property, or no property node to drive.
                        events.push(AnimationEvent::Aborted { id: spec.id });
                        continue;
                    }
                    w.anims.retain(|a| a.spec.id != spec.id);
                    let start = match spec.start {
                        StartTime::At(t) => Some(t),
                        StartTime::NextFrame | StartTime::Group(_) => None,
                    };
                    w.anims.push(Anim { spec, start, paused_at: None, started: false, holding: false, last: None });
                }
                AnimationCmd::Cancel(id) => {
                    if let Some(i) = w.anims.iter().position(|a| a.spec.id == id) {
                        let a = w.anims.remove(i);
                        clear_override(&mut w.tree, &a.spec);
                        if !a.holding {
                            events.push(AnimationEvent::Aborted { id });
                        }
                    }
                }
                AnimationCmd::Retarget { id, to } => {
                    if let Some(a) = w.anims.iter_mut().find(|a| a.spec.id == id) {
                        retarget(a, to, now);
                    }
                }
                AnimationCmd::Pause { id, at } => {
                    if let Some(a) = w.anims.iter_mut().find(|a| a.spec.id == id) {
                        a.paused_at.get_or_insert(at);
                    }
                }
                AnimationCmd::Resume { id, at } => {
                    if let Some(a) = w.anims.iter_mut().find(|a| a.spec.id == id) {
                        if let (Some(p), Some(s)) = (a.paused_at.take(), a.start) {
                            a.start = Some(s + (at - p));
                        }
                    }
                }
            }
        }
        for event in events {
            self.send(ToUi::Animation { window, event });
        }
    }

    /// Samples compositor animations and scroll motions at `target` into the property trees.
    fn sample(&mut self, window: WindowId, target: Time) {
        let mut out = Vec::new();
        let Some(w) = self.windows.get_mut(&window) else { return };
        let mut finished = Vec::new();
        for (i, a) in w.anims.iter_mut().enumerate() {
            if a.holding {
                continue;
            }
            let start = *a.start.get_or_insert(target);
            if !a.started {
                a.started = true;
                out.push(ToUi::Animation { window, event: AnimationEvent::Started { id: a.spec.id, at: start } });
            }
            let t = a.paused_at.unwrap_or(target);
            let s = skelter_anim::sample(&a.spec.generator, start, t);
            match &s.value {
                Some(v) => {
                    a.last = Some((v.clone(), s.velocity));
                    apply_value(&mut w.tree, &a.spec, v, &mut out, window);
                }
                None => clear_override(&mut w.tree, &a.spec),
            }
            if s.done && a.paused_at.is_none() {
                out.push(ToUi::Animation { window, event: AnimationEvent::Finished { id: a.spec.id, at: target } });
                if s.value.is_some() && holds(&a.spec.generator) {
                    a.holding = true;
                } else {
                    finished.push(i);
                }
            }
        }
        for i in finished.into_iter().rev() {
            let a = w.anims.remove(i);
            clear_override(&mut w.tree, &a.spec);
        }
        let mut done = Vec::new();
        for (node, m) in &w.motions {
            if let Some(offset) = w.tree.set_scroll_offset(*node, m.offset_at(target)) {
                out.push(ToUi::ScrollMoved { window, node: *node, offset });
            }
            if m.done_at(target) {
                done.push(*node);
            }
        }
        for n in done {
            w.motions.remove(&n);
            w.tree.compositor_scrolling.remove(&n);
        }
        for m in out {
            self.send(m);
        }
    }

    /// Fires what is due at `now`: timer vsync ticks, pacer deadlines, caret blinks, scheduled
    /// `BeginFrame`s.
    pub fn advance(&mut self, now: Time, gpu: &mut dyn Backend) {
        let mut due = Vec::new();
        self.scheduled.retain(|(t, m)| {
            if *t <= now {
                due.push(m.clone());
                false
            } else {
                true
            }
        });
        for m in due {
            self.send(m);
        }
        let ids: Vec<WindowId> = self.windows.keys().copied().collect();
        for id in ids {
            let Some(w) = self.windows.get_mut(&id) else { continue };
            if w.caret.next_toggle.is_some_and(|t| t <= now) {
                let c = &mut w.caret;
                c.on = c.blink.is_none_or(|b| b.visible(now - c.since));
                c.next_toggle = c.blink.and_then(|b| b.next_change(c.since, now));
                self.signal(id, now, Signal::CompositorDamage, gpu);
            }
            let Some(w) = self.windows.get_mut(&id) else { continue };
            if let Some(t) = w.next_vsync.filter(|t| *t <= now) {
                let interval = w.interval;
                // A late wake skips to the latest tick; never two frames back to back to catch up.
                let tick = latest_tick(t, interval, now);
                w.next_vsync = Some(tick + interval);
                self.signal(id, now, Signal::Vsync { target: tick + interval, interval }, gpu);
            }
            let Some(w) = self.windows.get_mut(&id) else { continue };
            let pacer_due = w.timer.filter(|t| *t <= now).is_some() || w.pacer.next_wakeup().is_some_and(|t| t <= now);
            if pacer_due {
                w.timer = None;
                self.signal(id, now, Signal::Timer, gpu);
            }
        }
        self.flush();
    }

    /// The earliest time [`Engine::advance`] has work.
    pub fn next_wakeup(&self) -> Option<Time> {
        let mut t: Option<Time> = self.scheduled.iter().map(|(t, _)| *t).min();
        let mut take = |x: Option<Time>| {
            if let Some(x) = x {
                t = Some(t.map_or(x, |t| t.min(x)));
            }
        };
        for w in self.windows.values() {
            take(w.next_vsync);
            take(w.timer);
            take(w.pacer.next_wakeup());
            take(w.caret.next_toggle);
        }
        t
    }
}

/// Writes an animation's sampled value into the property trees' animation overrides.
fn apply_value(
    tree: &mut WindowTree,
    spec: &AnimationSpec,
    v: &AnimValue,
    out: &mut Vec<ToUi>,
    window: WindowId,
) {
    let Some(node) = spec.target else { return };
    match (spec.property, v) {
        (AnimProperty::Opacity, AnimValue::Number(o)) => {
            tree.props.anim_opacity.insert(node, *o);
        }
        (AnimProperty::Transform, AnimValue::Transform(fns)) => {
            tree.props.anim_transform.insert(node, skelter_anim::interp::compose(fns));
        }
        (AnimProperty::Clip, AnimValue::Rect(r)) => {
            tree.props.anim_clip.insert(node, *r);
        }
        (AnimProperty::ScrollOffset, AnimValue::Vec2(o)) => {
            if let Some(offset) = tree.set_scroll_offset(node, *o) {
                out.push(ToUi::ScrollMoved { window, node, offset });
            }
        }
        (p, v) => log::debug!("skelter-compositor: animation value {v:?} does not fit property {p:?}"),
    }
}

fn clear_override(tree: &mut WindowTree, spec: &AnimationSpec) {
    let Some(node) = spec.target else { return };
    match spec.property {
        AnimProperty::Opacity => {
            tree.props.anim_opacity.remove(&node);
        }
        AnimProperty::Transform => {
            tree.props.anim_transform.remove(&node);
        }
        AnimProperty::Clip => {
            tree.props.anim_clip.remove(&node);
        }
        _ => {}
    }
}

/// Whether a finished animation keeps its last value (Web Animations `fill: forwards`/`both`; a
/// spring rests at its target).
fn holds(g: &Generator) -> bool {
    match g {
        Generator::Keyframes { timing, .. } => matches!(timing.fill, FillMode::Forwards | FillMode::Both),
        Generator::Spring { .. } => true,
    }
}

/// Heads for `to` from where the animation is now: a spring keeps its velocity
/// (`skelter_anim::retarget`); keyframes restart from the current value with the same timing.
fn retarget(a: &mut Anim, to: AnimValue, now: Time) {
    let Some(start) = a.start else {
        // Not started yet: only the end value changes.
        match &mut a.spec.generator {
            Generator::Spring { to: t, .. } => *t = to,
            Generator::Keyframes { keyframes, .. } => {
                if let Some(k) = keyframes.last_mut() {
                    k.value = to;
                }
            }
        }
        return;
    };
    match &mut a.spec.generator {
        Generator::Spring { .. } => {
            if let Some(g) = skelter_anim::retarget(&a.spec.generator, start, now, to) {
                a.spec.generator = g;
            }
        }
        Generator::Keyframes { keyframes, .. } => {
            let Some((current, _)) = a.last.clone() else { return };
            let easing = keyframes.first().and_then(|k| k.easing.clone());
            *keyframes = vec![
                Keyframe { offset: 0.0, value: current, easing },
                Keyframe { offset: 1.0, value: to, easing: None },
            ];
        }
    }
    a.start = Some(now);
    a.holding = false;
    a.started = true;
}

/// A compositor-owned scroll motion on one scroll node.
struct Motion {
    anim: MotionKind,
    /// Added to the curve's offset: a fling's offsets are distances from where it started; a smooth
    /// scroll's are absolute.
    base: Vec2,
    start: Time,
}

enum MotionKind {
    Fling(Fling),
    Smooth(SmoothScroll),
}

impl Motion {
    fn curve(&self) -> &dyn ScrollAnimation {
        match &self.anim {
            MotionKind::Fling(f) => f,
            MotionKind::Smooth(s) => s,
        }
    }
    fn offset_at(&self, t: Time) -> Vec2 {
        self.base + self.curve().offset_at(t - self.start)
    }
    fn done_at(&self, t: Time) -> bool {
        self.curve().done_at(t - self.start)
    }
}

fn caret_item(rect: Rect, color: Color) -> FlatItem {
    let mut list = DisplayList::new();
    list.push(
        Prim::Quad(Quad {
            rect,
            radii: Corners::ZERO,
            fill: Fill::Solid(color),
            border_widths: Insets::ZERO,
            border_color: Color::TRANSPARENT,
            border_style: Default::default(),
        }),
        rect,
    );
    FlatItem {
        layer: CARET_LAYER,
        node: CARET_NODE,
        transform: Default::default(),
        clip: None,
        opacity: 1.0,
        list: Arc::new(list),
        bounds: rect,
    }
}

/// The first multiple of `interval` strictly after `t`.
fn next_grid(t: Time, interval: Span) -> Time {
    let i = interval.0.max(1) as u64;
    Time((t.0 / i + 1) * i)
}

/// The latest tick of the grid through `t` that is not after `now`.
fn latest_tick(t: Time, interval: Span, now: Time) -> Time {
    let i = interval.0.max(1) as u64;
    if now.0 <= t.0 {
        return t;
    }
    Time(t.0 + (now.0 - t.0) / i * i)
}

