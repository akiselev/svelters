//! The seams implemented for the real crates: [`CoreDocument`] wraps `skelter_core::Document`,
//! [`CompositorHandle`] is a [`CompositorApi`], [`UiFrame`] adapts `skelter_frame::UiFrameState`, and
//! [`compositor`] builds the compositor once the platform's clock and waker exist.

use crate::app::{SpawnCompositor, ToUiSink};
use crate::seams::{CompositorApi, Dispatched, DocumentApi};
use skelter_base::accesskit::ActionRequest;
use skelter_base::anim::AnimationEvent;
use skelter_base::commit::{Commit, FastCommit};
use skelter_base::event::{InputEnvelope, WindowEvent};
use skelter_base::geom::{DeviceSize, Point, Rect, Scale, Size, Vec2};
use skelter_base::ids::{FrameId, PropNodeId, Revision, WindowId};
use skelter_base::input::InputPurpose;
use skelter_base::text::GlyphRasterizer;
use skelter_base::time::{Clock, Span, Time};
use skelter_compositor::{Compositor, CompositorConfig, CompositorHandle};
use skelter_core::{Document, JsEvent, KindId};
use skelter_frame::{CommitReport, UiFrameState, Urgency};
use skelter_platform::SurfaceTarget;
use std::sync::Arc;

/// `skelter_core::Document` behind [`DocumentApi`]. It opens each window's root with `root_kind` and
/// keeps the JS-facing events of the last dispatch for the script host ([`CoreDocument::take_js_events`]).
pub struct CoreDocument {
    pub doc: Document,
    root_kind: KindId,
    js_events: Vec<JsEvent>,
    caret_blink: Option<Option<skelter_compositor::CaretBlink>>,
    /// The native menu bar last handed to the platform (`None`: none sent).
    menu_sent: Option<skelter_platform::MenuDesc>,
    /// The windows opened in the document, in order.
    windows: Vec<WindowId>,
    taps: Vec<EventTap>,
}

/// Sees every dispatched input and the JS-facing events its dispatch produced, before the script host
/// does ([`CoreDocument::add_event_tap`]): document services that react to core's events outside the
/// kinds, e.g. `skelter_egui_island::IslandHost::bridge_events` (in-app drags into egui islands).
pub type EventTap = Box<dyn FnMut(&mut Document, &InputEnvelope, &[JsEvent])>;

/// skelter-widgets' `menu-bar` kind.
const MENU_BAR: KindId = KindId::of("menu-bar");

impl CoreDocument {
    pub fn new(doc: Document, root_kind: KindId) -> Self {
        Self {
            doc,
            root_kind,
            js_events: Vec::new(),
            caret_blink: None,
            menu_sent: None,
            windows: Vec::new(),
            taps: Vec::new(),
        }
    }
    /// Calls `tap` after every dispatch with the input and its JS-facing events (in the order added).
    /// An app with egui islands adds `skelter_egui_island::IslandHost::event_tap`, so in-app drags reach
    /// the islands.
    pub fn add_event_tap(&mut self, tap: impl FnMut(&mut Document, &InputEnvelope, &[JsEvent]) + 'static) {
        self.taps.push(Box::new(tap));
    }
    /// A document whose built-in kinds have their services: the text system, the platform's interaction
    /// policy and the theme (`skelter_widgets::host::install`). The clipboard service follows at
    /// `App::init` (`DocumentApi::install_clipboard`), once the platform's display is known.
    pub fn with_widgets(
        mut doc: Document,
        root_kind: KindId,
        text: skelter_text::TextSystem,
        policy: skelter_widgets::interact::Policy,
        theme: skelter_widgets::theme::Theme,
    ) -> Self {
        // GTK's one app-wide text selection follows the policy.
        doc.set_single_selection(policy.one_selection_owner);
        let tp = skelter_widgets::text::TextPolicy::from_policy(
            skelter_widgets::text::Platform::current(),
            &policy,
        );
        let blink = (tp.caret_blink_ms > 0).then(|| skelter_compositor::CaretBlink {
            cycle: std::time::Duration::from_millis(tp.caret_blink_ms as u64),
            timeout: tp
                .caret_blink_timeout_ms
                .map(|ms| std::time::Duration::from_millis(ms as u64)),
        });
        skelter_widgets::host::install(&mut doc, text, policy, theme);
        let mut d = Self::new(doc, root_kind);
        d.caret_blink = Some(blink);
        d
    }
    /// The `menu-bar` nodes of every window, in tree order.
    fn menu_bars(&self) -> Vec<skelter_core::skelter_base::ids::NodeId> {
        let mut out = Vec::new();
        let mut stack: Vec<_> = self.windows_roots();
        while let Some(n) = stack.pop() {
            if self.doc.kind_of(n) == Some(MENU_BAR) {
                out.push(n);
            }
            stack.extend(self.doc.children(n).iter().rev().copied());
        }
        out
    }
    fn windows_roots(&self) -> Vec<skelter_core::skelter_base::ids::NodeId> {
        self.windows
            .iter()
            .filter_map(|w| self.doc.window_root(*w))
            .collect()
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
        for tap in &mut self.taps {
            tap(&mut self.doc, ev, &d.events);
        }
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
    fn take_fast_commits(&mut self) -> Vec<FastCommit> {
        self.doc.take_fast_commits()
    }
    fn caret_blink(&self) -> Option<Option<skelter_compositor::CaretBlink>> {
        self.caret_blink
    }
    fn menu_activated(&mut self, id: &str) -> bool {
        // The pick goes to the menu bar kind as `menu-activated(id)` (it gets its EventCx there);
        // the bar's new check state reaches the platform with the next `take_menu_bar`.
        for node in self.menu_bars() {
            let Some(method) = self
                .doc
                .registry()
                .get(MENU_BAR)
                .and_then(|e| e.spec.method_named("menu-activated"))
            else {
                return false;
            };
            if let Ok((_, events)) =
                self.doc
                    .call(node, method, &[skelter_core::Value::Str(id.into())])
            {
                self.js_events.extend(events);
                return true;
            }
        }
        false
    }
    fn take_menu_bar(&mut self) -> Option<Option<skelter_platform::MenuDesc>> {
        let desc = self.menu_bars().into_iter().find_map(|n| {
            self.doc.with_kind(n, |k| {
                k.as_any()
                    .and_then(|a| a.downcast_ref::<skelter_widgets::views::menu::MenuBar>())
                    .map(|b| b.to_menu_desc())
            })?
        });
        if desc == self.menu_sent {
            return None;
        }
        self.menu_sent = desc.clone();
        Some(desc)
    }
    fn menu_bar_native(&mut self, native: bool) {
        let Some(prop) = self
            .doc
            .registry()
            .get(MENU_BAR)
            .and_then(|e| e.spec.prop_named("native-bar"))
        else {
            return;
        };
        for node in self.menu_bars() {
            if self.doc.get_prop(node, prop) != Some(skelter_core::Value::Bool(native)) {
                let _ = self
                    .doc
                    .set_prop(node, prop, skelter_core::Value::Bool(native));
            }
        }
    }
    fn install_clipboard(&mut self, clipboard: Box<dyn skelter_platform::Clipboard>) {
        // skelter-widgets' text kinds read it as `cx.service::<Box<dyn skelter_platform::Clipboard>>()`.
        self.doc.set_service(clipboard);
    }
    fn resize(&mut self, window: WindowId, size: Size, scale: Scale) {
        if self.doc.window_root(window).is_some() {
            self.doc.resize_window(window, size, scale);
        } else {
            self.doc.create_window(window, self.root_kind, size, scale);
            self.windows.push(window);
        }
    }
    fn close_window(&mut self, window: WindowId) {
        self.doc.close_window(window);
        self.windows.retain(|w| *w != window);
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
    fn set_refresh(&self, window: WindowId, interval: Span) {
        CompositorHandle::set_refresh(self, window, interval);
    }
    fn set_caret_blink(&self, window: WindowId, blink: Option<skelter_compositor::CaretBlink>) {
        CompositorHandle::set_caret_blink(self, window, blink);
    }
    fn frame_args(&self, window: WindowId) -> Option<skelter_frame::FrameArgs> {
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
    Box::new(
        move |sink: ToUiSink, clock: Arc<dyn Clock>, monitors: &[skelter_platform::Monitor]| {
            let (to_ui, wake_ui) = sink.callbacks();
            let mut cfg = config(clock);
            if let Some(m) = monitors.first() {
                cfg.ppi = logical_ppi(m.scale);
            }
            Compositor::spawn(cfg, glyphs, to_ui, wake_ui)
        },
    )
}

/// A monitor's pixels per inch for fling physics. `Monitor` carries no physical size, so this is the
/// OS's logical DPI convention times the scale: 72 on macOS (points), 96 elsewhere.
fn logical_ppi(scale: Scale) -> f32 {
    let base = if cfg!(target_os = "macos") {
        72.0
    } else {
        96.0
    };
    base * scale.0
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
