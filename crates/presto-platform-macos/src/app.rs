//! The application: `NSApplication` (our subclass), its delegate, the main-thread state, and the
//! one queue every platform callback goes through on its way to the [`MacAppHandler`]. Cmd-Q asks
//! [`AppHandler::quit_requested`] (AppKit's own termination is always cancelled; quitting ends the loop
//! like [`PlatformCx::exit`], so `run` returns).
//!
//! AppKit calls back re-entrantly (closing a window runs its delegate, a key press runs the input
//! method, which runs `insertText:`), so callbacks never call the handler directly: they push a
//! [`Pending`] item and drain the queue if the handler is not already running. The state lives in a
//! thread-local and is borrowed only for short, non-reentrant reads and writes; AppKit objects are
//! cloned out of it before any call that can call back.

use crate::cursor;
use crate::text_input::SimpleIme;
use crate::window::{self, WindowState};
use crate::{clipboard, layer, menu, monitor, text_input, time};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2::{ClassType, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate,
    NSApplicationTerminateReply, NSEvent, NSEventModifierFlags, NSEventType,
};
use objc2_foundation::{
    NSNotification, NSObject, NSObjectProtocol, NSPoint, NSRunLoop, NSRunLoopCommonModes, NSTimer,
};
use presto_base::accesskit;
use presto_base::cursor::CursorIcon;
use presto_base::event::{ImeEvent, InputEnvelope, InputEvent, WindowEvent};
use presto_base::geom::{Rect, Scale, Vec2};
use presto_base::ids::{InputSeq, SurfaceId, WindowId};
use presto_base::time::{Clock, Time};
use presto_platform::{
    A11yEvent, AppHandler, Clipboard, HitRegion, ImeState, Monitor, OsLayerCommitter, OsLayerDesc,
    PlatformCx, PlatformError, SurfaceTarget, Waker, WindowDesc,
};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub use crate::keymap::OptionAsAlt;

/// macOS-only application callbacks, on top of [`AppHandler`]. Every method has a default.
pub trait MacAppHandler: AppHandler {
    /// The Dock icon was clicked (or the app was opened again) while running.
    fn reopen(&mut self, _cx: &mut dyn PlatformCx, _has_visible_windows: bool) {}
    /// A muda menu item was chosen.
    fn menu(&mut self, _cx: &mut dyn PlatformCx, _id: &muda::MenuId) {}
    /// A live resize (the user dragging an edge) started or ended. During it the window's layer
    /// presents with the Core Animation transaction (`platform.md` §4, macOS).
    fn live_resize(&mut self, _cx: &mut dyn PlatformCx, _window: WindowId, _active: bool) {}
}

/// Options for [`run_mac`].
#[derive(Clone, Debug)]
pub struct MacConfig {
    /// Install a minimal menu bar (app menu with Hide, Quit; Edit with the standard text actions) so
    /// Cmd-Q, Cmd-H and the Character Viewer work before the app sets its own with
    /// [`set_menu_bar`](crate::set_menu_bar).
    pub default_menu: bool,
    pub option_as_alt: OptionAsAlt,
    /// AppKit's mouse-move coalescing; off gives every sample (pointer history, architecture §11.4).
    pub mouse_coalescing: bool,
}

impl Default for MacConfig {
    fn default() -> Self {
        MacConfig {
            default_menu: true,
            option_as_alt: OptionAsAlt::None,
            mouse_coalescing: false,
        }
    }
}

/// Runs the app until [`PlatformCx::exit`]. Main thread only.
pub fn run(handler: Box<dyn AppHandler>) -> Result<(), PlatformError> {
    run_with(handler, MacConfig::default())
}

pub(crate) fn run_with(
    handler: Box<dyn AppHandler>,
    config: MacConfig,
) -> Result<(), PlatformError> {
    run_mac(Box::new(Plain(handler)), config)
}

/// [`run`] with macOS callbacks and options.
pub fn run_mac(handler: Box<dyn MacAppHandler>, config: MacConfig) -> Result<(), PlatformError> {
    let mtm = MainThreadMarker::new()
        .ok_or_else(|| PlatformError::EventLoop("must run on the main thread".into()))?;
    if STATE.with(|s| s.borrow().is_some()) {
        return Err(PlatformError::EventLoop("already running".into()));
    }
    STATE.with(|s| *s.borrow_mut() = Some(AppState::new(&config)));
    HANDLER.with(|h| *h.borrow_mut() = Some(handler));

    let app = PrestoApplication::shared(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
    let delegate = AppDelegate::new(mtm);
    app.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
    NSEvent::setMouseCoalescingEnabled(config.mouse_coalescing);
    menu::forward_events();
    if config.default_menu {
        menu::install_default(mtm);
    }
    if LAUNCHED.with(Cell::get) {
        // A second run: AppKit does not send applicationDidFinishLaunching: again.
        post(Pending::Init);
    }
    app.run();
    QUEUE.with(|q| q.borrow_mut().clear());
    post(Pending::Exiting);

    let windows: Vec<WindowState> = STATE.with(|s| {
        s.borrow_mut()
            .as_mut()
            .map(|st| st.windows.drain().map(|(_, w)| w).collect())
            .unwrap_or_default()
    });
    for w in windows {
        w.destroy();
    }
    app.setDelegate(None);
    HANDLER.with(|h| h.borrow_mut().take());
    QUEUE.with(|q| q.borrow_mut().clear());
    let timer = STATE
        .with(|s| s.borrow_mut().take())
        .and_then(|st| st.timer);
    if let Some(t) = timer {
        t.invalidate();
    }
    Ok(())
}

/// Adapts a plain [`AppHandler`] (the macOS callbacks keep their defaults).
struct Plain(Box<dyn AppHandler>);

impl AppHandler for Plain {
    fn init(&mut self, cx: &mut dyn PlatformCx) {
        self.0.init(cx)
    }
    fn input(&mut self, cx: &mut dyn PlatformCx, ev: InputEnvelope) {
        self.0.input(cx, ev)
    }
    fn window_event(&mut self, cx: &mut dyn PlatformCx, window: WindowId, ev: WindowEvent) {
        self.0.window_event(cx, window, ev)
    }
    fn wake(&mut self, cx: &mut dyn PlatformCx) {
        self.0.wake(cx)
    }
    fn timer(&mut self, cx: &mut dyn PlatformCx, now: Time) {
        self.0.timer(cx, now)
    }
    fn raw_mouse_motion(&mut self, cx: &mut dyn PlatformCx, window: WindowId, delta: Vec2) {
        self.0.raw_mouse_motion(cx, window, delta)
    }
    fn a11y_event(&mut self, cx: &mut dyn PlatformCx, window: WindowId, ev: A11yEvent) {
        self.0.a11y_event(cx, window, ev)
    }
    fn quit_requested(&mut self, cx: &mut dyn PlatformCx) -> bool {
        self.0.quit_requested(cx)
    }
    fn exiting(&mut self, cx: &mut dyn PlatformCx) {
        self.0.exiting(cx)
    }
}

impl MacAppHandler for Plain {}

/// Work for the handler, in arrival order.
pub(crate) enum Pending {
    Init,
    Input(InputEnvelope),
    Window(WindowId, WindowEvent),
    Wake,
    Timer,
    Reopen(bool),
    QuitRequested,
    Menu(muda::MenuId),
    LiveResize(WindowId, bool),
    A11y(WindowId, A11yEvent),
    RawMotion(WindowId, Vec2),
    Exiting,
}

pub(crate) struct AppState {
    pub windows: HashMap<WindowId, WindowState>,
    next_window: u32,
    next_surface: u32,
    timer: Option<Retained<NSTimer>>,
    pub option_as_alt: OptionAsAlt,
    pub cursor_hidden: bool,
    /// The window whose pointer is captured (`set_mouse_capture`).
    pub mouse_capture: Option<WindowId>,
    exiting: bool,
    wake_pending: Arc<AtomicBool>,
}

impl AppState {
    fn new(config: &MacConfig) -> Self {
        AppState {
            windows: HashMap::new(),
            next_window: 1,
            next_surface: 1,
            timer: None,
            option_as_alt: config.option_as_alt,
            cursor_hidden: false,
            mouse_capture: None,
            exiting: false,
            wake_pending: Arc::new(AtomicBool::new(false)),
        }
    }
}

thread_local! {
    static STATE: RefCell<Option<AppState>> = const { RefCell::new(None) };
    static HANDLER: RefCell<Option<Box<dyn MacAppHandler>>> = const { RefCell::new(None) };
    static QUEUE: RefCell<VecDeque<Pending>> = const { RefCell::new(VecDeque::new()) };
    static LAUNCHED: Cell<bool> = const { Cell::new(false) };
    static SEQ: Cell<u64> = const { Cell::new(1) };
}

/// Runs `f` on the state; `None` when the platform isn't running or the state is borrowed further up
/// the stack (a re-entrant AppKit callback: it is dropped, which every caller tolerates).
pub(crate) fn with_app<R>(f: impl FnOnce(&mut AppState) -> R) -> Option<R> {
    STATE.with(|s| s.try_borrow_mut().ok().and_then(|mut s| s.as_mut().map(f)))
}

pub(crate) fn with_window<R>(id: WindowId, f: impl FnOnce(&mut WindowState) -> R) -> Option<R> {
    with_app(|st| st.windows.get_mut(&id).map(f)).flatten()
}

/// Queues `p` and delivers everything queued, unless the handler is already running (then the outer
/// drain delivers it when the current call returns).
pub(crate) fn post(p: Pending) {
    QUEUE.with(|q| q.borrow_mut().push_back(p));
    drain();
}

/// Queues an input event for `window`, stamped with the next [`InputSeq`]; dropped for a closed window.
pub(crate) fn post_input(window: WindowId, os_time: Time, event: InputEvent) {
    // AppKit can still deliver events to a closed window's view.
    if with_app(|st| st.windows.contains_key(&window)) == Some(false) {
        return;
    }
    let seq = SEQ.with(|c| {
        let s = c.get();
        c.set(s + 1);
        s
    });
    post(Pending::Input(InputEnvelope {
        seq: InputSeq(seq),
        window,
        os_time,
        recv_time: time::now(),
        event,
    }));
}

pub(crate) fn post_ime(window: WindowId, events: Vec<ImeEvent>) {
    let now = time::now();
    for e in events {
        post_input(window, now, InputEvent::Ime(e));
    }
}

fn drain() {
    HANDLER.with(|h| {
        let Ok(mut guard) = h.try_borrow_mut() else {
            return;
        };
        let Some(handler) = guard.as_mut() else {
            return;
        };
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        let mut cx = MacCx {
            mtm,
            clipboard: clipboard::MacClipboard::new(mtm),
        };
        while let Some(p) = QUEUE.with(|q| q.borrow_mut().pop_front()) {
            deliver(handler.as_mut(), &mut cx, p);
        }
    });
}

fn deliver(h: &mut dyn MacAppHandler, cx: &mut MacCx, p: Pending) {
    match p {
        Pending::Init => h.init(cx),
        Pending::Input(ev) => h.input(cx, ev),
        Pending::Window(id, ev) => h.window_event(cx, id, ev),
        Pending::Wake => h.wake(cx),
        Pending::Timer => h.timer(cx, time::now()),
        Pending::Reopen(visible) => h.reopen(cx, visible),
        Pending::QuitRequested => {
            if h.quit_requested(cx) {
                cx.exit();
            }
        }
        Pending::Menu(id) => h.menu(cx, &id),
        Pending::LiveResize(id, active) => h.live_resize(cx, id, active),
        Pending::A11y(id, ev) => h.a11y_event(cx, id, ev),
        Pending::RawMotion(id, d) => h.raw_mouse_motion(cx, id, d),
        Pending::Exiting => h.exiting(cx),
    }
}

/// The handler's view of the platform.
pub(crate) struct MacCx {
    mtm: MainThreadMarker,
    clipboard: clipboard::MacClipboard,
}

impl PlatformCx for MacCx {
    fn try_create_window(&mut self, desc: WindowDesc) -> Result<WindowId, PlatformError> {
        let id = with_app(|st| {
            let id = WindowId(st.next_window);
            st.next_window += 1;
            id
        })
        .ok_or_else(|| PlatformError::EventLoop("the platform is not running".into()))?;
        let (visible, maximized) = (desc.visible, desc.maximized);
        let mut state = WindowState::new(self.mtm, id, &desc);
        let (window, size) = (state.ns_window.clone(), state.device_size());
        state.last_size = Some((size.0, size.1.0));
        with_app(|st| st.windows.insert(id, state));
        window::finish_creation(&window, visible, maximized);
        post(Pending::Window(
            id,
            WindowEvent::Resized {
                size: size.0,
                scale: size.1,
            },
        ));
        Ok(id)
    }

    fn close_window(&mut self, id: WindowId) {
        if let Some(w) = with_app(|st| st.windows.remove(&id)).flatten() {
            w.destroy();
        }
    }

    fn surface_target(&self, id: WindowId) -> SurfaceTarget {
        with_window(id, |w| layer::target(&w.layer)).unwrap_or(SurfaceTarget::None)
    }

    fn create_os_layer(
        &mut self,
        id: WindowId,
        desc: OsLayerDesc,
    ) -> Option<(SurfaceId, SurfaceTarget)> {
        let surface = with_app(|st| {
            let s = SurfaceId(st.next_surface);
            st.next_surface += 1;
            s
        })?;
        with_window(id, |w| {
            let l = layer::create_os_layer(&w.layer, &desc, w.scale());
            let target = layer::target(&l);
            w.os_layers.insert(surface, l);
            (surface, target)
        })
    }

    fn set_os_layer_bounds(&mut self, id: WindowId, layer: SurfaceId, bounds: Rect) {
        with_window(id, |w| {
            if let Some(l) = w.os_layers.get(&layer) {
                layer::set_frame(l, &bounds);
            }
        });
    }

    fn remove_os_layer(&mut self, id: WindowId, layer: SurfaceId) {
        if let Some(l) = with_window(id, |w| w.os_layers.remove(&layer)).flatten() {
            layer::remove(&l);
        }
    }

    /// Core Animation needs no commit from the compositor thread (layer changes go in transactions).
    fn os_layer_committer(&self, _id: WindowId) -> Option<OsLayerCommitter> {
        None
    }

    fn set_cursor(&mut self, id: WindowId, c: CursorIcon) {
        let Some((window, view, changed)) = with_window(id, |w| {
            let changed = w.cursor != c;
            w.cursor = c;
            (w.ns_window.clone(), w.view.clone(), changed)
        }) else {
            return;
        };
        if changed {
            cursor::apply(self.mtm, c, &window, &view);
        }
    }

    fn set_ime(&mut self, id: WindowId, state: Option<ImeState>) {
        enum Change {
            None,
            Enabled,
            Disabled { had_marked: bool },
        }
        let Some((view, change)) = with_window(id, |w| {
            let change = match (&mut w.ime, state) {
                (Some(ime), Some(s)) => {
                    ime.area = s.area;
                    ime.purpose = s.purpose;
                    ime.composition_rects = s.composition_rects;
                    Change::None
                }
                (None, Some(s)) => {
                    let mut ime = SimpleIme::new(s.area, s.purpose);
                    ime.composition_rects = s.composition_rects;
                    w.ime = Some(ime);
                    Change::Enabled
                }
                (Some(ime), None) => {
                    let had_marked = ime.marked_text().is_some();
                    w.ime = None;
                    Change::Disabled { had_marked }
                }
                (None, None) => Change::None,
            };
            (w.view.clone(), change)
        }) else {
            return;
        };
        match change {
            Change::Enabled => post_ime(id, vec![ImeEvent::Enabled]),
            Change::Disabled { had_marked } => {
                if had_marked {
                    window::discard_marked_text(&view);
                    post_ime(
                        id,
                        vec![ImeEvent::Preedit {
                            text: String::new(),
                            cursor: None,
                        }],
                    );
                }
                post_ime(id, vec![ImeEvent::Disabled]);
            }
            Change::None => {}
        }
        window::invalidate_character_coordinates(&view);
    }

    /// The full `NSTextInputClient` bridge ([`crate::set_text_input_client`]).
    fn set_text_input(
        &mut self,
        id: WindowId,
        client: Option<Box<dyn presto_base::input::TextInputClient>>,
    ) {
        crate::set_text_input_client(id, client);
    }

    fn end_ime_composition(&mut self, id: WindowId, commit: bool) -> bool {
        let Some(view) = with_window(id, |w| w.view.clone()) else {
            return false;
        };
        crate::view::with_target(id, |t, ev| {
            if commit {
                t.unmark(ev);
            } else {
                t.set_marked(
                    "",
                    text_input::NsRange::NOT_FOUND,
                    text_input::NsRange::NOT_FOUND,
                    ev,
                );
            }
        });
        window::discard_marked_text(&view);
        true
    }

    fn clipboard(&mut self) -> &mut dyn Clipboard {
        &mut self.clipboard
    }

    fn set_timer(&mut self, at: Option<Time>) {
        let old = with_app(|st| st.timer.take()).flatten();
        if let Some(t) = old {
            t.invalidate();
        }
        let Some(at) = at else { return };
        let delay = (at - time::now()).as_secs_f64().max(0.0);
        let block = block2::RcBlock::new(|_t: std::ptr::NonNull<NSTimer>| {
            with_app(|st| st.timer = None);
            post(Pending::Timer);
        });
        // SAFETY: the block is 'static and runs on the main run loop.
        let timer = unsafe { NSTimer::timerWithTimeInterval_repeats_block(delay, false, &block) };
        // SAFETY: the common modes keep timers firing during live resize and menu tracking.
        unsafe { NSRunLoop::mainRunLoop().addTimer_forMode(&timer, NSRunLoopCommonModes) };
        with_app(|st| st.timer = Some(timer));
    }

    fn waker(&self) -> Waker {
        let flag = with_app(|st| st.wake_pending.clone()).unwrap_or_default();
        Waker::new(move || {
            if flag.swap(true, Ordering::AcqRel) {
                return;
            }
            let flag = flag.clone();
            dispatch2::DispatchQueue::main().exec_async(move || {
                flag.store(false, Ordering::Release);
                post(Pending::Wake);
            });
        })
    }

    fn clock(&self) -> Arc<dyn Clock> {
        Arc::new(time::MachClock)
    }

    fn monitors(&self) -> Vec<Monitor> {
        monitor::monitors(self.mtm)
    }

    fn set_hit_test_regions(&mut self, id: WindowId, regions: &[HitRegion]) {
        with_window(id, |w| w.hit_regions = regions.to_vec());
    }

    fn set_mouse_capture(&mut self, id: WindowId, capture: bool) {
        let changed = with_app(|st| {
            let new = capture.then_some(id);
            let changed = st.mouse_capture.is_some() != new.is_some();
            if capture || st.mouse_capture == Some(id) {
                st.mouse_capture = new;
            }
            changed
        })
        .unwrap_or(false);
        if changed {
            // SAFETY: plain CoreGraphics call.
            unsafe { CGAssociateMouseAndMouseCursorPosition(!capture as u8) };
            cursor::set_hidden(capture);
        }
    }

    fn set_title(&mut self, id: WindowId, title: &str) {
        if let Some(w) = with_window(id, |w| w.ns_window.clone()) {
            w.setTitle(&objc2_foundation::NSString::from_str(title));
        }
    }

    fn set_visible(&mut self, id: WindowId, visible: bool) {
        if let Some(w) = with_window(id, |w| w.ns_window.clone()) {
            if visible {
                w.makeKeyAndOrderFront(None)
            } else {
                w.orderOut(None)
            }
        }
    }

    fn drag_window(&mut self, id: WindowId) {
        let Some(w) = with_window(id, |w| w.ns_window.clone()) else {
            return;
        };
        let event = NSApplication::sharedApplication(self.mtm).currentEvent();
        if let Some(e) = event.filter(|e| {
            matches!(
                e.r#type(),
                NSEventType::LeftMouseDown | NSEventType::LeftMouseDragged
            )
        }) {
            w.performWindowDragWithEvent(&e);
        }
    }

    fn update_a11y(&mut self, id: WindowId, update: accesskit::TreeUpdate) {
        crate::a11y::a11y_update(id, update);
    }

    fn window_scale(&self, id: WindowId) -> Option<Scale> {
        with_window(id, |w| Scale(w.scale() as f32))
    }

    /// The best timing source this macOS has (`NSView.displayLink` on 14+, else `CVDisplayLink`).
    fn frame_source(
        &mut self,
        id: WindowId,
    ) -> Option<Arc<dyn presto_platform::frame::FrameSource>> {
        let config = crate::FrameSourceConfig::new(crate::FrameSourceKind::best_timing());
        crate::frame_source(id, config)
            .ok()
            .map(|s| Arc::new(s) as Arc<dyn presto_platform::frame::FrameSource>)
    }

    fn exit(&mut self) {
        let first = with_app(|st| !std::mem::replace(&mut st.exiting, true)).unwrap_or(false);
        if first {
            stop_app(self.mtm);
        }
    }
}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGAssociateMouseAndMouseCursorPosition(connected: u8) -> i32;
}

/// Ends `NSApplication.run` once the current event finishes (a posted event wakes the loop so the
/// stop takes effect at once).
fn stop_app(mtm: MainThreadMarker) {
    let app = NSApplication::sharedApplication(mtm);
    app.stop(None);
    let event = NSEvent::otherEventWithType_location_modifierFlags_timestamp_windowNumber_context_subtype_data1_data2(
        NSEventType::ApplicationDefined,
        NSPoint::new(0.0, 0.0),
        NSEventModifierFlags::empty(),
        0.0,
        0,
        None,
        0,
        0,
        0,
    );
    if let Some(event) = event {
        app.postEvent_atStart(&event, false);
    }
}

define_class!(
    // SAFETY: NSApplication allows subclassing; PrestoApplication adds no ivars and does not implement
    // Drop.
    #[unsafe(super(NSApplication))]
    #[thread_kind = MainThreadOnly]
    #[name = "PrestoApplication"]
    pub(crate) struct PrestoApplication;

    impl PrestoApplication {
        /// AppKit does not deliver key-up events while Cmd is down; forward them to the key window
        /// (winit does the same).
        #[unsafe(method(sendEvent:))]
        fn send_event(&self, event: &NSEvent) {
            if event.r#type() == NSEventType::KeyUp && event.modifierFlags().contains(NSEventModifierFlags::Command) {
                if let Some(key_window) = self.keyWindow() {
                    key_window.sendEvent(event);
                    return;
                }
            }
            // SAFETY: forwarding to NSApplication's implementation with the same argument.
            unsafe { msg_send![super(self), sendEvent: event] }
        }
    }
);

impl PrestoApplication {
    /// The shared application, created as our subclass unless something created `NSApp` first.
    fn shared(mtm: MainThreadMarker) -> Retained<NSApplication> {
        // SAFETY: `+sharedApplication` on an NSApplication subclass returns the shared instance.
        let _: Retained<AnyObject> =
            unsafe { msg_send![PrestoApplication::class(), sharedApplication] };
        NSApplication::sharedApplication(mtm)
    }
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements; AppDelegate does not implement Drop.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "PrestoAppDelegate"]
    struct AppDelegate;

    unsafe impl NSObjectProtocol for AppDelegate {}

    unsafe impl NSApplicationDelegate for AppDelegate {
        #[unsafe(method(applicationDidFinishLaunching:))]
        fn did_finish_launching(&self, _note: &NSNotification) {
            LAUNCHED.with(|l| l.set(true));
            let app = NSApplication::sharedApplication(self.mtm());
            #[allow(deprecated)]
            app.activateIgnoringOtherApps(true);
            post(Pending::Init);
        }

        #[unsafe(method(applicationShouldTerminate:))]
        fn should_terminate(&self, _sender: &NSApplication) -> NSApplicationTerminateReply {
            post(Pending::QuitRequested);
            NSApplicationTerminateReply::TerminateCancel
        }

        #[unsafe(method(applicationShouldHandleReopen:hasVisibleWindows:))]
        fn should_handle_reopen(&self, _sender: &NSApplication, has_visible_windows: bool) -> bool {
            post(Pending::Reopen(has_visible_windows));
            true
        }

        #[unsafe(method(applicationShouldTerminateAfterLastWindowClosed:))]
        fn terminate_after_last_window_closed(&self, _sender: &NSApplication) -> bool {
            false
        }
    }
);

impl AppDelegate {
    fn new(mtm: MainThreadMarker) -> Retained<AppDelegate> {
        let this = Self::alloc(mtm).set_ivars(());
        // SAFETY: NSObject's init.
        unsafe { msg_send![super(this), init] }
    }
}
