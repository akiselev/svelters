//! [`EguiHarness`]: egui_kittest's `Harness` for egui code that runs in a skelter egui island
//! (`skelter_egui_island::EguiIsland`, architecture §15). cadatomic's panes keep their
//! `|ui, state| …` closures while skelter owns the window, the input path and the pixels.
//!
//! The document is one island filling the window ([`IslandDoc`]): skelter input reaches it through the
//! app's dispatch (`EguiIsland::handle_input`), each pass's AccessKit update is grafted under the
//! host's node for it (queries descend into it), and `render` draws the island with egui-wgpu into its
//! texture on the harness's device, then the committed tree (the island's `Prim::Surface`) with
//! skelter-render. kittest's `ctx` is the island's context.

use crate::harness::{Harness, HarnessBuilder, Hooks};
use crate::render::GpuRender;
use crate::snapshot::SnapshotOptions;
use crate::convert::IntoSize;
use skelter::seams::{Dispatched, DocumentApi};
use skelter_base::accesskit::{self, ActionRequest, Node as AkNode, Role, TreeInfo, TreeUpdate};
use skelter_base::anim::AnimationEvent;
use skelter_base::commit::Commit;
use skelter_base::event::{InputEnvelope, WindowEvent};
use skelter_base::geom::{Affine, DeviceSize, Point, Rect, Scale, Size, Vec2};
use skelter_base::ids::{LayerId, NodeId, PropNodeId, Revision, TextureId, WindowId};
use skelter_base::layer::{ItemRef, LayerDelta, LayerDesc, LayerKind};
use skelter_base::props::{PropOp, TransformNode};
use skelter_base::time::{Clock, Span, Time};
use skelter_egui_island::{EguiIsland, IslandDesc, IslandOutput, TextureRegistry, island_tree_id};
use skelter_platform::clipboard::{Clipboard, ClipboardKind, MemoryClipboard};
use skelter_platform::{PlatformCx, Waker};
use std::cell::RefCell;
use std::ops::{Deref, DerefMut};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

/// egui repaints requested outside a pass (`ctx.request_repaint()` from another thread: app events,
/// jobs, RPC): they make the island due and wake the app.
#[derive(Default)]
struct Repaint {
    due: AtomicBool,
    in_pass: AtomicBool,
    waker: Mutex<Option<Waker>>,
}

/// The node whose display list is the island's texture.
const ISLAND_NODE: NodeId = NodeId::new(1, 1);
const ROOT_LAYER: LayerId = LayerId(1);
const SPACE: PropNodeId = PropNodeId(1);
const TEXTURE: TextureId = TextureId(1);
/// AccessKit ids of the host tree: the window and the graft node.
const AK_ROOT: accesskit::NodeId = accesskit::NodeId(1);
const AK_GRAFT: accesskit::NodeId = accesskit::NodeId(2);

type UiFn<S> = Box<dyn FnMut(&mut egui::Ui, &mut S)>;
type RendererSetup = Box<dyn FnOnce(&wgpu::Device, &wgpu::Queue, &mut egui_wgpu::Renderer)>;

/// A clipboard the island and the harness share (a paste puts its text here before the chord).
#[derive(Clone, Default)]
struct SharedClipboard(Rc<RefCell<MemoryClipboard>>);

impl Clipboard for SharedClipboard {
    fn get_text(&mut self, kind: ClipboardKind) -> Option<String> {
        self.0.borrow_mut().get_text(kind)
    }
    fn set_text(&mut self, kind: ClipboardKind, text: &str) -> bool {
        self.0.borrow_mut().set_text(kind, text)
    }
}

/// A document holding one egui island over the whole window, running `app` each pass.
pub struct IslandDoc<S> {
    island: EguiIsland,
    app: UiFn<S>,
    state: S,
    clock: Arc<dyn Clock>,
    clipboard: SharedClipboard,
    revision: u64,
    /// Layers and the host's AccessKit tree are not committed yet (first frame, resize).
    host_dirty: bool,
    sent_layers: bool,
    output: IslandOutput,
    passes: u64,
    repaint: Arc<Repaint>,
}

impl<S> IslandDoc<S> {
    fn new(
        app: UiFn<S>,
        state: S,
        clock: Arc<dyn Clock>,
        size: Size,
        scale: f32,
        device: Option<&wgpu::Device>,
        renderer: egui_wgpu::RendererOptions,
    ) -> Self {
        let desc = IslandDesc {
            texture: TEXTURE,
            tree: island_tree_id(1),
            rect: Rect::from_origin_size(Point::ZERO, size),
            scale: Scale(scale),
            renderer,
        };
        let mut island = match device {
            Some(d) => EguiIsland::with_device(d, desc),
            None => EguiIsland::new(desc),
        };
        island.enable_accesskit();
        // The island is the whole window: it has keyboard focus while the window does.
        island.set_focused(true);
        let clipboard = SharedClipboard::default();
        island.set_clipboard(Box::new(clipboard.clone()));
        let repaint = Arc::new(Repaint::default());
        let r = repaint.clone();
        island.context().set_request_repaint_callback(move |_| {
            // a pass's own requests are the island's `pass_at`
            if !r.in_pass.load(Ordering::Acquire) {
                r.due.store(true, Ordering::Release);
                if let Some(w) = r.waker.lock().unwrap().as_ref() {
                    w.wake();
                }
            }
        });
        IslandDoc {
            island,
            app,
            state,
            clock,
            clipboard,
            revision: 0,
            host_dirty: true,
            sent_layers: false,
            output: IslandOutput::default(),
            passes: 0,
            repaint,
        }
    }

    /// The app's waker, woken by an egui repaint requested outside a pass.
    pub fn set_waker(&self, waker: Waker) {
        *self.repaint.waker.lock().unwrap() = Some(waker);
    }

    /// Run a pass at the next frame (kittest runs at least one per step).
    pub fn request_pass(&self) {
        self.repaint.due.store(true, Ordering::Release);
    }

    /// kittest's settle rule: the last pass requested no immediate repaint and nothing is due.
    fn settled(&self, now: Time) -> bool {
        !self.due() && !self.host_dirty && !self.island.needs_pass(now)
    }

    fn due(&self) -> bool {
        self.repaint.due.load(Ordering::Acquire)
    }

    pub fn island(&self) -> &EguiIsland {
        &self.island
    }
    pub fn island_mut(&mut self) -> &mut EguiIsland {
        &mut self.island
    }
    pub fn state(&self) -> &S {
        &self.state
    }
    pub fn state_mut(&mut self) -> &mut S {
        &mut self.state
    }
    /// The last pass's skelter-facing output (cursor, IME, copied text, links, drag-and-drop).
    pub fn output(&self) -> &IslandOutput {
        &self.output
    }
    /// egui passes run so far.
    pub fn passes(&self) -> u64 {
        self.passes
    }

    fn host_tree(&self) -> TreeUpdate {
        let r = self.island.rect();
        let mut root = AkNode::new(Role::Window);
        root.set_children(vec![AK_GRAFT]);
        let mut graft = AkNode::new(Role::GenericContainer);
        graft.set_bounds(accesskit::Rect {
            x0: 0.0,
            y0: 0.0,
            x1: f64::from(r.width()),
            y1: f64::from(r.height()),
        });
        self.island.a11y().graft_node(&mut graft);
        TreeUpdate {
            nodes: vec![(AK_ROOT, root), (AK_GRAFT, graft)],
            tree: Some(TreeInfo::new(AK_ROOT)),
            tree_id: accesskit::TreeId::ROOT,
            // The island holds the keyboard focus: the root tree's focus is its graft node.
            focus: AK_GRAFT,
        }
    }

    fn layers(&self, c: &mut Commit) {
        c.props.ops.push(PropOp::Transform(
            SPACE,
            TransformNode {
                parent: None,
                local: Affine::IDENTITY,
                scroll: None,
            },
        ));
        c.layers.push(LayerDelta::Add(LayerDesc {
            id: ROOT_LAYER,
            parent: None,
            kind: LayerKind::Root,
            transform: SPACE,
            clip: None,
            effect: None,
            z: 0,
            bounds: Rect::from_origin_size(Point::ZERO, self.island.rect().size()),
            opaque: false,
        }));
        c.layers.push(LayerDelta::Items {
            layer: ROOT_LAYER,
            items: vec![ItemRef {
                node: ISLAND_NODE,
                transform: SPACE,
                clip: None,
                effect: None,
            }],
        });
    }
}

impl<S> DocumentApi for IslandDoc<S> {
    fn dispatch(&mut self, ev: &InputEnvelope) -> Dispatched {
        self.island.handle_input(ev);
        Dispatched {
            dirty: self.island.needs_pass(ev.recv_time),
            ..Dispatched::default()
        }
    }

    fn frame(&mut self, window: WindowId, target: Time) -> Option<Commit> {
        if self.repaint.due.swap(false, Ordering::AcqRel) {
            self.island.request_pass();
        }
        let pass = self.island.needs_pass(target);
        if !pass && !self.host_dirty {
            return None;
        }
        self.revision += 1;
        let mut c = Commit {
            window,
            revision: Revision(self.revision),
            ..Commit::default()
        };
        if self.host_dirty {
            if !std::mem::replace(&mut self.sent_layers, true) {
                self.layers(&mut c);
            } else {
                c.layers.push(LayerDelta::Update(LayerDesc {
                    id: ROOT_LAYER,
                    parent: None,
                    kind: LayerKind::Root,
                    transform: SPACE,
                    clip: None,
                    effect: None,
                    z: 0,
                    bounds: Rect::from_origin_size(Point::ZERO, self.island.rect().size()),
                    opaque: false,
                }));
            }
            c.a11y = Some(self.host_tree());
            self.host_dirty = false;
        }
        if pass {
            let IslandDoc {
                island, app, state, ..
            } = self;
            self.repaint.in_pass.store(true, Ordering::Release);
            let out = island.run(target, |ui| app(ui, state));
            self.repaint.in_pass.store(false, Ordering::Release);
            self.passes += 1;
            // The island's subtree reaches the platform adapter after the host tree that grafts it.
            if let Some(u) = &out.a11y {
                c.a11y_subtrees.push(u.clone());
            }
            c.cursor = Some(out.cursor);
            c.ime_area = out.ime.map(|i| i.cursor);
            self.output = out;
        }
        // The texture's content changed with the pass: re-sending the list damages the island.
        c.lists
            .push((ISLAND_NODE, Arc::new(self.island.display_list())));
        Some(c)
    }

    fn needs_frame(&self, _window: WindowId) -> bool {
        self.host_dirty || self.due() || self.island.needs_pass(self.clock.now())
    }

    fn next_wakeup(&self, _window: WindowId) -> Option<Time> {
        if self.due() { Some(self.clock.now()) } else { self.island.next_wakeup() }
    }

    fn resize(&mut self, _window: WindowId, size: Size, scale: Scale) {
        self.island
            .set_rect(Rect::from_origin_size(Point::ZERO, size), scale);
        self.host_dirty = true;
    }

    fn close_window(&mut self, _window: WindowId) {}
    fn scroll_moved(&mut self, _window: WindowId, _node: PropNodeId, _offset: Vec2) {}
    fn animation_event(&mut self, _window: WindowId, _ev: AnimationEvent) {}

    fn accessibility_action(&mut self, _window: WindowId, req: &ActionRequest) -> bool {
        self.island.handle_a11y_action(req.clone())
    }

    fn accessibility_activated(&mut self, _window: WindowId) {
        self.host_dirty = true;
        self.island.request_pass();
    }

    fn window_event(&mut self, _window: WindowId, ev: &WindowEvent) {
        if let WindowEvent::Focused(f) = ev {
            self.island.set_focused(*f);
        }
    }

    fn device_reset(&mut self) {
        self.host_dirty = true;
        self.island.request_pass();
    }
}

/// Collects islands' texture registrations for the harness's renderer.
#[derive(Default)]
pub(crate) struct Registrations(RefCell<Vec<(TextureId, Option<wgpu::Texture>)>>);

impl TextureRegistry for Registrations {
    fn register_texture(&self, id: TextureId, texture: wgpu::Texture) {
        self.0.borrow_mut().push((id, Some(texture)));
    }
    fn unregister_texture(&self, id: TextureId) {
        self.0.borrow_mut().push((id, None));
    }
}

/// Draw the island's last pass into its texture and register the texture with skelter-render.
fn render_island<S>(doc: &mut IslandDoc<S>, gpu: &mut GpuRender) {
    let regs = Registrations::default();
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("skelter_test_island"),
        });
    let user = doc
        .island
        .render(&gpu.device, &gpu.queue, &mut encoder, &regs);
    gpu.queue.submit(user.into_iter().chain([encoder.finish()]));
    register_textures(regs, gpu);
}

/// Hand the collected registrations to skelter-render.
pub(crate) fn register_textures(regs: Registrations, gpu: &mut GpuRender) {
    for (id, tex) in regs.0.into_inner() {
        match tex {
            Some(t) => {
                let size = DeviceSize {
                    w: t.width(),
                    h: t.height(),
                };
                let view = t.create_view(&wgpu::TextureViewDescriptor::default());
                gpu.renderer.register_texture(id, view, size);
            }
            None => gpu.renderer.unregister_texture(id),
        }
    }
}

/// egui_kittest's `Harness` on skelter: an egui closure in an island, driven through skelter.
/// Derefs to [`Harness`] for every verb and query.
pub struct EguiHarness<S: 'static = ()> {
    inner: Harness<IslandDoc<S>>,
    /// The island's egui context (kittest's `ctx`).
    pub ctx: egui::Context,
}

impl<S: 'static> Deref for EguiHarness<S> {
    type Target = Harness<IslandDoc<S>>;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<S: 'static> DerefMut for EguiHarness<S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl<S: 'static> EguiHarness<S> {
    #[track_caller]
    pub fn new_ui_state(app: impl FnMut(&mut egui::Ui, &mut S) + 'static, state: S) -> Self {
        EguiHarnessBuilder::default().build_ui_state(app, state)
    }
    #[track_caller]
    pub fn new_state(app: impl FnMut(&egui::Context, &mut S) + 'static, state: S) -> Self {
        EguiHarnessBuilder::default().build_state(app, state)
    }
    /// kittest's `state`: the closure's state.
    pub fn state(&self) -> &S {
        self.inner.state().state()
    }
    /// The closure's state; the island runs a pass at the next step (kittest's steps always run one).
    pub fn state_mut(&mut self) -> &mut S {
        let doc = self.inner.state_mut();
        doc.island.request_pass();
        doc.state_mut()
    }
    /// The island document (the island, its last output).
    pub fn doc(&self) -> &IslandDoc<S> {
        self.inner.state()
    }
    pub fn doc_mut(&mut self) -> &mut IslandDoc<S> {
        self.inner.state_mut()
    }
    /// The last pass's output (kittest's `output()` is egui's `FullOutput`; the island turns it into
    /// skelter's: cursor, IME, copied text, links, drag-and-drop).
    pub fn island_output(&self) -> &IslandOutput {
        self.doc().output()
    }
    /// The skelter harness, for drivers that own one (`automation::SkelterDriver`).
    pub fn into_harness(self) -> Harness<IslandDoc<S>> {
        self.inner
    }
    pub fn into_state(self) -> S
    where
        S: Default,
    {
        let mut me = self;
        std::mem::take(me.state_mut())
    }
}

impl EguiHarness<()> {
    /// The builder (the state type comes from `build_ui_state`/`build_state`).
    pub fn builder() -> EguiHarnessBuilder {
        EguiHarnessBuilder::default()
    }
    #[track_caller]
    pub fn new_ui(mut app: impl FnMut(&mut egui::Ui) + 'static) -> Self {
        EguiHarnessBuilder::default().build_ui_state(move |ui, _: &mut ()| app(ui), ())
    }
    #[track_caller]
    pub fn new(mut app: impl FnMut(&egui::Context) + 'static) -> Self {
        EguiHarnessBuilder::default().build_state(move |ctx, _: &mut ()| app(ctx), ())
    }
}

/// egui_kittest's `HarnessBuilder` for [`EguiHarness`]: kittest's defaults (800×600 points, scale 1,
/// a 1/4 s step, dark theme, accessibility check on), skelter's step budget (64: a skelter step is one
/// vsync, and input reaches the screen through the compositor's commit round trip).
pub struct EguiHarnessBuilder {
    base: HarnessBuilder,
    theme: egui::Theme,
    context_setup: Option<Box<dyn FnOnce(&egui::Context)>>,
    renderer_setup: Option<RendererSetup>,
    renderer: egui_wgpu::RendererOptions,
    pass_every_step: bool,
}

impl Default for EguiHarnessBuilder {
    fn default() -> Self {
        EguiHarnessBuilder {
            base: HarnessBuilder::default().with_step_dt(0.25),
            theme: egui::Theme::Dark,
            context_setup: None,
            renderer_setup: None,
            // kittest's: no dithering, so snapshots are stable
            renderer: egui_wgpu::RendererOptions::PREDICTABLE,
            pass_every_step: true,
        }
    }
}

impl EguiHarnessBuilder {
    pub fn with_size(mut self, size: impl IntoSize) -> Self {
        self.base = self.base.with_size(size);
        self
    }
    pub fn with_pixels_per_point(mut self, ppp: f32) -> Self {
        self.base = self.base.with_scale(ppp);
        self
    }
    pub fn with_step_dt(mut self, dt: f32) -> Self {
        self.base = self.base.with_step_dt(dt);
        self
    }
    pub fn with_interval(mut self, interval: Span) -> Self {
        self.base = self.base.with_interval(interval);
        self
    }
    pub fn with_max_steps(mut self, n: u64) -> Self {
        self.base = self.base.with_max_steps(n);
        self
    }
    pub fn with_render_every_step(mut self, on: bool) -> Self {
        self.base = self.base.with_render_every_step(on);
        self
    }
    #[track_caller]
    pub fn with_render_cursor(mut self, on: bool) -> Self {
        self.base = self.base.with_render_cursor(on);
        self
    }
    pub fn with_accessibility_check(mut self, on: bool) -> Self {
        self.base = self.base.with_accessibility_check(on);
        self
    }
    pub fn with_options(mut self, options: SnapshotOptions) -> Self {
        self.base = self.base.with_options(options);
        self
    }
    pub fn with_theme(mut self, theme: egui::Theme) -> Self {
        self.theme = theme;
        self
    }
    /// Configure the island's context before the first pass (fonts, style, shortcuts).
    pub fn with_context_setup(mut self, setup: impl FnOnce(&egui::Context) + 'static) -> Self {
        self.context_setup = Some(Box::new(setup));
        self
    }
    /// Set up paint callbacks' resources on the island's egui-wgpu renderer (kittest's
    /// `renderer(WgpuTestRenderer::from_render_state(..))`). Opens the harness's GPU at build, so the
    /// island renders on that device; the build panics without an adapter.
    pub fn with_renderer_setup(
        mut self,
        setup: impl FnOnce(&wgpu::Device, &wgpu::Queue, &mut egui_wgpu::Renderer) + 'static,
    ) -> Self {
        self.renderer_setup = Some(Box::new(setup));
        self
    }
    /// kittest parity (default on): every step runs one island pass whether or not the island is due,
    /// queued input is delivered one event per pass, and `run` settles when the last pass requested no repaint and no input is queued. Off: the
    /// island passes only when due and `run` settles when nothing needs a frame (skelter's own rule).
    pub fn with_pass_every_step(mut self, on: bool) -> Self {
        self.pass_every_step = on;
        self
    }
    /// The island's egui-wgpu renderer options (default `PREDICTABLE`, as kittest).
    pub fn with_renderer_options(mut self, options: egui_wgpu::RendererOptions) -> Self {
        self.renderer = options;
        self
    }
    /// The underlying skelter harness builder (glyphs, clear colour, app config).
    pub fn skelter(mut self, f: impl FnOnce(HarnessBuilder) -> HarnessBuilder) -> Self {
        self.base = f(self.base);
        self
    }

    #[track_caller]
    pub fn build_ui_state<S: 'static>(
        self,
        app: impl FnMut(&mut egui::Ui, &mut S) + 'static,
        state: S,
    ) -> EguiHarness<S> {
        self.build_boxed(Box::new(app), state)
    }

    #[track_caller]
    pub fn build_ui(self, mut app: impl FnMut(&mut egui::Ui) + 'static) -> EguiHarness<()> {
        self.build_ui_state(move |ui, _: &mut ()| app(ui), ())
    }

    #[track_caller]
    pub fn build_state<S: 'static>(
        self,
        mut app: impl FnMut(&egui::Context, &mut S) + 'static,
        state: S,
    ) -> EguiHarness<S> {
        self.build_ui_state(move |ui, s| app(ui.ctx(), s), state)
    }

    #[track_caller]
    pub fn build(self, mut app: impl FnMut(&egui::Context) + 'static) -> EguiHarness<()> {
        self.build_state(move |ctx, _: &mut ()| app(ctx), ())
    }

    #[track_caller]
    fn build_boxed<S: 'static>(self, app: UiFn<S>, state: S) -> EguiHarness<S> {
        let EguiHarnessBuilder {
            base,
            theme,
            context_setup,
            renderer_setup,
            renderer,
            pass_every_step,
        } = self;
        let gpu = renderer_setup.is_some().then(|| {
            GpuRender::new(base.default_glyphs())
                .unwrap_or_else(|e| panic!("with_renderer_setup needs an adapter: {e}"))
        });
        let (size, scale) = (base.size, base.scale);
        let mut ctx_out = None;
        let make_doc = |clock: Arc<dyn Clock>, gpu: Option<&GpuRender>| {
            let mut doc = IslandDoc::new(app, state, clock, size, scale, gpu.map(|g| &g.device), renderer);
            let ctx = doc.island.context().clone();
            ctx.set_theme(theme);
            if let Some(setup) = context_setup {
                setup(&ctx);
            }
            if let (Some(setup), Some(g)) = (renderer_setup, gpu) {
                let r = doc
                    .island
                    .renderer_mut()
                    .expect("an island made with a device has its renderer");
                setup(&g.device, &g.queue, r);
            }
            ctx_out = Some(ctx);
            doc
        };
        let hooks = Hooks {
            clipboard: Some(Box::new(|d: &mut IslandDoc<S>, text: &str| {
                d.clipboard.set_text(ClipboardKind::Clipboard, text);
            })),
            textures: Some(Box::new(|d: &mut IslandDoc<S>, g: &mut GpuRender| render_island(d, g))),
            before_step: pass_every_step.then(|| Box::new(|d: &mut IslandDoc<S>| d.request_pass()) as Box<dyn FnMut(&mut IslandDoc<S>)>),
            settled: pass_every_step.then(|| Box::new(|d: &IslandDoc<S>, now: Time| d.settled(now)) as Box<dyn Fn(&IslandDoc<S>, Time) -> bool>),
            one_event_per_step: pass_every_step,
        };
        let mut inner = base.build_with(make_doc, hooks, gpu);
        let waker = inner.platform_mut().cx().waker();
        inner.state().set_waker(waker);
        inner.run_ok();
        // kittest's build checks accessible names too (`with_accessibility_check`)
        if inner.check_accessibility {
            inner.check_accessibility();
        }
        EguiHarness {
            inner,
            ctx: ctx_out.expect("the document was made"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_harness_renders_predictably_unless_told_otherwise() {
        let b = EguiHarnessBuilder::default();
        assert!(!b.renderer.dithering && b.renderer.predictable_texture_filtering, "kittest's PREDICTABLE");
        let b = b.with_renderer_options(egui_wgpu::RendererOptions::default());
        assert!(b.renderer.dithering);
    }
}
