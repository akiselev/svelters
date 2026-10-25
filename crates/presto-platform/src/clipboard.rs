//! Clipboard and primary selection. winit 0.30 has neither: arboard serves Windows, macOS and X11;
//! smithay-clipboard serves Wayland, bound to winit's own `wl_display` (it needs the compositor's
//! serials, which only that connection sees).

/// The two selections; `Primary` exists only on Linux desktops.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ClipboardKind {
    Clipboard,
    Primary,
}

pub trait Clipboard {
    fn get_text(&mut self, kind: ClipboardKind) -> Option<String>;
    /// `false` when the selection does not exist here or the OS refused.
    fn set_text(&mut self, kind: ClipboardKind, text: &str) -> bool;
    fn has_primary(&self) -> bool {
        false
    }
}

/// An in-process clipboard (tests, headless, and the fallback when the OS clipboard fails to open).
#[derive(Clone, Debug, Default)]
pub struct MemoryClipboard {
    clipboard: Option<String>,
    primary: Option<String>,
}

impl Clipboard for MemoryClipboard {
    fn get_text(&mut self, kind: ClipboardKind) -> Option<String> {
        match kind {
            ClipboardKind::Clipboard => self.clipboard.clone(),
            ClipboardKind::Primary => self.primary.clone(),
        }
    }
    fn set_text(&mut self, kind: ClipboardKind, text: &str) -> bool {
        match kind {
            ClipboardKind::Clipboard => self.clipboard = Some(text.to_owned()),
            ClipboardKind::Primary => self.primary = Some(text.to_owned()),
        }
        true
    }
    fn has_primary(&self) -> bool {
        true
    }
}

/// The OS clipboard.
pub struct SystemClipboard {
    inner: Inner,
}

enum Inner {
    #[cfg(any(
        target_os = "windows",
        target_os = "macos",
        all(unix, not(any(target_os = "ios", target_os = "android")))
    ))]
    Arboard(arboard::Clipboard),
    #[cfg(all(
        unix,
        not(any(target_os = "macos", target_os = "ios", target_os = "android"))
    ))]
    Wayland(smithay_clipboard::Clipboard),
    Memory(MemoryClipboard),
}

impl SystemClipboard {
    /// Opens the OS clipboard for the display winit connected to. Falls back to an in-process
    /// clipboard (logged) when the OS one cannot be opened.
    pub fn new(display: Option<raw_window_handle::RawDisplayHandle>) -> Self {
        #[cfg(all(
            unix,
            not(any(target_os = "macos", target_os = "ios", target_os = "android"))
        ))]
        if let Some(raw_window_handle::RawDisplayHandle::Wayland(h)) = display {
            // SAFETY: the display pointer comes from winit's live event loop, which outlives this
            // clipboard (the backend drops the clipboard before the loop exits).
            let c = unsafe { smithay_clipboard::Clipboard::new(h.display.as_ptr()) };
            return SystemClipboard {
                inner: Inner::Wayland(c),
            };
        }
        let _ = display;
        #[cfg(any(
            target_os = "windows",
            target_os = "macos",
            all(unix, not(any(target_os = "ios", target_os = "android")))
        ))]
        match arboard::Clipboard::new() {
            Ok(c) => {
                return SystemClipboard {
                    inner: Inner::Arboard(c),
                };
            }
            Err(e) => log::warn!(
                "presto-platform: the OS clipboard is unavailable ({e}); using an in-process one"
            ),
        }
        SystemClipboard {
            inner: Inner::Memory(MemoryClipboard::default()),
        }
    }
}

impl Clipboard for SystemClipboard {
    fn get_text(&mut self, kind: ClipboardKind) -> Option<String> {
        match &mut self.inner {
            #[cfg(any(
                target_os = "windows",
                target_os = "macos",
                all(unix, not(any(target_os = "ios", target_os = "android")))
            ))]
            Inner::Arboard(c) => match kind {
                ClipboardKind::Clipboard => c.get_text().ok(),
                #[cfg(all(
                    unix,
                    not(any(target_os = "macos", target_os = "ios", target_os = "android"))
                ))]
                ClipboardKind::Primary => {
                    use arboard::{GetExtLinux, LinuxClipboardKind};
                    c.get().clipboard(LinuxClipboardKind::Primary).text().ok()
                }
                #[cfg(not(all(
                    unix,
                    not(any(target_os = "macos", target_os = "ios", target_os = "android"))
                )))]
                ClipboardKind::Primary => None,
            },
            #[cfg(all(
                unix,
                not(any(target_os = "macos", target_os = "ios", target_os = "android"))
            ))]
            Inner::Wayland(c) => match kind {
                ClipboardKind::Clipboard => c.load().ok(),
                ClipboardKind::Primary => c.load_primary().ok(),
            },
            Inner::Memory(m) => m.get_text(kind),
        }
    }

    fn set_text(&mut self, kind: ClipboardKind, text: &str) -> bool {
        match &mut self.inner {
            #[cfg(any(
                target_os = "windows",
                target_os = "macos",
                all(unix, not(any(target_os = "ios", target_os = "android")))
            ))]
            Inner::Arboard(c) => match kind {
                ClipboardKind::Clipboard => c.set_text(text).is_ok(),
                #[cfg(all(
                    unix,
                    not(any(target_os = "macos", target_os = "ios", target_os = "android"))
                ))]
                ClipboardKind::Primary => {
                    use arboard::{LinuxClipboardKind, SetExtLinux};
                    c.set()
                        .clipboard(LinuxClipboardKind::Primary)
                        .text(text.to_owned())
                        .is_ok()
                }
                #[cfg(not(all(
                    unix,
                    not(any(target_os = "macos", target_os = "ios", target_os = "android"))
                )))]
                ClipboardKind::Primary => false,
            },
            #[cfg(all(
                unix,
                not(any(target_os = "macos", target_os = "ios", target_os = "android"))
            ))]
            Inner::Wayland(c) => {
                match kind {
                    ClipboardKind::Clipboard => c.store(text),
                    ClipboardKind::Primary => c.store_primary(text),
                }
                true
            }
            Inner::Memory(m) => m.set_text(kind, text),
        }
    }

    fn has_primary(&self) -> bool {
        match &self.inner {
            #[cfg(all(
                unix,
                not(any(target_os = "macos", target_os = "ios", target_os = "android"))
            ))]
            Inner::Wayland(_) | Inner::Arboard(_) => true,
            #[cfg(any(target_os = "windows", target_os = "macos"))]
            Inner::Arboard(_) => false,
            Inner::Memory(_) => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_clipboard_keeps_selections_apart() {
        let mut c = MemoryClipboard::default();
        assert!(c.set_text(ClipboardKind::Clipboard, "a"));
        assert!(c.set_text(ClipboardKind::Primary, "b"));
        assert_eq!(c.get_text(ClipboardKind::Clipboard).as_deref(), Some("a"));
        assert_eq!(c.get_text(ClipboardKind::Primary).as_deref(), Some("b"));
    }
}
