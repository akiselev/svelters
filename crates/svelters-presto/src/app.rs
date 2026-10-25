//! svelters on presto's app runner (lane APP-TEST's `presto` facade): [`PrestoDocument`] is the tree
//! [`presto::App`] drives, and [`SveltersApp`] is the platform's app handler around it, which opens a
//! window for each `<window>` JS presents and keeps the host's timers and wakes on the platform loop.
//!
//! The host is attached (ABI `start`, as [`Embed`] does): the platform owns the loop, JS runs at the
//! points the backend calls the host (input delivery, timers, wakes, `requestAnimationFrame`), and
//! [`Embed::logic`] runs after every platform callback. Windows:
//!
//! 1. JS builds a `window` node and calls `present` ([`PrestoBackend::take_presents`]);
//! 2. after the callback, [`SveltersApp`] asks the platform for a window with the node's title and
//!    `default-width`/`default-height` (`App::open_window`);
//! 3. the app reports the window's size to the document, which attaches the JS-built node as its root
//!    ([`PrestoBackend::attach_presented`], presto-core's `attach_window`).
//!
//! `close` closes the platform window; a closed window frees its root's handle (`SVL_GONE_CLOSED`).

use crate::backend::PrestoBackend;
use crate::deliver::Output;
use crate::embed::Embed;
use presto::seams::{CompositorApi, Dispatched as AppDispatched, DocumentApi};
use presto::{App, AppConfig, SpawnCompositor};
use presto_base::accesskit::ActionRequest;
use presto_base::anim::AnimationEvent;
use presto_base::commit::Commit;
use presto_base::event::{InputEnvelope, WindowEvent};
use presto_base::geom::{Scale, Size, Vec2};
use presto_base::ids::{PropNodeId, Revision, WindowId};
use presto_base::time::{Clock, Span, Time};
use presto_platform::{A11yEvent, AppHandler, PlatformCx, WindowDesc};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Instant;
use svelters_abi::svl_handle;

/// Gives `backend`'s document what presto-widgets' kinds read: `text` (shaping), the OS's platform
/// policy and the built-in theme.
pub fn install_widget_services(backend: &PrestoBackend, text: presto_text::TextSystem) {
    use presto_widgets::interact::{Native, Platform};
    let os = if cfg!(target_os = "macos") {
        Platform::Macos
    } else if cfg!(target_os = "windows") {
        Platform::Windows
    } else {
        Platform::Linux
    };
    let policy = *Native::for_os(os).policy();
    backend.with_doc(|d| presto_widgets::host::install(d, text, policy, presto_widgets::theme::Theme::default()));
}

/// The backend's document as the tree [`presto::App`] drives.
pub struct PrestoDocument {
    backend: PrestoBackend,
    /// The revision each window last ran a frame at.
    framed: HashMap<WindowId, Revision>,
    /// Presented handles whose window is being opened, in order: the next new window's first size
    /// attaches the first of them.
    opening: VecDeque<svl_handle>,
}

impl PrestoDocument {
    pub fn new(backend: PrestoBackend) -> Self {
        PrestoDocument { backend, framed: HashMap::new(), opening: VecDeque::new() }
    }

    pub fn backend(&self) -> &PrestoBackend {
        &self.backend
    }

    /// The next window the app opens is presented handle `h`'s.
    pub fn opening(&mut self, h: svl_handle) {
        self.opening.push_back(h);
    }
}

impl DocumentApi for PrestoDocument {
    fn dispatch(&mut self, ev: &InputEnvelope) -> AppDispatched {
        let before = self.backend.revision();
        // native defaults, then JS at event time (cancelable events inside, the rest right after)
        let (d, _) = self.backend.dispatch(ev);
        AppDispatched { dirty: d.changed || self.backend.revision() != before, js_events: 0, fast: Vec::new() }
    }

    fn frame(&mut self, window: WindowId, target: Time) -> Option<Commit> {
        // `requestAnimationFrame` callbacks and native output first, then the UI phases
        self.backend.frame();
        self.backend.pump();
        let c = self.backend.with_doc(|d| d.frame(window, target));
        self.framed.insert(window, self.backend.revision());
        c
    }

    fn needs_frame(&self, window: WindowId) -> bool {
        self.backend.window_handle(window).is_some() && (self.backend.frame_pending() || self.framed.get(&window).is_none_or(|r| self.backend.revision() > *r))
    }

    fn resize(&mut self, window: WindowId, size: Size, scale: Scale) {
        if self.backend.window_handle(window).is_some() {
            self.backend.with_doc(|d| d.resize_window(window, size, scale));
        } else if let Some(h) = self.opening.pop_front()
            && let Err(e) = self.backend.attach_presented(h, window, size, scale)
        {
            eprintln!("svelters-presto: cannot attach window #{h}: {e}");
        }
    }

    fn close_window(&mut self, window: WindowId) {
        if let Some(h) = self.backend.window_handle(window) {
            self.backend.window_closed(h);
        }
        self.framed.remove(&window);
    }

    fn scroll_moved(&mut self, window: WindowId, node: PropNodeId, offset: Vec2) {
        self.backend.with_doc(|d| d.apply_scroll(window, node, offset));
    }

    fn animation_event(&mut self, _window: WindowId, ev: AnimationEvent) {
        self.backend.animation_event(ev);
        self.backend.pump();
    }

    fn accessibility_action(&mut self, window: WindowId, req: &ActionRequest) -> bool {
        let d = self.backend.with_doc(|doc| doc.a11y_action(window, req));
        let handled = d.handled;
        self.backend.queue(Output { events: d.events, changed: Vec::new(), revision: Some(d.revision) });
        self.backend.pump();
        handled
    }

    fn accessibility_activated(&mut self, _window: WindowId) {}
    fn window_event(&mut self, _window: WindowId, _ev: &WindowEvent) {}
    fn device_reset(&mut self) {}
}

/// The platform's app handler for a svelters app on presto (see the module docs).
pub struct SveltersApp<C: CompositorApi + 'static> {
    app: App<PrestoDocument, C>,
    backend: PrestoBackend,
    embed: Option<Embed>,
    clock: Option<Arc<dyn Clock>>,
    opened_any: bool,
}

impl<C: CompositorApi + 'static> SveltersApp<C> {
    /// An app over `embed`'s backend; the windows are the ones JS presents.
    pub fn new(embed: Embed, spawn: SpawnCompositor<C>) -> Self {
        let backend = embed.backend().clone();
        let mut app = SveltersApp::over(backend, spawn);
        app.embed = Some(embed);
        app
    }

    /// An app over a backend whose host the caller drives (tests).
    pub fn over(backend: PrestoBackend, spawn: SpawnCompositor<C>) -> Self {
        let cfg = AppConfig { windows: Vec::new(), exit_on_last_window: false, ..AppConfig::default() };
        let app = App::new(cfg, PrestoDocument::new(backend.clone()), spawn);
        SveltersApp { app, backend, embed: None, clock: None, opened_any: false }
    }

    pub fn app(&self) -> &App<PrestoDocument, C> {
        &self.app
    }

    pub fn backend(&self) -> &PrestoBackend {
        &self.backend
    }

    /// After every platform callback: run the embed's logic, deliver native output, open and close
    /// windows JS asked for, ask for frames, and move the platform timer to the host's deadline.
    fn sync(&mut self, cx: &mut dyn PlatformCx) {
        if let Some(e) = &self.embed {
            e.logic();
        }
        self.backend.pump();
        for h in self.backend.take_presents() {
            let (size, title) = self.backend.window_desc(h);
            self.app.doc_mut().opening(h);
            match self.app.open_window(cx, WindowDesc::new(&title, size)) {
                Ok(_) => self.opened_any = true,
                Err(e) => {
                    eprintln!("svelters-presto: cannot open window #{h}: {e}");
                    self.backend.window_closed(h);
                }
            }
        }
        for h in self.backend.take_closes() {
            match self.backend.window_id(h) {
                Some(w) => self.app.close_window(cx, w),
                None => self.backend.window_closed(h),
            }
        }
        // JS changed the tree outside input (a timer, a wake, rAF): ask the compositor for a frame
        let windows: Vec<WindowId> = self.app.windows().collect();
        if let Some(c) = self.app.compositor() {
            for w in windows {
                if self.app.doc().needs_frame(w) {
                    c.request_ui_frame(w);
                }
            }
        }
        let at = match (self.backend.deadline(), &self.clock) {
            (Some(d), Some(clock)) => Some(clock.now() + Span::from_micros(d.saturating_duration_since(Instant::now()).as_micros() as i64)),
            _ => None,
        };
        cx.set_timer(at);
        if self.backend.quitting() || (self.opened_any && self.backend.windows().is_empty()) {
            cx.exit();
        }
    }
}

impl<C: CompositorApi + 'static> AppHandler for SveltersApp<C> {
    fn init(&mut self, cx: &mut dyn PlatformCx) {
        self.clock = Some(cx.clock());
        let waker = cx.waker();
        self.backend.wake_state().set_notify(Some(Arc::new(move || waker.wake())));
        self.app.init(cx);
        self.sync(cx);
    }

    fn input(&mut self, cx: &mut dyn PlatformCx, ev: InputEnvelope) {
        self.app.input(cx, ev);
        self.sync(cx);
    }

    fn window_event(&mut self, cx: &mut dyn PlatformCx, window: WindowId, ev: WindowEvent) {
        self.app.window_event(cx, window, ev);
        self.sync(cx);
    }

    fn wake(&mut self, cx: &mut dyn PlatformCx) {
        self.backend.poll_wake();
        self.app.wake(cx);
        self.sync(cx);
    }

    fn timer(&mut self, cx: &mut dyn PlatformCx, now: Time) {
        if self.backend.deadline().is_some_and(|d| d <= Instant::now()) {
            self.backend.fire_timer();
        }
        self.app.timer(cx, now);
        self.sync(cx);
    }

    fn a11y_event(&mut self, cx: &mut dyn PlatformCx, window: WindowId, ev: A11yEvent) {
        self.app.a11y_event(cx, window, ev);
        self.sync(cx);
    }

    fn exiting(&mut self, cx: &mut dyn PlatformCx) {
        self.app.exiting(cx);
    }
}
