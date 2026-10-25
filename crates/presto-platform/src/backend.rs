//! The phase-1 backend: winit 0.30.13 drives the loop and windows; the per-OS shims in [`crate::os`]
//! add what winit lacks (hit testing, modal-loop timers, pointer history, IME requests, OS layers,
//! the quit veto). Every winit callback is translated and handed to the app's [`AppHandler`].

use crate::a11y::{A11yAdapter, A11ySink};
use crate::api::{
    A11yEvent, AppHandler, HitRegion, ImeState, Monitor, OsLayerCommitter, OsLayerDesc, PlatformCx,
    PlatformError, SurfaceTarget, Waker, WindowDesc,
};
use crate::clipboard::{Clipboard, SystemClipboard};
use crate::clock::{self, OsClock};
use crate::frame::{FrameSource, IntervalSource};
use crate::os::{self, ShimHooks, WindowShim};
use crate::translate::{self, Translated, WindowInput};
use presto_base::accesskit;
use presto_base::cursor::CursorIcon;
use presto_base::event::InputEnvelope;
use presto_base::geom::{DeviceSize, Rect, Scale, Vec2};
use presto_base::ids::{InputSeq, SurfaceId, WindowId};
use presto_base::input::InputPurpose;
use presto_base::time::{Clock, Span, Time};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalPosition, LogicalSize, PhysicalPosition};
use winit::event::{DeviceEvent, DeviceId, StartCause};
use winit::event_loop::{ActiveEventLoop, ControlFlow, DeviceEvents, EventLoop, EventLoopProxy};
use winit::window::{CursorGrabMode, ImePurpose, Window, WindowAttributes};

/// What other threads (and the shims) post to the loop.
#[derive(Debug)]
pub(crate) enum UserEvent {
    Wake,
    /// A modal-loop timer tick or a deadline check: run due timers.
    Tick,
    A11y(WindowId, A11yEvent),
    /// Make OS-layer changes visible (DirectComposition `Commit`).
    CommitOsLayers(WindowId),
    QuitRequested,
}

/// Runs the app on this (the main) thread until it exits.
pub fn run(handler: Box<dyn AppHandler>) -> Result<(), PlatformError> {
    let event_loop = EventLoop::<UserEvent>::with_user_event()
        .build()
        .map_err(|e| PlatformError::EventLoop(e.to_string()))?;
    // Raw input only feeds DeviceEvents, which presto uses only during mouse capture.
    event_loop.listen_device_events(DeviceEvents::Never);
    let proxy = event_loop.create_proxy();
    let poster = Poster(Arc::new(Mutex::new(proxy)));
    {
        let p = poster.clone();
        os::install_app_hooks(Box::new(move || p.post(UserEvent::QuitRequested)));
    }
    let mut app = WinitApp {
        handler,
        state: State::new(poster),
        started: false,
    };
    event_loop
        .run_app(&mut app)
        .map_err(|e| PlatformError::EventLoop(e.to_string()))
}

/// A `Send + Sync` way to post to the loop.
#[derive(Clone)]
pub(crate) struct Poster(Arc<Mutex<EventLoopProxy<UserEvent>>>);

impl Poster {
    pub(crate) fn post(&self, ev: UserEvent) {
        let proxy = self.0.lock().unwrap_or_else(|e| e.into_inner());
        // A closed loop is exiting; nothing is left to wake.
        let _ = proxy.send_event(ev);
    }
}

/// Fields drop in order: the accessibility adapter unhooks first (it subclassed last), then the
/// shim, then the window.
struct Win {
    a11y: Option<A11yAdapter>,
    shim: WindowShim,
    input: WindowInput,
    regions: Vec<HitRegion>,
    captured: bool,
    window: Window,
}

struct State {
    poster: Poster,
    wake_pending: Arc<AtomicBool>,
    windows: HashMap<WindowId, Win>,
    by_winit: HashMap<winit::window::WindowId, WindowId>,
    next_window: u32,
    next_surface: u32,
    next_seq: u64,
    timer: Option<Time>,
    clipboard: Option<SystemClipboard>,
}

impl State {
    fn new(poster: Poster) -> Self {
        State {
            poster,
            wake_pending: Arc::new(AtomicBool::new(false)),
            windows: HashMap::new(),
            by_winit: HashMap::new(),
            next_window: 1,
            next_surface: 1,
            next_seq: 1,
            timer: None,
            clipboard: None,
        }
    }
}

struct WinitApp {
    handler: Box<dyn AppHandler>,
    state: State,
    started: bool,
}

struct Cx<'a> {
    el: &'a ActiveEventLoop,
    st: &'a mut State,
}

impl WinitApp {
    fn with_cx<R>(
        &mut self,
        el: &ActiveEventLoop,
        f: impl FnOnce(&mut dyn AppHandler, &mut Cx<'_>) -> R,
    ) -> R {
        let mut cx = Cx {
            el,
            st: &mut self.state,
        };
        f(&mut *self.handler, &mut cx)
    }

    /// Fires the timer while its deadline has passed (the handler may set a new one).
    fn run_due_timers(&mut self, el: &ActiveEventLoop) {
        for _ in 0..16 {
            let now = clock::now();
            match self.state.timer {
                Some(t) if t <= now => {
                    self.state.timer = None;
                    self.with_cx(el, |h, cx| h.timer(cx, now));
                }
                _ => return,
            }
        }
    }
}

impl ApplicationHandler<UserEvent> for WinitApp {
    fn new_events(&mut self, el: &ActiveEventLoop, _cause: StartCause) {
        if self.started {
            self.run_due_timers(el);
        }
    }

    /// winit's place to create windows; it comes once on desktops, after `Init`.
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if !self.started {
            self.started = true;
            self.with_cx(el, |h, cx| h.init(cx));
        }
    }

    fn user_event(&mut self, el: &ActiveEventLoop, ev: UserEvent) {
        match ev {
            UserEvent::Wake => {
                self.state.wake_pending.store(false, Ordering::SeqCst);
                self.with_cx(el, |h, cx| h.wake(cx));
            }
            UserEvent::Tick => {}
            UserEvent::A11y(w, ev) => self.with_cx(el, |h, cx| h.a11y_event(cx, w, ev)),
            UserEvent::CommitOsLayers(w) => {
                if let Some(win) = self.state.windows.get_mut(&w) {
                    win.shim.commit_layers();
                }
            }
            UserEvent::QuitRequested => {
                if self.with_cx(el, |h, cx| h.quit_requested(cx)) {
                    el.exit();
                }
            }
        }
        self.run_due_timers(el);
    }

    fn window_event(
        &mut self,
        el: &ActiveEventLoop,
        wid: winit::window::WindowId,
        ev: winit::event::WindowEvent,
    ) {
        let Some(&id) = self.state.by_winit.get(&wid) else {
            return;
        };
        let recv = clock::now();
        let Some(win) = self.state.windows.get_mut(&id) else {
            return;
        };
        if let Some(a) = &mut win.a11y {
            a.window_event(&win.window, &ev);
        }
        win.shim.window_event(&ev);
        if matches!(ev, winit::event::WindowEvent::CursorMoved { .. }) {
            win.input.pending_history = win.shim.take_history();
        }
        let os_time = win.shim.event_time().unwrap_or(recv);
        let Some(t) = win.input.translate(&ev) else {
            return;
        };
        // Windows answers caption drags in WM_NCHITTEST; elsewhere a caption press starts the move.
        if !cfg!(target_os = "windows") {
            if let Translated::Input(presto_base::event::InputEvent::Pointer(p)) = &t {
                if p.phase == presto_base::event::PointerPhase::Down
                    && p.button == Some(presto_base::event::MouseButton::Primary)
                    && crate::api::hit_test(&win.regions, p.pos) == crate::api::HitKind::Caption
                {
                    if let Err(e) = win.window.drag_window() {
                        log::debug!("presto-platform: drag_window: {e}");
                    }
                }
            }
        }
        match t {
            Translated::Input(event) => {
                let seq = InputSeq(self.state.next_seq);
                self.state.next_seq += 1;
                let env = InputEnvelope {
                    seq,
                    window: id,
                    os_time,
                    recv_time: recv,
                    event,
                };
                self.with_cx(el, |h, cx| h.input(cx, env));
            }
            Translated::Window(event) => self.with_cx(el, |h, cx| h.window_event(cx, id, event)),
        }
    }

    fn device_event(&mut self, el: &ActiveEventLoop, _device: DeviceId, ev: DeviceEvent) {
        let DeviceEvent::MouseMotion { delta } = ev else {
            return;
        };
        let Some(id) = self
            .state
            .windows
            .iter()
            .find(|(_, w)| w.captured)
            .map(|(id, _)| *id)
        else {
            return;
        };
        self.with_cx(el, |h, cx| {
            h.raw_mouse_motion(cx, id, Vec2::new(delta.0 as f32, delta.1 as f32))
        });
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        self.run_due_timers(el);
        el.set_control_flow(match self.state.timer {
            Some(t) => ControlFlow::WaitUntil(clock::to_instant(t)),
            None => ControlFlow::Wait,
        });
    }

    fn exiting(&mut self, el: &ActiveEventLoop) {
        self.with_cx(el, |h, cx| h.exiting(cx));
        // The Wayland clipboard holds winit's display pointer: drop it before the loop goes.
        self.state.clipboard = None;
        self.state.windows.clear();
    }
}

fn window_attributes(desc: &WindowDesc) -> WindowAttributes {
    let mut a = Window::default_attributes()
        .with_title(desc.title.clone())
        .with_inner_size(LogicalSize::new(desc.size.w as f64, desc.size.h as f64))
        .with_decorations(desc.decorations)
        .with_transparent(desc.transparent)
        .with_resizable(desc.resizable)
        .with_maximized(desc.maximized)
        // Shown after the accessibility adapter is attached (AccessKit requires it).
        .with_visible(false);
    if let Some(s) = desc.min_size {
        a = a.with_min_inner_size(LogicalSize::new(s.w as f64, s.h as f64));
    }
    if let Some(s) = desc.max_size {
        a = a.with_max_inner_size(LogicalSize::new(s.w as f64, s.h as f64));
    }
    if let Some((x, y)) = desc.position {
        a = a.with_position(PhysicalPosition::new(x, y));
    }
    #[cfg(all(
        unix,
        not(any(target_os = "macos", target_os = "ios", target_os = "android"))
    ))]
    if let Some(app_id) = &desc.app_id {
        a = winit::platform::wayland::WindowAttributesExtWayland::with_name(
            a,
            app_id.clone(),
            app_id.clone(),
        );
        a = winit::platform::x11::WindowAttributesExtX11::with_name(
            a,
            app_id.clone(),
            app_id.clone(),
        );
    }
    os::prepare_attributes(a, desc)
}

fn ime_purpose(p: InputPurpose) -> ImePurpose {
    match p {
        InputPurpose::Password => ImePurpose::Password,
        InputPurpose::Code => ImePurpose::Terminal,
        _ => ImePurpose::Normal,
    }
}

impl Cx<'_> {
    fn win(&self, id: WindowId) -> Option<&Win> {
        self.st.windows.get(&id)
    }
    fn win_mut(&mut self, id: WindowId) -> Option<&mut Win> {
        self.st.windows.get_mut(&id)
    }

    fn create(&mut self, desc: WindowDesc) -> Result<WindowId, PlatformError> {
        let window = self
            .el
            .create_window(window_attributes(&desc))
            .map_err(|e| PlatformError::Os(e.to_string()))?;
        let id = WindowId(self.st.next_window);
        self.st.next_window += 1;
        let sink: A11ySink = {
            let p = self.st.poster.clone();
            Arc::new(move |ev| p.post(UserEvent::A11y(id, ev)))
        };
        let hooks = ShimHooks {
            tick: {
                let p = self.st.poster.clone();
                Arc::new(move || p.post(UserEvent::Tick))
            },
            commit: {
                let p = self.st.poster.clone();
                Arc::new(move || p.post(UserEvent::CommitOsLayers(id)))
            },
        };
        let mut shim = WindowShim::install(&window, &desc, hooks);
        shim.set_timer(self.st.timer);
        let a11y = A11yAdapter::new(&window, sink);
        let scale = window.scale_factor() as f32;
        if desc.visible {
            window.set_visible(true);
        }
        self.st.by_winit.insert(window.id(), id);
        self.st.windows.insert(
            id,
            Win {
                a11y,
                shim,
                input: WindowInput::new(scale),
                regions: Vec::new(),
                captured: false,
                window,
            },
        );
        Ok(id)
    }
}

impl PlatformCx for Cx<'_> {
    fn try_create_window(&mut self, desc: WindowDesc) -> Result<WindowId, PlatformError> {
        self.create(desc)
    }

    fn close_window(&mut self, id: WindowId) {
        if let Some(w) = self.st.windows.remove(&id) {
            self.st.by_winit.remove(&w.window.id());
        }
    }

    fn surface_target(&self, id: WindowId) -> SurfaceTarget {
        let Some(w) = self.win(id) else {
            return SurfaceTarget::None;
        };
        if let Some(t) = w.shim.surface_target() {
            return t;
        }
        match (w.window.window_handle(), w.window.display_handle()) {
            (Ok(wh), Ok(dh)) => SurfaceTarget::Raw {
                window: wh.as_raw(),
                display: dh.as_raw(),
            },
            _ => SurfaceTarget::None,
        }
    }

    fn create_os_layer(
        &mut self,
        id: WindowId,
        desc: OsLayerDesc,
    ) -> Option<(SurfaceId, SurfaceTarget)> {
        let sid = SurfaceId(self.st.next_surface);
        let w = self.st.windows.get_mut(&id)?;
        let scale = w.input.scale;
        let target = w.shim.create_layer(&w.window, sid, desc, scale)?;
        self.st.next_surface += 1;
        Some((sid, target))
    }

    fn set_os_layer_bounds(&mut self, id: WindowId, layer: SurfaceId, bounds: Rect) {
        if let Some(w) = self.win_mut(id) {
            let scale = w.input.scale;
            w.shim.set_layer_bounds(layer, bounds, scale);
        }
    }

    fn remove_os_layer(&mut self, id: WindowId, layer: SurfaceId) {
        if let Some(w) = self.win_mut(id) {
            w.shim.remove_layer(layer);
        }
    }

    fn os_layer_committer(&self, id: WindowId) -> Option<OsLayerCommitter> {
        self.win(id)?.shim.committer()
    }

    fn set_cursor(&mut self, id: WindowId, c: CursorIcon) {
        let Some(w) = self.win(id) else { return };
        match translate::cursor_icon(c) {
            Some(icon) => {
                w.window.set_cursor_visible(true);
                w.window.set_cursor(icon);
            }
            None => w.window.set_cursor_visible(false),
        }
    }

    fn set_ime(&mut self, id: WindowId, state: Option<ImeState>) {
        let Some(w) = self.win_mut(id) else { return };
        match &state {
            Some(s) => {
                w.window.set_ime_allowed(true);
                w.window.set_ime_purpose(ime_purpose(s.purpose));
                w.window.set_ime_cursor_area(
                    LogicalPosition::new(s.area.min.x as f64, s.area.min.y as f64),
                    LogicalSize::new(
                        s.area.width().max(1.0) as f64,
                        s.area.height().max(1.0) as f64,
                    ),
                );
            }
            None => w.window.set_ime_allowed(false),
        }
        w.shim.set_ime(state.as_ref());
    }

    fn end_ime_composition(&mut self, id: WindowId, commit: bool) -> bool {
        let Some(w) = self.win_mut(id) else {
            return false;
        };
        if let Some(done) = w.shim.end_composition(commit) {
            return done;
        }
        if commit {
            return false;
        }
        // winit has no cancel: turning IME off and on drops the composition (egui's way).
        w.window.set_ime_allowed(false);
        w.window.set_ime_allowed(true);
        true
    }

    fn clipboard(&mut self) -> &mut dyn Clipboard {
        if self.st.clipboard.is_none() {
            let display = self.el.display_handle().ok().map(|d| d.as_raw());
            self.st.clipboard = Some(SystemClipboard::new(display));
        }
        self.st.clipboard.as_mut().expect("just set")
    }

    fn set_timer(&mut self, at: Option<Time>) {
        self.st.timer = at;
        for w in self.st.windows.values_mut() {
            w.shim.set_timer(at);
        }
        self.el.set_control_flow(match at {
            Some(t) => ControlFlow::WaitUntil(clock::to_instant(t)),
            None => ControlFlow::Wait,
        });
    }

    fn waker(&self) -> Waker {
        let pending = self.st.wake_pending.clone();
        let poster = self.st.poster.clone();
        Waker::new(move || {
            if !pending.swap(true, Ordering::SeqCst) {
                poster.post(UserEvent::Wake);
            }
        })
    }

    fn clock(&self) -> Arc<dyn Clock> {
        Arc::new(OsClock)
    }

    fn monitors(&self) -> Vec<Monitor> {
        self.el
            .available_monitors()
            .map(|m| {
                let pos = m.position();
                let size = m.size();
                Monitor {
                    name: m.name(),
                    position: (pos.x, pos.y),
                    size: DeviceSize {
                        w: size.width,
                        h: size.height,
                    },
                    work_area: os::monitor_work_area(&m),
                    scale: Scale(m.scale_factor() as f32),
                    refresh_millihertz: m.refresh_rate_millihertz(),
                }
            })
            .collect()
    }

    fn set_hit_test_regions(&mut self, id: WindowId, regions: &[HitRegion]) {
        if let Some(w) = self.win_mut(id) {
            w.regions = regions.to_vec();
            w.shim.set_hit_regions(regions);
        }
    }

    fn set_mouse_capture(&mut self, id: WindowId, capture: bool) {
        let Some(w) = self.st.windows.get_mut(&id) else {
            return;
        };
        w.captured = capture;
        if capture {
            if w.window.set_cursor_grab(CursorGrabMode::Locked).is_err() {
                if let Err(e) = w.window.set_cursor_grab(CursorGrabMode::Confined) {
                    log::warn!("presto-platform: cannot grab the pointer: {e}");
                }
            }
            w.window.set_cursor_visible(false);
        } else {
            let _ = w.window.set_cursor_grab(CursorGrabMode::None);
            w.window.set_cursor_visible(true);
        }
        let any = self.st.windows.values().any(|w| w.captured);
        self.el.listen_device_events(if any {
            DeviceEvents::Always
        } else {
            DeviceEvents::Never
        });
    }

    fn set_title(&mut self, id: WindowId, title: &str) {
        if let Some(w) = self.win(id) {
            w.window.set_title(title);
        }
    }

    fn set_visible(&mut self, id: WindowId, visible: bool) {
        if let Some(w) = self.win(id) {
            w.window.set_visible(visible);
        }
    }

    fn drag_window(&mut self, id: WindowId) {
        if let Some(w) = self.win(id) {
            if let Err(e) = w.window.drag_window() {
                log::debug!("presto-platform: drag_window: {e}");
            }
        }
    }

    fn update_a11y(&mut self, id: WindowId, update: accesskit::TreeUpdate) {
        if let Some(a) = self.win_mut(id).and_then(|w| w.a11y.as_mut()) {
            a.update(update);
        }
    }

    fn window_scale(&self, id: WindowId) -> Option<Scale> {
        self.win(id).map(|w| Scale(w.input.scale))
    }

    fn frame_source(&mut self, id: WindowId) -> Option<Arc<dyn FrameSource>> {
        let w = self.st.windows.get_mut(&id)?;
        let refresh = w
            .window
            .current_monitor()
            .and_then(|m| m.refresh_rate_millihertz());
        let interval = refresh
            .filter(|r| *r >= 1_000)
            .map(|r| Span(1_000_000_000_000 / r as i64));
        if let Some(s) = w.shim.frame_source(interval) {
            return Some(s);
        }
        Some(Arc::new(IntervalSource::from_refresh(
            refresh,
            Arc::new(OsClock),
        )))
    }

    fn exit(&mut self) {
        self.el.exit();
    }
}
