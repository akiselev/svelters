//! SV75: the desktop clipboard (`platform::SystemClipboard`, feature `system-clipboard`) without
//! egui: a write reaches the system clipboard, the in-process types come back with it, and a copy
//! by someone else (the system text changed) drops them. It needs a clipboard: on Linux a display
//! (CI runs the workspace under Xvfb); it skips with a note where there is none. It puts back the
//! text the clipboard held.
#![cfg(feature = "system-clipboard")]

use svelters_host::platform::{ClipboardBackend, ClipboardKind, MimeData, SystemClipboard, TEXT_PLAIN};

fn wait_for(cb: &SystemClipboard, want: &str) -> Option<String> {
    let mut got = None;
    for _ in 0..20 {
        got = cb.read(ClipboardKind::Clipboard, &[TEXT_PLAIN]).ok().and_then(|d| d.plain_text().map(str::to_string));
        if got.as_deref() == Some(want) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    got
}

#[test]
fn system_clipboard_keeps_app_types_beside_the_system_text() {
    #[cfg(target_os = "linux")]
    if std::env::var_os("DISPLAY").is_none() && std::env::var_os("WAYLAND_DISPLAY").is_none() {
        eprintln!("skipped: no display, so no clipboard");
        return;
    }
    let cb = SystemClipboard::new();
    let saved = match cb.read(ClipboardKind::Clipboard, &[TEXT_PLAIN]) {
        Ok(d) => d.plain_text().map(str::to_string),
        Err(e) => {
            eprintln!("skipped: no OS clipboard ({e})");
            return;
        }
    };
    let marker = format!("svelters-sv75-{}", std::process::id());
    let mut data = MimeData::text(marker.clone());
    data.set("application/x-sv75+json", "{\"a\":1}");
    cb.write(ClipboardKind::Clipboard, data).expect("write the system clipboard");
    assert_eq!(wait_for(&cb, &marker).as_deref(), Some(marker.as_str()), "the text did not reach the system clipboard");
    let back = cb.read(ClipboardKind::Clipboard, &["application/*"]).expect("read");
    assert_eq!(back.get_str("application/x-sv75+json"), Some("{\"a\":1}"), "the in-process type did not come back");

    // another writer (a second handle: what another app does) replaces the text
    let other = SystemClipboard::new();
    let theirs = format!("from-another-app-{}", std::process::id());
    other.write(ClipboardKind::Clipboard, MimeData::text(theirs.clone())).expect("write");
    assert_eq!(wait_for(&cb, &theirs).as_deref(), Some(theirs.as_str()));
    let back = cb.read(ClipboardKind::Clipboard, &[]).expect("read");
    assert_eq!(back.get_str("application/x-sv75+json"), None, "a stale in-process type survived another copy");

    if let Some(t) = saved {
        let _ = cb.write(ClipboardKind::Clipboard, MimeData::text(t));
    }
}
