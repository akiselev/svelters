//! The general pasteboard as presto's [`Clipboard`] (plain text).

use crate::shim::Clipboard;
use objc2::MainThreadMarker;
use objc2_app_kit::{NSPasteboard, NSPasteboardTypeString};
use objc2_foundation::NSString;

pub(crate) struct MacClipboard {
    _mtm: MainThreadMarker,
}

impl MacClipboard {
    pub(crate) fn new(mtm: MainThreadMarker) -> Self {
        MacClipboard { _mtm: mtm }
    }
}

impl Clipboard for MacClipboard {
    fn get_text(&mut self) -> Option<String> {
        let pb = NSPasteboard::generalPasteboard();
        // SAFETY: NSPasteboardTypeString is a valid pasteboard type.
        let s = unsafe { pb.stringForType(NSPasteboardTypeString) }?;
        Some(s.to_string())
    }

    fn set_text(&mut self, text: &str) {
        let pb = NSPasteboard::generalPasteboard();
        pb.clearContents();
        // SAFETY: as above.
        unsafe { pb.setString_forType(&NSString::from_str(text), NSPasteboardTypeString) };
    }
}
