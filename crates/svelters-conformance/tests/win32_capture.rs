//! NA1 capture on Win32 (WG3w): every pin of `conformance/pins/` on two common-controls v6 `EDIT` controls and a
//! `BUTTON` in a dialog-class window (DLGWINDOWEXTRA + `DefDlgProcW`, `IsDialogMessageW` in the loop, so Tab,
//! Shift+Tab and focus restore are the dialog manager's), driven by `svelters_conformance::win::UiaDriver`:
//! `SendInput` for keys and clicks, UI Automation for text, selection, caret and focus.
//!
//! The window runs on its own thread (a UIA client must not share the UI thread). build.rs links the test with
//! a manifest dependency on Microsoft.Windows.Common-Controls 6.0, as a themed Win32 app has; the capture
//! records the comctl32.dll it loaded.
//!
//! Writes `target/na/windows/win32-edit-<comctl32 version>.json` and the traces; compares with
//! `conformance/golden/windows/` when that version has a golden; `NA_UPDATE=1` rewrites it. `NA_PINS=a,b` runs
//! only those pin ids (no golden comparison then). Needs an unlocked interactive session; do not touch the mouse.

#![cfg(windows)]

use std::sync::mpsc;
use std::thread::JoinHandle;
use svelters_conformance::golden::{Capture, finish};
use svelters_conformance::pins::{BUTTON_LABEL, Keys, load_pins};
use svelters_conformance::run_pin;
use svelters_conformance::win::{UiaDriver, WinFixture, WinResult, find_window, init_thread, os_build, post_close};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{COLOR_BTNFACE, CreateFontIndirectW, HFONT};
use windows::Win32::System::LibraryLoader::{GetModuleFileNameW, GetModuleHandleW};
use windows::Win32::UI::Controls::{ICC_STANDARD_CLASSES, INITCOMMONCONTROLSEX, InitCommonControlsEx};
use windows::Win32::UI::HiDpi::{AdjustWindowRectExForDpi, GetDpiForSystem};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{PCWSTR, w};

const CLASS: PCWSTR = w!("svelters-na-dialog");
/// DLGWINDOWEXTRA is 30 on 32-bit Windows and larger on 64-bit: room to spare.
const DLG_EXTRA: i32 = 64;

unsafe extern "system" fn wndproc(h: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_CLOSE => {
                let _ = DestroyWindow(h);
                LRESULT(0)
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefDlgProcW(h, msg, wp, lp),
        }
    }
}

fn register() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| unsafe {
        let icc = INITCOMMONCONTROLSEX { dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32, dwICC: ICC_STANDARD_CLASSES };
        let _ = InitCommonControlsEx(&icc);
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(wndproc),
            cbWndExtra: DLG_EXTRA,
            hInstance: GetModuleHandleW(None).unwrap().into(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hbrBackground: windows::Win32::Graphics::Gdi::HBRUSH((COLOR_BTNFACE.0 + 1) as usize as *mut _),
            lpszClassName: CLASS,
            ..Default::default()
        };
        assert_ne!(RegisterClassExW(&wc), 0, "RegisterClassExW");
    });
}

/// The dialog font (the message font of the non-client metrics), as a themed dialog uses.
fn dialog_font() -> HFONT {
    unsafe {
        let mut m = NONCLIENTMETRICSW { cbSize: std::mem::size_of::<NONCLIENTMETRICSW>() as u32, ..Default::default() };
        let _ = SystemParametersInfoW(SPI_GETNONCLIENTMETRICS, m.cbSize, Some((&mut m as *mut NONCLIENTMETRICSW).cast()), SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0));
        CreateFontIndirectW(&m.lfMessageFont)
    }
}

/// The UI thread: the window, its controls and the dialog loop, until the window is destroyed.
fn ui_thread(title: String, tx: mpsc::Sender<WinResult<isize>>) {
    init_thread();
    register();
    unsafe {
        let dpi = GetDpiForSystem();
        let s = |v: i32| v * dpi as i32 / 96;
        let (cw, ch) = (s(640), s(130));
        let style = WS_CAPTION | WS_SYSMENU | WS_VISIBLE;
        let ex = WS_EX_TOPMOST | WS_EX_CONTROLPARENT | WS_EX_DLGMODALFRAME;
        let mut r = RECT { left: 0, top: 0, right: cw, bottom: ch };
        let _ = AdjustWindowRectExForDpi(&mut r, style, false, ex, dpi);
        let wide: Vec<u16> = title.encode_utf16().chain([0]).collect();
        let hwnd = match CreateWindowExW(ex, CLASS, PCWSTR(wide.as_ptr()), style, s(80), s(80), r.right - r.left, r.bottom - r.top, None, None, None, None) {
            Ok(h) => h,
            Err(e) => {
                let _ = tx.send(Err(format!("CreateWindowExW: {e}")));
                return;
            }
        };
        let font = dialog_font();
        let child = |class: PCWSTR, text: PCWSTR, style: i32, ex: WINDOW_EX_STYLE, id: usize, y: i32, w: i32| -> HWND {
            let h = CreateWindowExW(
                ex,
                class,
                text,
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(style as u32),
                s(12),
                y,
                w,
                s(26),
                Some(hwnd),
                Some(HMENU(id as *mut _)),
                None,
                None,
            )
            .expect("child control");
            SendMessageW(h, WM_SETFONT, Some(WPARAM(font.0 as usize)), Some(LPARAM(1)));
            h
        };
        child(w!("EDIT"), w!(""), ES_AUTOHSCROLL | ES_LEFT, WS_EX_CLIENTEDGE, 101, s(12), s(616));
        child(w!("EDIT"), w!(""), ES_AUTOHSCROLL | ES_LEFT, WS_EX_CLIENTEDGE, 102, s(50), s(616));
        let label: Vec<u16> = BUTTON_LABEL.encode_utf16().chain([0]).collect();
        child(w!("BUTTON"), PCWSTR(label.as_ptr()), BS_PUSHBUTTON, WINDOW_EX_STYLE(0), 103, s(90), s(90));
        let _ = tx.send(Ok(hwnd.0 as isize));
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            if !IsDialogMessageW(hwnd, &msg).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }
}

#[derive(Default)]
struct Win32 {
    thread: Option<JoinHandle<()>>,
    hwnd: isize,
}

impl WinFixture for Win32 {
    fn open(&mut self, title: &str) -> WinResult<HWND> {
        let (tx, rx) = mpsc::channel();
        let t = title.to_string();
        self.thread = Some(std::thread::spawn(move || ui_thread(t, tx)));
        self.hwnd = rx.recv().map_err(|e| e.to_string())??;
        find_window(title, 5000)
    }

    fn close(&mut self) -> WinResult<()> {
        post_close(HWND(self.hwnd as *mut _));
        if let Some(t) = self.thread.take() {
            t.join().map_err(|_| "the UI thread panicked".to_string())?;
        }
        Ok(())
    }
}

/// The loaded comctl32.dll: its path and file version (`major.minor.build.revision`).
fn comctl32() -> (String, String) {
    use windows::Win32::Storage::FileSystem::{GetFileVersionInfoSizeW, GetFileVersionInfoW, VS_FIXEDFILEINFO, VerQueryValueW};
    unsafe {
        let Ok(m) = GetModuleHandleW(w!("comctl32.dll")) else { return ("not loaded".into(), "unknown".into()) };
        let mut buf = [0u16; 520];
        let n = GetModuleFileNameW(Some(m), &mut buf) as usize;
        let path = String::from_utf16_lossy(&buf[..n]);
        let pw = PCWSTR(buf.as_ptr());
        let size = GetFileVersionInfoSizeW(pw, None);
        let mut data = vec![0u8; size as usize];
        if size == 0 || GetFileVersionInfoW(pw, None, size, data.as_mut_ptr().cast()).is_err() {
            return (path, "unknown".into());
        }
        let mut p: *mut std::ffi::c_void = std::ptr::null_mut();
        let mut len = 0u32;
        if !VerQueryValueW(data.as_ptr().cast(), w!("\\"), &mut p, &mut len).as_bool() || p.is_null() {
            return (path, "unknown".into());
        }
        let fi = &*(p as *const VS_FIXEDFILEINFO);
        let v = format!("{}.{}.{}.{}", fi.dwFileVersionMS >> 16, fi.dwFileVersionMS & 0xffff, fi.dwFileVersionLS >> 16, fi.dwFileVersionLS & 0xffff);
        (path, v)
    }
}

#[test]
fn win32_edit_capture() {
    let keys = Keys::load("windows").unwrap();
    let mut pins = load_pins().unwrap();
    let only = std::env::var("NA_PINS").ok();
    if let Some(o) = &only {
        pins.retain(|p| o.split(',').any(|id| id == p.id));
    }
    let mut d = UiaDriver::new(Win32::default()).unwrap();
    let mut results = Vec::new();
    let mut traces = Vec::new();
    for pin in &pins {
        let (r, t) = run_pin(&mut d, pin, &keys).unwrap_or_else(|e| {
            let _ = d.fixture.close();
            panic!("{e}")
        });
        eprintln!("NA1 win32: {} done", pin.id);
        results.push(r);
        traces.push((pin.id.clone(), t));
    }
    let (os, release) = os_build();
    let (path, version) = comctl32();
    assert!(path.to_ascii_lowercase().contains("winsxs"), "comctl32 v6 is not loaded (the manifest of build.rs): {path}");
    let mut settings = vec![
        ("os".to_string(), format!("\"{os} ({release})\"")),
        ("comctl32".to_string(), format!("\"{version} (WinSxS, manifest dependency Microsoft.Windows.Common-Controls 6.0)\"")),
        ("window".to_string(), "\"dialog class (DefDlgProcW, IsDialogMessageW)\"".to_string()),
    ];
    settings.extend(d.settings.clone());
    settings.sort();
    let ev = d.evidence;
    let cap = Capture {
        platform: "windows".into(),
        toolkit: "win32-edit".into(),
        version,
        injector: "sendinput+uia".into(),
        settings,
        observe: vec![
            ("focus".into(), format!("{{\"changes\":{},\"announced\":{}}}", ev.changes, ev.announced)),
            ("caret-range".into(), format!("{{\"readings\":{},\"from-caret-range\":{}}}", ev.caret_ranges.1, ev.caret_ranges.0)),
        ],
        pins: results,
    };
    if only.is_some() {
        let dir = svelters_conformance::golden::out_dir("windows", "partial");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(cap.file_name()), cap.to_json()).unwrap();
        eprintln!("{}", cap.to_json());
        return;
    }
    finish(&cap, &traces).unwrap();
}
