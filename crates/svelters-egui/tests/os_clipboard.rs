//! CB1: egui's text fields copy and paste through egui-winit's clipboard (the integration applies
//! `OutputCommand::CopyText` and reads it for `Event::Paste`). svelters-egui turns on egui-winit's
//! `clipboard` feature for every app that links it, so that clipboard is the OS one; without the
//! feature it is a string inside the process and nothing reaches other apps (found on macOS by
//! cadatomic's SH1 acceptance: Cmd+C in a Svelte field left NSPasteboard empty).
//!
//! The check writes through egui-winit's clipboard and reads the OS clipboard through a separate
//! arboard handle, then puts back what the clipboard held. It needs a clipboard: on Linux a display
//! (CI runs it under Xvfb); it skips with a note where there is none.

#[test]
fn egui_winit_clipboard_is_the_os_clipboard() {
    #[cfg(target_os = "linux")]
    if std::env::var_os("DISPLAY").is_none() && std::env::var_os("WAYLAND_DISPLAY").is_none() {
        eprintln!("skipped: no display, so no clipboard");
        return;
    }
    let mut os = match arboard::Clipboard::new() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("skipped: no OS clipboard ({e})");
            return;
        }
    };
    let saved = os.get_text().ok();
    let marker = format!("svelters-cb1-{}", std::process::id());
    // SAFETY: `new` is unsafe for the raw display handle it may keep; with `None` there is none
    // (arboard then opens its own connection).
    let mut egui_side = unsafe { egui_winit::clipboard::Clipboard::new(None) };
    egui_side.set_text(marker.clone());
    // X11 serves the selection from arboard's own thread; give it a moment
    let mut got = None;
    for _ in 0..20 {
        got = os.get_text().ok();
        if got.as_deref() == Some(marker.as_str()) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    // the other direction: what another app put there is what a paste reads
    let other = format!("from-another-app-{}", std::process::id());
    os.set_text(other.clone()).expect("write the OS clipboard");
    let mut read = None;
    for _ in 0..20 {
        read = egui_side.get();
        if read.as_deref() == Some(other.as_str()) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    if let Some(t) = saved {
        let _ = os.set_text(t);
    }
    assert_eq!(
        got.as_deref(),
        Some(marker.as_str()),
        "a copy in an egui text field did not reach the OS clipboard: egui-winit's `clipboard` feature is off"
    );
    assert_eq!(
        read.as_deref(),
        Some(other.as_str()),
        "a paste in an egui text field did not read the OS clipboard"
    );
}
