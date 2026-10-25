//! The Windows shim: a `SetWindowSubclass` procedure on winit's HWND and a DirectComposition tree.
//!
//! The subclass answers what winit does not:
//! - `WM_NCHITTEST` from the app's hit regions (custom title bar; `HTMAXBUTTON` gets Windows 11 snap
//!   layouts), with the caption buttons' clicks turned into the system commands;
//! - modal loops (`WM_ENTERSIZEMOVE…WM_EXITSIZEMOVE`, menu loops): winit ignores `WaitUntil` there
//!   (winit #3272), so a `SetTimer` posts the app's timer ticks, and `WM_PAINT` is validated instead of
//!   re-arming a redraw on every paint (winit #4708);
//! - pointer history from `GetMouseMovePointsEx` for each `WM_MOUSEMOVE`;
//! - `WM_IME_REQUEST`: per-character candidate placement (`IMR_QUERYCHARPOSITION`) and context for
//!   the IME (`IMR_DOCUMENTFEED`); composition commit and cancel through `ImmNotifyIME`.
//!
//! With `os_compositing` the window has no redirection bitmap and its content is a DirectComposition
//! visual; OS layers are sibling visuals above it.

use super::ShimHooks;
use crate::api::{hit_test, HitKind, HitRegion, ImeState, OsLayerCommitter, OsLayerDesc, SurfaceTarget, WindowDesc};
use crate::clock;
use presto_base::event::PointerSample;
use presto_base::geom::{DeviceRect, Point, Rect};
use presto_base::ids::SurfaceId;
use presto_base::time::{Span, Time};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use smallvec::SmallVec;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use windows::core::Interface;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::DirectComposition::{
    DCompositionCreateDevice2, IDCompositionDesktopDevice, IDCompositionTarget, IDCompositionVisual, IDCompositionVisual2,
};
use windows::Win32::Graphics::Gdi::{ClientToScreen, GetMonitorInfoW, ScreenToClient, ValidateRect, HMONITOR, MONITORINFO};
use windows::Win32::System::SystemInformation::GetTickCount;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::Ime::{
    ImmGetContext, ImmNotifyIME, ImmReleaseContext, CPS_CANCEL, CPS_COMPLETE, IMECHARPOSITION, IMR_DOCUMENTFEED,
    IMR_QUERYCHARPOSITION, NI_COMPOSITIONSTR, RECONVERTSTRING,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetMouseMovePointsEx, GMMP_USE_DISPLAY_POINTS, MOUSEMOVEPOINT};
use windows::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    GetMessageTime, IsZoomed, KillTimer, PostMessageW, SetTimer, HTCAPTION, HTCLIENT, HTCLOSE, HTMAXBUTTON, HTMINBUTTON,
    HTSYSMENU, SC_CLOSE, SC_MAXIMIZE, SC_MINIMIZE, SC_RESTORE, WM_ENTERMENULOOP, WM_ENTERSIZEMOVE, WM_EXITMENULOOP,
    WM_EXITSIZEMOVE, WM_IME_REQUEST, WM_MOUSEMOVE, WM_NCDESTROY, WM_NCHITTEST, WM_NCLBUTTONDBLCLK, WM_NCLBUTTONDOWN,
    WM_NCLBUTTONUP, WM_PAINT, WM_SYSCOMMAND, WM_TIMER,
};
use winit::platform::windows::{MonitorHandleExtWindows, WindowAttributesExtWindows};
use winit::window::{Window, WindowAttributes};

const SUBCLASS_ID: usize = 0x7072_6573; // "pres"
const TIMER_ID: usize = 0x7072_6574;
/// `GetMouseMovePointsEx` keeps the last 64 points.
const HISTORY_MAX: usize = 64;

/// State the subclass procedure shares with the shim (main thread only; the mutexes guard against
/// re-entrant messages, not threads).
struct Shared {
    hooks: ShimHooks,
    regions: Mutex<Vec<HitRegion>>,
    in_modal: AtomicBool,
    timer: Mutex<Option<Time>>,
    history: Mutex<SmallVec<[PointerSample; 4]>>,
    last_point: Mutex<Option<MOUSEMOVEPOINT>>,
    ime: Mutex<Option<ImeState>>,
}

struct Dcomp {
    device: IDCompositionDesktopDevice,
    _target: IDCompositionTarget,
    root: IDCompositionVisual2,
    content: IDCompositionVisual2,
    /// Bottom to top, above `content`.
    layers: Vec<(SurfaceId, IDCompositionVisual2)>,
}

pub struct WindowShim {
    hwnd: HWND,
    shared: Arc<Shared>,
    dcomp: Option<Dcomp>,
    hooks: ShimHooks,
}

fn hwnd_of(window: &Window) -> Option<HWND> {
    match window.window_handle().ok()?.as_raw() {
        RawWindowHandle::Win32(h) => Some(HWND(h.hwnd.get() as *mut _)),
        _ => None,
    }
}

fn scale_of(hwnd: HWND) -> f32 {
    // SAFETY: `hwnd` is a live window owned by this thread.
    let dpi = unsafe { GetDpiForWindow(hwnd) };
    if dpi == 0 { 1.0 } else { dpi as f32 / 96.0 }
}

/// A `GetTickCount`-based OS time (milliseconds since boot, wrapping) on presto's clock; `None` when
/// it is not plausibly recent (a message older than 10 s, or no message time at all).
fn time_from_ticks(t: u32) -> Option<Time> {
    // SAFETY: plain query.
    let now = unsafe { GetTickCount() };
    let age = now.wrapping_sub(t);
    (age < 10_000).then(|| clock::now() - Span::from_millis(age as i64))
}

fn lparam_point(l: LPARAM) -> POINT {
    // GET_X_LPARAM / GET_Y_LPARAM: signed 16-bit halves.
    POINT { x: (l.0 & 0xFFFF) as u16 as i16 as i32, y: ((l.0 >> 16) & 0xFFFF) as u16 as i16 as i32 }
}

impl WindowShim {
    pub fn install(window: &Window, desc: &WindowDesc, hooks: ShimHooks) -> Self {
        let hwnd = hwnd_of(window).expect("winit windows on Windows have an HWND");
        let shared = Arc::new(Shared {
            hooks: hooks.clone(),
            regions: Mutex::new(Vec::new()),
            in_modal: AtomicBool::new(false),
            timer: Mutex::new(None),
            history: Mutex::new(SmallVec::new()),
            last_point: Mutex::new(None),
            ime: Mutex::new(None),
        });
        let raw = Arc::into_raw(shared.clone());
        // SAFETY: `hwnd` is winit's live window on this thread; the reference passed as `dwrefdata`
        // is released in `subclass_proc` on WM_NCDESTROY (or below if installing fails).
        let ok = unsafe { SetWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID, raw as usize) };
        if !ok.as_bool() {
            log::warn!("presto-platform: SetWindowSubclass failed; hit testing, modal timers and pointer history are off");
            // SAFETY: the subclass was not installed, so this is the only owner of that reference.
            drop(unsafe { Arc::from_raw(raw) });
        }
        let dcomp = if desc.os_compositing {
            match create_dcomp(hwnd) {
                Ok(d) => Some(d),
                Err(e) => {
                    log::warn!("presto-platform: DirectComposition unavailable ({e}); OS layers are off");
                    None
                }
            }
        } else {
            None
        };
        WindowShim { hwnd, shared, dcomp, hooks }
    }

    pub fn window_event(&mut self, _ev: &winit::event::WindowEvent) {}

    pub fn surface_target(&self) -> Option<SurfaceTarget> {
        let d = self.dcomp.as_ref()?;
        let v: IDCompositionVisual = d.content.cast().ok()?;
        Some(SurfaceTarget::CompositionVisual(v.as_raw()))
    }

    pub fn create_layer(&mut self, _window: &Window, id: SurfaceId, desc: OsLayerDesc, scale: f32) -> Option<SurfaceTarget> {
        let d = self.dcomp.as_mut()?;
        let r = (|| -> windows::core::Result<IDCompositionVisual2> {
            // SAFETY: COM calls on objects this shim owns, on the thread that created them.
            unsafe {
                let v = d.device.CreateVisual()?;
                v.SetOffsetX2(desc.bounds.min.x * scale)?;
                v.SetOffsetY2(desc.bounds.min.y * scale)?;
                let top = d.layers.last().map_or(&d.content, |(_, v)| v);
                d.root.AddVisual(&v, true, top)?;
                d.device.Commit()?;
                Ok(v)
            }
        })();
        match r {
            Ok(v) => {
                let raw: IDCompositionVisual = v.cast().ok()?;
                let ptr = raw.as_raw();
                d.layers.push((id, v));
                Some(SurfaceTarget::CompositionVisual(ptr))
            }
            Err(e) => {
                log::warn!("presto-platform: creating a DirectComposition layer failed: {e}");
                None
            }
        }
    }

    pub fn set_layer_bounds(&mut self, id: SurfaceId, bounds: Rect, scale: f32) {
        let Some(d) = &self.dcomp else { return };
        let Some((_, v)) = d.layers.iter().find(|(l, _)| *l == id) else { return };
        // SAFETY: COM calls on objects this shim owns, on their thread.
        let r = unsafe {
            v.SetOffsetX2(bounds.min.x * scale).and_then(|_| v.SetOffsetY2(bounds.min.y * scale)).and_then(|_| d.device.Commit())
        };
        if let Err(e) = r {
            log::warn!("presto-platform: moving a DirectComposition layer failed: {e}");
        }
    }

    pub fn remove_layer(&mut self, id: SurfaceId) {
        let Some(d) = &mut self.dcomp else { return };
        let Some(i) = d.layers.iter().position(|(l, _)| *l == id) else { return };
        let (_, v) = d.layers.remove(i);
        // SAFETY: COM calls on objects this shim owns, on their thread.
        if let Err(e) = unsafe { d.root.RemoveVisual(&v).and_then(|_| d.device.Commit()) } {
            log::warn!("presto-platform: removing a DirectComposition layer failed: {e}");
        }
    }

    pub fn committer(&self) -> Option<OsLayerCommitter> {
        self.dcomp.as_ref()?;
        Some(OsLayerCommitter(self.hooks.commit.clone()))
    }

    pub fn commit_layers(&mut self) {
        if let Some(d) = &self.dcomp {
            // SAFETY: COM call on the device this shim owns, on its thread.
            if let Err(e) = unsafe { d.device.Commit() } {
                log::warn!("presto-platform: DirectComposition Commit failed: {e}");
            }
        }
    }

    pub fn set_timer(&mut self, at: Option<Time>) {
        *self.shared.timer.lock().unwrap_or_else(|e| e.into_inner()) = at;
        if self.shared.in_modal.load(Ordering::SeqCst) {
            arm_modal_timer(self.hwnd, at);
        }
    }

    pub fn take_history(&mut self) -> SmallVec<[PointerSample; 4]> {
        std::mem::take(&mut *self.shared.history.lock().unwrap_or_else(|e| e.into_inner()))
    }

    /// The OS time of the message being dispatched (winit translates inside its window procedure).
    pub fn event_time(&self) -> Option<Time> {
        // SAFETY: plain query on this thread's message state.
        let t = unsafe { GetMessageTime() };
        if t == 0 { None } else { time_from_ticks(t as u32) }
    }

    pub fn set_hit_regions(&mut self, regions: &[HitRegion]) {
        *self.shared.regions.lock().unwrap_or_else(|e| e.into_inner()) = regions.to_vec();
    }

    pub fn set_ime(&mut self, state: Option<&ImeState>) {
        *self.shared.ime.lock().unwrap_or_else(|e| e.into_inner()) = state.cloned();
    }

    pub fn frame_source(&mut self, _interval: Option<Span>) -> Option<Arc<dyn crate::frame::FrameSource>> {
        static SOURCE: std::sync::OnceLock<Arc<crate::frame::windows::DwmSource>> = std::sync::OnceLock::new();
        Some(SOURCE.get_or_init(|| Arc::new(crate::frame::windows::DwmSource::new())).clone())
    }

    pub fn end_composition(&mut self, commit: bool) -> Option<bool> {
        // SAFETY: IMM calls on this thread's window; the context is released before returning.
        unsafe {
            let himc = ImmGetContext(self.hwnd);
            if himc.is_invalid() {
                return Some(false);
            }
            let ok = ImmNotifyIME(himc, NI_COMPOSITIONSTR, if commit { CPS_COMPLETE } else { CPS_CANCEL }, 0);
            let _ = ImmReleaseContext(self.hwnd, himc);
            Some(ok.as_bool())
        }
    }
}

fn create_dcomp(hwnd: HWND) -> windows::core::Result<Dcomp> {
    // SAFETY: DirectComposition objects created and used on this (the window's) thread; the target
    // binds to winit's live HWND, created without a redirection bitmap.
    unsafe {
        let device: IDCompositionDesktopDevice = DCompositionCreateDevice2(None)?;
        let target = device.CreateTargetForHwnd(hwnd, true)?;
        let root = device.CreateVisual()?;
        let content = device.CreateVisual()?;
        root.AddVisual(&content, false, None)?;
        target.SetRoot(&root)?;
        device.Commit()?;
        Ok(Dcomp { device, _target: target, root, content, layers: Vec::new() })
    }
}

fn arm_modal_timer(hwnd: HWND, at: Option<Time>) {
    // SAFETY: timer calls on this thread's window.
    unsafe {
        match at {
            Some(t) => {
                let ms = (t - clock::now()).as_millis_f64().ceil().clamp(1.0, u32::MAX as f64) as u32;
                SetTimer(Some(hwnd), TIMER_ID, ms, None);
            }
            None => {
                let _ = KillTimer(Some(hwnd), TIMER_ID);
            }
        }
    }
}

/// Collects the points `GetMouseMovePointsEx` saw since the previous `WM_MOUSEMOVE`, oldest first,
/// in client points.
fn record_history(hwnd: HWND, shared: &Shared, lparam: LPARAM) {
    let mut pt = lparam_point(lparam);
    // SAFETY: plain coordinate conversion and queries on this thread's window.
    unsafe {
        let _ = ClientToScreen(hwnd, &mut pt);
        let time = GetMessageTime() as u32;
        // Display-point coordinates are 16-bit (MSDN `GetMouseMovePointsEx`).
        let input = MOUSEMOVEPOINT { x: pt.x & 0xFFFF, y: pt.y & 0xFFFF, time, dwExtraInfo: 0 };
        let mut buf = [MOUSEMOVEPOINT::default(); HISTORY_MAX];
        let n = GetMouseMovePointsEx(std::mem::size_of::<MOUSEMOVEPOINT>() as u32, &input, &mut buf, GMMP_USE_DISPLAY_POINTS);
        let mut last = shared.last_point.lock().unwrap_or_else(|e| e.into_inner());
        let prev = last.replace(input);
        if n <= 1 {
            return;
        }
        let Some(prev) = prev else { return };
        let mut origin = POINT::default();
        let _ = ClientToScreen(hwnd, &mut origin);
        let scale = scale_of(hwnd);
        // buf[0] is the current point; walk back to the previous message's point.
        let mut newer: SmallVec<[PointerSample; 16]> = SmallVec::new();
        for p in &buf[1..n as usize] {
            if (p.x, p.y, p.time) == (prev.x, prev.y, prev.time) || p.time.wrapping_sub(prev.time) > u32::MAX / 2 {
                break;
            }
            let x = if p.x > 32767 { p.x - 65536 } else { p.x };
            let y = if p.y > 32767 { p.y - 65536 } else { p.y };
            let Some(t) = time_from_ticks(p.time) else { break };
            newer.push(PointerSample {
                pos: Point::new((x - origin.x) as f32 / scale, (y - origin.y) as f32 / scale),
                time: t,
                pressure: 0.0,
            });
        }
        let mut h = shared.history.lock().unwrap_or_else(|e| e.into_inner());
        h.extend(newer.into_iter().rev());
        let excess = h.len().saturating_sub(HISTORY_MAX);
        h.drain(..excess);
    }
}

fn hit_code(kind: HitKind) -> u32 {
    match kind {
        HitKind::Client => HTCLIENT,
        HitKind::Caption => HTCAPTION,
        HitKind::Minimize => HTMINBUTTON,
        HitKind::Maximize => HTMAXBUTTON,
        HitKind::Close => HTCLOSE,
        HitKind::SystemMenu => HTSYSMENU,
    }
}

/// Answers `WM_IME_REQUEST`; `None` = let the default procedure answer.
fn ime_request(hwnd: HWND, shared: &Shared, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
    let ime = shared.ime.lock().unwrap_or_else(|e| e.into_inner());
    let state = ime.as_ref()?;
    let scale = scale_of(hwnd);
    match wparam.0 as u32 {
        IMR_QUERYCHARPOSITION => {
            if lparam.0 == 0 {
                return None;
            }
            // SAFETY: for IMR_QUERYCHARPOSITION, lParam points to the IME's IMECHARPOSITION.
            let pos = unsafe { &mut *(lparam.0 as *mut IMECHARPOSITION) };
            let r = state.composition_rects.get(pos.dwCharPos as usize).copied().unwrap_or(state.area);
            let mut pt = POINT { x: (r.min.x * scale).round() as i32, y: (r.min.y * scale).round() as i32 };
            let mut doc = RECT::default();
            // SAFETY: coordinate conversion on this thread's window.
            unsafe {
                let _ = ClientToScreen(hwnd, &mut pt);
                let _ = windows::Win32::UI::WindowsAndMessaging::GetClientRect(hwnd, &mut doc);
                let mut tl = POINT { x: doc.left, y: doc.top };
                let mut br = POINT { x: doc.right, y: doc.bottom };
                let _ = ClientToScreen(hwnd, &mut tl);
                let _ = ClientToScreen(hwnd, &mut br);
                doc = RECT { left: tl.x, top: tl.y, right: br.x, bottom: br.y };
            }
            pos.pt = pt;
            pos.cLineHeight = (r.height() * scale).round().max(1.0) as u32;
            pos.rcDocument = doc;
            Some(LRESULT(1))
        }
        IMR_DOCUMENTFEED => {
            let (text, sel) = state.surrounding.as_ref()?;
            let units: Vec<u16> = text.encode_utf16().collect();
            let header = std::mem::size_of::<RECONVERTSTRING>();
            let size = header + units.len() * 2;
            if lparam.0 == 0 {
                return Some(LRESULT(size as isize));
            }
            // SAFETY: for IMR_DOCUMENTFEED with a non-null lParam, the IME passes a buffer of the
            // size we returned for the null query, starting with a RECONVERTSTRING.
            unsafe {
                let rs = &mut *(lparam.0 as *mut RECONVERTSTRING);
                if (rs.dwSize as usize) < size {
                    return Some(LRESULT(0));
                }
                let start = sel.start.min(units.len());
                let len = sel.end.min(units.len()).saturating_sub(start);
                rs.dwVersion = 0;
                rs.dwStrLen = units.len() as u32;
                rs.dwStrOffset = header as u32;
                rs.dwCompStrLen = len as u32;
                rs.dwCompStrOffset = (start * 2) as u32;
                rs.dwTargetStrLen = len as u32;
                rs.dwTargetStrOffset = (start * 2) as u32;
                let dst = (lparam.0 as *mut u8).add(header) as *mut u16;
                std::ptr::copy_nonoverlapping(units.as_ptr(), dst, units.len());
            }
            Some(LRESULT(size as isize))
        }
        _ => None,
    }
}

unsafe extern "system" fn subclass_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    data: usize,
) -> LRESULT {
    // SAFETY: `data` is the `Arc<Shared>` reference installed with this subclass; it stays valid
    // until WM_NCDESTROY below releases it.
    let shared = unsafe { &*(data as *const Shared) };
    match msg {
        WM_NCHITTEST => {
            // SAFETY: forwarding to the next procedure in the chain.
            let r = unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) };
            if r.0 as u32 != HTCLIENT {
                return r;
            }
            let regions = shared.regions.lock().unwrap_or_else(|e| e.into_inner());
            if regions.is_empty() {
                return r;
            }
            let mut pt = lparam_point(lparam);
            // SAFETY: coordinate conversion on this thread's window.
            let _ = unsafe { ScreenToClient(hwnd, &mut pt) };
            let scale = scale_of(hwnd);
            let p = Point::new(pt.x as f32 / scale, pt.y as f32 / scale);
            LRESULT(hit_code(hit_test(&regions, p)) as isize)
        }
        // Our caption buttons: swallow the press (the default draws classic buttons) and run the
        // command on release.
        WM_NCLBUTTONDOWN | WM_NCLBUTTONDBLCLK if matches!(wparam.0 as u32, HTMINBUTTON | HTMAXBUTTON | HTCLOSE) => LRESULT(0),
        WM_NCLBUTTONUP if matches!(wparam.0 as u32, HTMINBUTTON | HTMAXBUTTON | HTCLOSE) => {
            let cmd = match wparam.0 as u32 {
                HTMINBUTTON => SC_MINIMIZE,
                // SAFETY: plain query on this thread's window.
                HTMAXBUTTON if unsafe { IsZoomed(hwnd) }.as_bool() => SC_RESTORE,
                HTMAXBUTTON => SC_MAXIMIZE,
                _ => SC_CLOSE,
            };
            // SAFETY: posting a system command to our own window.
            let _ = unsafe { PostMessageW(Some(hwnd), WM_SYSCOMMAND, WPARAM(cmd as usize), LPARAM(0)) };
            LRESULT(0)
        }
        WM_ENTERSIZEMOVE | WM_ENTERMENULOOP => {
            shared.in_modal.store(true, Ordering::SeqCst);
            arm_modal_timer(hwnd, *shared.timer.lock().unwrap_or_else(|e| e.into_inner()));
            // SAFETY: forwarding to the next procedure in the chain.
            unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
        }
        WM_EXITSIZEMOVE | WM_EXITMENULOOP => {
            shared.in_modal.store(false, Ordering::SeqCst);
            arm_modal_timer(hwnd, None);
            // The loop's own deadline handling resumes; check timers once now.
            (shared.hooks.tick)();
            // SAFETY: forwarding to the next procedure in the chain.
            unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
        }
        WM_TIMER if wparam.0 == TIMER_ID => {
            arm_modal_timer(hwnd, None);
            (shared.hooks.tick)();
            LRESULT(0)
        }
        WM_PAINT if shared.in_modal.load(Ordering::SeqCst) => {
            // The compositor presents on its own; validating stops the per-paint redraw re-arm.
            // SAFETY: validating our own window's update region.
            let _ = unsafe { ValidateRect(Some(hwnd), None) };
            LRESULT(0)
        }
        WM_MOUSEMOVE => {
            record_history(hwnd, shared, lparam);
            // SAFETY: forwarding to the next procedure in the chain.
            unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) }
        }
        WM_IME_REQUEST => match ime_request(hwnd, shared, wparam, lparam) {
            Some(r) => r,
            // SAFETY: forwarding to the next procedure in the chain.
            None => unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) },
        },
        WM_NCDESTROY => {
            // SAFETY: removing our own subclass, then releasing the reference it held (installed in
            // `WindowShim::install`); nothing touches `shared` after this.
            unsafe {
                let _ = RemoveWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID);
                let r = DefSubclassProc(hwnd, msg, wparam, lparam);
                drop(Arc::from_raw(data as *const Shared));
                r
            }
        }
        // SAFETY: forwarding to the next procedure in the chain.
        _ => unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) },
    }
}

pub fn install_app_hooks(quit: Box<dyn Fn() + Send + Sync>) {
    // Windows has no app-level quit request; closing windows is the app's call.
    drop(quit);
}

pub fn prepare_attributes(a: WindowAttributes, desc: &WindowDesc) -> WindowAttributes {
    a.with_no_redirection_bitmap(desc.os_compositing)
}

pub fn monitor_work_area(m: &winit::monitor::MonitorHandle) -> Option<DeviceRect> {
    let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
    // SAFETY: `hmonitor` is winit's live monitor handle; `info` is sized for MONITORINFO.
    let ok = unsafe { GetMonitorInfoW(HMONITOR(m.hmonitor() as *mut _), &mut info) };
    if !ok.as_bool() {
        return None;
    }
    let r = info.rcWork;
    Some(DeviceRect { x: r.left, y: r.top, w: (r.right - r.left).max(0) as u32, h: (r.bottom - r.top).max(0) as u32 })
}
