//! The general pasteboard as presto's [`Clipboard`] (plain text; macOS has no primary selection).

use objc2::MainThreadMarker;
use objc2_app_kit::{NSPasteboard, NSPasteboardTypeString};
use objc2_foundation::NSString;
use presto_platform::{Clipboard, ClipboardKind};

pub(crate) struct MacClipboard {
    _mtm: MainThreadMarker,
}

impl MacClipboard {
    pub(crate) fn new(mtm: MainThreadMarker) -> Self {
        MacClipboard { _mtm: mtm }
    }
}

impl Clipboard for MacClipboard {
    fn get_text(&mut self, kind: ClipboardKind) -> Option<String> {
        if kind != ClipboardKind::Clipboard {
            return None;
        }
        let pb = NSPasteboard::generalPasteboard();
        // SAFETY: NSPasteboardTypeString is a valid pasteboard type.
        let s = unsafe { pb.stringForType(NSPasteboardTypeString) }?;
        Some(s.to_string())
    }

    /// macOS has no primary selection.
    fn set_text(&mut self, kind: ClipboardKind, text: &str) -> bool {
        if kind != ClipboardKind::Clipboard {
            return false;
        }
        let pb = NSPasteboard::generalPasteboard();
        pb.clearContents();
        // SAFETY: as above.
        unsafe { pb.setString_forType(&NSString::from_str(text), NSPasteboardTypeString) }
    }
}
