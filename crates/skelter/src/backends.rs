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
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

/// `skelter_core::Document` behind [`DocumentApi`]. It opens each window's root with `root_kind` and
/// keeps the JS-facing events of the last dispatch for the script host ([`CoreDocument::take_js_events`]).
pub struct CoreDocument<H: DocHandle = Document> {
    /// The document: owned, or shared with the script host ([`CoreDocument::shared`]).
    pub doc: H,
    root_kind: KindId,
    js_events: Vec<JsEvent>,
    /// The built-in kinds' services are installed: the caret blink follows the document's policy.
    widgets: bool,
    /// The native menu bar last handed to the platform (`None`: none sent).
    menu_sent: Option<skelter_platform::MenuDesc>,
    /// The windows opened in the document, in order.
    windows: Vec<WindowId>,
    taps: Vec<EventTap>,
    pre_taps: Vec<PreDispatchTap>,
}

/// Sees every dispatched input and the JS-facing events its dispatch produced, before the script host
/// does ([`CoreDocument::add_event_tap`]): document services that react to core's events outside the
/// kinds, e.g. `skelter_egui_island::IslandHost::bridge_events` (in-app drags into egui islands).
pub type EventTap = Box<dyn FnMut(&mut Document, &InputEnvelope, &[JsEvent])>;

/// What a pre-dispatch tap ([`CoreDocument::add_pre_dispatch_tap`]) sees. Fields are added over time
/// (as `skelter_core::KeyHookCx`'s), so it is built only here.
#[non_exhaustive]
pub struct PreDispatchCx<'a> {
    /// The document, before the input reaches it.
    pub doc: &'a mut Document,
    pub input: &'a InputEnvelope,
}

/// Sees every input before skelter-core dispatches it; returning true consumes it (no dispatch).
pub type PreDispatchTap = Box<dyn FnMut(&mut PreDispatchCx<'_>) -> bool>;

/// skelter-widgets' `menu-bar` kind.
const MENU_BAR: KindId = KindId::of("menu-bar");

/// How [`CoreDocument`] holds its document: owned (`Document`) or shared (`Rc<RefCell<Document>>`,
/// the one `svelters_skelter::SkelterBackend::shared` holds). A shared document is borrowed for each
/// call only, so the script host can borrow it between calls.
pub trait DocHandle: 'static {
    type Ref<'a>: std::ops::Deref<Target = Document>
    where
        Self: 'a;
    type Mut<'a>: std::ops::DerefMut<Target = Document>
    where
        Self: 'a;
    fn lend(&self) -> Self::Ref<'_>;
    fn lend_mut(&mut self) -> Self::Mut<'_>;
}

impl DocHandle for Document {
    type Ref<'a> = &'a Document;
    type Mut<'a> = &'a mut Document;
    fn lend(&self) -> &Document {
        self
    }
    fn lend_mut(&mut self) -> &mut Document {
        self
    }
}

impl DocHandle for Rc<RefCell<Document>> {
    type Ref<'a> = std::cell::Ref<'a, Document>;
    type Mut<'a> = std::cell::RefMut<'a, Document>;
    fn lend(&self) -> std::cell::Ref<'_, Document> {
        self.borrow()
    }
    fn lend_mut(&mut self) -> std::cell::RefMut<'_, Document> {
        self.borrow_mut()
    }
}

impl CoreDocument {
    pub fn new(doc: Document, root_kind: KindId) -> Self {
        Self::over(doc, root_kind)
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
        skelter_widgets::host::install(&mut doc, text, policy, theme);
        let mut d = Self::new(doc, root_kind);
        d.widgets = true;
        d
    }
}

/// The caret blink a policy asks for (its text policy's cycle and timeout); `None`: a steady caret.
pub fn caret_blink_of(policy: &skelter_widgets::interact::Policy) -> Option<skelter_compositor::CaretBlink> {
    let tp = skelter_widgets::text::TextPolicy::from_policy(skelter_widgets::text::Platform::current(), policy);
    (tp.caret_blink_ms > 0).then(|| skelter_compositor::CaretBlink {
        cycle: std::time::Duration::from_millis(tp.caret_blink_ms as u64),
        timeout: tp.caret_blink_timeout_ms.map(|ms| std::time::Duration::from_millis(ms as u64)),
    })
}

impl CoreDocument<Rc<RefCell<Document>>> {
    /// Over a document shared with the script host (`SkelterBackend::shared`), so embedders don't
    /// re-implement [`DocumentApi`].
    pub fn shared(doc: Rc<RefCell<Document>>, root_kind: KindId) -> Self {
        Self::over(doc, root_kind)
    }
}

impl<H: DocHandle> CoreDocument<H> {
    pub fn over(doc: H, root_kind: KindId) -> Self {
        Self {
            doc,
            root_kind,
            js_events: Vec::new(),
            widgets: false,
            menu_sent: None,
            windows: Vec::new(),
            taps: Vec::new(),
            pre_taps: Vec::new(),
        }
    }
    /// Calls `tap` after every dispatch with the input and its JS-facing events (in the order added).
    /// An app with egui islands adds `skelter_egui_island::IslandHost::event_tap`, so in-app drags reach
    /// the islands.
    pub fn add_event_tap(&mut self, tap: impl FnMut(&mut Document, &InputEnvelope, &[JsEvent]) + 'static) {
        self.taps.push(Box::new(tap));
    }
    /// Calls `tap` before every dispatch, in the order added, until one returns true: that tap
    /// consumed the input, so skelter-core never dispatches it (no hit test, native default or JS
    /// event, and the [`add_event_tap`](Self::add_event_tap) taps do not run). The twin of
    /// [`add_event_tap`](Self::add_event_tap), for host services that must see an input first.
    /// Pointer and wheel events after hit testing have `skelter_core::Document::add_input_filter`.
    pub fn add_pre_dispatch_tap(&mut self, tap: impl FnMut(&mut PreDispatchCx<'_>) -> bool + 'static) {
        self.pre_taps.push(Box::new(tap));
    }
    /// The `menu-bar` nodes of every window, in tree order.
    fn menu_bars(&self) -> Vec<skelter_core::skelter_base::ids::NodeId> {
        let mut out = Vec::new();
        let mut stack: Vec<_> = self.windows_roots();
        while let Some(n) = stack.pop() {
            if self.doc.lend().kind_of(n) == Some(MENU_BAR) {
                out.push(n);
            }
            stack.extend(self.doc.lend().children(n).iter().rev().copied());
        }
        out
    }
    fn windows_roots(&self) -> Vec<skelter_core::skelter_base::ids::NodeId> {
        self.windows
            .iter()
            .filter_map(|w| self.doc.lend().window_root(*w))
            .collect()
    }
    /// The JS-facing events of the last dispatch, then those queued outside dispatch (focus moves,
    /// hover enter/leave, AccessKit actions).
    pub fn take_js_events(&mut self) -> Vec<JsEvent> {
        let mut out = std::mem::take(&mut self.js_events);
        out.extend(self.doc.lend_mut().take_events());
        out
    }
}

impl<H: DocHandle> DocumentApi for CoreDocument<H> {
    fn dispatch(&mut self, ev: &InputEnvelope) -> Dispatched {
        if !self.pre_taps.is_empty() {
            let mut doc = self.doc.lend_mut();
            let before = doc.revision();
            let mut cx = PreDispatchCx { doc: &mut doc, input: ev };
            if self.pre_taps.iter_mut().any(|tap| tap(&mut cx)) {
                let dirty = doc.revision() != before;
                let fast = doc.take_fast_commits();
                return Dispatched { dirty, js_events: 0, fast };
            }
        }
        let d = self.doc.lend_mut().dispatch(ev);
        let js_events = d.events.len();
        let mut doc = self.doc.lend_mut();
        for tap in &mut self.taps {
            tap(&mut doc, ev, &d.events);
        }
        let fast = doc.take_fast_commits();
        drop(doc);
        self.js_events.extend(d.events);
        Dispatched {
            dirty: d.changed,
            js_events,
            fast,
        }
    }
    fn frame(&mut self, window: WindowId, target: Time) -> Option<Commit> {
        self.doc.lend_mut().frame(window, target)
    }
    fn needs_frame(&self, window: WindowId) -> bool {
        self.doc.lend().needs_frame(window)
    }
    fn hover(&mut self, window: WindowId, pos: Point) -> bool {
        self.doc.lend_mut().hover(window, pos)
    }
    fn hover_at(&mut self, window: WindowId, pos: Point, now: Time) -> bool {
        self.doc.lend_mut().hover_at(window, pos, now)
    }
    fn ime_state(&mut self, window: WindowId) -> Option<(Rect, InputPurpose)> {
        self.doc.lend_mut().ime_state(window)
    }
    fn take_fast_commits(&mut self) -> Vec<FastCommit> {
        self.doc.lend_mut().take_fast_commits()
    }
    fn reduced_motion(&self) -> Option<bool> {
        Some(self.doc.lend().media_preferences().reduced_motion)
    }
    fn caret_blink(&self) -> Option<Option<skelter_compositor::CaretBlink>> {
        // read on use: `skelter_widgets::host::set_policy` reaches open windows (SV62)
        if !self.widgets {
            return None;
        }
        let doc = self.doc.lend();
        doc.service_ref::<skelter_widgets::interact::Policy>().map(caret_blink_of)
    }
    fn menu_activated(&mut self, id: &str) -> bool {
        // The pick goes to the menu bar kind as `menu-activated(id)` (it gets its EventCx there);
        // the bar's new check state reaches the platform with the next `take_menu_bar`.
        for node in self.menu_bars() {
            let Some(method) = self
                .doc
                .lend_mut().registry()
                .get(MENU_BAR)
                .and_then(|e| e.spec.method_named("menu-activated"))
            else {
                return false;
            };
            if let Ok((_, events)) =
                self.doc
                    .lend_mut().call(node, method, &[skelter_core::Value::Str(id.into())])
            {
                self.js_events.extend(events);
                return true;
            }
        }
        false
    }
    fn take_menu_bar(&mut self) -> Option<Option<skelter_platform::MenuDesc>> {
        let desc = self.menu_bars().into_iter().find_map(|n| {
            self.doc.lend_mut().with_kind(n, |k| {
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
            .lend_mut().registry()
            .get(MENU_BAR)
            .and_then(|e| e.spec.prop_named("native-bar"))
        else {
            return;
        };
        for node in self.menu_bars() {
            if self.doc.lend_mut().get_prop(node, prop) != Some(skelter_core::Value::Bool(native)) {
                let _ = self
                    .doc
                    .lend_mut().set_prop(node, prop, skelter_core::Value::Bool(native));
            }
        }
    }
    fn install_clipboard(&mut self, clipboard: Box<dyn skelter_platform::Clipboard>) {
        // skelter-widgets' text kinds read it as `cx.service::<Box<dyn skelter_platform::Clipboard>>()`.
        self.doc.lend_mut().set_service(clipboard);
    }
    fn resize(&mut self, window: WindowId, size: Size, scale: Scale) {
        if self.doc.lend_mut().window_root(window).is_some() {
            self.doc.lend_mut().resize_window(window, size, scale);
        } else {
            self.doc.lend_mut().create_window(window, self.root_kind, size, scale);
            self.windows.push(window);
        }
    }
    fn close_window(&mut self, window: WindowId) {
        self.doc.lend_mut().close_window(window);
        self.windows.retain(|w| *w != window);
    }
    fn scroll_moved(&mut self, window: WindowId, node: PropNodeId, offset: Vec2) {
        self.doc.lend_mut().apply_scroll(window, node, offset);
    }
    fn animation_event(&mut self, _window: WindowId, ev: AnimationEvent) {
        self.doc.lend_mut().animation_event(ev);
    }
    fn accessibility_action(&mut self, window: WindowId, req: &ActionRequest) -> bool {
        self.doc.lend_mut().accessibility_action(window, req)
    }
    fn accessibility_activated(&mut self, window: WindowId) {
        self.doc.lend_mut().a11y_full_tree(window);
    }
    fn window_event(&mut self, window: WindowId, ev: &WindowEvent) {
        self.doc.lend_mut().window_event(window, ev);
    }
    fn device_reset(&mut self) {
        self.doc.lend_mut().device_reset();
    }
    fn commit_applied(&mut self, window: WindowId, revision: Revision) {
        self.doc.lend_mut().commit_applied(window, revision);
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
    fn set_ppi(&self, window: WindowId, ppi: f32) {
        CompositorHandle::set_ppi(self, window, ppi);
    }
    fn set_reduced_motion(&self, window: WindowId, on: bool) {
        CompositorHandle::set_reduced_motion(self, window, on);
    }
    fn set_caret_blink(&self, window: WindowId, blink: Option<skelter_compositor::CaretBlink>) {
        CompositorHandle::set_caret_blink(self, window, blink);
    }
    fn frame_args(&self, window: WindowId) -> Option<skelter_frame::FrameArgs> {
        CompositorHandle::frame_args(self, window)
    }
    fn read_window(&self, window: WindowId) -> Option<skelter_compositor::WindowImage> {
        CompositorHandle::read_window(self, window, std::time::Duration::from_secs(10))
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
                cfg.ppi = crate::app::monitor_ppi(m);
            }
            Compositor::spawn(cfg, glyphs, to_ui, wake_ui)
        },
    )
}

/// A monitor's pixels per inch when the OS does not report its physical size: the OS's logical DPI
/// convention times the scale, 72 on macOS (points), 96 elsewhere.
pub(crate) fn logical_ppi(scale: Scale) -> f32 {
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

#[cfg(test)]
mod tests {
    use super::*;
    use skelter_base::event::{InputEvent, Modifiers, MouseButton, PointerEvent, PointerPhase, PointerType};
    use skelter_base::ids::InputSeq;
    use skelter_core::testkinds::{BoxKind, FixedText};
    use skelter_core::{KindRegistry, StyleInput};

    const W: WindowId = WindowId(1);
    const BOX: KindId = KindId::of("box");

    fn press(seq: u64) -> InputEnvelope {
        let p = PointerEvent {
            phase: PointerPhase::Down,
            pointer: 0,
            kind: PointerType::Mouse,
            button: Some(MouseButton::Primary),
            buttons: 1,
            pos: Point::new(5.0, 5.0),
            modifiers: Modifiers::default(),
            pressure: 0.5,
            tilt: Vec2::ZERO,
            history: Default::default(),
        };
        InputEnvelope { seq: InputSeq(seq), window: W, os_time: Time(seq), recv_time: Time(seq), event: InputEvent::Pointer(p) }
    }

    #[test]
    fn a_pre_dispatch_tap_sees_the_input_first_and_may_consume_it() {
        let mut r = KindRegistry::new();
        r.register::<BoxKind>().unwrap();
        let mut doc = Document::new(Arc::new(r), Box::new(FixedText));
        let root = doc.create_window(W, BOX, Size::new(100.0, 100.0), Scale(1.0));
        let a = doc.create(BOX);
        doc.set_style(a, StyleInput { inline: Some("width: 20px; height: 20px".into()), ..Default::default() });
        doc.insert(root, a, None, None).unwrap();
        doc.frame(W, Time(0));
        doc.listen(a, skelter_core::common::POINTERDOWN, skelter_core::ListenFlags::EMPTY);
        let mut core = CoreDocument::new(doc, BOX);
        let seen = Rc::new(RefCell::new(Vec::new()));
        let (s, t) = (seen.clone(), seen.clone());
        // consumes the first input only; it sees the document before dispatch (no press yet)
        core.add_pre_dispatch_tap(move |cx: &mut PreDispatchCx<'_>| {
            s.borrow_mut().push(("pre", cx.input.seq.0, cx.doc.state(a).contains(skelter_core::StateMask::PRESSED)));
            cx.input.seq.0 == 1
        });
        core.add_event_tap(move |_: &mut Document, ev: &InputEnvelope, events: &[JsEvent]| t.borrow_mut().push(("post", ev.seq.0, !events.is_empty())));
        let d = core.dispatch(&press(1));
        assert_eq!((d.dirty, d.js_events), (false, 0), "consumed: nothing dispatched");
        assert!(core.take_js_events().is_empty());
        assert!(!core.doc.state(a).contains(skelter_core::StateMask::PRESSED));
        let d = core.dispatch(&press(2));
        assert_eq!(d.js_events, 1, "the pointerdown listener's event");
        assert!(core.doc.state(a).contains(skelter_core::StateMask::PRESSED));
        assert_eq!(*seen.borrow(), [("pre", 1, false), ("pre", 2, false), ("post", 2, true)], "the post-dispatch taps skip a consumed input");
    }
}
