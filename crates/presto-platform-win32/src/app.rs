//! The event loop: one UI thread (an OLE STA) running `GetMessage`, the app's handler, and the
//! policies that keep presto responsive where winit's loop stalls (`round2/winit.md` §4.1).
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
//! - **Native modals from the app** (an OLE drag) start after the handler returns, never inside it.
//! - **Keys** pass through the TSF keystroke manager before `TranslateMessage`, so the active TIP
//!   sees them first (IME-first routing, architecture §11.2).

use crate::clipboard::Win32Clipboard;
use crate::dcomp::DComp;
use crate::drag;
use crate::frame::Win32FrameSource;
use crate::keyboard::KeyTranslator;
use crate::monitor;
use crate::shim::*;
use crate::tsf::{ClientAccess, Tsf, TsfDocument};
use crate::util::{QpcClock, wide};
use crate::window::{self, Window};
use presto_base::cursor::CursorIcon;
use presto_base::event::{InputEnvelope, InputEvent, WindowEvent};
use presto_base::ids::{InputSeq, SurfaceId, WindowId};
use presto_base::input::TextInputClient;
use presto_base::time::{Clock, Time};
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
pub(crate) const WM_PRESTO_WAKE: u32 = WM_APP + 1;
/// Posted to the message window when the timer's deadline passes.
pub(crate) const WM_PRESTO_TIMER: u32 = WM_APP + 2;

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
    DragFinished(DragResult),
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
    timer: TimerThread,
    clock: Arc<QpcClock>,
    exit: Cell<bool>,
    seq: Cell<u64>,
    drag: RefCell<Option<DragData>>,
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

    pub fn window_event(&self, window: WindowId, ev: WindowEvent) {
        self.deliver(Pending::Window(window, ev));
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
                    Pending::DragFinished(r) => handler.drag_finished(&mut cx, r),
                }
            }
            *self.handler.borrow_mut() = Some(handler);
            self.after_handler();
            let drag = self.drag.borrow_mut().take();
            if let Some(data) = drag {
                let result = drag::run_drag(data);
                self.queue.borrow_mut().push_back(Pending::DragFinished(result));
            }
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

    /// The handler's text field in `window`, while the handler is not running.
    pub fn with_text_client(&self, window: WindowId, f: &mut dyn FnMut(&mut dyn TextInputClient)) -> bool {
        let Ok(mut slot) = self.handler.try_borrow_mut() else { return false };
        let Some(handler) = slot.as_mut() else { return false };
        match handler.text_input(window) {
            Some(client) => {
                f(client);
                true
            }
            None => false,
        }
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

    pub fn on_timer(&self) {
        self.deliver(Pending::Timer);
    }
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
    fn spawn(msg_hwnd: HWND, clock: Arc<QpcClock>) -> Result<Self, PlatformError> {
        let rearm = unsafe { CreateEventW(None, false, false, None) }.map_err(|e| PlatformError::win("CreateEventW", e))?;
        let shared = Arc::new(TimerShared { state: Mutex::new(TimerState::default()), rearm: SendHandle(rearm) });
        let target = msg_hwnd.0 as isize;
        let s = shared.clone();
        let thread = std::thread::Builder::new()
            .name("presto-timer".into())
            .spawn(move || timer_loop(&s, target, &*clock))
            .map_err(|_| PlatformError::Os { call: "spawn timer thread", code: 0 })?;
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
        let _ = unsafe { PostMessageW(Some(HWND(target as *mut _)), WM_PRESTO_TIMER, WPARAM(0), LPARAM(0)) };
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
    fn create_window(&mut self, desc: WindowDesc) -> WindowId {
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
                None => SurfaceTarget::Win32 { hwnd: w.hwnd.0 as isize, hinstance: self.app.hinstance.0 as isize },
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

    fn update_os_layer(&mut self, id: WindowId, layer: SurfaceId, desc: OsLayerDesc) {
        let Some(dcomp) = self.app.dcomp.as_ref() else { return };
        let mut inner = self.app.inner.borrow_mut();
        if let Some(w) = inner.windows.get_mut(&id) {
            let scale = w.scale;
            if let Some(tree) = w.tree.as_mut() {
                let _ = tree.update_layer(dcomp, layer, desc, scale);
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

    fn set_cursor(&mut self, id: WindowId, c: CursorIcon) {
        window::set_cursor(self.app, id, c);
    }

    fn set_ime(&mut self, id: WindowId, state: Option<ImeState>) {
        window::set_ime(self.app, id, state);
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
                    unsafe { PostMessageW(Some(HWND(target as *mut _)), WM_PRESTO_WAKE, WPARAM(0), LPARAM(0)) };
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

    fn set_hit_test_regions(&mut self, id: WindowId, regions: &[HitRegion]) {
        if let Some(w) = self.app.inner.borrow_mut().windows.get_mut(&id) {
            w.regions = regions.to_vec();
        }
    }

    fn exit(&mut self) {
        self.app.exit.set(true);
    }

    fn frame_source(&mut self, id: WindowId) -> Option<Box<dyn FrameSource>> {
        self.app.hwnd(id)?;
        Some(Box::new(Win32FrameSource::new()))
    }

    fn start_drag(&mut self, id: WindowId, data: DragData) {
        if self.app.hwnd(id).is_some() {
            *self.app.drag.borrow_mut() = Some(data);
        }
    }

    fn set_pointer_capture(&mut self, id: WindowId, capture: bool) {
        window::set_pointer_capture(self.app, id, capture);
    }

    fn set_title(&mut self, id: WindowId, title: &str) {
        window::set_title(self.app, id, title);
    }

    fn set_visible(&mut self, id: WindowId, visible: bool) {
        window::set_visible(self.app, id, visible);
    }

    fn set_window_state(&mut self, id: WindowId, state: WindowState) {
        window::set_state(self.app, id, state);
    }

    fn end_composition(&mut self, id: WindowId, commit: bool) {
        let doc = self.app.inner.borrow().windows.get(&id).and_then(|w| w.tsf.clone());
        if let Some(doc) = doc {
            doc.end_composition(commit);
        }
    }
}

/// Runs the app until it calls [`PlatformCx::exit`] (or the last `WM_QUIT`).
pub fn run(handler: Box<dyn AppHandler>) -> Result<(), PlatformError> {
    if current().is_some() {
        return Err(PlatformError::Unsupported("run is already running on this thread"));
    }
    unsafe { OleInitialize(None) }.map_err(|e| PlatformError::win("OleInitialize", e))?;
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
    let hinstance: HINSTANCE = unsafe { GetModuleHandleW(None) }.map_err(|e| PlatformError::win("GetModuleHandleW", e))?.into();
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
    .map_err(|e| PlatformError::win("CreateWindowExW (message window)", e))?;
    let clock = Arc::new(QpcClock);
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
        timer,
        clock,
        exit: Cell::new(false),
        seq: Cell::new(0),
        drag: RefCell::new(None),
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
            continue;
        }
        unsafe {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    let hwnds: Vec<HWND> = app.inner.borrow().windows.values().map(|w| w.hwnd).collect();
    for hwnd in hwnds {
        let _ = unsafe { DestroyWindow(hwnd) };
    }
    let _ = unsafe { DestroyWindow(msg_hwnd) };
    APP.with(|a| a.borrow_mut().take());
    Ok(())
}
