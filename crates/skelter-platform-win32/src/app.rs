//! The event loop: one UI thread (an OLE STA) running `GetMessage`, the app's handler, and the
//! policies that keep skelter responsive where winit's loop stalls (`round2/winit.md` §4.1).
//!
//! - **Re-entrancy.** The handler is taken out of [`App`] while it runs. Window messages that arrive
//!   meanwhile (an OS call made from inside the handler sends `WM_SIZE`, a native modal pumps) are
//!   queued and delivered, in order, as soon as the handler returns.
//! - **Modal loops.** Moving, sizing and menu loops run inside `DefWindowProc` with the handler not
//!   on the stack, so the window procedure delivers everything at once: resizes synchronously (the
//!   compositor handshake), and wakes and timers too, because both arrive as posted messages to a
//!   message-only window (the compositor's wake through [`Waker`], deadlines from a
//!   high-resolution waitable timer on a timer thread) rather than from this loop's own wait. Frames
//!   therefore keep coming from the compositor's wake through every modal loop.
//! - **Native modals from the app** (a window move from `drag_window`) start after the handler
//!   returns, never inside it.
//! - **IME edits** (TSF, `WM_IME_REQUEST`) reach the app as `ImeEvent`s queued while the OS call
//!   runs and delivered when it returns ([`App::flush`]).
//! - **Keys** pass through the TSF keystroke manager before `TranslateMessage`, so the active TIP
//!   sees them first (IME-first routing, architecture §11.2).

use crate::clipboard::Win32Clipboard;
use crate::dcomp::DComp;
use crate::drag;
use crate::frame::Win32Frames;
use crate::keyboard::KeyTranslator;
use crate::monitor;
use crate::tsf::{ClientAccess, Tsf, TsfDocument};
use crate::util::wide;
use crate::window::{self, Window};
use skelter_base::accesskit;
use skelter_base::cursor::CursorIcon;
use skelter_base::event::{InputEnvelope, InputEvent, WindowEvent};
use skelter_base::geom::{Rect, Scale, Vec2};
use skelter_base::ids::{InputSeq, SurfaceId, WindowId};
use skelter_base::input::TextInputClient;
use skelter_base::time::{Clock, Time};
use skelter_platform::clock::OsClock;
use skelter_platform::frame::FrameSource;
use skelter_platform::{
    A11yEvent, AppHandler, Clipboard, DragData, DragId, DragOutcome, HitRegion, ImeState, Monitor, OsLayerCommitter,
    OsLayerDesc, PlatformCx, PlatformError, PopupDesc, SurfaceTarget, Waker, WindowDesc, WindowStateRequest,
};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::rc::{Rc, Weak};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use windows::Win32::Foundation::{HANDLE, HINSTANCE, HWND, LPARAM, WAIT_OBJECT_0, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Ole::{OleInitialize, OleUninitialize};
use windows::Win32::System::Threading::{
    CREATE_WAITABLE_TIMER_HIGH_RESOLUTION, CreateEventW, CreateWaitableTimerExW, INFINITE, SetEvent,
    SetWaitableTimer, TIMER_ALL_ACCESS, WaitForMultipleObjects, WaitForSingleObject,
};
use windows::Win32::UI::HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext};
use windows::Win32::UI::Input::Pointer::EnableMouseInPointer;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, DispatchMessageW, GetMessageW, HWND_MESSAGE, MSG, PostMessageW, PostQuitMessage,
    TranslateMessage, WINDOW_EX_STYLE, WINDOW_STYLE, WM_APP,
};

/// Posted to the message window by [`Waker::wake`].
pub(crate) const WM_SKELTER_WAKE: u32 = WM_APP + 1;
/// Posted to the message window when the timer's deadline passes.
pub(crate) const WM_SKELTER_TIMER: u32 = WM_APP + 2;
/// Posted to the message window when an accessibility event is queued (they arrive on any thread).
pub(crate) const WM_SKELTER_A11Y: u32 = WM_APP + 3;
/// Posted to the message window by `start_drag`: the OLE drag's modal loop runs outside the handler.
pub(crate) const WM_SKELTER_DRAG: u32 = WM_APP + 4;

thread_local! {
    static APP: RefCell<Option<Rc<App>>> = const { RefCell::new(None) };
}

/// The running app on this thread, if any.
pub(crate) fn current() -> Option<Rc<App>> {
    APP.with(|a| a.borrow().clone())
}

/// Work for the handler, in arrival order.
pub(crate) enum Pending {
    Init,
    Input(InputEnvelope),
    Window(WindowId, WindowEvent),
    Wake,
    Timer,
    RawMotion(WindowId, Vec2),
    A11y(WindowId, A11yEvent),
    DragEnded(DragId, DragOutcome),
}

pub(crate) struct Inner {
    pub windows: HashMap<WindowId, Window>,
    pub by_hwnd: HashMap<isize, WindowId>,
    next_window: u32,
    next_surface: u32,
}

impl Inner {
    pub fn window_of(&self, hwnd: HWND) -> Option<WindowId> {
        self.by_hwnd.get(&(hwnd.0 as isize)).copied()
    }
}

pub(crate) struct App {
    handler: RefCell<Option<Box<dyn AppHandler>>>,
    queue: RefCell<VecDeque<Pending>>,
    pub inner: RefCell<Inner>,
    pub hinstance: HINSTANCE,
    pub msg_hwnd: Cell<HWND>,
    pub dcomp: Option<DComp>,
    pub tsf: Option<Tsf>,
    pub keys: RefCell<KeyTranslator>,
    /// A high surrogate from a lone `WM_CHAR`, waiting for its low half.
    pub high_surrogate: Cell<Option<u16>>,
    wake_pending: Arc<AtomicBool>,
    /// Accessibility events from any thread, for the main thread.
    pub a11y_queue: Arc<Mutex<Vec<(WindowId, A11yEvent)>>>,
    timer: TimerThread,
    clock: Arc<OsClock>,
    exit: Cell<bool>,
    seq: Cell<u64>,
    /// The drag `start_drag` asked for, run when `WM_SKELTER_DRAG` arrives.
    drag: RefCell<Option<(DragId, drag::DragData)>>,
    next_drag: Cell<u64>,
    weak: Weak<App>,
}

impl App {
    pub fn now(&self) -> Time {
        self.clock.now()
    }

    /// Queues an input event (stamped with the next [`InputSeq`]) and runs the handler if it is free.
    pub fn input(&self, window: WindowId, os_time: Time, event: InputEvent) {
        let seq = self.seq.get() + 1;
        self.seq.set(seq);
        let env = InputEnvelope { seq: InputSeq(seq), window, os_time, recv_time: self.now(), event };
        self.deliver(Pending::Input(env));
    }

    /// Queues an input event without running the handler (IME edits made inside an OS call);
    /// [`App::flush`] delivers it.
    pub fn enqueue_input(&self, window: WindowId, event: InputEvent) {
        let seq = self.seq.get() + 1;
        self.seq.set(seq);
        let now = self.now();
        let env = InputEnvelope { seq: InputSeq(seq), window, os_time: now, recv_time: now, event };
        self.queue.borrow_mut().push_back(Pending::Input(env));
    }

    pub fn window_event(&self, window: WindowId, ev: WindowEvent) {
        self.deliver(Pending::Window(window, ev));
    }

    /// Delivers what was queued without running the handler.
    pub fn flush(&self) {
        if !self.queue.borrow().is_empty() {
            self.drain();
        }
    }

    /// Asks the app whether the user may quit (logoff/shutdown); `true` while the handler is busy.
    pub fn quit_requested(&self) -> bool {
        let Some(this) = self.weak.upgrade() else { return true };
        let Some(mut handler) = self.handler.borrow_mut().take() else { return true };
        let mut cx = Cx { app: &this, clipboard: Win32Clipboard { owner: self.msg_hwnd.get() } };
        let ok = handler.quit_requested(&mut cx);
        *self.handler.borrow_mut() = Some(handler);
        ok
    }

    pub fn deliver(&self, ev: Pending) {
        self.queue.borrow_mut().push_back(ev);
        self.drain();
    }

    /// Runs the handler over the queue unless it is already running further up the stack (then
    /// that call delivers what was queued when it returns).
    fn drain(&self) {
        let Some(this) = self.weak.upgrade() else { return };
        loop {
            let Some(mut handler) = self.handler.borrow_mut().take() else { return };
            loop {
                let next = self.queue.borrow_mut().pop_front();
                let Some(ev) = next else { break };
                let mut cx = Cx { app: &this, clipboard: Win32Clipboard { owner: self.msg_hwnd.get() } };
                match ev {
                    Pending::Init => handler.init(&mut cx),
                    Pending::Input(env) => handler.input(&mut cx, env),
                    Pending::Window(w, e) => handler.window_event(&mut cx, w, e),
                    Pending::Wake => handler.wake(&mut cx),
                    Pending::Timer => handler.timer(&mut cx, self.now()),
                    Pending::RawMotion(w, d) => handler.raw_mouse_motion(&mut cx, w, d),
                    Pending::A11y(w, e) => handler.a11y_event(&mut cx, w, e),
                    Pending::DragEnded(d, o) => handler.drag_ended(&mut cx, d, o),
                }
            }
            *self.handler.borrow_mut() = Some(handler);
            self.after_handler();
            if self.queue.borrow().is_empty() {
                break;
            }
        }
        if self.exit.get() {
            unsafe { PostQuitMessage(0) };
        }
    }

    /// With the handler back: TSF sees the app's edits and gets locks it asked for meanwhile.
    fn after_handler(&self) {
        let docs: Vec<Rc<TsfDocument>> = match self.inner.try_borrow() {
            Ok(inner) => inner.windows.values().filter(|w| w.focused).filter_map(|w| w.tsf.clone()).collect(),
            Err(_) => return,
        };
        for doc in docs {
            doc.grant_pending();
            doc.sync();
        }
    }

    /// The IME's view of `window`'s text field ([`crate::shadow::Field`]); edits `f` makes to a shadow
    /// are queued as input events. `false` when the window has none.
    pub fn with_text_client(&self, window: WindowId, f: &mut dyn FnMut(&mut dyn TextInputClient)) -> bool {
        let shadow = match self.inner.try_borrow() {
            Ok(inner) => inner.windows.get(&window).map(|w| w.field.clone()),
            Err(_) => None,
        };
        let Some(shadow) = shadow else { return false };
        let events = {
            let Ok(mut field) = shadow.try_borrow_mut() else { return false };
            f(field.client());
            field.take_events()
        };
        for ev in events {
            self.enqueue_input(window, InputEvent::Ime(ev));
        }
        true
    }

    pub fn client_access(&self) -> ClientAccess {
        let weak = self.weak.clone();
        Rc::new(move |w, f| weak.upgrade().is_some_and(|a| a.with_text_client(w, f)))
    }

    pub fn alloc_window_id(&self) -> WindowId {
        let mut inner = self.inner.borrow_mut();
        inner.next_window += 1;
        WindowId(inner.next_window)
    }

    fn alloc_surface_id(&self) -> SurfaceId {
        let mut inner = self.inner.borrow_mut();
        inner.next_surface += 1;
        SurfaceId(inner.next_surface)
    }

    pub fn hwnd(&self, id: WindowId) -> Option<HWND> {
        self.inner.borrow().windows.get(&id).map(|w| w.hwnd)
    }

    /// The message window's wake: runs DirectManipulation for running gestures (a frame passed) and
    /// tells the app.
    pub fn on_wake(&self) {
        self.wake_pending.store(false, Ordering::Release);
        window::pump_gestures(self);
        self.deliver(Pending::Wake);
    }

    pub fn on_a11y(&self) {
        let events = std::mem::take(&mut *self.a11y_queue.lock().unwrap_or_else(|e| e.into_inner()));
        for (w, e) in events {
            self.queue.borrow_mut().push_back(Pending::A11y(w, e));
        }
        self.flush();
    }

    /// Runs the drag `start_drag` queued (`DoDragDrop`'s modal loop keeps dispatching messages, so the
    /// app keeps getting events and frames) and reports how it ended.
    pub fn on_drag(&self) {
        let Some((id, data)) = self.drag.borrow_mut().take() else { return };
        let outcome = match drag::run_drag(data) {
            drag::DragResult::Copied | drag::DragResult::Moved => DragOutcome::Dropped,
            drag::DragResult::Cancelled => DragOutcome::Cancelled,
        };
        self.deliver(Pending::DragEnded(id, outcome));
    }

    pub fn raw_motion(&self, window: WindowId, delta: Vec2) {
        self.deliver(Pending::RawMotion(window, delta));
    }

    pub fn on_timer(&self) {
        self.deliver(Pending::Timer);
    }
}

pub(crate) fn os_err(call: &str, e: windows_core::Error) -> PlatformError {
    PlatformError::Os(format!("{call}: {e}"))
}

/// The deadline thread: a high-resolution waitable timer whose expiry is posted to the message
/// window, so app timers fire inside modal loops too.
struct TimerThread {
    shared: Arc<TimerShared>,
    thread: Option<std::thread::JoinHandle<()>>,
}

struct TimerShared {
    state: Mutex<TimerState>,
    rearm: SendHandle,
}

#[derive(Default)]
struct TimerState {
    deadline: Option<Time>,
    quit: bool,
}

#[derive(Clone, Copy)]
struct SendHandle(HANDLE);
// SAFETY: kernel handles may be used from any thread.
unsafe impl Send for SendHandle {}
unsafe impl Sync for SendHandle {}

impl TimerThread {
    fn spawn(msg_hwnd: HWND, clock: Arc<OsClock>) -> Result<Self, PlatformError> {
        let rearm = unsafe { CreateEventW(None, false, false, None) }.map_err(|e| os_err("CreateEventW", e))?;
        let shared = Arc::new(TimerShared { state: Mutex::new(TimerState::default()), rearm: SendHandle(rearm) });
        let target = msg_hwnd.0 as isize;
        let s = shared.clone();
        let thread = std::thread::Builder::new()
            .name("skelter-timer".into())
            .spawn(move || timer_loop(&s, target, &*clock))
            .map_err(|e| PlatformError::Os(format!("timer thread: {e}")))?;
        Ok(Self { shared, thread: Some(thread) })
    }

    fn set(&self, at: Option<Time>) {
        self.shared.state.lock().unwrap_or_else(|e| e.into_inner()).deadline = at;
        let _ = unsafe { SetEvent(self.shared.rearm.0) };
    }
}

impl Drop for TimerThread {
    fn drop(&mut self) {
        self.shared.state.lock().unwrap_or_else(|e| e.into_inner()).quit = true;
        let _ = unsafe { SetEvent(self.shared.rearm.0) };
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn timer_loop(shared: &TimerShared, target: isize, clock: &dyn Clock) {
    // High resolution needs Windows 10 1803; older systems get the default resolution.
    let timer = unsafe { CreateWaitableTimerExW(None, None, CREATE_WAITABLE_TIMER_HIGH_RESOLUTION, TIMER_ALL_ACCESS.0) }
        .or_else(|_| unsafe { CreateWaitableTimerExW(None, None, 0, TIMER_ALL_ACCESS.0) });
    let Ok(timer) = timer else { return };
    let post = || {
        let _ = unsafe { PostMessageW(Some(HWND(target as *mut _)), WM_SKELTER_TIMER, WPARAM(0), LPARAM(0)) };
    };
    loop {
        let (deadline, quit) = {
            let st = shared.state.lock().unwrap_or_else(|e| e.into_inner());
            (st.deadline, st.quit)
        };
        if quit {
            return;
        }
        let Some(at) = deadline else {
            unsafe { WaitForSingleObject(shared.rearm.0, INFINITE) };
            continue;
        };
        let now = clock.now();
        if at <= now {
            let mut st = shared.state.lock().unwrap_or_else(|e| e.into_inner());
            if st.deadline == Some(at) {
                st.deadline = None;
                drop(st);
                post();
            }
            continue;
        }
        // Relative due time in 100 ns units (negative = relative).
        let due = -(((at - now).0 / 100).max(1));
        if unsafe { SetWaitableTimer(timer, &due, 0, None, None, false) }.is_err() {
            return;
        }
        let r = unsafe { WaitForMultipleObjects(&[timer, shared.rearm.0], false, INFINITE) };
        if r == WAIT_OBJECT_0 {
            let mut st = shared.state.lock().unwrap_or_else(|e| e.into_inner());
            if st.deadline == Some(at) {
                st.deadline = None;
                drop(st);
                post();
            }
        }
    }
}

/// The [`PlatformCx`] handed to the handler.
pub(crate) struct Cx<'a> {
    app: &'a Rc<App>,
    clipboard: Win32Clipboard,
}

impl PlatformCx for Cx<'_> {
    fn try_create_window(&mut self, desc: WindowDesc) -> Result<WindowId, PlatformError> {
        window::create(self.app, desc)
    }

    fn close_window(&mut self, id: WindowId) {
        if let Some(hwnd) = self.app.hwnd(id) {
            let _ = unsafe { DestroyWindow(hwnd) };
        }
    }

    fn surface_target(&self, id: WindowId) -> SurfaceTarget {
        let inner = self.app.inner.borrow();
        match inner.windows.get(&id) {
            None => SurfaceTarget::None,
            Some(w) => match &w.tree {
                Some(tree) => SurfaceTarget::CompositionVisual(tree.main_visual_ptr()),
                None => window::raw_target(w.hwnd, self.app.hinstance),
            },
        }
    }

    fn create_os_layer(&mut self, id: WindowId, desc: OsLayerDesc) -> Option<(SurfaceId, SurfaceTarget)> {
        let dcomp = self.app.dcomp.as_ref()?;
        let surface = self.app.alloc_surface_id();
        let mut inner = self.app.inner.borrow_mut();
        let w = inner.windows.get_mut(&id)?;
        let scale = w.scale;
        let tree = w.tree.as_mut()?;
        tree.add_layer(dcomp, surface, desc, scale).ok()?;
        let ptr = tree.layer_visual_ptr(surface)?;
        Some((surface, SurfaceTarget::CompositionVisual(ptr)))
    }

    fn set_os_layer_bounds(&mut self, id: WindowId, layer: SurfaceId, bounds: Rect) {
        let Some(dcomp) = self.app.dcomp.as_ref() else { return };
        let mut inner = self.app.inner.borrow_mut();
        if let Some(w) = inner.windows.get_mut(&id) {
            let scale = w.scale;
            if let Some(tree) = w.tree.as_mut() {
                let _ = tree.set_layer_bounds(dcomp, layer, bounds, scale);
            }
        }
    }

    fn remove_os_layer(&mut self, id: WindowId, layer: SurfaceId) {
        let Some(dcomp) = self.app.dcomp.as_ref() else { return };
        let mut inner = self.app.inner.borrow_mut();
        if let Some(tree) = inner.windows.get_mut(&id).and_then(|w| w.tree.as_mut()) {
            let _ = tree.remove_layer(dcomp, layer);
        }
    }

    /// Holds DirectComposition commits so the layer's new bounds and the compositor's present at the new
    /// size land in one commit at [`PlatformCx::end_os_layer_resize`].
    fn begin_os_layer_resize(&mut self, id: WindowId, _layer: SurfaceId) -> bool {
        let Some(dcomp) = self.app.dcomp.as_ref() else { return false };
        if self.app.inner.borrow().windows.get(&id).and_then(|w| w.tree.as_ref()).is_none() {
            return false;
        }
        dcomp.begin_deferral();
        true
    }

    fn end_os_layer_resize(&mut self, id: WindowId, _layer: SurfaceId) {
        let Some(dcomp) = self.app.dcomp.as_ref() else { return };
        if self.app.inner.borrow().windows.get(&id).and_then(|w| w.tree.as_ref()).is_some() {
            let _ = dcomp.end_deferral();
        }
    }

    /// Commits the DirectComposition device (wgpu sets a visual's content but never commits).
    fn os_layer_committer(&self, id: WindowId) -> Option<OsLayerCommitter> {
        let inner = self.app.inner.borrow();
        inner.windows.get(&id)?.tree.as_ref()?;
        let handle = self.app.dcomp.as_ref()?.commit_handle();
        Some(OsLayerCommitter::new(move || {
            let _ = handle.commit();
        }))
    }

    fn set_cursor(&mut self, id: WindowId, c: CursorIcon) {
        window::set_cursor(self.app, id, c);
    }

    fn set_ime(&mut self, id: WindowId, state: Option<ImeState>) {
        window::set_ime(self.app, id, state);
    }

    fn set_text_input(&mut self, id: WindowId, client: Option<Box<dyn TextInputClient>>) {
        window::set_text_input(self.app, id, client);
    }

    fn end_ime_composition(&mut self, id: WindowId, commit: bool) -> bool {
        window::end_composition(self.app, id, commit)
    }

    fn clipboard(&mut self) -> &mut dyn Clipboard {
        &mut self.clipboard
    }

    fn set_timer(&mut self, at: Option<Time>) {
        self.app.timer.set(at);
    }

    fn waker(&self) -> Waker {
        let pending = self.app.wake_pending.clone();
        let target = self.app.msg_hwnd.get().0 as isize;
        Waker::new(move || {
            if !pending.swap(true, Ordering::AcqRel) {
                let posted =
                    unsafe { PostMessageW(Some(HWND(target as *mut _)), WM_SKELTER_WAKE, WPARAM(0), LPARAM(0)) };
                if posted.is_err() {
                    pending.store(false, Ordering::Release);
                }
            }
        })
    }

    fn clock(&self) -> Arc<dyn Clock> {
        self.app.clock.clone()
    }

    fn monitors(&self) -> Vec<Monitor> {
        monitor::monitors()
    }

    fn monitor_of(&self, id: WindowId) -> Option<Monitor> {
        monitor::monitor_info(monitor::monitor_handle(self.app.hwnd(id)?))
    }

    fn create_popup(&mut self, desc: WindowDesc, popup: PopupDesc) -> Result<WindowId, PlatformError> {
        window::create_popup(self.app, desc, &popup)
    }

    fn create_child_window(&mut self, desc: WindowDesc, parent: WindowId) -> Result<WindowId, PlatformError> {
        let owner = self.app.hwnd(parent).ok_or_else(|| PlatformError::Os("parent window is gone".into()))?;
        window::create_owned(self.app, desc, Some(owner))
    }

    fn set_window_state(&mut self, id: WindowId, req: WindowStateRequest) -> bool {
        window::set_window_state(self.app, id, req)
    }

    /// Text and `file:` URIs only (see [`drag::from_platform`]); the drag starts once the handler returns.
    fn start_drag(&mut self, window: WindowId, data: DragData) -> Result<DragId, PlatformError> {
        if self.app.hwnd(window).is_none() {
            return Err(PlatformError::Os("drag source window is gone".into()));
        }
        let data = drag::from_platform(&data).ok_or(PlatformError::Unsupported("drag payloads other than text and files"))?;
        if self.app.drag.borrow().is_some() {
            return Err(PlatformError::Os("a drag is already starting".into()));
        }
        let id = DragId(self.app.next_drag.get() + 1);
        self.app.next_drag.set(id.0);
        *self.app.drag.borrow_mut() = Some((id, data));
        let posted = unsafe { PostMessageW(Some(self.app.msg_hwnd.get()), WM_SKELTER_DRAG, WPARAM(0), LPARAM(0)) };
        if let Err(e) = posted {
            self.app.drag.borrow_mut().take();
            return Err(os_err("PostMessageW (drag)", e));
        }
        Ok(id)
    }

    fn set_hit_test_regions(&mut self, id: WindowId, regions: &[HitRegion]) {
        if let Some(w) = self.app.inner.borrow_mut().windows.get_mut(&id) {
            w.regions = regions.to_vec();
        }
    }

    fn set_mouse_capture(&mut self, id: WindowId, capture: bool) {
        window::set_mouse_capture(self.app, id, capture);
    }

    fn set_title(&mut self, id: WindowId, title: &str) {
        window::set_title(self.app, id, title);
    }

    fn set_visible(&mut self, id: WindowId, visible: bool) {
        window::set_visible(self.app, id, visible);
    }

    fn drag_window(&mut self, id: WindowId) {
        window::drag_window(self.app, id);
    }

    fn update_a11y(&mut self, id: WindowId, update: accesskit::TreeUpdate) {
        window::update_a11y(self.app, id, update);
    }

    fn window_scale(&self, id: WindowId) -> Option<Scale> {
        self.app.inner.borrow().windows.get(&id).map(|w| w.scale)
    }

    /// The window keeps a handle to boost the compositor clock while a touchpad gesture runs.
    fn frame_source(&mut self, id: WindowId) -> Option<Arc<dyn FrameSource>> {
        let frames = Arc::new(Win32Frames::new());
        self.app.inner.borrow_mut().windows.get_mut(&id)?.frames = Some(frames.clone());
        Some(frames)
    }

    fn exit(&mut self) {
        self.app.exit.set(true);
    }
}

/// Runs the app until it calls [`PlatformCx::exit`] (or the last `WM_QUIT`).
pub fn run(handler: Box<dyn AppHandler>) -> Result<(), PlatformError> {
    if current().is_some() {
        return Err(PlatformError::EventLoop("already running on this thread".into()));
    }
    unsafe { OleInitialize(None) }.map_err(|e| os_err("OleInitialize", e))?;
    let result = run_inner(handler);
    unsafe { OleUninitialize() };
    result
}

fn run_inner(handler: Box<dyn AppHandler>) -> Result<(), PlatformError> {
    unsafe {
        // Fails when a manifest already chose the awareness; per-monitor v2 is what both ask for.
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        // Fails when the process already enabled it; mouse input then still comes as WM_POINTER.
        let _ = EnableMouseInPointer(true);
    }
    let hinstance: HINSTANCE = unsafe { GetModuleHandleW(None) }.map_err(|e| os_err("GetModuleHandleW", e))?.into();
    window::register_class(hinstance)?;
    let class = wide(window::CLASS_NAME);
    let msg_hwnd = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            windows_core::PCWSTR(class.as_ptr()),
            None,
            WINDOW_STYLE(0),
            0,
            0,
            0,
            0,
            Some(HWND_MESSAGE),
            None,
            Some(hinstance),
            None,
        )
    }
    .map_err(|e| os_err("CreateWindowExW (message window)", e))?;
    let clock = Arc::new(OsClock);
    let timer = TimerThread::spawn(msg_hwnd, clock.clone())?;
    let app = Rc::new_cyclic(|weak| App {
        handler: RefCell::new(Some(handler)),
        queue: RefCell::new(VecDeque::new()),
        inner: RefCell::new(Inner { windows: HashMap::new(), by_hwnd: HashMap::new(), next_window: 0, next_surface: 0 }),
        hinstance,
        msg_hwnd: Cell::new(msg_hwnd),
        dcomp: DComp::new().ok(),
        tsf: Tsf::new().ok(),
        keys: RefCell::new(KeyTranslator::default()),
        high_surrogate: Cell::new(None),
        wake_pending: Arc::new(AtomicBool::new(false)),
        a11y_queue: Arc::new(Mutex::new(Vec::new())),
        timer,
        clock,
        exit: Cell::new(false),
        seq: Cell::new(0),
        drag: RefCell::new(None),
        next_drag: Cell::new(0),
        weak: weak.clone(),
    });
    APP.with(|a| *a.borrow_mut() = Some(app.clone()));
    app.deliver(Pending::Init);

    let mut msg = MSG::default();
    while !app.exit.get() {
        let r = unsafe { GetMessageW(&mut msg, None, 0, 0) }.0;
        if r == 0 || r == -1 {
            break;
        }
        if app.tsf.as_ref().is_some_and(|t| t.filter_key(&msg)) {
            app.flush();
            continue;
        }
        unsafe {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        app.flush();
    }

    let exiting = app.handler.borrow_mut().take();
    if let Some(mut handler) = exiting {
        let mut cx = Cx { app: &app, clipboard: Win32Clipboard { owner: msg_hwnd } };
        handler.exiting(&mut cx);
        *app.handler.borrow_mut() = Some(handler);
    }

    let hwnds: Vec<HWND> = app.inner.borrow().windows.values().map(|w| w.hwnd).collect();
    for hwnd in hwnds {
        let _ = unsafe { DestroyWindow(hwnd) };
    }
    let _ = unsafe { DestroyWindow(msg_hwnd) };
    APP.with(|a| a.borrow_mut().take());
    Ok(())
}
