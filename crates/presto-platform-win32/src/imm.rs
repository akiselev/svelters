//! `WM_IME_REQUEST`, which IMM32 TIPs send even when TSF is on: ATOK and Mozc implement reverse
//! conversion (再変換) through `IMR_RECONVERTSTRING` (Chromium's `input_method_win_tsf.cc`), MS-IME
//! reads context through `IMR_DOCUMENTFEED`, and `IMR_QUERYCHARPOSITION` places candidate windows
//! per character. Answers come from the focused field's [`TextInputClient`] (UTF-16 offsets).

use presto_base::geom::Rect;
use presto_base::input::{TextInputClient, Utf16Range};
use windows::Win32::Foundation::{POINT, RECT};
use windows::Win32::UI::Input::Ime::{
    IMECHARPOSITION, IMR_DOCUMENTFEED, IMR_QUERYCHARPOSITION, IMR_RECONVERTSTRING, RECONVERTSTRING,
};

/// How much text around the selection a reconversion string carries, each side (UTF-16 units).
pub const CONTEXT: usize = 1024;

/// A `RECONVERTSTRING` block: the header followed by `text` (and a NUL), with the composition and
/// target ranges relative to `text`. Offsets in the header are bytes from the string's start,
/// lengths are UTF-16 units.
pub fn reconvert_block(text: &[u16], comp: &Utf16Range, target: &Utf16Range) -> Vec<u8> {
    let header = std::mem::size_of::<RECONVERTSTRING>();
    let size = header + (text.len() + 1) * 2;
    let rs = RECONVERTSTRING {
        dwSize: size as u32,
        dwVersion: 0,
        dwStrLen: text.len() as u32,
        dwStrOffset: header as u32,
        dwCompStrLen: (comp.end - comp.start) as u32,
        dwCompStrOffset: (comp.start * 2) as u32,
        dwTargetStrLen: (target.end - target.start) as u32,
        dwTargetStrOffset: (target.start * 2) as u32,
    };
    let mut out = Vec::with_capacity(size);
    for v in [
        rs.dwSize,
        rs.dwVersion,
        rs.dwStrLen,
        rs.dwStrOffset,
        rs.dwCompStrLen,
        rs.dwCompStrOffset,
        rs.dwTargetStrLen,
        rs.dwTargetStrOffset,
    ] {
        out.extend_from_slice(&v.to_le_bytes());
    }
    for u in text.iter().copied().chain(std::iter::once(0)) {
        out.extend_from_slice(&u.to_le_bytes());
    }
    out
}

/// The text window around `focus` (document offsets) and `focus` relative to it.
pub fn context_window(focus: &Utf16Range, doc_len: usize) -> (Utf16Range, Utf16Range) {
    let start = focus.start.saturating_sub(CONTEXT);
    let end = (focus.end + CONTEXT).min(doc_len);
    (start..end, focus.start - start..focus.end - start)
}

/// The `RECONVERTSTRING` for a request, if the field has something to give: reconversion of a
/// non-empty selection outside a composition, or the document around the composition (or caret)
/// for `IMR_DOCUMENTFEED`.
pub fn reconvert_for(request: u32, c: &dyn TextInputClient) -> Option<Vec<u8>> {
    let sel = c.selected_range()?;
    let marked = c.marked_range();
    let focus = match request {
        IMR_RECONVERTSTRING if marked.is_none() && !sel.is_empty() => sel,
        IMR_DOCUMENTFEED => marked.unwrap_or(sel),
        _ => return None,
    };
    let doc_len = c.text_for_range(0..usize::MAX).map(|s| s.encode_utf16().count()).unwrap_or(0);
    let (window, rel) = context_window(&focus, doc_len);
    let text: Vec<u16> = c.text_for_range(window)?.encode_utf16().collect();
    let rel = rel.start.min(text.len())..rel.end.min(text.len());
    Some(reconvert_block(&text, &rel, &rel))
}

/// Answers a `WM_IME_REQUEST`. `to_screen` maps window points to screen pixels; `field` is the field
/// in window points (the document rectangle). The return value is the message's result.
///
/// # Safety
/// `lparam` must be what the OS passed with the message.
pub unsafe fn handle_request(
    request: u32,
    lparam: isize,
    c: &dyn TextInputClient,
    to_screen: impl Fn(Rect) -> RECT,
    field: Rect,
) -> isize {
    match request {
        IMR_RECONVERTSTRING | IMR_DOCUMENTFEED => {
            let Some(block) = reconvert_for(request, c) else { return 0 };
            if lparam == 0 {
                return block.len() as isize;
            }
            let dst = lparam as *mut RECONVERTSTRING;
            let room = unsafe { (*dst).dwSize } as usize;
            if room < block.len() {
                return 0;
            }
            unsafe { std::ptr::copy_nonoverlapping(block.as_ptr(), dst as *mut u8, block.len()) };
            block.len() as isize
        }
        IMR_QUERYCHARPOSITION => {
            if lparam == 0 {
                return 0;
            }
            let pos = unsafe { &mut *(lparam as *mut IMECHARPOSITION) };
            let base = c.marked_range().or_else(|| c.selected_range()).map(|r| r.start).unwrap_or(0);
            let at = base + pos.dwCharPos as usize;
            let Some(bounds) = c.bounds_for_range(at..at + 1).or_else(|| c.bounds_for_range(at..at)) else {
                return 0;
            };
            let r = to_screen(bounds);
            pos.pt = POINT { x: r.left, y: r.top };
            pos.cLineHeight = (r.bottom - r.top).max(1) as u32;
            pos.rcDocument = to_screen(field);
            1
        }
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u32_at(b: &[u8], i: usize) -> u32 {
        u32::from_le_bytes(b[i * 4..i * 4 + 4].try_into().unwrap())
    }

    #[test]
    fn reconvert_block_layout() {
        let text: Vec<u16> = "日本語を入力".encode_utf16().collect();
        let b = reconvert_block(&text, &(2..4), &(2..4));
        assert_eq!(u32_at(&b, 0) as usize, b.len());
        assert_eq!(u32_at(&b, 2), 6, "dwStrLen");
        assert_eq!(u32_at(&b, 3), 32, "dwStrOffset is the header size");
        assert_eq!(u32_at(&b, 4), 2, "dwCompStrLen");
        assert_eq!(u32_at(&b, 5), 4, "dwCompStrOffset in bytes");
        assert_eq!(u16::from_le_bytes([b[32], b[33]]), '日' as u16);
        assert_eq!(&b[b.len() - 2..], &[0, 0], "NUL-terminated");
    }

    #[test]
    fn context_windows_clamp() {
        assert_eq!(context_window(&(10..12), 20), (0..20, 10..12));
        let (w, rel) = context_window(&(5000..5004), 10_000);
        assert_eq!(w, 5000 - CONTEXT..5004 + CONTEXT);
        assert_eq!(rel, CONTEXT..CONTEXT + 4);
    }
}
