//! Windows: the class, the window procedure, and the per-window state.
//!
//! - **Frames.** System frames are the OS's. Custom frames keep `WS_OVERLAPPEDWINDOW` (so snapping,
//!   the shadow and the resize borders stay native) and drop only the caption in `WM_NCCALCSIZE`;
//!   `WM_NCHITTEST` then answers the top resize band and the app's [`HitRegion`]s, and the caption
//!   buttons (`HTMINBUTTON`/`HTMAXBUTTON`/`HTCLOSE`) get the app's pointer events while the OS
//!   shows Windows 11's snap layouts over `HTMAXBUTTON`.
//! - **DPI.** Per-monitor v2: `WM_GETDPISCALEDSIZE` keeps the content size in points across
//!   monitors, `WM_DPICHANGED` applies the OS's rectangle and reports the new scale before the
//!   resize it causes.
//! - **Painting.** None: the window has no redirection bitmap and its content is DirectComposition
//!   visuals, so `WM_PAINT` only validates.
//! - **Input.** `WM_POINTER*` ([`crate::pointer`]), keys ([`crate::keyboard`]), DirectManipulation
//!   gestures ([`crate::dmanip`]), `WM_IME_REQUEST` ([`crate::imm`]) and file drops
//!   ([`crate::drag`]).

use crate::app::{App, WM_PRESTO_TIMER, WM_PRESTO_WAKE, current};
use crate::cursor;
use crate::dcomp::WindowTree;
use crate::dmanip::{DirectManipulation, os_modifiers};
use crate::drag::DropTarget;
use crate::hit_test;
use crate::imm;
use crate::monitor::{BASE_DPI, dpi_to_scale};
use crate::pointer::{self, ClientMap, PointerMsg};
use crate::shim::*;
use crate::tsf::TsfDocument;
use crate::util::{hiword, loword, lparam_point, message_time_to_time, wheel_delta, wide};
use presto_base::cursor::CursorIcon;
use presto_base::event::{
    ImeEvent, InputEvent, MouseButton, PointerEvent, PointerPhase, PointerType, Theme, WindowEvent,
};
use presto_base::geom::{DeviceSize, Point, Rect, Scale, Size, Vec2};
use presto_base::ids::WindowId;
use smallvec::SmallVec;
use std::rc::Rc;
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
use windows::Win32::Graphics::Dwm::{DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute};
use windows::Win32::Graphics::Gdi::{
    ClientToScreen, GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow, ScreenToClient,
    ValidateRect,
};
use windows::Win32::System::Ole::{RegisterDragDrop, RevokeDragDrop};
use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
use windows::Win32::UI::Controls::WM_MOUSELEAVE;
use windows::Win32::UI::HiDpi::{AdjustWindowRectExForDpi, GetDpiForWindow, GetSystemMetricsForDpi};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    ReleaseCapture, SetCapture, TME_LEAVE, TME_NONCLIENT, TRACKMOUSEEVENT, TrackMouseEvent, VK_F4, VK_SPACE,
};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows_core::{BOOL, PCWSTR, w};

pub const CLASS_NAME: &str = "presto.window";
/// The `SetTimer` id that keeps DirectManipulation updating while a gesture or its inertia runs.
const DM_TIMER: usize = 1;
const DM_TIMER_MS: u32 = 8;

pub(crate) struct Window {
    pub hwnd: HWND,
    pub id: WindowId,
    pub scale: Scale,
    /// Client size in physical pixels.
    pub size: DeviceSize,
    pub decorations: Decorations,
    pub kind: WindowKind,
    pub resizable: bool,
    pub min_size: Option<Size>,
    pub regions: Vec<HitRegion>,
    pub cursor: CursorIcon,
    pub tree: Option<WindowTree>,
    pub dm: Option<Rc<DirectManipulation>>,
    pub tsf: Option<Rc<TsfDocument>>,
    pub ime: Option<ImeState>,
    pub focused: bool,
    /// TSF's focus is on this window's document (set only on changes: re-focusing restarts TIPs).
    pub ime_focused: bool,
    pub minimized: bool,
    /// The mouse is over the client area (pointer enter seen, leave not yet).
    pub hovering: bool,
    /// The caption button pressed (custom frames), released over the same button = click.
    pub nc_pressed: Option<HitKind>,
    pub nc_hovering: bool,
    pub dm_timer: bool,
    /// Style and placement to restore when leaving fullscreen.
    pub restore: Option<(WINDOW_STYLE, WINDOWPLACEMENT)>,
    pub drop_target: Option<windows::Win32::System::Ole::IDropTarget>,
}

/// Window and extended styles for a description.
pub fn styles(desc: &WindowDesc) -> (WINDOW_STYLE, WINDOW_EX_STYLE) {
    let mut ex = WS_EX_NOREDIRECTIONBITMAP;
    let style = match desc.kind {
        WindowKind::Normal => match desc.decorations {
            Decorations::System | Decorations::Custom => {
                let mut s = WS_OVERLAPPEDWINDOW;
                if !desc.resizable {
                    s &= !(WS_THICKFRAME | WS_MAXIMIZEBOX);
                }
                s
            }
            Decorations::None => WS_POPUP,
        },
        WindowKind::Popup | WindowKind::Tooltip => {
            ex |= WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TOPMOST;
            WS_POPUP
        }
    };
    (style | WS_CLIPCHILDREN | WS_CLIPSIBLINGS, ex)
}

/// The frame around the client area at `dpi` (left, top, right, bottom), as `AdjustWindowRectEx`
/// computes it; custom frames have no top (the caption is gone).
pub fn frame_insets(style: WINDOW_STYLE, ex: WINDOW_EX_STYLE, dpi: u32, decorations: Decorations) -> (i32, i32, i32, i32) {
    if style.0 & WS_POPUP.0 != 0 && style.0 & WS_THICKFRAME.0 == 0 {
        return (0, 0, 0, 0);
    }
    let mut r = RECT::default();
    let _ = unsafe { AdjustWindowRectExForDpi(&mut r, style, false, ex, dpi) };
    let top = if decorations == Decorations::Custom { 0 } else { -r.top };
    (-r.left, top, r.right, r.bottom)
}

/// The outer size for a client size in physical pixels.
pub fn outer_for_client(client: (i32, i32), insets: (i32, i32, i32, i32)) -> (i32, i32) {
    (client.0 + insets.0 + insets.2, client.1 + insets.1 + insets.3)
}

/// The client size at a new DPI that keeps the content size in points.
pub fn scaled_client(client: (i32, i32), old_dpi: u32, new_dpi: u32) -> (i32, i32) {
    let f = new_dpi as f64 / old_dpi.max(1) as f64;
    ((client.0 as f64 * f).round() as i32, (client.1 as f64 * f).round() as i32)
}

/// The window events a `WM_SIZE` means, given the last client size and minimised state.
pub fn size_events(kind: u32, size: DeviceSize, scale: Scale, last: DeviceSize, was_minimized: bool) -> SmallVec<[WindowEvent; 2]> {
    let mut out = SmallVec::new();
    if kind == SIZE_MINIMIZED {
        if !was_minimized {
            out.push(WindowEvent::Occluded(true));
        }
        return out;
    }
    if was_minimized {
        out.push(WindowEvent::Occluded(false));
    }
    if size != last {
        out.push(WindowEvent::Resized { size, scale });
    }
    out
}

/// The caption button a hit-test code names.
pub fn caption_button(ht: u32) -> Option<HitKind> {
    match ht {
        HTMINBUTTON => Some(HitKind::Minimize),
        HTMAXBUTTON => Some(HitKind::Maximize),
        HTCLOSE => Some(HitKind::Close),
        _ => None,
    }
}

pub fn register_class(hinstance: HINSTANCE) -> Result<(), PlatformError> {
    let class = wide(CLASS_NAME);
    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: CS_DBLCLKS,
        lpfnWndProc: Some(wndproc),
        hInstance: hinstance,
        hCursor: unsafe { LoadCursorW(None, IDC_ARROW) }.unwrap_or_default(),
        lpszClassName: PCWSTR(class.as_ptr()),
        ..Default::default()
    };
    // A second `run` on another thread registers the same class: that is fine.
    if unsafe { RegisterClassExW(&wc) } == 0 {
        let e = windows_core::Error::from_thread();
        if e.code() != windows::Win32::Foundation::ERROR_CLASS_ALREADY_EXISTS.to_hresult() {
            return Err(PlatformError::win("RegisterClassExW", e));
        }
    }
    Ok(())
}

/// The system theme for apps (`AppsUseLightTheme`).
pub fn system_theme() -> Theme {
    let mut value = 1u32;
    let mut size = std::mem::size_of::<u32>() as u32;
    let r = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            w!("AppsUseLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some(&mut value as *mut u32 as *mut _),
            Some(&mut size),
        )
    };
    if r.is_ok() && value == 0 { Theme::Dark } else { Theme::Light }
}

fn apply_dark_title_bar(hwnd: HWND, theme: Theme) {
    let dark = BOOL::from(theme == Theme::Dark);
    let _ = unsafe {
        DwmSetWindowAttribute(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, &dark as *const BOOL as *const _, std::mem::size_of::<BOOL>() as u32)
    };
}

pub(crate) fn create(app: &Rc<App>, desc: WindowDesc) -> WindowId {
    let id = app.alloc_window_id();
    let (style, ex) = styles(&desc);
    let owner = desc.parent.and_then(|p| app.hwnd(p));
    let class = wide(CLASS_NAME);
    let title = wide(&desc.title);
    let (x, y) = desc.position.unwrap_or((CW_USEDEFAULT, CW_USEDEFAULT));
    let hwnd = match unsafe {
        CreateWindowExW(
            ex,
            PCWSTR(class.as_ptr()),
            PCWSTR(title.as_ptr()),
            style,
            x,
            y,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            owner,
            None,
            Some(app.hinstance),
            None,
        )
    } {
        Ok(h) => h,
        // The id stays unknown to every other call (surface_target answers `None`).
        Err(_) => return id,
    };
    let dpi = unsafe { GetDpiForWindow(hwnd) }.max(1);
    let scale = dpi_to_scale(dpi);
    let tree = app.dcomp.as_ref().and_then(|dc| dc.window_tree(hwnd).ok());
    let dm = DirectManipulation::new(hwnd, scale.0).ok().map(Rc::new);
    let drop_target = {
        let weak = Rc::downgrade(app);
        let emit: Rc<dyn Fn(InputEvent)> = Rc::new(move |ev| {
            if let Some(app) = weak.upgrade() {
                let now = app.now();
                app.input(id, now, ev);
            }
        });
        let target = DropTarget::create(emit);
        unsafe { RegisterDragDrop(hwnd, &target) }.ok().map(|_| target)
    };
    {
        let mut inner = app.inner.borrow_mut();
        inner.by_hwnd.insert(hwnd.0 as isize, id);
        inner.windows.insert(
            id,
            Window {
                hwnd,
                id,
                scale,
                size: DeviceSize { w: 0, h: 0 },
                decorations: desc.decorations,
                kind: desc.kind,
                resizable: desc.resizable,
                min_size: desc.min_size,
                regions: Vec::new(),
                cursor: CursorIcon::Default,
                tree,
                dm,
                tsf: None,
                ime: None,
                focused: false,
                ime_focused: false,
                minimized: false,
                hovering: false,
                nc_pressed: None,
                nc_hovering: false,
                dm_timer: false,
                restore: None,
                drop_target,
            },
        );
    }
    if desc.decorations == Decorations::System && desc.kind == WindowKind::Normal {
        apply_dark_title_bar(hwnd, system_theme());
    }
    let insets = frame_insets(style, ex, dpi, desc.decorations);
    let client = ((desc.size.w * scale.0).round() as i32, (desc.size.h * scale.0).round() as i32);
    let (ow, oh) = outer_for_client(client, insets);
    let mut flags = SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED;
    if desc.position.is_none() {
        flags |= SWP_NOMOVE;
    }
    let _ = unsafe { SetWindowPos(hwnd, None, x, y, ow, oh, flags) };
    app.window_event(id, WindowEvent::ThemeChanged(system_theme()));
    if desc.visible {
        let cmd = match (desc.kind, desc.state) {
            (WindowKind::Popup | WindowKind::Tooltip, _) => SW_SHOWNOACTIVATE,
            (_, WindowState::Maximized) => SW_SHOWMAXIMIZED,
            (_, WindowState::Minimized) => SW_SHOWMINIMIZED,
            _ => SW_SHOW,
        };
        let _ = unsafe { ShowWindow(hwnd, cmd) };
        if desc.state == WindowState::Fullscreen {
            set_state(app, id, WindowState::Fullscreen);
        }
    }
    id
}

pub(crate) fn set_cursor(app: &App, id: WindowId, c: CursorIcon) {
    let hovering = {
        let mut inner = app.inner.borrow_mut();
        let Some(w) = inner.windows.get_mut(&id) else { return };
        w.cursor = c;
        w.hovering
    };
    if hovering {
        unsafe { SetCursor(cursor::load(c)) };
    }
}

pub(crate) fn set_ime(app: &App, id: WindowId, state: Option<ImeState>) {
    let Some(tsf) = app.tsf.as_ref() else { return };
    let (doc, refocus) = {
        let mut inner = app.inner.borrow_mut();
        let Some(w) = inner.windows.get_mut(&id) else { return };
        w.ime = state.clone();
        if state.is_some() && w.tsf.is_none() {
            w.tsf = tsf.create_document(w.hwnd, id, app.client_access()).ok().map(Rc::new);
            if let Some(d) = &w.tsf {
                d.set_scale(w.scale.0);
            }
        }
        let want = w.focused && state.is_some() && w.tsf.is_some();
        let refocus = w.focused && want != w.ime_focused;
        if w.focused {
            w.ime_focused = want;
        }
        (w.tsf.clone(), refocus)
    };
    if let (Some(doc), Some(s)) = (&doc, &state) {
        doc.set_field(s.area, s.purpose);
    }
    if refocus {
        tsf.focus(if state.is_some() { doc.as_deref() } else { None });
    }
}

pub(crate) fn set_pointer_capture(app: &App, id: WindowId, capture: bool) {
    let Some(hwnd) = app.hwnd(id) else { return };
    unsafe {
        if capture {
            SetCapture(hwnd);
        } else {
            let _ = ReleaseCapture();
        }
    }
}

pub(crate) fn set_title(app: &App, id: WindowId, title: &str) {
    if let Some(hwnd) = app.hwnd(id) {
        let t = wide(title);
        let _ = unsafe { SetWindowTextW(hwnd, PCWSTR(t.as_ptr())) };
    }
}

pub(crate) fn set_visible(app: &App, id: WindowId, visible: bool) {
    let Some((hwnd, kind)) = app.inner.borrow().windows.get(&id).map(|w| (w.hwnd, w.kind)) else { return };
    let cmd = match (visible, kind) {
        (false, _) => SW_HIDE,
        (true, WindowKind::Normal) => SW_SHOW,
        (true, _) => SW_SHOWNOACTIVATE,
    };
    let _ = unsafe { ShowWindow(hwnd, cmd) };
}

pub(crate) fn set_state(app: &App, id: WindowId, state: WindowState) {
    let Some((hwnd, restore)) = app.inner.borrow().windows.get(&id).map(|w| (w.hwnd, w.restore)) else { return };
    // Leaving fullscreen first restores the style and placement it replaced.
    if let Some((style, placement)) = restore
        && state != WindowState::Fullscreen
    {
        unsafe {
            SetWindowLongPtrW(hwnd, GWL_STYLE, style.0 as isize);
            let _ = SetWindowPlacement(hwnd, &placement);
            let _ = SetWindowPos(hwnd, None, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED);
        }
        if let Some(w) = app.inner.borrow_mut().windows.get_mut(&id) {
            w.restore = None;
        }
    }
    match state {
        WindowState::Normal => {
            let _ = unsafe { ShowWindow(hwnd, SW_RESTORE) };
        }
        WindowState::Minimized => {
            let _ = unsafe { ShowWindow(hwnd, SW_MINIMIZE) };
        }
        WindowState::Maximized => {
            let _ = unsafe { ShowWindow(hwnd, SW_MAXIMIZE) };
        }
        WindowState::Fullscreen if restore.is_none() => {
            let style = WINDOW_STYLE(unsafe { GetWindowLongPtrW(hwnd, GWL_STYLE) } as u32);
            let mut placement = WINDOWPLACEMENT { length: std::mem::size_of::<WINDOWPLACEMENT>() as u32, ..Default::default() };
            let _ = unsafe { GetWindowPlacement(hwnd, &mut placement) };
            let mon = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
            let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
            if !unsafe { GetMonitorInfoW(mon, &mut info) }.as_bool() {
                return;
            }
            if let Some(w) = app.inner.borrow_mut().windows.get_mut(&id) {
                w.restore = Some((style, placement));
            }
            let r = info.rcMonitor;
            unsafe {
                SetWindowLongPtrW(hwnd, GWL_STYLE, ((style.0 & !WS_OVERLAPPEDWINDOW.0) | WS_POPUP.0) as isize);
                let _ = SetWindowPos(hwnd, Some(HWND_TOP), r.left, r.top, r.right - r.left, r.bottom - r.top, SWP_FRAMECHANGED | SWP_NOOWNERZORDER);
            }
        }
        WindowState::Fullscreen => {}
    }
}

/// Runs the DirectManipulation recognisers of windows with a running gesture and delivers what they
/// produced.
pub(crate) fn pump_gestures(app: &App) {
    let active: Vec<(WindowId, Rc<DirectManipulation>)> = match app.inner.try_borrow() {
        Ok(inner) => inner.windows.values().filter_map(|w| w.dm.clone().filter(|d| d.active()).map(|d| (w.id, d))).collect(),
        Err(_) => return,
    };
    for (id, dm) in active {
        for ev in dm.update() {
            app.input(id, app.now(), ev);
        }
    }
}

/// The window's client origin on screen and its point mapping.
fn client_map(hwnd: HWND, scale: Scale) -> ClientMap {
    let mut p = POINT::default();
    let _ = unsafe { ClientToScreen(hwnd, &mut p) };
    ClientMap { origin: (p.x, p.y), scale }
}

fn track_leave(hwnd: HWND, nonclient: bool) {
    let mut t = TRACKMOUSEEVENT {
        cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
        dwFlags: if nonclient { TME_LEAVE | TME_NONCLIENT } else { TME_LEAVE },
        hwndTrack: hwnd,
        dwHoverTime: 0,
    };
    let _ = unsafe { TrackMouseEvent(&mut t) };
}

fn mouse_event(phase: PointerPhase, button: Option<MouseButton>, buttons: u32, pos: Point, app: &App) -> InputEvent {
    InputEvent::Pointer(PointerEvent {
        phase,
        pointer: 0,
        kind: PointerType::Mouse,
        button,
        buttons,
        pos,
        modifiers: app.keys.try_borrow().map(|k| k.mods.modifiers()).unwrap_or_else(|_| os_modifiers()),
        pressure: if buttons != 0 { 0.5 } else { 0.0 },
        tilt: Vec2::ZERO,
        history: SmallVec::new(),
    })
}

/// A screen point (`LPARAM` of non-client and wheel messages) in window points.
fn screen_to_points(hwnd: HWND, scale: Scale, lparam: LPARAM) -> Point {
    let (x, y) = lparam_point(lparam.0);
    let mut p = POINT { x, y };
    let _ = unsafe { ScreenToClient(hwnd, &mut p) };
    Point::new(p.x as f32 / scale.0, p.y as f32 / scale.0)
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let Some(app) = current() else { return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) } };
    if hwnd == app.msg_hwnd.get() {
        match msg {
            WM_PRESTO_WAKE => app.on_wake(),
            WM_PRESTO_TIMER => app.on_timer(),
            WM_SETTINGCHANGE => on_setting_change(&app, lparam),
            _ => return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
        }
        return LRESULT(0);
    }
    let id = match app.inner.try_borrow() {
        Ok(inner) => inner.window_of(hwnd),
        Err(_) => None,
    };
    let Some(id) = id else { return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) } };
    match handle(&app, id, hwnd, msg, wparam, lparam) {
        Some(r) => LRESULT(r),
        None => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn on_setting_change(app: &App, lparam: LPARAM) {
    if lparam.0 == 0 {
        return;
    }
    let name = unsafe { PCWSTR(lparam.0 as *const u16).to_string() }.unwrap_or_default();
    if name != "ImmersiveColorSet" {
        return;
    }
    let theme = system_theme();
    let windows: Vec<(WindowId, HWND, bool)> = app
        .inner
        .borrow()
        .windows
        .values()
        .map(|w| (w.id, w.hwnd, w.decorations == Decorations::System && w.kind == WindowKind::Normal))
        .collect();
    for (id, hwnd, system) in windows {
        if system {
            apply_dark_title_bar(hwnd, theme);
        }
        app.window_event(id, WindowEvent::ThemeChanged(theme));
    }
}

/// A snapshot of what message handling needs, taken without holding the borrow across OS calls.
struct State {
    scale: Scale,
    decorations: Decorations,
    kind: WindowKind,
    resizable: bool,
}

fn state(app: &App, id: WindowId) -> Option<State> {
    let inner = app.inner.try_borrow().ok()?;
    let w = inner.windows.get(&id)?;
    Some(State { scale: w.scale, decorations: w.decorations, kind: w.kind, resizable: w.resizable })
}

fn with_window<R>(app: &App, id: WindowId, f: impl FnOnce(&mut Window) -> R) -> Option<R> {
    let mut inner = app.inner.try_borrow_mut().ok()?;
    inner.windows.get_mut(&id).map(f)
}

fn handle(app: &Rc<App>, id: WindowId, hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> Option<isize> {
    let st = state(app, id)?;
    match msg {
        WM_NCCALCSIZE if wparam.0 != 0 && st.decorations == Decorations::Custom => {
            // Keep the OS's side and bottom borders; drop the caption.
            let params = unsafe { &mut *(lparam.0 as *mut NCCALCSIZE_PARAMS) };
            let top = params.rgrc[0].top;
            let r = unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
            if r.0 != 0 {
                return Some(r.0);
            }
            params.rgrc[0].top = top;
            if unsafe { IsZoomed(hwnd) }.as_bool() {
                // A maximised window overhangs the monitor by its frame.
                let dpi = unsafe { GetDpiForWindow(hwnd) };
                params.rgrc[0].top += unsafe {
                    GetSystemMetricsForDpi(SM_CYFRAME, dpi) + GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi)
                };
            }
            Some(0)
        }
        WM_NCHITTEST => {
            if st.kind == WindowKind::Tooltip {
                return Some(HTTRANSPARENT as isize);
            }
            if st.decorations != Decorations::Custom {
                return None;
            }
            let r = unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
            if r.0 != HTCLIENT as isize {
                return Some(r.0);
            }
            let p = screen_to_points(hwnd, st.scale, lparam);
            let dpi = unsafe { GetDpiForWindow(hwnd) };
            let border = unsafe { GetSystemMetricsForDpi(SM_CYFRAME, dpi) + GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi) };
            let maximized = unsafe { IsZoomed(hwnd) }.as_bool();
            let top = (st.resizable && !maximized).then_some(border as f32 / st.scale.0);
            let (regions, width) = with_window(app, id, |w| (w.regions.clone(), w.size.w as f32 / w.scale.0))?;
            Some(hit_test::hit_test(&regions, width, p, top) as isize)
        }
        WM_NCMOUSEMOVE if st.decorations == Decorations::Custom => {
            if caption_button(wparam.0 as u32).is_some() {
                let first = with_window(app, id, |w| !std::mem::replace(&mut w.nc_hovering, true))?;
                if first {
                    track_leave(hwnd, true);
                }
                let p = screen_to_points(hwnd, st.scale, lparam);
                app.input(id, message_time_to_time(unsafe { GetMessageTime() }), mouse_event(PointerPhase::Move, None, 0, p, app));
                return Some(0);
            }
            None
        }
        WM_NCMOUSELEAVE => {
            if with_window(app, id, |w| std::mem::replace(&mut w.nc_hovering, false))? {
                app.input(id, app.now(), mouse_event(PointerPhase::Leave, None, 0, Point::new(-1.0, -1.0), app));
            }
            None
        }
        WM_NCLBUTTONDOWN | WM_NCLBUTTONDBLCLK if st.decorations == Decorations::Custom => {
            let button = caption_button(wparam.0 as u32)?;
            with_window(app, id, |w| w.nc_pressed = Some(button));
            let p = screen_to_points(hwnd, st.scale, lparam);
            let t = message_time_to_time(unsafe { GetMessageTime() });
            app.input(id, t, mouse_event(PointerPhase::Down, Some(MouseButton::Primary), 1, p, app));
            Some(0)
        }
        WM_NCLBUTTONUP if st.decorations == Decorations::Custom => {
            let pressed = with_window(app, id, |w| w.nc_pressed.take())??;
            let p = screen_to_points(hwnd, st.scale, lparam);
            let t = message_time_to_time(unsafe { GetMessageTime() });
            app.input(id, t, mouse_event(PointerPhase::Up, Some(MouseButton::Primary), 0, p, app));
            if caption_button(wparam.0 as u32) == Some(pressed) {
                unsafe {
                    match pressed {
                        HitKind::Minimize => {
                            let _ = ShowWindow(hwnd, SW_MINIMIZE);
                        }
                        HitKind::Maximize => {
                            let cmd = if IsZoomed(hwnd).as_bool() { SW_RESTORE } else { SW_MAXIMIZE };
                            let _ = ShowWindow(hwnd, cmd);
                        }
                        _ => {
                            let _ = PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
                        }
                    }
                }
            }
            Some(0)
        }
        WM_POINTERDOWN | WM_POINTERUP | WM_POINTERUPDATE | WM_POINTERENTER | WM_POINTERLEAVE
        | WM_POINTERCAPTURECHANGED => {
            let pmsg = PointerMsg::from_msg(msg)?;
            let pointer_id = loword(wparam.0) as u32;
            let (snap, history) = pointer::read_pointer(pointer_id)?;
            if snap.kind == PointerType::Mouse {
                match pmsg {
                    PointerMsg::Leave => {
                        if !with_window(app, id, |w| std::mem::replace(&mut w.hovering, false))? {
                            return Some(0);
                        }
                    }
                    _ => {
                        if !with_window(app, id, |w| std::mem::replace(&mut w.hovering, true))? {
                            track_leave(hwnd, false);
                        }
                    }
                }
            }
            let mods = app.keys.try_borrow().map(|k| k.mods.modifiers()).unwrap_or_else(|_| os_modifiers());
            let ev = pointer::translate(pmsg, &snap, &history, client_map(hwnd, st.scale), mods);
            app.input(id, pointer::snapshot_time(&snap), InputEvent::Pointer(ev));
            Some(0)
        }
        // The legacy wheel messages come from drivers that bypass the pointer stack; when the pointer
        // message is handled the OS generates no legacy one, so nothing arrives twice.
        WM_POINTERWHEEL | WM_POINTERHWHEEL | WM_MOUSEWHEEL | WM_MOUSEHWHEEL => {
            let p = screen_to_points(hwnd, st.scale, lparam);
            let mods = app.keys.try_borrow().map(|k| k.mods.modifiers()).unwrap_or_else(|_| os_modifiers());
            let horizontal = msg == WM_POINTERHWHEEL || msg == WM_MOUSEHWHEEL;
            let ev = pointer::wheel(p, wheel_delta(wparam.0), horizontal, mods);
            app.input(id, message_time_to_time(unsafe { GetMessageTime() }), InputEvent::Wheel(ev));
            Some(0)
        }
        WM_MOUSELEAVE => {
            if with_window(app, id, |w| std::mem::replace(&mut w.hovering, false))? {
                app.input(id, app.now(), mouse_event(PointerPhase::Leave, None, 0, Point::new(-1.0, -1.0), app));
            }
            Some(0)
        }
        DM_POINTERHITTEST => {
            let dm = with_window(app, id, |w| w.dm.clone())?;
            if let Some(dm) = dm {
                dm.on_pointer_hit_test(wparam);
                let start = with_window(app, id, |w| !std::mem::replace(&mut w.dm_timer, true))?;
                if start {
                    unsafe { SetTimer(Some(hwnd), DM_TIMER, DM_TIMER_MS, None) };
                }
            }
            None
        }
        WM_TIMER if wparam.0 == DM_TIMER => {
            let dm = with_window(app, id, |w| w.dm.clone())?;
            let Some(dm) = dm else { return Some(0) };
            for ev in dm.update() {
                app.input(id, app.now(), ev);
            }
            if !dm.active() {
                with_window(app, id, |w| w.dm_timer = false);
                let _ = unsafe { KillTimer(Some(hwnd), DM_TIMER) };
            }
            Some(0)
        }
        WM_KEYDOWN | WM_KEYUP | WM_SYSKEYDOWN | WM_SYSKEYUP => {
            let ev = app.keys.try_borrow_mut().ok()?.key(hwnd, msg, wparam, lparam, unsafe { GetMessageTime() });
            if let Some(ev) = ev {
                app.input(id, message_time_to_time(unsafe { GetMessageTime() }), InputEvent::Key(ev));
            }
            if msg == WM_SYSKEYDOWN {
                // Keep Alt+F4 and Alt+Space (system menu); everything else, Alt and F10 included,
                // must not enter the menu loop.
                let vk = wparam.0 as u16;
                if vk == VK_F4.0 || vk == VK_SPACE.0 {
                    return None;
                }
            }
            Some(0)
        }
        WM_CHAR => {
            // Characters not taken with their key-down: SendInput's VK_PACKET leftovers, IMM32 results.
            let unit = wparam.0 as u16;
            let units: SmallVec<[u16; 2]> = if (0xD800..0xDC00).contains(&unit) {
                app.high_surrogate.set(Some(unit));
                return Some(0);
            } else if let Some(high) = app.high_surrogate.take() {
                SmallVec::from_slice(&[high, unit])
            } else {
                SmallVec::from_slice(&[unit])
            };
            if let Some(text) = crate::keyboard::text_from_utf16(&units) {
                app.input(id, message_time_to_time(unsafe { GetMessageTime() }), InputEvent::Ime(ImeEvent::Commit(text)));
            }
            Some(0)
        }
        WM_SETFOCUS | WM_KILLFOCUS => {
            let focused = msg == WM_SETFOCUS;
            if let Ok(mut k) = app.keys.try_borrow_mut() {
                k.resync();
            }
            let doc = with_window(app, id, |w| {
                w.focused = focused;
                let doc = w.ime.as_ref().and(w.tsf.clone());
                w.ime_focused = focused && doc.is_some();
                doc
            })?;
            if focused && let Some(tsf) = app.tsf.as_ref() {
                tsf.focus(doc.as_deref());
            }
            app.window_event(id, WindowEvent::Focused(focused));
            Some(0)
        }
        WM_SIZE => {
            let size = DeviceSize { w: loword(lparam.0 as usize) as u32, h: hiword(lparam.0 as usize) as u32 };
            let events = with_window(app, id, |w| {
                let evs = size_events(wparam.0 as u32, size, w.scale, w.size, w.minimized);
                w.minimized = wparam.0 as u32 == SIZE_MINIMIZED;
                if !w.minimized {
                    w.size = size;
                }
                evs
            })?;
            for ev in events {
                app.window_event(id, ev);
            }
            Some(0)
        }
        WM_MOVE => {
            let mut r = RECT::default();
            let _ = unsafe { GetWindowRect(hwnd, &mut r) };
            app.window_event(id, WindowEvent::Moved { x: r.left, y: r.top });
            Some(0)
        }
        WM_GETDPISCALEDSIZE => {
            let new_dpi = wparam.0 as u32;
            let old_dpi = unsafe { GetDpiForWindow(hwnd) };
            let mut client = RECT::default();
            let _ = unsafe { GetClientRect(hwnd, &mut client) };
            let scaled = scaled_client((client.right, client.bottom), old_dpi, new_dpi);
            let style = WINDOW_STYLE(unsafe { GetWindowLongPtrW(hwnd, GWL_STYLE) } as u32);
            let ex = WINDOW_EX_STYLE(unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) } as u32);
            let (w, h) = outer_for_client(scaled, frame_insets(style, ex, new_dpi, st.decorations));
            let out = unsafe { &mut *(lparam.0 as *mut SIZE) };
            out.cx = w;
            out.cy = h;
            Some(1)
        }
        WM_DPICHANGED => {
            let scale = dpi_to_scale(loword(wparam.0) as u32);
            let (dm, doc) = with_window(app, id, |w| {
                w.scale = scale;
                if let (Some(tree), Some(dc)) = (w.tree.as_mut(), app.dcomp.as_ref()) {
                    let _ = tree.rescale(dc, scale);
                }
                (w.dm.clone(), w.tsf.clone())
            })?;
            if let Some(dm) = dm {
                dm.set_scale(scale.0);
            }
            if let Some(doc) = doc {
                doc.set_scale(scale.0);
            }
            app.window_event(id, WindowEvent::ScaleChanged { scale });
            let r = unsafe { *(lparam.0 as *const RECT) };
            let _ = unsafe {
                SetWindowPos(hwnd, None, r.left, r.top, r.right - r.left, r.bottom - r.top, SWP_NOZORDER | SWP_NOACTIVATE)
            };
            Some(0)
        }
        WM_GETMINMAXINFO => {
            let min = with_window(app, id, |w| w.min_size)??;
            let dpi = unsafe { GetDpiForWindow(hwnd) };
            let style = WINDOW_STYLE(unsafe { GetWindowLongPtrW(hwnd, GWL_STYLE) } as u32);
            let ex = WINDOW_EX_STYLE(unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) } as u32);
            let s = dpi as f32 / BASE_DPI as f32;
            let (w, h) = outer_for_client(
                ((min.w * s).round() as i32, (min.h * s).round() as i32),
                frame_insets(style, ex, dpi, st.decorations),
            );
            let info = unsafe { &mut *(lparam.0 as *mut MINMAXINFO) };
            info.ptMinTrackSize = POINT { x: w, y: h };
            Some(0)
        }
        WM_SETCURSOR if loword(lparam.0 as usize) as u32 == HTCLIENT => {
            let c = with_window(app, id, |w| w.cursor)?;
            unsafe { SetCursor(cursor::load(c)) };
            Some(1)
        }
        WM_MOUSEACTIVATE if matches!(st.kind, WindowKind::Popup | WindowKind::Tooltip) => Some(MA_NOACTIVATE as isize),
        WM_PAINT => {
            let _ = unsafe { ValidateRect(Some(hwnd), None) };
            Some(0)
        }
        WM_ERASEBKGND => Some(1),
        WM_CLOSE => {
            app.window_event(id, WindowEvent::CloseRequested);
            Some(0)
        }
        WM_IME_REQUEST => {
            let (scale, area) = with_window(app, id, |w| (w.scale, w.ime.as_ref().map(|i| i.area)))?;
            let size = with_window(app, id, |w| w.size)?;
            let field = Rect::from_origin_size(Point::ZERO, Size::new(size.w as f32 / scale.0, size.h as f32 / scale.0));
            let map = client_map(hwnd, scale);
            let to_screen = |r: Rect| RECT {
                left: map.origin.0 + (r.min.x * scale.0).floor() as i32,
                top: map.origin.1 + (r.min.y * scale.0).floor() as i32,
                right: map.origin.0 + (r.max.x * scale.0).ceil() as i32,
                bottom: map.origin.1 + (r.max.y * scale.0).ceil() as i32,
            };
            let _ = area;
            let mut result = None;
            app.with_text_client(id, &mut |c| {
                result = Some(unsafe { imm::handle_request(wparam.0 as u32, lparam.0, c, to_screen, field) });
            });
            result
        }
        WM_DESTROY => {
            let removed = {
                let mut inner = app.inner.try_borrow_mut().ok()?;
                inner.by_hwnd.remove(&(hwnd.0 as isize));
                inner.windows.remove(&id)
            };
            if let Some(w) = removed {
                if w.drop_target.is_some() {
                    let _ = unsafe { RevokeDragDrop(hwnd) };
                }
                if w.focused
                    && w.tsf.is_some()
                    && let Some(tsf) = app.tsf.as_ref()
                {
                    tsf.focus(None);
                }
                drop(w);
            }
            Some(0)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn styles_per_kind() {
        let normal = WindowDesc::default();
        let (s, ex) = styles(&normal);
        assert!(s.contains(WS_OVERLAPPEDWINDOW));
        assert!(ex.contains(WS_EX_NOREDIRECTIONBITMAP));
        let fixed = WindowDesc { resizable: false, ..WindowDesc::default() };
        let (s, _) = styles(&fixed);
        assert!(!s.contains(WS_THICKFRAME) && !s.contains(WS_MAXIMIZEBOX));
        let popup = WindowDesc { kind: WindowKind::Popup, ..WindowDesc::default() };
        let (s, ex) = styles(&popup);
        assert!(s.contains(WS_POPUP));
        assert!(ex.contains(WS_EX_NOACTIVATE) && ex.contains(WS_EX_TOOLWINDOW));
    }

    #[test]
    fn outer_sizes_and_dpi_scaling() {
        assert_eq!(outer_for_client((800, 600), (8, 31, 8, 8)), (816, 639));
        assert_eq!(outer_for_client((800, 600), (8, 0, 8, 8)), (816, 608), "custom frame: no caption");
        assert_eq!(scaled_client((800, 600), 96, 144), (1200, 900));
        assert_eq!(scaled_client((1201, 901), 144, 96), (801, 601));
    }

    #[test]
    fn size_messages_to_events() {
        let s = |w, h| DeviceSize { w, h };
        let scale = Scale(1.5);
        assert_eq!(
            size_events(SIZE_RESTORED, s(800, 600), scale, s(0, 0), false).to_vec(),
            vec![WindowEvent::Resized { size: s(800, 600), scale }]
        );
        assert_eq!(size_events(SIZE_RESTORED, s(800, 600), scale, s(800, 600), false).len(), 0, "no change");
        assert_eq!(size_events(SIZE_MINIMIZED, s(0, 0), scale, s(800, 600), false).to_vec(), vec![WindowEvent::Occluded(true)]);
        assert_eq!(size_events(SIZE_MINIMIZED, s(0, 0), scale, s(800, 600), true).len(), 0);
        assert_eq!(
            size_events(SIZE_RESTORED, s(800, 600), scale, s(800, 600), true).to_vec(),
            vec![WindowEvent::Occluded(false)],
            "restored at the same size"
        );
    }

    #[test]
    fn caption_buttons() {
        assert_eq!(caption_button(HTMAXBUTTON), Some(HitKind::Maximize));
        assert_eq!(caption_button(HTCLOSE), Some(HitKind::Close));
        assert_eq!(caption_button(HTCAPTION), None);
    }
}
