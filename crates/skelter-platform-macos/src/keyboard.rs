//! The current keyboard layout's characters per key (`TISCopyCurrentKeyboardLayoutInputSource` and
//! `UCKeyTranslate`), for naming key presses on any layout ([`keymap::key_chars`]).
//!
//! The approach follows GPUI's `chars_for_modified_key`, `crates/gpui_macos/src/events.rs` of
//! zed-industries/zed at a5c4ca8 (Apache-2.0). Modifications by the svelters authors.

use crate::keymap::{self, LayoutChars};
use std::ffi::c_void;

type CFTypeRef = *const c_void;

#[link(name = "Carbon", kind = "framework")]
unsafe extern "C" {
    fn TISCopyCurrentKeyboardLayoutInputSource() -> CFTypeRef;
    fn TISGetInputSourceProperty(source: CFTypeRef, key: CFTypeRef) -> CFTypeRef;
    static kTISPropertyUnicodeKeyLayoutData: CFTypeRef;
    fn LMGetKbdType() -> u8;
    fn UCKeyTranslate(
        layout: *const c_void,
        virtual_key_code: u16,
        key_action: u16,
        modifier_key_state: u32,
        keyboard_type: u32,
        key_translate_options: u32,
        dead_key_state: *mut u32,
        max_string_length: usize,
        actual_string_length: *mut usize,
        unicode_string: *mut u16,
    ) -> i32;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFDataGetBytePtr(data: CFTypeRef) -> *const u8;
    fn CFRelease(cf: CFTypeRef);
}

/// `UCKeyTranslate` modifier states (Carbon `EventModifiers >> 8`).
pub mod uc {
    pub const CMD: u32 = 1;
    pub const SHIFT: u32 = 2;
}

const UC_KEY_ACTION_DOWN: u16 = 0;
const UC_NO_DEAD_KEYS: u32 = 1;

/// The current layout's `uchr` data, held for a batch of translations. Main thread.
pub struct Layout {
    source: CFTypeRef,
    data: *const u8,
    keyboard_type: u32,
}

impl Layout {
    pub fn current() -> Option<Layout> {
        // SAFETY: TIS calls on the main thread; the copied source is released in Drop, and the layout
        // data it owns lives as long as the source.
        unsafe {
            let source = TISCopyCurrentKeyboardLayoutInputSource();
            if source.is_null() {
                return None;
            }
            let data = TISGetInputSourceProperty(source, kTISPropertyUnicodeKeyLayoutData);
            if data.is_null() {
                CFRelease(source);
                return None;
            }
            Some(Layout {
                source,
                data: CFDataGetBytePtr(data),
                keyboard_type: LMGetKbdType() as u32,
            })
        }
    }

    /// What `code` types under `mods` (`uc::*`), dead keys giving their spacing character.
    pub fn translate(&self, code: u16, mods: u32) -> String {
        let mut dead = 0u32;
        let mut buf = [0u16; 8];
        let mut len = 0usize;
        // SAFETY: `data` is the layout's uchr data (alive while `source` is); the buffers are valid for
        // their stated lengths.
        let status = unsafe {
            UCKeyTranslate(
                self.data.cast(),
                code,
                UC_KEY_ACTION_DOWN,
                mods,
                self.keyboard_type,
                UC_NO_DEAD_KEYS,
                &mut dead,
                buf.len(),
                &mut len,
                buf.as_mut_ptr(),
            )
        };
        if status != 0 {
            return String::new();
        }
        String::from_utf16_lossy(&buf[..len.min(buf.len())])
    }

    /// The four layers [`keymap::key_chars`] chooses from, and whether the layout names keys from its
    /// Cmd layer.
    pub fn chars(&self, code: u16) -> (LayoutChars, bool) {
        let chars = LayoutChars {
            plain: self.translate(code, 0),
            shift: self.translate(code, uc::SHIFT),
            cmd: self.translate(code, uc::CMD),
            cmd_shift: self.translate(code, uc::CMD | uc::SHIFT),
        };
        let always = keymap::always_cmd_layout(
            &self.translate(keymap::vk::ANSI_A, 0),
            &self.translate(keymap::vk::ANSI_A, uc::CMD),
        );
        (chars, always)
    }
}

impl Drop for Layout {
    fn drop(&mut self) {
        // SAFETY: `source` came from a TIS copy function.
        unsafe { CFRelease(self.source) };
    }
}
