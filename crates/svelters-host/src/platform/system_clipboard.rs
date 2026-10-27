//! DX2 / SV75: the desktop system clipboard ([`SystemClipboard`], arboard) behind
//! `svelters:clipboard`, with the Linux primary selection ([`system_primary`]). Feature
//! `system-clipboard`; moved here from svelters-egui (which re-exports it as
//! `svelters_egui::platform::SystemClipboard`) so an embedder without egui (svelters-skelter) builds
//! the desktop [`Platform::system`](super::Platform::system).
//!
//! Writer lane: DX2 (plans/milestones/DX2-clipboard-files.md).

use std::sync::{Mutex, MutexGuard};
use super::{is_text_type, ClipError, ClipboardBackend, ClipboardKind, MimeData, TEXT_HTML, TEXT_PLAIN};

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// The system clipboard through arboard. The system holds `text/plain` (and `text/html` on the
/// clipboard); every other type (an app's `application/x-…+json`, `cad/part`) is kept in
/// process next to the text it was written with. A read returns the whole written data while
/// the system text is still that text (nobody copied since), and the system's text otherwise,
/// so other apps paste the text and this process gets every format back.
///
/// The plain-text fallback written for data without `text/plain` is its first text entry (JSON
/// pastes as JSON in a text editor), or empty.
pub struct SystemClipboard {
    /// Created on first use (it needs a display); kept alive, since on X11 and Wayland the
    /// data lives as long as its owner.
    cb: Mutex<Option<arboard::Clipboard>>,
    /// What this process wrote last, per kind: the text it put on the system, and all of it.
    written: Mutex<[Option<(String, MimeData)>; 2]>,
}

impl Default for SystemClipboard {
    fn default() -> Self {
        SystemClipboard { cb: Mutex::new(None), written: Mutex::new([None, None]) }
    }
}

fn slot(k: ClipboardKind) -> usize {
    match k {
        ClipboardKind::Clipboard => 0,
        ClipboardKind::Primary => 1,
    }
}

fn failed(e: arboard::Error) -> ClipError {
    ClipError::Failed(e.to_string())
}

#[cfg(any(target_os = "linux", target_os = "freebsd", target_os = "dragonfly", target_os = "netbsd", target_os = "openbsd"))]
fn linux_kind(k: ClipboardKind) -> Result<arboard::LinuxClipboardKind, ClipError> {
    Ok(match k {
        ClipboardKind::Clipboard => arboard::LinuxClipboardKind::Clipboard,
        ClipboardKind::Primary => arboard::LinuxClipboardKind::Primary,
    })
}

impl SystemClipboard {
    pub fn new() -> SystemClipboard {
        SystemClipboard::default()
    }

    fn with<R>(&self, f: impl FnOnce(&mut arboard::Clipboard) -> Result<R, ClipError>) -> Result<R, ClipError> {
        let mut g = lock(&self.cb);
        if g.is_none() {
            *g = Some(arboard::Clipboard::new().map_err(|e| ClipError::Unsupported(e.to_string()))?);
        }
        f(g.as_mut().expect("created above"))
    }

    /// The system's text (`""` when the clipboard holds no text).
    fn system_text(&self, k: ClipboardKind) -> Result<String, ClipError> {
        self.with(|cb| {
            #[cfg(any(target_os = "linux", target_os = "freebsd", target_os = "dragonfly", target_os = "netbsd", target_os = "openbsd"))]
            let r = {
                use arboard::GetExtLinux;
                cb.get().clipboard(linux_kind(k)?).text()
            };
            #[cfg(not(any(target_os = "linux", target_os = "freebsd", target_os = "dragonfly", target_os = "netbsd", target_os = "openbsd")))]
            let r = match k {
                ClipboardKind::Clipboard => cb.get_text(),
                ClipboardKind::Primary => return Err(ClipError::Unsupported("no primary selection on this platform".into())),
            };
            match r {
                Ok(t) => Ok(t),
                Err(arboard::Error::ContentNotAvailable) => Ok(String::new()),
                Err(e) => Err(failed(e)),
            }
        })
    }
}

impl ClipboardBackend for SystemClipboard {
    fn read(&self, k: ClipboardKind, accept: &[&str]) -> Result<MimeData, ClipError> {
        let text = self.system_text(k)?;
        if let Some((t, data)) = &lock(&self.written)[slot(k)]
            && *t == text
        {
            return Ok(data.filter(accept));
        }
        let mut data = MimeData::new();
        if !text.is_empty() {
            data.set(TEXT_PLAIN, text);
        }
        let wants_html = accept.is_empty() || MimeData::single(TEXT_HTML, "").best(accept).is_some();
        if k == ClipboardKind::Clipboard && wants_html {
            // Not every platform reads HTML back; the text is enough then.
            if let Ok(html) = self.with(|cb| cb.get().html().map_err(failed)) {
                data.set(TEXT_HTML, html);
            }
        }
        Ok(data.filter(accept))
    }

    fn write(&self, k: ClipboardKind, data: MimeData) -> Result<(), ClipError> {
        let text = data
            .plain_text()
            .or_else(|| data.types().find(|t| is_text_type(t)).and_then(|t| data.get_str(t)))
            .unwrap_or("")
            .to_string();
        let html = data.get_str(TEXT_HTML).map(str::to_string);
        self.with(|cb| {
            #[cfg(any(target_os = "linux", target_os = "freebsd", target_os = "dragonfly", target_os = "netbsd", target_os = "openbsd"))]
            let r = {
                use arboard::SetExtLinux;
                match (&html, k) {
                    (Some(h), ClipboardKind::Clipboard) => cb.set().clipboard(linux_kind(k)?).html(h.as_str(), Some(text.as_str())),
                    _ => cb.set().clipboard(linux_kind(k)?).text(text.as_str()),
                }
            };
            #[cfg(not(any(target_os = "linux", target_os = "freebsd", target_os = "dragonfly", target_os = "netbsd", target_os = "openbsd")))]
            let r = match (&html, k) {
                (_, ClipboardKind::Primary) => return Err(ClipError::Unsupported("no primary selection on this platform".into())),
                (Some(h), _) => cb.set().html(h.as_str(), Some(text.as_str())),
                (None, _) => cb.set().text(text.as_str()),
            };
            r.map_err(failed)
        })?;
        lock(&self.written)[slot(k)] = Some((text, data));
        Ok(())
    }
}

/// SV75 (W5's helper): a [`SystemClipboard`] whose [`ClipboardKind::Primary`] works, the X11 or
/// Wayland primary selection. `None` outside Linux and the BSDs, and where there is no display.
pub fn system_primary() -> Option<SystemClipboard> {
    if !PRIMARY_SUPPORTED {
        return None;
    }
    let system = SystemClipboard::new();
    system.formats(ClipboardKind::Primary).ok()?;
    Some(system)
}

/// True where the system has a primary selection (Linux and the BSDs: X11 and Wayland).
pub const PRIMARY_SUPPORTED: bool =
    cfg!(any(target_os = "linux", target_os = "freebsd", target_os = "dragonfly", target_os = "netbsd", target_os = "openbsd"));
