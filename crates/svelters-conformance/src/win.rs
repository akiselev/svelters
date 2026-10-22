//! The Windows user path (WG3w): input through `SendInput`, state through UI Automation.
//!
//! [`UiaDriver`] runs a pin on any fixture window that UI Automation can see: two `Edit` elements (`a`, `b`, in
//! tree order) and a `Button` named `Done` (`c`). A [`WinFixture`] opens and closes that window (a Win32 dialog of
//! `EDIT` controls in this crate's `win32_capture` test, WinUI 3 `TextBox`es in `examples/counter-win`'s
//! `na_capture` test).
//!
//! - Setup is programmatic and also through UI Automation: `ValuePattern.SetValue` for the text,
//!   `TextRange.Select` for the selection (every setup selection is forward or collapsed), `SetFocus` for focus.
//! - Keys: one `SendInput` batch per chord (modifiers down, key down and up, modifiers up) with virtual-key
//!   codes and scan codes, the navigation keys flagged extended (else Shift+arrow reads as the numeric pad).
//! - Clicks and drags: absolute pointer moves and button events through `SendInput`; the press point is 30% into the
//!   character (its UIA bounding rectangle), as on Linux. Before every click or drag the driver waits longer than
//!   the double-click time, so a sequence never joins the previous one.
//! - State: text from the document range; selection from `TextPattern.GetSelection`, its direction from
//!   `TextPattern2.GetCaretRange` (the caret is the end the caret range sits on). Offsets are measured as the
//!   UTF-16 length of the text before an endpoint and converted to Unicode scalars.
//! - Focus: `IUIAutomation::GetFocusedElement` compared with each anchor, counted only while the fixture window is
//!   the foreground window. A focus-changed event handler records what UIA announced; the capture reports, per
//!   checkpoint-to-checkpoint focus change, whether the last event named the new focus (`observe.focus`).
//!
//! The process is made per-monitor DPI aware, so UIA rectangles, pointer input and the window share physical
//! pixels. Input goes to the interactive desktop: a capture needs an unlocked session and nobody using the mouse.

use crate::Driver;
use crate::pins::{BUTTON, BUTTON_LABEL, CLIPBOARD, Chord, FIELDS, Pin};
use crate::trace::{Checkpoint, State, Trace, scalar_to_utf16, utf16_to_scalar};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{HWND, LPARAM, RECT, WPARAM};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, SAFEARRAY};
use windows::Win32::System::Ole::SafeArrayDestroy;
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows::Win32::UI::Accessibility::*;
use windows::Win32::UI::HiDpi::{DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForWindow, SetProcessDpiAwarenessContext};
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{BOOL, BSTR, implement};

pub type WinResult<T> = Result<T, String>;

fn e<E: std::fmt::Display>(what: &'static str) -> impl Fn(E) -> String {
    move |err| format!("{what}: {err}")
}

fn sleep(ms: u64) {
    std::thread::sleep(Duration::from_millis(ms));
}

/// Once per process: per-monitor DPI awareness and an MTA for UI Automation on this thread.
pub fn init_thread() {
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }
}

// ---- input -------------------------------------------------------------------------------------------------------

/// A key name of `keys.json` (X keysym names) as a virtual key, whether it is an extended key, and whether it
/// needs Shift. The dead keys are those of the US-International layout (the Windows row of `keys.json`), which the
/// driver activates for pins that use them.
fn vk_of(key: &str) -> WinResult<(VIRTUAL_KEY, bool, bool)> {
    Ok(match key {
        "dead_acute" => (VK_OEM_7, false, false),
        "dead_diaeresis" => (VK_OEM_7, false, true),
        k => {
            let (vk, ext) = vk_plain(k)?;
            (vk, ext, false)
        }
    })
}

fn vk_plain(key: &str) -> WinResult<(VIRTUAL_KEY, bool)> {
    Ok(match key {
        "Insert" => (VK_INSERT, true),
        "space" => (VK_SPACE, false),
        "Left" => (VK_LEFT, true),
        "Right" => (VK_RIGHT, true),
        "Up" => (VK_UP, true),
        "Down" => (VK_DOWN, true),
        "Home" => (VK_HOME, true),
        "End" => (VK_END, true),
        "Delete" => (VK_DELETE, true),
        "BackSpace" => (VK_BACK, false),
        "Tab" => (VK_TAB, false),
        "Escape" => (VK_ESCAPE, false),
        "Return" => (VK_RETURN, false),
        k if k.len() == 1 && k.chars().all(|c| c.is_ascii_alphanumeric()) => {
            (VIRTUAL_KEY(k.to_ascii_uppercase().as_bytes()[0] as u16), false)
        }
        other => return Err(format!("no virtual key for {other:?}")),
    })
}

fn key_input(vk: VIRTUAL_KEY, extended: bool, up: bool) -> INPUT {
    let scan = unsafe { MapVirtualKeyW(vk.0 as u32, MAPVK_VK_TO_VSC) } as u16;
    let mut flags = KEYBD_EVENT_FLAGS(0);
    if extended {
        flags |= KEYEVENTF_EXTENDEDKEY;
    }
    if up {
        flags |= KEYEVENTF_KEYUP;
    }
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: vk, wScan: scan, dwFlags: flags, time: 0, dwExtraInfo: 0 } },
    }
}

fn mouse_input(flags: MOUSE_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 { mi: MOUSEINPUT { dx: 0, dy: 0, mouseData: 0, dwFlags: flags, time: 0, dwExtraInfo: 0 } },
    }
}

fn send(inputs: &[INPUT]) -> WinResult<()> {
    let n = unsafe { SendInput(inputs, std::mem::size_of::<INPUT>() as i32) };
    if n as usize != inputs.len() {
        return Err(format!(
            "SendInput sent {n} of {} events ({}; a locked session or a higher-integrity foreground window blocks input)",
            inputs.len(),
            windows::core::Error::from_thread()
        ));
    }
    Ok(())
}

/// One chord: modifiers down, the key down and up, modifiers up.
pub fn key(chord: &Chord) -> WinResult<()> {
    let (vk, ext, shift) = vk_of(&chord.key)?;
    let mut mods = Vec::new();
    if chord.ctrl {
        mods.push(VK_LCONTROL);
    }
    if chord.alt {
        mods.push(VK_LMENU);
    }
    if chord.shift || shift {
        mods.push(VK_LSHIFT);
    }
    if chord.cmd {
        mods.push(VK_LWIN);
    }
    let mut v: Vec<INPUT> = mods.iter().map(|m| key_input(*m, false, false)).collect();
    v.push(key_input(vk, ext, false));
    v.push(key_input(vk, ext, true));
    v.extend(mods.iter().rev().map(|m| key_input(*m, false, true)));
    send(&v)
}

/// Moves the pointer to a screen point (physical pixels) with an absolute `SendInput` move over the virtual
/// desktop. `SetCursorPos` is not enough: it posts `WM_MOUSEMOVE` but no pointer input, so WinUI (which reads
/// `WM_POINTER*`) never saw a drag. Normalized coordinates round, so the move is corrected until the cursor is
/// exactly on the point.
pub fn move_to(x: i32, y: i32) -> WinResult<()> {
    let (vx, vy, vw, vh) = unsafe {
        (GetSystemMetrics(SM_XVIRTUALSCREEN), GetSystemMetrics(SM_YVIRTUALSCREEN), GetSystemMetrics(SM_CXVIRTUALSCREEN), GetSystemMetrics(SM_CYVIRTUALSCREEN))
    };
    let norm = |p: i32, o: i32, size: i32| (((p - o) as i64 * 65536 + size as i64 - 1) / size as i64) as i32;
    let (mut nx, mut ny) = (norm(x, vx, vw), norm(y, vy, vh));
    let mut seen = Vec::new();
    for _ in 0..8 {
        let mut i = mouse_input(MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK);
        unsafe {
            i.Anonymous.mi.dx = nx;
            i.Anonymous.mi.dy = ny;
        }
        send(&[i])?;
        sleep(5);
        let mut p = windows::Win32::Foundation::POINT::default();
        unsafe { GetCursorPos(&mut p) }.map_err(e("GetCursorPos"))?;
        if p.x == x && p.y == y {
            return Ok(());
        }
        seen.push((p.x, p.y));
        nx += (x - p.x) * (65536 / vw).max(1);
        ny += (y - p.y) * (65536 / vh).max(1);
    }
    Err(format!("the pointer did not reach ({x}, {y}): it went to {seen:?} (virtual screen {vx},{vy} {vw}x{vh})"))
}

pub fn button(down: bool) -> WinResult<()> {
    button_n(1, down)
}

/// Button 1 (left, primary) or 2 (middle).
pub fn button_n(n: u32, down: bool) -> WinResult<()> {
    let f = match (n, down) {
        (2, true) => MOUSEEVENTF_MIDDLEDOWN,
        (2, false) => MOUSEEVENTF_MIDDLEUP,
        (_, true) => MOUSEEVENTF_LEFTDOWN,
        (_, false) => MOUSEEVENTF_LEFTUP,
    };
    send(&[mouse_input(f)])
}

/// Types one character as the key that makes it on keyboard layout `hkl` (`VkKeyScanExW`, with Shift when the
/// layout needs it), so dead keys compose with it; a character the layout has no key for goes as a Unicode
/// packet (`KEYEVENTF_UNICODE`).
pub fn type_char(c: char, hkl: HKL) -> WinResult<()> {
    let mut buf = [0u16; 2];
    let units = c.encode_utf16(&mut buf);
    let scan = if units.len() == 1 { unsafe { VkKeyScanExW(units[0], hkl) } } else { -1 };
    if scan != -1 && (scan as u16 >> 8) & !1 == 0 {
        let vk = VIRTUAL_KEY(scan as u16 & 0xff);
        let shift = (scan as u16 >> 8) & 1 == 1;
        let mut v = Vec::new();
        if shift {
            v.push(key_input(VK_LSHIFT, false, false));
        }
        v.push(key_input(vk, false, false));
        v.push(key_input(vk, false, true));
        if shift {
            v.push(key_input(VK_LSHIFT, false, true));
        }
        return send(&v);
    }
    let mut v = Vec::new();
    for u in units.iter() {
        for up in [false, true] {
            let mut flags = KEYEVENTF_UNICODE;
            if up {
                flags |= KEYEVENTF_KEYUP;
            }
            v.push(INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: VIRTUAL_KEY(0), wScan: *u, dwFlags: flags, time: 0, dwExtraInfo: 0 } },
            });
        }
    }
    send(&v)
}

/// The clipboard's text (`CF_UNICODETEXT`), `""` when it holds none.
pub fn clipboard_text() -> WinResult<String> {
    use windows::Win32::Foundation::HGLOBAL;
    use windows::Win32::System::DataExchange::{CloseClipboard, GetClipboardData};
    use windows::Win32::System::Memory::{GlobalLock, GlobalUnlock};
    open_clipboard()?;
    let out = unsafe {
        match GetClipboardData(13) {
            Ok(h) if !h.is_invalid() => {
                let g = HGLOBAL(h.0);
                let p = GlobalLock(g) as *const u16;
                let mut s = String::new();
                if !p.is_null() {
                    let mut n = 0;
                    while *p.add(n) != 0 {
                        n += 1;
                    }
                    s = String::from_utf16_lossy(std::slice::from_raw_parts(p, n));
                    let _ = GlobalUnlock(g);
                }
                s
            }
            _ => String::new(),
        }
    };
    unsafe {
        let _ = CloseClipboard();
    }
    Ok(out)
}

/// Empties the clipboard, then puts `text` on it (owned by this process, not the fixture) when given.
pub fn set_clipboard(text: Option<&str>) -> WinResult<()> {
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::DataExchange::{CloseClipboard, EmptyClipboard, SetClipboardData};
    use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
    open_clipboard()?;
    let r = unsafe {
        (|| -> WinResult<()> {
            EmptyClipboard().map_err(e("EmptyClipboard"))?;
            if let Some(t) = text {
                let wide: Vec<u16> = t.encode_utf16().chain([0]).collect();
                let g = GlobalAlloc(GMEM_MOVEABLE, wide.len() * 2).map_err(e("GlobalAlloc"))?;
                let p = GlobalLock(g) as *mut u16;
                std::ptr::copy_nonoverlapping(wide.as_ptr(), p, wide.len());
                let _ = GlobalUnlock(g);
                SetClipboardData(13, Some(HANDLE(g.0))).map_err(e("SetClipboardData"))?;
            }
            Ok(())
        })()
    };
    unsafe {
        let _ = CloseClipboard();
    }
    r
}

fn open_clipboard() -> WinResult<()> {
    use windows::Win32::System::DataExchange::OpenClipboard;
    for _ in 0..100 {
        if unsafe { OpenClipboard(None) }.is_ok() {
            return Ok(());
        }
        sleep(10);
    }
    Err("OpenClipboard: the clipboard stayed busy".into())
}

/// Activates keyboard layout `klid` (`"00020409"`, US-International) in the window's thread, loading it when
/// needed. Returns the layout to restore and whether this call loaded the new one (to unload it after).
pub fn switch_layout(hwnd: HWND, klid: &str) -> WinResult<(HKL, Option<HKL>)> {
    unsafe {
        let thread = GetWindowThreadProcessId(hwnd, None);
        let before = GetKeyboardLayout(thread);
        let n = GetKeyboardLayoutList(None).max(0) as usize;
        let mut list = vec![HKL::default(); n];
        GetKeyboardLayoutList(Some(&mut list));
        let wide: Vec<u16> = klid.encode_utf16().chain([0]).collect();
        let hkl = LoadKeyboardLayoutW(windows::core::PCWSTR(wide.as_ptr()), KLF_NOTELLSHELL).map_err(e("LoadKeyboardLayoutW"))?;
        let loaded = (!list.contains(&hkl)).then_some(hkl);
        activate_layout(hwnd, hkl)?;
        Ok((before, loaded))
    }
}

/// Asks the window's thread to activate `hkl` and waits until it has.
pub fn activate_layout(hwnd: HWND, hkl: HKL) -> WinResult<()> {
    unsafe {
        let thread = GetWindowThreadProcessId(hwnd, None);
        for _ in 0..100 {
            if GetKeyboardLayout(thread) == hkl {
                return Ok(());
            }
            let _ = PostMessageW(Some(hwnd), WM_INPUTLANGCHANGEREQUEST, WPARAM(0), LPARAM(hkl.0 as isize));
            sleep(20);
        }
        Err(format!("keyboard layout {:?} did not activate in the fixture (it has {:?})", hkl, GetKeyboardLayout(thread)))
    }
}

/// Milliseconds a click sequence waits so it never joins the previous one.
pub fn click_gap() -> u64 {
    (unsafe { GetDoubleClickTime() } as u64 + 200).max(700)
}

/// `count` clicks of button `n` (1 left, 2 middle) at a screen point (physical pixels), 40 ms apart.
pub fn click_at(x: i32, y: i32, count: u32, n: u32) -> WinResult<()> {
    move_to(x, y)?;
    sleep(30);
    for i in 0..count {
        if i > 0 {
            sleep(40);
        }
        button_n(n, true)?;
        sleep(20);
        button_n(n, false)?;
    }
    Ok(())
}

/// `count` presses at `from`, the last one held while the pointer moves to `to` in 12 steps.
pub fn drag(from: (i32, i32), to: (i32, i32), count: u32) -> WinResult<()> {
    move_to(from.0, from.1)?;
    sleep(30);
    for i in 0..count {
        if i > 0 {
            sleep(40);
        }
        button(true)?;
        if i + 1 < count {
            sleep(20);
            button(false)?;
        }
    }
    let steps = 12;
    for s in 1..=steps {
        sleep(15);
        let x = from.0 + (to.0 - from.0) * s / steps;
        let y = from.1 + (to.1 - from.1) * s / steps;
        move_to(x, y)?;
    }
    sleep(40);
    button(false)
}

/// Makes `hwnd` the foreground window (topmost), attaching to the current foreground thread's input for the
/// call, as the focus-stealing rules require. Fails when the window still is not in front.
pub fn bring_to_front(hwnd: HWND) -> WinResult<()> {
    unsafe {
        let _ = SetWindowPos(hwnd, Some(HWND_TOPMOST), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW);
        for attempt in 0..20 {
            if GetForegroundWindow() == hwnd {
                return Ok(());
            }
            let fg = GetForegroundWindow();
            let fg_thread = GetWindowThreadProcessId(fg, None);
            let me = GetCurrentThreadId();
            let attached = fg_thread != 0 && fg_thread != me && AttachThreadInput(me, fg_thread, true).as_bool();
            if attempt >= 2 {
                // The process that sent the last input may take the foreground: a lone Alt tap makes it ours.
                let _ = send(&[key_input(VK_LMENU, false, false), key_input(VK_LMENU, false, true)]);
            }
            let _ = ShowWindow(hwnd, SW_RESTORE);
            let _ = BringWindowToTop(hwnd);
            let _ = SetForegroundWindow(hwnd);
            if attached {
                let _ = AttachThreadInput(me, fg_thread, false);
            }
            sleep(50);
        }
        if GetForegroundWindow() == hwnd {
            Ok(())
        } else {
            Err(format!("could not bring window {hwnd:?} to the foreground (foreground is {:?})", GetForegroundWindow()))
        }
    }
}

/// Finds a top-level window by exact title, waiting up to `ms`.
pub fn find_window(title: &str, ms: u64) -> WinResult<HWND> {
    let wide: Vec<u16> = title.encode_utf16().chain([0]).collect();
    let end = Instant::now() + Duration::from_millis(ms);
    loop {
        if let Ok(h) = unsafe { FindWindowW(None, windows::core::PCWSTR(wide.as_ptr())) }
            && !h.is_invalid()
        {
            return Ok(h);
        }
        if Instant::now() > end {
            return Err(format!("no window titled {title:?} after {ms} ms"));
        }
        sleep(50);
    }
}

pub fn post_close(hwnd: HWND) {
    unsafe {
        let _ = PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
    }
}

/// Settings that decide the pointer behaviour, as golden `settings` entries.
pub fn input_settings(hwnd: HWND) -> Vec<(String, String)> {
    unsafe {
        let layout = GetKeyboardLayout(GetWindowThreadProcessId(hwnd, None));
        vec![
            ("doubleClickTime".into(), GetDoubleClickTime().to_string()),
            ("doubleClickRect".into(), format!("[{},{}]", GetSystemMetrics(SM_CXDOUBLECLK), GetSystemMetrics(SM_CYDOUBLECLK))),
            ("dragRect".into(), format!("[{},{}]", GetSystemMetrics(SM_CXDRAG), GetSystemMetrics(SM_CYDRAG))),
            ("dpi".into(), GetDpiForWindow(hwnd).to_string()),
            ("keyboardLayout".into(), format!("\"{:08x}\"", layout.0 as usize as u32)),
        ]
    }
}

/// The OS build, `10.0.<build>.<ubr>` (registry `CurrentBuild` and `UBR`), and the edition (`DisplayVersion`).
pub fn os_build() -> (String, String) {
    use windows::Win32::System::Registry::*;
    let key = windows::core::w!("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion");
    let read_str = |name: windows::core::PCWSTR| -> String {
        let mut buf = [0u16; 128];
        let mut len = (buf.len() * 2) as u32;
        let r = unsafe { RegGetValueW(HKEY_LOCAL_MACHINE, key, name, RRF_RT_REG_SZ, None, Some(buf.as_mut_ptr().cast()), Some(&mut len)) };
        if r.is_err() {
            return String::new();
        }
        String::from_utf16_lossy(&buf[..(len as usize / 2).saturating_sub(1)])
    };
    let mut ubr = 0u32;
    let mut len = 4u32;
    let _ = unsafe { RegGetValueW(HKEY_LOCAL_MACHINE, key, windows::core::w!("UBR"), RRF_RT_REG_DWORD, None, Some((&mut ubr as *mut u32).cast()), Some(&mut len)) };
    let build = read_str(windows::core::w!("CurrentBuild"));
    (format!("10.0.{build}.{ubr}"), read_str(windows::core::w!("DisplayVersion")))
}

// ---- UI Automation -----------------------------------------------------------------------------------------------

/// What the focus-changed handler saw.
#[derive(Default)]
struct FocusLog {
    events: u32,
    last: Option<IUIAutomationElement>,
}

/// UIA elements are free-threaded in the MTA the handler and the driver share.
struct Shared(Mutex<FocusLog>);
unsafe impl Send for Shared {}
unsafe impl Sync for Shared {}

#[implement(IUIAutomationFocusChangedEventHandler)]
struct FocusHandler(Arc<Shared>);

impl IUIAutomationFocusChangedEventHandler_Impl for FocusHandler_Impl {
    fn HandleFocusChangedEvent(&self, sender: windows::core::Ref<IUIAutomationElement>) -> windows::core::Result<()> {
        let mut l = self.0.0.lock().unwrap();
        l.events += 1;
        l.last = sender.cloned();
        Ok(())
    }
}

pub struct Uia {
    pub auto: IUIAutomation,
    focus: Arc<Shared>,
    handler: Option<IUIAutomationFocusChangedEventHandler>,
}

impl Uia {
    pub fn new() -> WinResult<Uia> {
        init_thread();
        let auto: IUIAutomation = unsafe { CoCreateInstance(&CUIAutomation8, None, CLSCTX_INPROC_SERVER) }.map_err(e("CoCreateInstance(CUIAutomation8)"))?;
        let focus = Arc::new(Shared(Mutex::new(FocusLog::default())));
        let h: IUIAutomationFocusChangedEventHandler = FocusHandler(focus.clone()).into();
        unsafe { auto.AddFocusChangedEventHandler(None, &h) }.map_err(e("AddFocusChangedEventHandler"))?;
        Ok(Uia { auto, focus, handler: Some(h) })
    }

    fn same(&self, a: &IUIAutomationElement, b: &IUIAutomationElement) -> bool {
        unsafe { self.auto.CompareElements(a, b) }.is_ok_and(|b| b.as_bool())
    }

    /// The focus-changed events so far and the element of the last one.
    fn last_focus_event(&self) -> (u32, Option<IUIAutomationElement>) {
        let l = self.focus.0.lock().unwrap();
        (l.events, l.last.clone())
    }
}

impl Drop for Uia {
    fn drop(&mut self) {
        if let Some(h) = self.handler.take() {
            unsafe {
                let _ = self.auto.RemoveFocusChangedEventHandler(&h);
            }
        }
    }
}

fn bstr(b: windows::core::Result<BSTR>) -> String {
    b.map(|b| b.to_string()).unwrap_or_default()
}

fn u16_len(s: &str) -> usize {
    s.encode_utf16().count()
}

/// Where a field reading's selection direction came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    /// An empty selection: no direction to read.
    Collapsed = 0,
    /// The system caret of the field's own window (Win32 `EDIT`).
    SystemCaret = 1,
    /// `TextPattern2.GetCaretRange`.
    CaretRange = 2,
    /// Not observable (an unfocused `EDIT`): recorded forward.
    Unknown = 3,
}

impl FocusEvidence {
    /// The golden `observe` entry for the directions.
    pub fn directions_json(&self) -> String {
        let d = self.directions;
        format!("{{\"collapsed\":{},\"system-caret\":{},\"caret-range\":{},\"unknown\":{}}}", d[0], d[1], d[2], d[3])
    }
}

/// A text field's patterns.
struct Field {
    el: IUIAutomationElement,
    text: IUIAutomationTextPattern,
    text2: Option<IUIAutomationTextPattern2>,
}

impl Field {
    fn new(el: IUIAutomationElement) -> WinResult<Field> {
        let text: IUIAutomationTextPattern = unsafe { el.GetCurrentPatternAs(UIA_TextPatternId) }.map_err(e("TextPattern"))?;
        let text2 = unsafe { el.GetCurrentPatternAs::<IUIAutomationTextPattern2>(UIA_TextPattern2Id) }.ok();
        Ok(Field { el, text, text2 })
    }

    fn doc(&self) -> WinResult<IUIAutomationTextRange> {
        unsafe { self.text.DocumentRange() }.map_err(e("DocumentRange"))
    }

    fn text(&self) -> WinResult<String> {
        unsafe { self.doc()?.GetText(-1) }.map(|b| b.to_string()).map_err(e("GetText"))
    }

    /// UTF-16 offset of an endpoint of `r`: the length of the text from the document start to it.
    fn offset(&self, r: &IUIAutomationTextRange, end: TextPatternRangeEndpoint) -> WinResult<usize> {
        unsafe {
            let d = self.doc()?;
            d.MoveEndpointByRange(TextPatternRangeEndpoint_End, r, end).map_err(e("MoveEndpointByRange"))?;
            Ok(u16_len(&bstr(d.GetText(-1))))
        }
    }

    /// The range over UTF-16 offsets `[from, to)`: moved one character unit at a time until the text before each
    /// endpoint has the wanted length, whatever the provider counts as a character.
    fn range(&self, from: usize, to: usize) -> WinResult<IUIAutomationTextRange> {
        unsafe {
            let r = self.doc()?;
            r.MoveEndpointByRange(TextPatternRangeEndpoint_End, &r, TextPatternRangeEndpoint_Start).map_err(e("collapse"))?;
            for (ep, want) in [(TextPatternRangeEndpoint_End, to), (TextPatternRangeEndpoint_Start, from)] {
                let mut guard = 0;
                while self.offset(&r, ep)? < want {
                    if r.MoveEndpointByUnit(ep, TextUnit_Character, 1).map_err(e("MoveEndpointByUnit"))? == 0 || guard > 4096 {
                        break;
                    }
                    guard += 1;
                }
                let got = self.offset(&r, ep)?;
                if got != want {
                    return Err(format!("text range: endpoint at {got}, wanted {want} (UTF-16)"));
                }
            }
            Ok(r)
        }
    }

    /// For a field with its own window (a Win32 `EDIT`) that owns the system caret: whether the caret sits on the
    /// selection start `s` (UTF-16), compared by x with `EM_POSFROMCHAR`. `None` when the field has no window or
    /// does not own the caret (not focused).
    fn system_caret_at_start(&self, s: usize) -> Option<bool> {
        const EM_POSFROMCHAR: u32 = 0x00D6;
        unsafe {
            let hwnd = self.el.CurrentNativeWindowHandle().ok().filter(|h| !h.is_invalid())?;
            let mut gti = GUITHREADINFO { cbSize: std::mem::size_of::<GUITHREADINFO>() as u32, ..Default::default() };
            GetGUIThreadInfo(GetWindowThreadProcessId(hwnd, None), &mut gti).ok()?;
            if gti.hwndCaret != hwnd {
                return None;
            }
            let p = SendMessageW(hwnd, EM_POSFROMCHAR, Some(WPARAM(s)), None).0 as u32;
            if p == u32::MAX {
                return None;
            }
            let x = (p & 0xffff) as i16 as i32;
            Some((x - gti.rcCaret.left).abs() <= 1)
        }
    }

    /// `[anchor, caret]` in UTF-16 offsets, and where the direction of a non-empty selection came from.
    ///
    /// The direction: for a field with its own window, the system caret (the Win32 EDIT proxy's
    /// `TextPattern2.GetCaretRange` reports the selection end whatever the caret, so it is not used there);
    /// otherwise `TextPattern2.GetCaretRange`. With neither (an unfocused EDIT), the selection reads forward.
    fn sel16(&self) -> WinResult<([usize; 2], Direction)> {
        unsafe {
            let ranges = self.text.GetSelection().map_err(e("GetSelection"))?;
            let n = ranges.Length().map_err(e("selection length"))?;
            if n == 0 {
                return Err("no selection range".into());
            }
            let r = ranges.GetElement(0).map_err(e("selection range"))?;
            let (s, t) = (self.offset(&r, TextPatternRangeEndpoint_Start)?, self.offset(&r, TextPatternRangeEndpoint_End)?);
            if s == t {
                return Ok(([s, t], Direction::Collapsed));
            }
            let has_window = self.el.CurrentNativeWindowHandle().is_ok_and(|h| !h.is_invalid());
            if has_window {
                return Ok(match self.system_caret_at_start(s) {
                    Some(true) => ([t, s], Direction::SystemCaret),
                    Some(false) => ([s, t], Direction::SystemCaret),
                    None => ([s, t], Direction::Unknown),
                });
            }
            if let Some(t2) = &self.text2 {
                let mut active = BOOL(0);
                if let Ok(c) = t2.GetCaretRange(&mut active) {
                    let caret = self.offset(&c, TextPatternRangeEndpoint_Start)?;
                    return Ok((if caret == s { [t, s] } else { [s, t] }, Direction::CaretRange));
                }
            }
            Ok(([s, t], Direction::Unknown))
        }
    }

    /// Screen rectangle of the UTF-16 range `[from, to)` (the first line box).
    fn rect(&self, from: usize, to: usize) -> WinResult<(f64, f64, f64, f64)> {
        unsafe {
            let r = self.range(from, to)?;
            let sa: *mut SAFEARRAY = r.GetBoundingRectangles().map_err(e("GetBoundingRectangles"))?;
            if sa.is_null() {
                return Err("no bounding rectangles".into());
            }
            let n = (*sa).rgsabound[0].cElements as usize;
            let data = (*sa).pvData as *const f64;
            let out = if n >= 4 { Ok((*data, *data.add(1), *data.add(2), *data.add(3))) } else { Err(format!("{n} rectangle values")) };
            let _ = SafeArrayDestroy(sa);
            out
        }
    }
}

/// Opens and closes one fixture window per pin.
pub trait WinFixture {
    /// Creates the fixture window titled `title` and returns its handle (top-level, visible).
    fn open(&mut self, title: &str) -> WinResult<HWND>;
    fn close(&mut self) -> WinResult<()>;
}

/// Focus evidence: checkpoint-to-checkpoint focus changes, and how many of them the last UIA focus-changed event
/// announced (it named the newly focused element).
#[derive(Default, Clone, Copy, Debug)]
pub struct FocusEvidence {
    pub changes: u32,
    pub announced: u32,
    /// Field readings at checkpoints by where the selection's direction came from, indexed by [`Direction`].
    pub directions: [u32; 4],
}

pub struct UiaDriver<F: WinFixture> {
    pub fixture: F,
    pub uia: Uia,
    trace: Trace,
    hwnd: HWND,
    fields: Vec<(String, Field)>,
    button: Option<IUIAutomationElement>,
    last_focus: Option<Option<String>>,
    seen_events: u32,
    /// Field readings of the last `read` by [`Direction`].
    reading: [u32; 4],
    pub evidence: FocusEvidence,
    /// The settings read at the first setup.
    pub settings: Vec<(String, String)>,
    /// The pin's pseudo-anchors (`clipboard`, `primary`).
    observe: Vec<String>,
    /// While a pin uses dead keys: the layout to restore and the one this driver loaded.
    layout: Option<(HKL, Option<HKL>)>,
}

const HANDLES: [(&str, u64); 3] = [("a", 2), ("b", 3), ("c", 4)];

fn handle_of(anchor: &str) -> u64 {
    HANDLES.iter().find(|(a, _)| *a == anchor).map_or(0, |h| h.1)
}

impl<F: WinFixture> UiaDriver<F> {
    pub fn new(fixture: F) -> WinResult<Self> {
        Ok(UiaDriver {
            fixture,
            uia: Uia::new()?,
            trace: Trace::default(),
            hwnd: HWND::default(),
            fields: Vec::new(),
            button: None,
            last_focus: None,
            seen_events: 0,
            reading: [0; 4],
            evidence: FocusEvidence::default(),
            settings: Vec::new(),
            observe: Vec::new(),
            layout: None,
        })
    }

    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

    fn field(&self, anchor: &str) -> WinResult<&Field> {
        self.fields.iter().find(|(a, _)| a == anchor).map(|(_, f)| f).ok_or_else(|| format!("no field {anchor}"))
    }

    /// The fixture's two `Edit` elements (tree order) and its `Done` button, waiting until UIA shows them.
    fn find_elements(&mut self) -> WinResult<()> {
        let end = Instant::now() + Duration::from_secs(15);
        loop {
            let found = unsafe {
                let root = self.uia.auto.ElementFromHandle(self.hwnd).map_err(e("ElementFromHandle"))?;
                let all = root.FindAll(TreeScope_Descendants, &self.uia.auto.CreateTrueCondition().map_err(e("condition"))?).map_err(e("FindAll"))?;
                let mut edits = Vec::new();
                let mut button = None;
                for i in 0..all.Length().unwrap_or(0) {
                    let el = all.GetElement(i).map_err(e("GetElement"))?;
                    let ct = el.CurrentControlType().unwrap_or_default();
                    if ct == UIA_EditControlTypeId {
                        edits.push(el);
                    } else if ct == UIA_ButtonControlTypeId && bstr(el.CurrentName()) == BUTTON_LABEL && button.is_none() {
                        button = Some(el);
                    }
                }
                (edits, button)
            };
            if let (edits, Some(b)) = found
                && edits.len() >= 2
            {
                self.fields.clear();
                for (a, el) in FIELDS.iter().zip(edits) {
                    self.fields.push((a.to_string(), Field::new(el)?));
                }
                self.button = Some(b);
                return Ok(());
            }
            if Instant::now() > end {
                return Err("the fixture's two Edit elements and its Done button did not appear in UIA".into());
            }
            sleep(100);
        }
    }

    /// Screen point of a click target: 30% into the character at `at` (UTF-16 rect), or the button's centre.
    fn point(&self, target: &str, at: Option<usize>) -> WinResult<(i32, i32)> {
        if target == BUTTON {
            let r: RECT = unsafe { self.button.as_ref().ok_or("no button")?.CurrentBoundingRectangle() }.map_err(e("button rect"))?;
            return Ok(((r.left + r.right) / 2, (r.top + r.bottom) / 2));
        }
        let f = self.field(target)?;
        let text = f.text()?;
        let at = at.unwrap_or(0);
        let len = text.chars().count();
        let (x, y, w, h) = if at < len {
            let s = scalar_to_utf16(&text, at);
            let t = scalar_to_utf16(&text, at + 1);
            let (x, y, w, h) = f.rect(s, t)?;
            (x + w * 0.3, y, 0.0, h)
        } else {
            let s = scalar_to_utf16(&text, len - 1);
            let (x, y, w, h) = f.rect(s, u16_len(&text))?;
            (x + w + 6.0, y, 0.0, h)
        };
        let _ = w;
        Ok((x.round() as i32, (y + h / 2.0).round() as i32))
    }

    fn read_field(&mut self, anchor: &str) -> WinResult<State> {
        let f = self.field(anchor)?;
        let text = f.text()?;
        let ([a, c], dir) = f.sel16()?;
        let focus = self.focused()?.as_deref() == Some(anchor);
        self.reading[dir as usize] += 1;
        Ok(State { sel: Some([utf16_to_scalar(&text, a), utf16_to_scalar(&text, c)]), text: Some(text), focus })
    }

    /// The anchor UIA reports focused while the fixture window is in front.
    fn focused(&self) -> WinResult<Option<String>> {
        if unsafe { GetForegroundWindow() } != self.hwnd {
            return Ok(None);
        }
        let Ok(f) = (unsafe { self.uia.auto.GetFocusedElement() }) else { return Ok(None) };
        Ok(self.anchor_of(&f))
    }

    fn anchor_of(&self, el: &IUIAutomationElement) -> Option<String> {
        for (a, fd) in &self.fields {
            if self.uia.same(el, &fd.el) {
                return Some(a.clone());
            }
        }
        if self.button.as_ref().is_some_and(|b| self.uia.same(el, b)) {
            return Some(BUTTON.to_string());
        }
        None
    }

    fn read(&mut self) -> WinResult<Checkpoint> {
        self.reading = [0; 4];
        let mut cp = Vec::new();
        for a in FIELDS {
            cp.push((a.to_string(), self.read_field(a)?));
        }
        let focus = self.focused()?.as_deref() == Some(BUTTON);
        cp.push((BUTTON.to_string(), State { text: None, sel: None, focus }));
        Ok(cp)
    }

    fn wait_idle(&self) {
        sleep(60);
    }
}

impl<F: WinFixture> Driver for UiaDriver<F> {
    fn setup(&mut self, pin: &Pin) -> Result<(), String> {
        let title = format!("svelters-na {}", pin.id);
        self.trace = Trace::default();
        self.hwnd = self.fixture.open(&title)?;
        self.trace.create("window", 1);
        self.trace.set_str(1, "title", &title);
        bring_to_front(self.hwnd)?;
        self.find_elements()?;
        // Pins 5-6 (WG3b): an empty clipboard, then the pin's seed, owned by this process; the US-International
        // layout (the Windows row of keys.json) while a pin uses dead keys.
        set_clipboard(pin.setup.clipboard.as_deref())?;
        self.observe = pin.observe.clone();
        let dead = pin.actions.iter().any(|a| matches!(a, crate::pins::Action::Key { intent, .. } if intent.starts_with("dead-")));
        if dead {
            self.layout = Some(switch_layout(self.hwnd, "00020409")?);
            self.trace.call(1, "keyboardLayout", "[\"00020409\"]");
        }
        if self.settings.is_empty() {
            self.settings = input_settings(self.hwnd);
        }
        for (a, h) in HANDLES {
            self.trace.create(if a == BUTTON { "button" } else { "text-field" }, h);
            self.trace.insert(1, h);
        }
        // Text, then the selection of every unfocused field, focus, then the focused field's selection.
        let focus = pin.setup.focus.clone();
        for a in FIELDS {
            let f = pin.setup.field(a);
            let vp: IUIAutomationValuePattern = unsafe { self.field(a)?.el.GetCurrentPatternAs(UIA_ValuePatternId) }.map_err(e("ValuePattern"))?;
            unsafe { vp.SetValue(&BSTR::from(f.text.as_str())) }.map_err(e("SetValue"))?;
            self.trace.call(handle_of(a), "setValue", &format!("[{}]", serde_json::to_string(&f.text).unwrap()));
        }
        self.wait_idle();
        let select = |d: &mut Self, a: &str| -> WinResult<()> {
            let f = pin.setup.field(a);
            let [s, t] = f.sel();
            let (s16, t16) = (scalar_to_utf16(&f.text, s), scalar_to_utf16(&f.text, t));
            let r = d.field(a)?.range(s16.min(t16), s16.max(t16))?;
            unsafe { r.Select() }.map_err(e("Select"))?;
            d.trace.call(handle_of(a), "select", &format!("[{s},{t}]"));
            Ok(())
        };
        for a in FIELDS {
            if focus.as_deref() != Some(a) {
                select(self, a)?;
            }
        }
        if let Some(fa) = &focus {
            let el = if fa == BUTTON { self.button.clone().ok_or("no button")? } else { self.field(fa)?.el.clone() };
            unsafe { el.SetFocus() }.map_err(e("SetFocus"))?;
            self.trace.call(handle_of(fa), "setFocus", "[]");
            self.wait_idle();
            if fa != BUTTON {
                select(self, fa)?;
            }
        }
        self.wait_idle();
        self.last_focus = None;
        self.seen_events = self.uia.last_focus_event().0;
        Ok(())
    }

    fn key(&mut self, chord: &Chord) -> Result<(), String> {
        bring_to_front(self.hwnd)?;
        key(chord)
    }

    fn click(&mut self, target: &str, at: Option<usize>, count: u32, button: u32) -> Result<(), String> {
        sleep(click_gap());
        let (x, y) = self.point(target, at)?;
        bring_to_front(self.hwnd)?;
        click_at(x, y, count, button)
    }

    fn type_text(&mut self, text: &str) -> Result<(), String> {
        bring_to_front(self.hwnd)?;
        let hkl = unsafe { GetKeyboardLayout(GetWindowThreadProcessId(self.hwnd, None)) };
        for c in text.chars() {
            type_char(c, hkl)?;
            sleep(30);
        }
        Ok(())
    }

    fn drag(&mut self, target: &str, from: usize, to: usize, count: u32) -> Result<(), String> {
        sleep(click_gap());
        let a = self.point(target, Some(from))?;
        let b = self.point(target, Some(to))?;
        bring_to_front(self.hwnd)?;
        drag(a, b, count)
    }

    fn observe(&mut self) -> Result<Checkpoint, String> {
        // Settle: read until two readings 50 ms apart agree (at most 2 s).
        self.wait_idle();
        let end = Instant::now() + Duration::from_secs(2);
        let mut prev = self.read()?;
        loop {
            sleep(50);
            let cur = self.read()?;
            if cur == prev || Instant::now() > end {
                prev = cur;
                break;
            }
            prev = cur;
        }
        for (d, r) in self.evidence.directions.iter_mut().zip(self.reading) {
            *d += r;
        }
        let focus = prev.iter().find(|(_, s)| s.focus).map(|(a, _)| a.clone());
        // Pseudo-anchors: the clipboard's text; Windows has no PRIMARY selection, so `primary` is always "".
        for name in self.observe.clone() {
            let text = if name == CLIPBOARD { clipboard_text()? } else { String::new() };
            prev.push((name, State::selection(text)));
        }
        if let Some(last) = &self.last_focus
            && *last != focus
        {
            self.evidence.changes += 1;
            let (n, el) = self.uia.last_focus_event();
            let named = el.as_ref().and_then(|el| self.anchor_of(el));
            if n > self.seen_events && named == focus {
                self.evidence.announced += 1;
            }
        }
        self.seen_events = self.uia.last_focus_event().0;
        self.last_focus = Some(focus);
        for (a, s) in &prev {
            let h = if self.observe.contains(a) { crate::selection_handle(a) } else { handle_of(a) };
            self.trace.state(h, s);
        }
        Ok(prev)
    }

    fn teardown(&mut self) -> Result<String, String> {
        if let Some((before, loaded)) = self.layout.take() {
            let restored = activate_layout(self.hwnd, before);
            if let Some(hkl) = loaded {
                unsafe {
                    let _ = UnloadKeyboardLayout(hkl);
                }
            }
            restored?;
        }
        self.fields.clear();
        self.button = None;
        self.fixture.close()?;
        self.trace.destroy(1);
        Ok(self.trace.text())
    }
}

/// The file version (`major.minor.build.revision`) of a file, from its version resource.
pub fn file_version(path: &str) -> Option<String> {
    use windows::Win32::Storage::FileSystem::{GetFileVersionInfoSizeW, GetFileVersionInfoW, VS_FIXEDFILEINFO, VerQueryValueW};
    let wide: Vec<u16> = path.encode_utf16().chain([0]).collect();
    let pw = windows::core::PCWSTR(wide.as_ptr());
    unsafe {
        let size = GetFileVersionInfoSizeW(pw, None);
        if size == 0 {
            return None;
        }
        let mut data = vec![0u8; size as usize];
        GetFileVersionInfoW(pw, None, size, data.as_mut_ptr().cast()).ok()?;
        let mut p: *mut std::ffi::c_void = std::ptr::null_mut();
        let mut len = 0u32;
        if !VerQueryValueW(data.as_ptr().cast(), windows::core::w!("\\"), &mut p, &mut len).as_bool() || p.is_null() {
            return None;
        }
        let fi = &*(p as *const VS_FIXEDFILEINFO);
        Some(format!("{}.{}.{}.{}", fi.dwFileVersionMS >> 16, fi.dwFileVersionMS & 0xffff, fi.dwFileVersionLS >> 16, fi.dwFileVersionLS & 0xffff))
    }
}

/// The path of the module named `name` (file name, any case: `comctl32.dll`) loaded in process `pid`.
pub fn module_path(pid: u32, name: &str) -> Option<String> {
    use windows::Win32::Foundation::{CloseHandle, HMODULE};
    use windows::Win32::System::ProcessStatus::{EnumProcessModulesEx, GetModuleFileNameExW, LIST_MODULES_ALL};
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ};
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, pid).ok()?;
        let mut mods = vec![HMODULE::default(); 1024];
        let mut needed = 0u32;
        let size = (mods.len() * std::mem::size_of::<HMODULE>()) as u32;
        let mut out = None;
        if EnumProcessModulesEx(h, mods.as_mut_ptr(), size, &mut needed, LIST_MODULES_ALL).is_ok() {
            let n = (needed as usize / std::mem::size_of::<HMODULE>()).min(mods.len());
            for m in &mods[..n] {
                let mut buf = [0u16; 1024];
                let len = GetModuleFileNameExW(Some(h), Some(*m), &mut buf) as usize;
                let path = String::from_utf16_lossy(&buf[..len]);
                if path.rsplit('\\').next().is_some_and(|f| f.eq_ignore_ascii_case(name)) {
                    out = Some(path);
                    break;
                }
            }
        }
        let _ = CloseHandle(h);
        out
    }
}
