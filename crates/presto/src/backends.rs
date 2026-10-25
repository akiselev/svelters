//! The seams implemented for the real crates: [`CoreDocument`] wraps `presto_core::Document`,
//! [`CompositorHandle`] is a [`CompositorApi`], [`UiFrame`] adapts `presto_frame::UiFrameState`, and
//! [`compositor`] builds the compositor once the platform's clock and waker exist.

use crate::app::{SpawnCompositor, ToUiSink};
use crate::seams::{CompositorApi, Dispatched, DocumentApi};
use presto_base::accesskit::ActionRequest;
use presto_base::anim::AnimationEvent;
use presto_base::commit::{Commit, FastCommit};
use presto_base::event::{InputEnvelope, WindowEvent};
use presto_base::geom::{DeviceSize, Point, Rect, Scale, Size, Vec2};
use presto_base::ids::{FrameId, PropNodeId, Revision, WindowId};
use presto_base::input::InputPurpose;
use presto_base::text::GlyphRasterizer;
use presto_base::time::{Clock, Span, Time};
use presto_compositor::{Compositor, CompositorConfig, CompositorHandle};
use presto_core::{Document, JsEvent, KindId};
use presto_frame::{CommitReport, UiFrameState, Urgency};
use presto_platform::SurfaceTarget;
use std::sync::Arc;

/// `presto_core::Document` behind [`DocumentApi`]. It opens each window's root with `root_kind` and
/// keeps the JS-facing events of the last dispatch for the script host ([`CoreDocument::take_js_events`]).
pub struct CoreDocument {
    pub doc: Document,
    root_kind: KindId,
    js_events: Vec<JsEvent>,
}

impl CoreDocument {
    pub fn new(doc: Document, root_kind: KindId) -> Self {
        Self {
            doc,
            root_kind,
            js_events: Vec::new(),
        }
    }
    /// A document whose built-in kinds have their services: the text system, the platform's interaction
    /// policy and the theme (`presto_widgets::host::install`). Text editing also needs presto-widgets'
    /// text services, which the app installs beside these (see the TODO in `App::init`).
    pub fn with_widgets(
        mut doc: Document,
        root_kind: KindId,
        text: presto_text::TextSystem,
        policy: presto_widgets::interact::Policy,
        theme: presto_widgets::theme::Theme,
    ) -> Self {
        presto_widgets::host::install(&mut doc, text, policy, theme);
        Self::new(doc, root_kind)
    }
    /// The JS-facing events of the last dispatch, then those queued outside dispatch (focus moves,
    /// hover enter/leave, AccessKit actions).
    pub fn take_js_events(&mut self) -> Vec<JsEvent> {
        let mut out = std::mem::take(&mut self.js_events);
        out.extend(self.doc.take_events());
        out
    }
}

impl DocumentApi for CoreDocument {
    fn dispatch(&mut self, ev: &InputEnvelope) -> Dispatched {
        let d = self.doc.dispatch(ev);
        let js_events = d.events.len();
        self.js_events.extend(d.events);
        Dispatched {
            dirty: d.changed,
            js_events,
            fast: self.doc.take_fast_commits(),
        }
    }
    fn frame(&mut self, window: WindowId, target: Time) -> Option<Commit> {
        self.doc.frame(window, target)
    }
    fn needs_frame(&self, window: WindowId) -> bool {
        self.doc.needs_frame(window)
    }
    fn hover(&mut self, window: WindowId, pos: Point) -> bool {
        self.doc.hover(window, pos)
    }
    fn ime_state(&mut self, window: WindowId) -> Option<(Rect, InputPurpose)> {
        self.doc.ime_state(window)
    }
    fn resize(&mut self, window: WindowId, size: Size, scale: Scale) {
        if self.doc.window_root(window).is_some() {
            self.doc.resize_window(window, size, scale);
        } else {
            self.doc.create_window(window, self.root_kind, size, scale);
        }
    }
    fn close_window(&mut self, window: WindowId) {
        self.doc.close_window(window);
    }
    fn scroll_moved(&mut self, window: WindowId, node: PropNodeId, offset: Vec2) {
        self.doc.apply_scroll(window, node, offset);
    }
    fn animation_event(&mut self, _window: WindowId, ev: AnimationEvent) {
        self.doc.animation_event(ev);
    }
    fn accessibility_action(&mut self, window: WindowId, req: &ActionRequest) -> bool {
        self.doc.accessibility_action(window, req)
    }
    fn accessibility_activated(&mut self, window: WindowId) {
        self.doc.a11y_full_tree(window);
    }
    fn window_event(&mut self, window: WindowId, ev: &WindowEvent) {
        self.doc.window_event(window, ev);
    }
    fn device_reset(&mut self) {
        self.doc.device_reset();
    }
}

impl CompositorApi for CompositorHandle {
    fn add_window(
        &self,
        window: WindowId,
        target: SurfaceTarget,
        size: DeviceSize,
        scale: Scale,
    ) -> Result<(), String> {
        let unsafe_target = match target {
            SurfaceTarget::Raw { window, display } => wgpu::SurfaceTargetUnsafe::RawHandle {
                raw_display_handle: Some(display),
                raw_window_handle: window,
            },
            #[cfg(windows)]
            SurfaceTarget::CompositionVisual(v) => wgpu::SurfaceTargetUnsafe::CompositionVisual(v),
            #[cfg(target_os = "macos")]
            SurfaceTarget::CoreAnimationLayer(l) => {
                wgpu::SurfaceTargetUnsafe::CoreAnimationLayer(l)
            }
            other => return Err(format!("no surface for {other:?}")),
        };
        // SAFETY: the platform's handles stay valid while the window lives; the compositor drops the
        // surface in `remove_window`, which the app sends before the platform closes the window.
        let surface = unsafe { self.instance().create_surface_unsafe(unsafe_target) }
            .map_err(|e| e.to_string())?;
        CompositorHandle::add_window(self, window, surface, size, scale);
        Ok(())
    }
    fn remove_window(&self, window: WindowId) {
        CompositorHandle::remove_window(self, window);
    }
    fn commit(&self, c: Commit) {
        CompositorHandle::commit(self, c);
    }
    fn fast(&self, f: FastCommit) {
        CompositorHandle::fast(self, f);
    }
    fn resize(&self, window: WindowId, size: DeviceSize, scale: Scale, seq: u64) {
        CompositorHandle::resize(self, window, size, scale, seq);
    }
    fn set_visible(&self, window: WindowId, visible: bool) {
        CompositorHandle::set_visible(self, window, visible);
    }
    fn request_ui_frame(&self, window: WindowId) {
        CompositorHandle::needs_ui_frame(self, window, true);
    }
    fn high_rate_input(&self, window: WindowId) {
        CompositorHandle::high_rate_input(self, window);
    }
    fn ui_cost(&self, window: WindowId, frame: FrameId, cost: Span, input: bool) {
        if input {
            CompositorHandle::ui_input_cost(self, window, frame, cost);
        } else {
            CompositorHandle::ui_cost(self, window, frame, cost);
        }
    }
    fn ui_frame_skipped(&self, window: WindowId, frame: FrameId) {
        CompositorHandle::ui_frame_skipped(self, window, frame);
    }
    fn frame_args(&self, window: WindowId) -> Option<presto_frame::FrameArgs> {
        CompositorHandle::frame_args(self, window)
    }
    fn shutdown(&self) {
        self.clone().shutdown();
    }
}

/// The real compositor, spawned in `init` with the platform's clock: `config` builds its
/// configuration from that clock (renderer factory, sampler, refresh).
pub fn compositor(
    config: impl FnOnce(Arc<dyn Clock>) -> CompositorConfig + 'static,
    glyphs: Arc<dyn GlyphRasterizer>,
) -> SpawnCompositor<CompositorHandle> {
    Box::new(move |sink: ToUiSink, clock: Arc<dyn Clock>| {
        let (to_ui, wake_ui) = sink.callbacks();
        Compositor::spawn(config(clock), glyphs, to_ui, wake_ui)
    })
}

/// One window's `UiFrameState`, with the document's commit revisions mapped onto its own (the
/// compositor's `CommitApplied` names the document's).
#[derive(Default)]
pub struct UiFrame {
    pub state: UiFrameState,
    /// (document revision, frame-state revision) of the commit in flight.
    in_flight: Option<(Revision, Revision)>,
    /// A `BeginFrame` was refused while a commit was in flight: ask again once it is applied.
    retry: bool,
}

impl UiFrame {
    pub fn new() -> Self {
        Self::default()
    }
    /// The scene is dirty; `true` when the compositor must be asked for a frame.
    pub fn mark_dirty(&mut self) -> bool {
        self.state.mark_dirty()
    }
    pub fn is_dirty(&self) -> bool {
        self.state.is_dirty()
    }
    /// Discrete input dirtied the scene: run now (the urgent frame is started) or wait. With `Wait`,
    /// the flag says whether the compositor must be asked for a frame (the request is a level).
    pub fn urgent(&mut self, now: Time) -> (Urgency, bool) {
        // Marked here, not only inside `on_urgent_input`, to learn whether this is a new request.
        let notify = self.state.mark_dirty();
        match self.state.on_urgent_input(now) {
            Urgency::RunNow { target } if self.state.start_urgent(now, target) => {
                (Urgency::RunNow { target }, false)
            }
            _ => (Urgency::Wait, notify),
        }
    }
    pub fn begin(&mut self, now: Time, frame: FrameId, target: Time, deadline: Time) -> bool {
        let ok = self.state.on_begin_frame(now, frame, target, deadline);
        if !ok {
            self.retry = true;
        }
        ok
    }
    pub fn committed(&mut self, now: Time, doc_revision: Revision) -> Option<CommitReport> {
        let r = self.state.commit_sent(now)?;
        self.in_flight = Some((doc_revision, r.revision));
        Some(r)
    }
    pub fn skipped(&mut self) {
        self.state.frame_skipped();
    }
    /// The compositor applied the document's `revision`; `true` when a frame must be asked for again.
    pub fn applied(&mut self, revision: Revision) -> bool {
        let Some((doc, ui)) = self.in_flight else {
            return false;
        };
        if revision < doc {
            return false;
        }
        self.in_flight = None;
        let again = self.state.commit_applied(ui);
        again || (std::mem::take(&mut self.retry) && self.state.is_dirty())
    }
}
