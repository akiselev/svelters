//! The seams implemented for the real crates: [`CoreDocument`] wraps `presto_core::Document`,
//! [`CompositorHandle`] is a [`CompositorApi`], [`UiFrame`] adapts `presto_frame::UiFrameState`, and
//! [`compositor`] builds the compositor once the platform's clock and waker exist.

use crate::app::{SpawnCompositor, ToUiSink};
use crate::seams::{CompositorApi, Dispatched, DocumentApi};
use presto_base::accesskit::{Action, ActionRequest};
use presto_base::anim::AnimationEvent;
use presto_base::commit::{Commit, FastCommit};
use presto_base::event::{InputEnvelope, WindowEvent};
use presto_base::geom::{DeviceSize, Scale, Size, Vec2};
use presto_base::ids::{FrameId, NodeId, PropNodeId, Revision, WindowId};
use presto_base::text::GlyphRasterizer;
use presto_base::time::{Clock, Span, Time};
use presto_compositor::{Compositor, CompositorConfig, CompositorHandle};
use presto_core::{Document, FocusReason, JsEvent, KindId};
use presto_frame::{CommitReport, UiFrameState, Urgency};
use presto_platform::SurfaceTarget;
use std::collections::HashMap;
use std::sync::Arc;

/// `presto_core::Document` behind [`DocumentApi`]. It opens each window's root with `root_kind` and
/// keeps the JS-facing events of the last dispatch for the script host ([`CoreDocument::take_js_events`]).
///
/// Where `Document` has no API yet, this says so rather than pretending:
/// - `needs_frame` compares the document's revision with the one each window last ran a frame at, so
///   any transaction (even one that changed nothing) asks for a frame, which then commits nothing;
/// - animation events, window focus/theme, device resets and AT activation are not consumed;
/// - AccessKit actions: `Focus` focuses the node whose id equals the AccessKit id; others are unhandled.
pub struct CoreDocument {
    pub doc: Document,
    root_kind: KindId,
    framed: HashMap<WindowId, Revision>,
    js_events: Vec<JsEvent>,
}

impl CoreDocument {
    pub fn new(doc: Document, root_kind: KindId) -> Self {
        Self {
            doc,
            root_kind,
            framed: HashMap::new(),
            js_events: Vec::new(),
        }
    }
    /// The JS-facing events of the last dispatch, then those queued outside dispatch (focus moves).
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
            fast: Vec::new(),
        }
    }
    fn frame(&mut self, window: WindowId, target: Time) -> Option<Commit> {
        let c = self.doc.frame(window, target);
        self.framed.insert(window, self.doc.revision());
        c
    }
    fn needs_frame(&self, window: WindowId) -> bool {
        self.doc.window_root(window).is_some()
            && self
                .framed
                .get(&window)
                .is_none_or(|r| self.doc.revision() > *r)
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
        self.framed.remove(&window);
    }
    fn scroll_moved(&mut self, window: WindowId, node: PropNodeId, offset: Vec2) {
        self.doc.apply_scroll(window, node, offset);
    }
    fn animation_event(&mut self, _window: WindowId, _ev: AnimationEvent) {}
    fn accessibility_action(&mut self, _window: WindowId, req: &ActionRequest) -> bool {
        let node = NodeId(req.target_node.0);
        match req.action {
            Action::Focus if self.doc.contains(node) => {
                self.doc.focus(node, FocusReason::Programmatic);
                true
            }
            _ => false,
        }
    }
    fn accessibility_activated(&mut self, _window: WindowId) {}
    fn window_event(&mut self, _window: WindowId, _ev: &WindowEvent) {}
    fn device_reset(&mut self) {}
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
