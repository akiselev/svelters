//! The keyboard's xkb state: the compositor's keymap, modifier state, keysyms, text and dead-key compose,
//! through libxkbcommon loaded at runtime (`xkbcommon-dl`, as winit does) so presto needs no link-time
//! dependency on it.
//!
//! On Wayland the compositor owns the modifier state and sends it with `wl_keyboard.modifiers`; the client
//! never feeds key presses into `xkb_state` (`xkb_state_update_mask` only).

use presto_base::event::Modifiers;
use std::ffi::CString;
use std::os::fd::OwnedFd;
use std::os::raw::c_char;
use std::os::unix::fs::FileExt;
use std::ptr::NonNull;
use xkbcommon_dl::{
    XkbCommon, XkbCommonCompose, xkb_compose_compile_flags, xkb_compose_feed_result, xkb_compose_state,
    xkb_compose_state_flags, xkb_compose_status, xkb_compose_table, xkb_context, xkb_context_flags, xkb_keymap,
    xkb_keymap_compile_flags, xkb_keymap_format, xkb_state, xkb_state_component,
};

use crate::keymap::XKB_EVDEV_OFFSET;

/// What one key press means under the current state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyMeaning {
    pub keysym: u32,
    /// Text it types, after compose; `None` for dead keys, compose in progress and control characters.
    pub text: Option<String>,
    /// A dead key or compose sequence is in progress (the key is swallowed).
    pub composing: bool,
}

pub struct Xkb {
    lib: &'static XkbCommon,
    context: NonNull<xkb_context>,
    keymap: Option<NonNull<xkb_keymap>>,
    state: Option<NonNull<xkb_state>>,
    compose: Option<Compose>,
}

struct Compose {
    lib: &'static XkbCommonCompose,
    table: NonNull<xkb_compose_table>,
    state: NonNull<xkb_compose_state>,
}

impl Xkb {
    /// `None` when libxkbcommon is not installed.
    pub fn new() -> Option<Self> {
        let lib = xkbcommon_dl::xkbcommon_option()?;
        // SAFETY: plain constructor; null on failure, checked.
        let context = NonNull::new(unsafe { (lib.xkb_context_new)(xkb_context_flags::XKB_CONTEXT_NO_FLAGS) })?;
        let compose = Compose::new(context);
        Some(Xkb { lib, context, keymap: None, state: None, compose })
    }

    /// Loads the keymap the compositor sent (`wl_keyboard.keymap`, format `xkb_v1`). The fd may be shared with
    /// other clients, so it is read with `pread` and never seeks.
    pub fn set_keymap(&mut self, fd: OwnedFd, size: u32) -> std::io::Result<()> {
        let file = std::fs::File::from(fd);
        let mut buf = vec![0u8; size as usize];
        file.read_exact_at(&mut buf, 0)?;
        // The keymap string is NUL-terminated inside `size`; make sure it is.
        if let Some(nul) = buf.iter().position(|&b| b == 0) {
            buf.truncate(nul);
        }
        let text = CString::new(buf).map_err(|_| std::io::Error::other("keymap has an interior NUL"))?;
        // SAFETY: `context` is live; `text` is a valid C string for the call's duration.
        let keymap = unsafe {
            (self.lib.xkb_keymap_new_from_string)(
                self.context.as_ptr(),
                text.as_ptr(),
                xkb_keymap_format::XKB_KEYMAP_FORMAT_TEXT_V1,
                xkb_keymap_compile_flags::XKB_KEYMAP_COMPILE_NO_FLAGS,
            )
        };
        let keymap = NonNull::new(keymap).ok_or_else(|| std::io::Error::other("xkb could not compile the keymap"))?;
        // SAFETY: `keymap` is live; null on failure, checked.
        let Some(state) = NonNull::new(unsafe { (self.lib.xkb_state_new)(keymap.as_ptr()) }) else {
            // SAFETY: we own the only reference.
            unsafe { (self.lib.xkb_keymap_unref)(keymap.as_ptr()) };
            return Err(std::io::Error::other("xkb could not create a state"));
        };
        self.release_keymap();
        self.keymap = Some(keymap);
        self.state = Some(state);
        Ok(())
    }

    fn release_keymap(&mut self) {
        // SAFETY: we own these references; they are dropped from `self` before use can recur.
        unsafe {
            if let Some(s) = self.state.take() {
                (self.lib.xkb_state_unref)(s.as_ptr());
            }
            if let Some(k) = self.keymap.take() {
                (self.lib.xkb_keymap_unref)(k.as_ptr());
            }
        }
    }

    pub fn has_keymap(&self) -> bool {
        self.state.is_some()
    }

    /// `wl_keyboard.modifiers`.
    pub fn update_mask(&mut self, depressed: u32, latched: u32, locked: u32, group: u32) {
        if let Some(s) = self.state {
            // SAFETY: `s` is live.
            unsafe { (self.lib.xkb_state_update_mask)(s.as_ptr(), depressed, latched, locked, 0, 0, group) };
        }
    }

    /// The effective modifiers. AltGr is xkb's level-3 shift, which keymaps bind to `Mod5`.
    pub fn modifiers(&self) -> Modifiers {
        let Some(s) = self.state else { return Modifiers::default() };
        let active = |name: &[u8]| {
            // SAFETY: `s` is live; `name` is a NUL-terminated literal.
            unsafe {
                (self.lib.xkb_state_mod_name_is_active)(
                    s.as_ptr(),
                    name.as_ptr() as *const c_char,
                    xkb_state_component::XKB_STATE_MODS_EFFECTIVE,
                ) > 0
            }
        };
        let mut m = Modifiers::default();
        if active(xkbcommon_dl::XKB_MOD_NAME_SHIFT) {
            m = m.union(Modifiers::SHIFT);
        }
        if active(xkbcommon_dl::XKB_MOD_NAME_CTRL) {
            m = m.union(Modifiers::CTRL);
        }
        if active(xkbcommon_dl::XKB_MOD_NAME_ALT) {
            m = m.union(Modifiers::ALT);
        }
        if active(xkbcommon_dl::XKB_MOD_NAME_LOGO) {
            m = m.union(Modifiers::SUPER);
        }
        if active(b"Mod5\0") {
            m = m.union(Modifiers::ALT_GR);
        }
        m
    }

    /// Whether the keymap marks the key as repeating.
    pub fn repeats(&self, evdev: u32) -> bool {
        let Some(k) = self.keymap else { return false };
        // SAFETY: `k` is live.
        unsafe { (self.lib.xkb_keymap_key_repeats)(k.as_ptr(), evdev + XKB_EVDEV_OFFSET) != 0 }
    }

    /// The meaning of pressing `evdev` now. Feeds the compose state, so call it once per press (and per
    /// repeat).
    pub fn press(&mut self, evdev: u32) -> Option<KeyMeaning> {
        let s = self.state?;
        let code = evdev + XKB_EVDEV_OFFSET;
        // SAFETY: `s` is live.
        let keysym = unsafe { (self.lib.xkb_state_key_get_one_sym)(s.as_ptr(), code) };
        if let Some(c) = &mut self.compose {
            match c.feed(keysym) {
                ComposeResult::Composing => return Some(KeyMeaning { keysym, text: None, composing: true }),
                ComposeResult::Composed { keysym: composed, text } => {
                    return Some(KeyMeaning { keysym: composed.unwrap_or(keysym), text: filter_text(text), composing: false });
                }
                ComposeResult::Cancelled => return Some(KeyMeaning { keysym, text: None, composing: false }),
                ComposeResult::Nothing => {}
            }
        }
        Some(KeyMeaning { keysym, text: filter_text(self.utf8(s, code)), composing: false })
    }

    /// The keysym of `evdev` without feeding compose (releases).
    pub fn keysym(&self, evdev: u32) -> u32 {
        let Some(s) = self.state else { return 0 };
        // SAFETY: `s` is live.
        unsafe { (self.lib.xkb_state_key_get_one_sym)(s.as_ptr(), evdev + XKB_EVDEV_OFFSET) }
    }

    /// Drops a pending dead key (focus left, the IME took over).
    pub fn reset_compose(&mut self) {
        if let Some(c) = &mut self.compose {
            c.reset();
        }
    }

    fn utf8(&self, s: NonNull<xkb_state>, code: u32) -> Option<String> {
        let mut buf = [0u8; 64];
        // SAFETY: `s` is live; the buffer and its length match. Returns the length without the NUL.
        let n = unsafe { (self.lib.xkb_state_key_get_utf8)(s.as_ptr(), code, buf.as_mut_ptr() as *mut c_char, buf.len()) };
        if n <= 0 || n as usize >= buf.len() {
            return None;
        }
        String::from_utf8(buf[..n as usize].to_vec()).ok()
    }
}

impl Drop for Xkb {
    fn drop(&mut self) {
        self.compose = None;
        self.release_keymap();
        // SAFETY: we own the context reference.
        unsafe { (self.lib.xkb_context_unref)(self.context.as_ptr()) };
    }
}

enum ComposeResult {
    Nothing,
    Composing,
    Composed { keysym: Option<u32>, text: Option<String> },
    Cancelled,
}

impl Compose {
    fn new(context: NonNull<xkb_context>) -> Option<Self> {
        let lib = xkbcommon_dl::xkbcommon_compose_option()?;
        let locale = compose_locale();
        let locale = CString::new(locale).ok()?;
        // SAFETY: `context` is live; `locale` a valid C string. Null when the locale has no compose table.
        let table = NonNull::new(unsafe {
            (lib.xkb_compose_table_new_from_locale)(
                context.as_ptr(),
                locale.as_ptr(),
                xkb_compose_compile_flags::XKB_COMPOSE_COMPILE_NO_FLAGS,
            )
        })?;
        // SAFETY: `table` is live.
        let Some(state) =
            NonNull::new(unsafe { (lib.xkb_compose_state_new)(table.as_ptr(), xkb_compose_state_flags::XKB_COMPOSE_STATE_NO_FLAGS) })
        else {
            // SAFETY: we own the table reference.
            unsafe { (lib.xkb_compose_table_unref)(table.as_ptr()) };
            return None;
        };
        Some(Compose { lib, table, state })
    }

    fn feed(&mut self, keysym: u32) -> ComposeResult {
        let st = self.state.as_ptr();
        // SAFETY: `st` is live for all calls below.
        unsafe {
            if (self.lib.xkb_compose_state_feed)(st, keysym) == xkb_compose_feed_result::XKB_COMPOSE_FEED_IGNORED {
                return ComposeResult::Nothing;
            }
            match (self.lib.xkb_compose_state_get_status)(st) {
                xkb_compose_status::XKB_COMPOSE_NOTHING => ComposeResult::Nothing,
                xkb_compose_status::XKB_COMPOSE_COMPOSING => ComposeResult::Composing,
                xkb_compose_status::XKB_COMPOSE_CANCELLED => {
                    (self.lib.xkb_compose_state_reset)(st);
                    ComposeResult::Cancelled
                }
                xkb_compose_status::XKB_COMPOSE_COMPOSED => {
                    let mut buf = [0u8; 64];
                    let n = (self.lib.xkb_compose_state_get_utf8)(st, buf.as_mut_ptr() as *mut c_char, buf.len());
                    let text = (n > 0 && (n as usize) < buf.len())
                        .then(|| String::from_utf8(buf[..n as usize].to_vec()).ok())
                        .flatten();
                    let sym = (self.lib.xkb_compose_state_get_one_sym)(st);
                    (self.lib.xkb_compose_state_reset)(st);
                    ComposeResult::Composed { keysym: (sym != 0).then_some(sym), text }
                }
            }
        }
    }

    fn reset(&mut self) {
        // SAFETY: the state is live.
        unsafe { (self.lib.xkb_compose_state_reset)(self.state.as_ptr()) };
    }
}

impl Drop for Compose {
    fn drop(&mut self) {
        // SAFETY: we own both references.
        unsafe {
            (self.lib.xkb_compose_state_unref)(self.state.as_ptr());
            (self.lib.xkb_compose_table_unref)(self.table.as_ptr());
        }
    }
}

/// The locale compose tables are chosen by, as libxkbcommon documents: `LC_ALL`, then `LC_CTYPE`, then `LANG`,
/// else `C`.
fn compose_locale() -> String {
    ["LC_ALL", "LC_CTYPE", "LANG"]
        .iter()
        .filter_map(|v| std::env::var(v).ok())
        .find(|v| !v.is_empty())
        .unwrap_or_else(|| "C".to_owned())
}

/// Text a key types: control characters (Ctrl+C gives U+0003, Backspace U+0008, Delete U+007F) are commands,
/// not text.
pub fn filter_text(text: Option<String>) -> Option<String> {
    text.filter(|t| !t.is_empty() && !t.chars().any(|c| c.is_control()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_characters_are_not_text() {
        assert_eq!(filter_text(Some("\u{3}".into())), None);
        assert_eq!(filter_text(Some("\u{7f}".into())), None);
        assert_eq!(filter_text(Some("\r".into())), None);
        assert_eq!(filter_text(Some(String::new())), None);
        assert_eq!(filter_text(Some("é".into())), Some("é".into()));
        assert_eq!(filter_text(None), None);
    }
}
