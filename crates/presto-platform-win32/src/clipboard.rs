//! The clipboard: Unicode text and Explorer's file lists (`CF_HDROP`).

use crate::shim::{Clipboard, PlatformError};
use std::path::PathBuf;
use windows::Win32::Foundation::{HANDLE, HGLOBAL, HWND};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock};
use windows::Win32::System::Ole::{CF_HDROP, CF_UNICODETEXT};
use windows::Win32::UI::Shell::{DragQueryFileW, HDROP};

/// The clipboard, opened on behalf of the app's hidden message window.
pub struct Win32Clipboard {
    pub(crate) owner: HWND,
}

/// Holds the clipboard open; closes it on drop.
struct Open;

impl Open {
    /// Another process may hold the clipboard for a moment (clipboard managers, RDP): retry briefly.
    fn new(owner: HWND) -> Result<Self, PlatformError> {
        for attempt in 0..5 {
            if unsafe { OpenClipboard(Some(owner)) }.is_ok() {
                return Ok(Open);
            }
            if attempt < 4 {
                std::thread::sleep(std::time::Duration::from_millis(2));
            }
        }
        Err(PlatformError::ClipboardBusy)
    }
}

impl Drop for Open {
    fn drop(&mut self) {
        let _ = unsafe { CloseClipboard() };
    }
}

/// Decodes NUL-terminated UTF-16 that may lack its terminator.
pub fn utf16_until_nul(units: &[u16]) -> String {
    let end = units.iter().position(|&u| u == 0).unwrap_or(units.len());
    String::from_utf16_lossy(&units[..end])
}

/// `text` as NUL-terminated UTF-16 with `\n` turned into the `\r\n` other Windows apps expect
/// (existing `\r\n` pairs stay as they are).
pub fn clipboard_utf16(text: &str) -> Vec<u16> {
    let mut out = Vec::with_capacity(text.len() + 1);
    let mut prev_cr = false;
    for u in text.encode_utf16() {
        if u == '\n' as u16 && !prev_cr {
            out.push('\r' as u16);
        }
        prev_cr = u == '\r' as u16;
        out.push(u);
    }
    out.push(0);
    out
}

impl Clipboard for Win32Clipboard {
    fn get_text(&mut self) -> Result<Option<String>, PlatformError> {
        if unsafe { IsClipboardFormatAvailable(CF_UNICODETEXT.0 as u32) }.is_err() {
            return Ok(None);
        }
        let _open = Open::new(self.owner)?;
        let handle = match unsafe { GetClipboardData(CF_UNICODETEXT.0 as u32) } {
            Ok(h) => h,
            Err(_) => return Ok(None),
        };
        let mem = HGLOBAL(handle.0);
        let ptr = unsafe { GlobalLock(mem) } as *const u16;
        if ptr.is_null() {
            return Ok(None);
        }
        let units = unsafe { std::slice::from_raw_parts(ptr, GlobalSize(mem) / 2) };
        let text = utf16_until_nul(units);
        let _ = unsafe { GlobalUnlock(mem) };
        Ok(Some(text))
    }

    fn set_text(&mut self, text: &str) -> Result<(), PlatformError> {
        let units = clipboard_utf16(text);
        let _open = Open::new(self.owner)?;
        unsafe { EmptyClipboard() }.map_err(|e| PlatformError::win("EmptyClipboard", e))?;
        let mem = unsafe { GlobalAlloc(GMEM_MOVEABLE, units.len() * 2) }
            .map_err(|e| PlatformError::win("GlobalAlloc", e))?;
        let ptr = unsafe { GlobalLock(mem) } as *mut u16;
        if ptr.is_null() {
            return Err(PlatformError::Os { call: "GlobalLock", code: 0 });
        }
        unsafe {
            std::ptr::copy_nonoverlapping(units.as_ptr(), ptr, units.len());
            let _ = GlobalUnlock(mem);
        }
        // On success the clipboard owns the memory.
        unsafe { SetClipboardData(CF_UNICODETEXT.0 as u32, Some(HANDLE(mem.0))) }
            .map_err(|e| PlatformError::win("SetClipboardData", e))?;
        Ok(())
    }

    fn get_files(&mut self) -> Result<Vec<PathBuf>, PlatformError> {
        if unsafe { IsClipboardFormatAvailable(CF_HDROP.0 as u32) }.is_err() {
            return Ok(Vec::new());
        }
        let _open = Open::new(self.owner)?;
        let handle = match unsafe { GetClipboardData(CF_HDROP.0 as u32) } {
            Ok(h) => h,
            Err(_) => return Ok(Vec::new()),
        };
        Ok(hdrop_paths(HDROP(handle.0)))
    }
}

/// The paths in an `HDROP` (clipboard file lists, OLE drops).
pub fn hdrop_paths(drop: HDROP) -> Vec<PathBuf> {
    let count = unsafe { DragQueryFileW(drop, u32::MAX, None) };
    (0..count)
        .filter_map(|i| {
            let len = unsafe { DragQueryFileW(drop, i, None) } as usize;
            let mut buf = vec![0u16; len + 1];
            let got = unsafe { DragQueryFileW(drop, i, Some(&mut buf)) } as usize;
            (got > 0).then(|| PathBuf::from(utf16_until_nul(&buf[..got])))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newlines_become_crlf_once() {
        let s = String::from_utf16(&clipboard_utf16("a\nb\r\nc")).unwrap();
        assert_eq!(s, "a\r\nb\r\nc\0");
    }

    #[test]
    fn nul_terminated_decoding() {
        assert_eq!(utf16_until_nul(&[104, 105, 0, 120]), "hi");
        assert_eq!(utf16_until_nul(&[104, 105]), "hi");
    }
}
