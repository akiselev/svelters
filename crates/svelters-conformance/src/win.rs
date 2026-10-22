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
//! - Clicks and drags: `SetCursorPos`, then button events through `SendInput`; the press point is 30% into the
//!   character (its UIA bounding rectangle), as on Linux. Before every click or drag the driver waits longer than
//!   the double-click time, so a sequence never joins the previous one.
//! - State: text from the document range; selection from `TextPattern.GetSelection`, its direction from
//!   `TextPattern2.GetCaretRange` (the caret is the end the caret range sits on). Offsets are measured as the
//!   UTF-16 length of the text before an endpoint and converted to Unicode scalars.
//! - Focus: `IUIAutomation::GetFocusedElement` compared with each anchor, counted only while the fixture window is
//!   the foreground window. A focus-changed event handler records what UIA announced; the capture reports, per
//!   checkpoint-to-checkpoint focus change, whether the last event named the new focus (`observe.focus`).
//!
//! The process is made per-monitor DPI aware, so UIA rectangles, `SetCursorPos` and the window share physical
//! pixels. Input goes to the interactive desktop: a capture needs an unlocked session and nobody using the mouse.

use crate::Driver;
use crate::pins::{BUTTON, BUTTON_LABEL, Chord, FIELDS, Pin};
use crate::trace::{Checkpoint, State, Trace, scalar_to_utf16, utf16_to_scalar};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{HWND, LPARAM, POINT, RECT, WPARAM};
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

fn vk_of(key: &str) -> WinResult<(VIRTUAL_KEY, bool)> {
    Ok(match key {
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
    let (vk, ext) = vk_of(&chord.key)?;
    let mut mods = Vec::new();
    if chord.ctrl {
        mods.push(VK_LCONTROL);
    }
    if chord.alt {
        mods.push(VK_LMENU);
    }
    if chord.shift {
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

pub fn move_to(x: i32, y: i32) -> WinResult<()> {
    unsafe { SetCursorPos(x, y) }.map_err(e("SetCursorPos"))
}

pub fn button(down: bool) -> WinResult<()> {
    send(&[mouse_input(if down { MOUSEEVENTF_LEFTDOWN } else { MOUSEEVENTF_LEFTUP })])
}

/// Milliseconds a click sequence waits so it never joins the previous one.
pub fn click_gap() -> u64 {
    (unsafe { GetDoubleClickTime() } as u64 + 200).max(700)
}

/// `count` clicks at a screen point (physical pixels), 40 ms apart.
pub fn click_at(x: i32, y: i32, count: u32) -> WinResult<()> {
    move_to(x, y)?;
    sleep(30);
    for i in 0..count {
        if i > 0 {
            sleep(40);
        }
        button(true)?;
        sleep(20);
        button(false)?;
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

    /// `[anchor, caret]` in UTF-16 offsets, and whether the direction came from the caret range.
    fn sel16(&self) -> WinResult<([usize; 2], bool)> {
        unsafe {
            let ranges = self.text.GetSelection().map_err(e("GetSelection"))?;
            let n = ranges.Length().map_err(e("selection length"))?;
            if n == 0 {
                return Err("no selection range".into());
            }
            let r = ranges.GetElement(0).map_err(e("selection range"))?;
            let (s, t) = (self.offset(&r, TextPatternRangeEndpoint_Start)?, self.offset(&r, TextPatternRangeEndpoint_End)?);
            if let Some(t2) = &self.text2 {
                let mut active = BOOL(0);
                if let Ok(c) = t2.GetCaretRange(&mut active) {
                    let caret = self.offset(&c, TextPatternRangeEndpoint_Start)?;
                    return Ok((if caret == s && s != t { [t, s] } else { [s, t] }, true));
                }
            }
            Ok(([s, t], false))
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
    /// Checkpoints whose selection direction came from `TextPattern2.GetCaretRange`, of all field readings.
    pub caret_ranges: (u32, u32),
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
    /// Field readings of the last `read`, and how many took their direction from the caret range.
    reading: (u32, u32),
    pub evidence: FocusEvidence,
    /// The settings read at the first setup.
    pub settings: Vec<(String, String)>,
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
            reading: (0, 0),
            evidence: FocusEvidence::default(),
            settings: Vec::new(),
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
        let ([a, c], from_caret) = f.sel16()?;
        let focus = self.focused()?.as_deref() == Some(anchor);
        self.reading.1 += 1;
        if from_caret {
            self.reading.0 += 1;
        }
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
        self.reading = (0, 0);
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

    fn click(&mut self, target: &str, at: Option<usize>, count: u32) -> Result<(), String> {
        sleep(click_gap());
        let (x, y) = self.point(target, at)?;
        bring_to_front(self.hwnd)?;
        click_at(x, y, count)
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
        self.evidence.caret_ranges.0 += self.reading.0;
        self.evidence.caret_ranges.1 += self.reading.1;
        let focus = prev.iter().find(|(_, s)| s.focus).map(|(a, _)| a.clone());
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
            self.trace.state(handle_of(a), s);
        }
        Ok(prev)
    }

    fn teardown(&mut self) -> Result<String, String> {
        self.fields.clear();
        self.button = None;
        self.fixture.close()?;
        self.trace.destroy(1);
        Ok(self.trace.text())
    }
}

/// `POINT` helper for fixtures.
pub fn client_to_screen(hwnd: HWND, x: i32, y: i32) -> (i32, i32) {
    let mut p = POINT { x, y };
    unsafe {
        let _ = windows::Win32::Graphics::Gdi::ClientToScreen(hwnd, &mut p);
    }
    (p.x, p.y)
}
