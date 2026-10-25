//! [`EguiIsland`]: one `egui::Context` hosted in a presto layer.
//!
//! A frame of an island is two calls on the UI thread:
//! 1. [`EguiIsland::run`] — one egui pass over the input queued since the last pass, tessellated, with its
//!    platform output turned into presto's ([`IslandOutput`]: cursor, IME area, clipboard, the grafted
//!    AccessKit update, drag-and-drop);
//! 2. [`EguiIsland::render`] — egui-wgpu's `Renderer` (texture deltas → `update_buffers` → `render`) into the
//!    island's own `Rgba8Unorm` texture on presto's device. The texture is registered with the compositor
//!    whenever it is (re)allocated and composited as a [`Prim::Surface`] (`display_list`).
//!
//! egui renders gamma-space premultiplied colour into a non-sRGB target, which is presto's blending space
//! (architecture §8.3), so the compositor samples the texture as it samples its own surfaces. The render
//! is submitted on presto's queue before the commit that shows it, so queue order makes it visible.
//!
//! Passes run only when needed ([`EguiIsland::needs_pass`]): when input reached egui, the rect or scale
//! changed, a drag is crossing the island, or egui asked for a repaint (its `repaint_delay`). An idle island
//! sets no deadline, so presto's idle barrier holds.

use crate::a11y::A11yGraft;
use crate::dnd::{DndBridge, DndOutput, DragData, PayloadCodec};
use crate::input::{self, InputTranslator, Routed};
use presto_platform::clipboard::{Clipboard, ClipboardKind};
use presto_base::cursor::CursorIcon;
use presto_base::display::{DisplayList, Prim, SurfacePrim};
use presto_base::event::{InputEnvelope, InputEvent};
use presto_base::geom::{DeviceSize, Point, Rect, Scale};
use presto_base::ids::TextureId;
use presto_base::layer::LayerKind;
use presto_base::time::{Span, Time};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

/// The island's target format: `Renderer::register_native_texture` and presto's surfaces both want it.
pub const TARGET_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// Where an island's texture is registered for compositing: the compositor, or a test's recorder.
pub trait TextureRegistry {
    /// Register (or replace) the texture composited for `id`. The island calls it whenever it
    /// (re)allocates its target, so after a resize the compositor samples the new texture.
    fn register_texture(&self, id: TextureId, texture: wgpu::Texture);
    fn unregister_texture(&self, id: TextureId);
}

impl TextureRegistry for presto_compositor::CompositorHandle {
    fn register_texture(&self, id: TextureId, texture: wgpu::Texture) {
        presto_compositor::CompositorHandle::register_texture(self, id, texture);
    }
    fn unregister_texture(&self, id: TextureId) {
        presto_compositor::CompositorHandle::unregister_texture(self, id);
    }
}

pub struct IslandDesc {
    /// The id the island's texture is registered and composited under (the host allocates it).
    pub texture: TextureId,
    /// The island's AccessKit subtree ([`crate::a11y::island_tree_id`]).
    pub tree: accesskit::TreeId,
    /// The island's rect in window points.
    pub rect: Rect,
    pub scale: Scale,
    /// egui-wgpu's options (MSAA and depth for paint callbacks that draw 3D).
    pub renderer: egui_wgpu::RendererOptions,
}

/// presto-facing results of one pass.
#[derive(Debug, Default)]
pub struct IslandOutput {
    /// The cursor egui wants while the pointer is over the island.
    pub cursor: CursorIcon,
    /// egui's IME request: the text field's rect and the caret in window points (`None`: IME off).
    pub ime: Option<ImeArea>,
    /// The AccessKit update of the island's subtree (only with accessibility on).
    pub a11y: Option<accesskit::TreeUpdate>,
    /// Text egui copied (also written to the clipboard when one is set).
    pub copied: Vec<String>,
    /// Links egui asked to open (`OpenUrl` commands): `(url, new_tab)`.
    pub open_urls: Vec<(String, bool)>,
    pub dnd: DndOutput,
    /// When egui wants its next pass without new input (`None`: not until input arrives).
    pub repaint_at: Option<Time>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImeArea {
    pub rect: Rect,
    pub cursor: Rect,
    pub interrupt: bool,
}

struct Target {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    size: DeviceSize,
}

/// One pass's tessellated output, waiting for [`EguiIsland::render`].
struct Pending {
    jobs: Vec<egui::ClippedPrimitive>,
    textures: egui::TexturesDelta,
    pixels_per_point: f32,
}

impl Drop for Pending {
    /// Rendered deltas were applied; an island dropped before its render has nothing left to apply them
    /// to (epaint asserts on unapplied deltas).
    fn drop(&mut self) {
        self.textures.clear();
    }
}

pub struct EguiIsland {
    ctx: egui::Context,
    /// Created with the device ([`EguiIsland::with_device`]) or at the first render.
    renderer: Option<egui_wgpu::Renderer>,
    renderer_options: egui_wgpu::RendererOptions,
    texture_id: TextureId,
    target: Option<Target>,
    rect: Rect,
    scale: Scale,
    input: InputTranslator,
    events: Vec<egui::Event>,
    pending: Option<Pending>,
    /// The next pass is due at this time (`None`: wait for input).
    pass_at: Option<Time>,
    graft: A11yGraft,
    /// The host set its own placement for the AccessKit subtree (else it follows the rect).
    a11y_placement_set: bool,
    dnd: DndBridge,
    clipboard: Option<Box<dyn Clipboard>>,
    max_texture_side: usize,
    cursor: CursorIcon,
    /// egui's textures as uploaded, for a new renderer after a device reset (egui sends each texture
    /// once, as deltas).
    mirror: TextureMirror,
    /// The renderer is new (first render, or after a device reset): upload the mirror before the
    /// pending deltas.
    replay: bool,
}

impl EguiIsland {
    /// An island whose renderer is created at its first [`render`](Self::render) (passes, input and
    /// accessibility work without a device).
    pub fn new(desc: IslandDesc) -> Self {
        let mut island = EguiIsland {
            ctx: egui::Context::default(),
            renderer: None,
            renderer_options: desc.renderer,
            texture_id: desc.texture,
            target: None,
            rect: desc.rect,
            scale: desc.scale,
            input: InputTranslator::new(desc.rect),
            events: Vec::new(),
            pending: None,
            pass_at: Some(Time::ZERO),
            graft: A11yGraft::new(desc.tree),
            a11y_placement_set: false,
            dnd: DndBridge::default(),
            clipboard: None,
            // wgpu's default `max_texture_dimension_2d`, until the device says otherwise.
            max_texture_side: 8192,
            cursor: CursorIcon::Default,
            mirror: TextureMirror::default(),
            replay: false,
        };
        island.place_a11y();
        island
    }

    /// An island with its renderer on `device` (so paint callbacks can set up their resources in
    /// [`renderer_mut`](Self::renderer_mut) before the first frame).
    pub fn with_device(device: &wgpu::Device, desc: IslandDesc) -> Self {
        let mut island = EguiIsland::new(desc);
        island.init_renderer(device);
        island
    }

    fn init_renderer(&mut self, device: &wgpu::Device) -> &mut egui_wgpu::Renderer {
        self.max_texture_side = device.limits().max_texture_dimension_2d as usize;
        self.renderer
            .get_or_insert_with(|| egui_wgpu::Renderer::new(device, TARGET_FORMAT, self.renderer_options))
    }

    /// The GPU device was lost and presto recreated it (`ToUi::DeviceReset`, `Document::device_reset`). The
    /// renderer and the island's texture belonged to the old device: the next pass re-renders on the device
    /// [`render`](Self::render) is given, re-uploading egui's textures from the island's copies and
    /// registering a new texture. What lived on the old renderer is gone: paint callbacks set up their
    /// `callback_resources` again ([`renderer_mut`](Self::renderer_mut) after the next render or after
    /// [`init_renderer_on`](Self::init_renderer_on)), and native textures are registered again.
    pub fn device_reset(&mut self) {
        self.renderer = None;
        self.target = None;
        self.replay = true;
        self.pass_at = Some(Time::ZERO);
    }

    /// Creates the renderer on `device` now (after [`device_reset`](Self::device_reset), so paint callbacks
    /// can set up their resources before the next render).
    pub fn init_renderer_on(&mut self, device: &wgpu::Device) -> &mut egui_wgpu::Renderer {
        self.init_renderer(device)
    }

    /// The island's egui context: the host installs fonts, style, image loaders and plugins on it.
    pub fn context(&self) -> &egui::Context {
        &self.ctx
    }

    /// egui-wgpu's renderer: paint callbacks' shared resources live in its `callback_resources`, and
    /// native textures (an offscreen viewport shown in the island) are registered on it.
    /// `None` until the renderer exists.
    pub fn renderer_mut(&mut self) -> Option<&mut egui_wgpu::Renderer> {
        self.renderer.as_mut()
    }

    pub fn texture_id(&self) -> TextureId {
        self.texture_id
    }

    /// The compositor layer kind the host gives the island.
    pub fn layer_kind(&self) -> LayerKind {
        LayerKind::Texture(self.texture_id)
    }

    pub fn rect(&self) -> Rect {
        self.rect
    }

    pub fn scale(&self) -> Scale {
        self.scale
    }

    /// The island moved, resized or changed scale (window points). A size or scale change needs a pass
    /// (and a new texture at the next render); a move only moves the composited quad and the routing.
    pub fn set_rect(&mut self, rect: Rect, scale: Scale) {
        if rect.size() != self.rect.size() || scale != self.scale {
            self.pass_at = Some(Time::ZERO);
        }
        self.rect = rect;
        self.scale = scale;
        self.input.set_rect(rect);
        self.place_a11y();
    }

    /// Accessibility on: egui builds its AccessKit tree from the next pass on.
    pub fn enable_accesskit(&mut self) {
        self.ctx.enable_accesskit();
        self.pass_at = Some(Time::ZERO);
    }

    /// Moves the island's AccessKit subtree to `tree` (when it is mounted on another node). The next pass
    /// sends the whole tree, with its tree info, under the new id.
    pub fn set_tree_id(&mut self, tree: accesskit::TreeId) {
        if tree != self.graft.tree_id() {
            let placement = self.graft.placement();
            self.graft = A11yGraft::new(tree);
            self.graft.set_placement(placement);
            self.pass_at = Some(Time::ZERO);
        }
    }

    /// The cursor egui asked for in its last pass.
    pub fn cursor(&self) -> CursorIcon {
        self.cursor
    }

    pub fn a11y(&self) -> &A11yGraft {
        &self.graft
    }

    /// Where the island's points sit in its graft node's coordinate space, when the host's graft node
    /// isn't in window points (by default: the island's offset in window points).
    pub fn set_a11y_placement(&mut self, placement: accesskit::Affine) {
        self.a11y_placement_set = true;
        self.graft.set_placement(placement);
    }

    fn place_a11y(&mut self) {
        if !self.a11y_placement_set {
            let o = self.rect.min;
            self.graft.set_placement(accesskit::Affine::translate((f64::from(o.x), f64::from(o.y))));
        }
    }

    pub fn set_clipboard(&mut self, clipboard: Box<dyn Clipboard>) {
        self.clipboard = Some(clipboard);
    }

    pub fn set_dnd_codec(&mut self, codec: Box<dyn PayloadCodec>) {
        self.dnd.set_codec(codec);
    }

    /// Keyboard focus entered or left the island (presto's focus model owns the decision).
    pub fn set_focused(&mut self, focused: bool) {
        let before = self.events.len();
        self.input.set_focused(focused, &mut self.events);
        if self.events.len() > before {
            self.pass_at = Some(Time::ZERO);
        }
    }

    pub fn focused(&self) -> bool {
        self.input.focused()
    }

    /// A presto input event. Returns whether the island took it (the host stops dispatching it).
    pub fn handle_input(&mut self, ev: &InputEnvelope) -> bool {
        self.handle_event(&ev.event, ev.recv_time)
    }

    /// [`handle_input`](Self::handle_input) for an event that arrived at `at` (positions in window points).
    pub fn handle_event(&mut self, ev: &InputEvent, at: Time) -> bool {
        let Routed { consumed, queued, paste } = self.input.translate(ev, &mut self.events);
        if paste {
            let text = self.clipboard.as_mut().and_then(|c| c.get_text(ClipboardKind::Clipboard)).map(|t| t.replace("\r\n", "\n"));
            if let Some(text) = text.filter(|t| !t.is_empty()) {
                self.events.push(egui::Event::Paste(text));
            }
        }
        if queued {
            self.request_pass_at(at);
        }
        consumed
    }

    /// An AccessKit action from the platform adapter. Returns whether it was aimed at this island.
    pub fn handle_a11y_action(&mut self, req: accesskit::ActionRequest) -> bool {
        match self.graft.map_action(req) {
            Some(req) => {
                self.events.push(egui::Event::AccessKitActionRequest(req));
                self.pass_at = Some(Time::ZERO);
                true
            }
            None => false,
        }
    }

    /// Ask for a pass now (the host changed state the island's UI reads).
    pub fn request_pass(&mut self) {
        self.pass_at = Some(Time::ZERO);
    }

    fn request_pass_at(&mut self, t: Time) {
        self.pass_at = Some(self.pass_at.map_or(t, |p| p.min(t)));
    }

    /// A pass is due at `now`.
    pub fn needs_pass(&self, now: Time) -> bool {
        self.pass_at.is_some_and(|t| t <= now)
    }

    /// When the island next needs a pass without input (`None`: idle).
    pub fn next_wakeup(&self) -> Option<Time> {
        self.pass_at
    }

    /// A presto drag entered the island at `pos` (window points).
    pub fn drag_enter(&mut self, drag: &DragData, pos: Point) {
        self.dnd.enter(&self.ctx, drag);
        self.drag_move(pos);
    }

    /// The presto drag moved over the island.
    pub fn drag_move(&mut self, pos: Point) {
        let local = egui::pos2(pos.x - self.rect.min.x, pos.y - self.rect.min.y);
        self.events.push(egui::Event::PointerMoved(local));
        self.pass_at = Some(Time::ZERO);
    }

    /// The presto drag left the island without dropping on it.
    pub fn drag_leave(&mut self) {
        self.dnd.leave(&self.ctx);
        self.events.push(egui::Event::PointerGone);
        self.pass_at = Some(Time::ZERO);
    }

    /// The presto drag was released over the island at `pos`. The next pass delivers it; its
    /// [`DndOutput::dropped`] says whether an egui target took it.
    pub fn drag_drop(&mut self, pos: Point) {
        if !self.dnd.has_incoming() {
            return;
        }
        let local = egui::pos2(pos.x - self.rect.min.x, pos.y - self.rect.min.y);
        self.events.push(egui::Event::PointerMoved(local));
        // A release with no press: egui drop zones see `any_released`, and no widget sees a click.
        self.events.push(egui::Event::PointerButton {
            pos: local,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: self.input.egui_modifiers(),
        });
        self.dnd.drop_here();
        self.pass_at = Some(Time::ZERO);
    }

    /// The pointer that pressed inside the island (the one an exported drag continues with).
    pub fn pointer_id(&self) -> u32 {
        self.input.pointer_id()
    }

    /// presto took over the drag [`DndOutput::drag_out`] reported (`Document::start_drag`): the held
    /// pointer is presto's now, so egui sees it released and gone, which ends egui's drag in the next pass.
    pub fn drag_out_started(&mut self) {
        self.input.release_capture(&mut self.events);
        self.pass_at = Some(Time::ZERO);
    }

    /// The presto drag started from this island ([`DndOutput::drag_out`]) ended.
    pub fn drag_out_finished(&mut self, accepted: bool) {
        self.dnd.drag_out_finished(accepted);
    }

    fn raw_input(&mut self, now: Time) -> egui::RawInput {
        let size = self.rect.size();
        let mut viewports = egui::ViewportIdMap::default();
        viewports.insert(
            egui::ViewportId::ROOT,
            egui::ViewportInfo {
                native_pixels_per_point: Some(self.scale.0),
                focused: Some(self.input.focused()),
                inner_rect: Some(egui::Rect::from_min_size(
                    egui::pos2(self.rect.min.x, self.rect.min.y),
                    egui::vec2(size.w, size.h),
                )),
                ..Default::default()
            },
        );
        let files = &mut self.input.files;
        egui::RawInput {
            viewport_id: egui::ViewportId::ROOT,
            viewports,
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(size.w, size.h))),
            max_texture_side: Some(self.max_texture_side),
            time: Some(now.as_secs_f64()),
            events: std::mem::take(&mut self.events),
            hovered_files: files
                .hovered
                .iter()
                .map(|p| egui::HoveredFile { path: Some(p.clone()), ..Default::default() })
                .collect(),
            dropped_files: files
                .dropped
                .drain(..)
                .map(|p| Arc::new(PathFile(p)) as egui::DroppedFileHandle)
                .collect(),
            focused: self.input.focused(),
            ..Default::default()
        }
    }

    /// One egui pass at `now`, running `ui` (cadatomic's `PaneHost::ui` for the pane the island hosts).
    pub fn run(&mut self, now: Time, mut ui: impl FnMut(&mut egui::Ui)) -> IslandOutput {
        let raw = self.raw_input(now);
        let full = self.ctx.run_ui(raw, |root| ui(root));
        let jobs = self.ctx.tessellate(full.shapes, full.pixels_per_point);
        match &mut self.pending {
            // The previous pass was never rendered: keep its uploads (the new jobs may sample them),
            // its frees apply after the new jobs render.
            Some(p) => {
                p.textures.append(full.textures_delta);
                p.jobs = jobs;
                p.pixels_per_point = full.pixels_per_point;
            }
            None => {
                self.pending = Some(Pending { jobs, textures: full.textures_delta, pixels_per_point: full.pixels_per_point });
            }
        }

        let delay = full.viewport_output.get(&egui::ViewportId::ROOT).map_or(Duration::MAX, |v| v.repaint_delay);
        let repaint_at = repaint_time(now, delay);
        self.pass_at = repaint_at;

        let po = full.platform_output;
        self.cursor = input::cursor(po.cursor_icon);
        let mut out = IslandOutput {
            cursor: self.cursor,
            repaint_at,
            ..Default::default()
        };
        let origin = self.rect.min;
        let to_window = |r: egui::Rect| Rect::new(r.min.x + origin.x, r.min.y + origin.y, r.max.x + origin.x, r.max.y + origin.y);
        out.ime = po.ime.map(|ime| ImeArea {
            rect: to_window(ime.rect),
            cursor: to_window(ime.cursor_rect),
            interrupt: ime.should_interrupt_composition,
        });
        for cmd in po.commands {
            match cmd {
                egui::OutputCommand::CopyText(text) => {
                    if let Some(c) = &mut self.clipboard {
                        c.set_text(ClipboardKind::Clipboard, &text);
                    }
                    out.copied.push(text);
                }
                egui::OutputCommand::OpenUrl(open) => out.open_urls.push((open.url, open.new_tab)),
                // Image copies and selection-settled notices have no presto counterpart yet.
                _ => {}
            }
        }
        out.a11y = po.accesskit_update.map(|u| self.graft.graft(u));
        let leaving = self.input.captured() && !self.input.pointer_inside();
        out.dnd = self.dnd.after_pass(&self.ctx, leaving);
        out
    }

    /// The device-pixel size of the island's texture.
    pub fn target_size(&self) -> DeviceSize {
        let s = self.rect.size();
        DeviceSize {
            w: ((s.w * self.scale.0).ceil() as u32).clamp(1, self.max_texture_side as u32),
            h: ((s.h * self.scale.0).ceil() as u32).clamp(1, self.max_texture_side as u32),
        }
    }

    /// Render the last pass into the island's texture. Returns paint callbacks' own command buffers;
    /// submit them **before** `encoder`'s (eframe's order: `queue.submit(user_buffers.chain([encoder]))`).
    /// Does nothing when no pass ran since the last render.
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        registry: &dyn TextureRegistry,
    ) -> Vec<wgpu::CommandBuffer> {
        let Some(pending) = self.pending.take() else {
            return Vec::new();
        };
        self.init_renderer(device);
        let size = self.target_size();
        if self.target.as_ref().is_none_or(|t| t.size != size) {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("presto_egui_island"),
                size: wgpu::Extent3d { width: size.w, height: size.h, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: TARGET_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            registry.register_texture(self.texture_id, texture.clone());
            self.target = Some(Target { texture, view, size });
        }
        let (Some(target), Some(renderer)) = (&self.target, &mut self.renderer) else {
            return Vec::new();
        };

        if std::mem::take(&mut self.replay) {
            for (id, delta) in self.mirror.full_deltas() {
                renderer.update_texture(device, queue, id, &delta);
            }
        }
        for (id, deltas) in &pending.textures.set {
            for delta in deltas {
                renderer.update_texture(device, queue, *id, delta);
                self.mirror.apply(*id, delta);
            }
        }
        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [size.w, size.h],
            pixels_per_point: pending.pixels_per_point,
        };
        let user_buffers = renderer.update_buffers(device, queue, encoder, &pending.jobs, &screen);
        {
            let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("presto_egui_island"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &target.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            // egui-wgpu's `render` takes a `'static` pass; the pass ends at this block's end, before
            // the encoder is used again.
            let mut pass = pass.forget_lifetime();
            renderer.render(&mut pass, &pending.jobs, &screen);
        }
        for id in &pending.textures.free {
            renderer.free_texture(id);
            self.mirror.free(id);
        }
        user_buffers
    }

    /// The island's texture (after its first render).
    pub fn texture(&self) -> Option<&wgpu::Texture> {
        self.target.as_ref().map(|t| &t.texture)
    }

    /// The island node's display list: its texture over its whole box (node-local points).
    pub fn display_list(&self) -> DisplayList {
        let rect = Rect::from_origin_size(Point::ZERO, self.rect.size());
        let mut list = DisplayList::new();
        list.push(Prim::Surface(SurfacePrim { texture: self.texture_id, rect, opacity: 1.0 }), rect);
        list
    }

    /// Unregister the island's texture (before dropping the island).
    pub fn release(&mut self, registry: &dyn TextureRegistry) {
        if self.target.take().is_some() {
            registry.unregister_texture(self.texture_id);
        }
    }
}

/// CPU copies of egui's textures as they were uploaded (whole images with partial updates applied).
#[derive(Default)]
struct TextureMirror {
    images: HashMap<egui::TextureId, (Arc<egui::ColorImage>, egui::TextureOptions)>,
}

impl TextureMirror {
    fn apply(&mut self, id: egui::TextureId, delta: &egui::epaint::ImageDelta) {
        let egui::ImageData::Color(image) = &delta.image;
        match delta.pos {
            None => {
                self.images.insert(id, (image.clone(), delta.options));
            }
            Some([x, y]) => {
                let Some((full, options)) = self.images.get_mut(&id) else { return };
                let full = Arc::make_mut(full);
                let (w, [pw, ph]) = (full.size[0], image.size);
                for row in 0..ph {
                    let dst = (y + row) * w + x;
                    full.pixels[dst..dst + pw].copy_from_slice(&image.pixels[row * pw..(row + 1) * pw]);
                }
                *options = delta.options;
            }
        }
    }

    fn free(&mut self, id: &egui::TextureId) {
        self.images.remove(id);
    }

    fn full_deltas(&self) -> Vec<(egui::TextureId, egui::epaint::ImageDelta)> {
        self.images.iter().map(|(id, (image, options))| (*id, egui::epaint::ImageDelta::full(image.clone(), *options))).collect()
    }
}

/// egui's `repaint_delay` as a deadline (`Duration::MAX`: no repaint asked).
fn repaint_time(now: Time, delay: Duration) -> Option<Time> {
    (delay != Duration::MAX).then(|| now + Span::from_nanos(delay.as_nanos().min(i64::MAX as u128) as i64))
}

/// A file dropped from the OS, read on demand.
#[derive(Debug)]
struct PathFile(PathBuf);

impl egui::DroppedFile for PathFile {
    fn path(&self) -> &Path {
        &self.0
    }

    fn bytes(&self) -> Result<Vec<u8>, String> {
        std::fs::read(&self.0).map_err(|e| e.to_string())
    }
}
