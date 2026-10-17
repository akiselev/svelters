//! GTK runtime-version compatibility (docs/notes/fg1-gtk-compat.md).
//!
//! One build of the backend links against GTK [`MIN`] and runs on any GTK at or above it. Kinds and
//! properties added after `MIN` carry their GIR `since` in the generated tables (`KindInfo::since`,
//! `PropInfo::since`); their `*_get_type` is looked up at run time ([`call_get_type`]) instead of
//! linked, and using them on an older GTK fails with a [`require`] error that names both versions.

use std::ffi::{c_void, CStr};

/// The oldest GTK the backend links and runs against. Keep in sync with `MIN_GTK` in
/// `tools/metadata/gtk/gen_backend.py`.
pub const MIN: (u16, u16) = (4, 14);

/// The running GTK version (major, minor, micro).
pub fn runtime() -> (u32, u32, u32) {
    unsafe { (gtk::ffi::gtk_get_major_version(), gtk::ffi::gtk_get_minor_version(), gtk::ffi::gtk_get_micro_version()) }
}

/// Whether the running GTK has a feature added in `since` ((0, 0): part of the minimum).
pub fn has(since: (u16, u16)) -> bool {
    let (a, b, _) = runtime();
    (a, b) >= (since.0 as u32, since.1 as u32)
}

/// `Err("<what> needs GTK x.y; the running GTK is a.b.c")` when the running GTK predates `since`.
pub fn require(since: (u16, u16), what: &str) -> Result<(), String> {
    if has(since) {
        return Ok(());
    }
    let (a, b, c) = runtime();
    Err(format!("{what} needs GTK {}.{}; the running GTK is {a}.{b}.{c}", since.0, since.1))
}

/// Calls the GTK function `sym` (a `*_get_type`) if the loaded GTK has it, without linking it.
/// Returns the GType, or 0 when the symbol is missing.
pub fn call_get_type(sym: &CStr) -> usize {
    let f = lookup(sym);
    if f.is_null() {
        return 0;
    }
    let f: unsafe extern "C" fn() -> usize = unsafe { std::mem::transmute(f) };
    unsafe { f() }
}

/// Looks `sym` up in the library that provides `gtk_get_major_version` (the GTK actually loaded,
/// even when this backend sits in a library loaded with RTLD_LOCAL), then in the global scope.
#[cfg(unix)]
fn lookup(sym: &CStr) -> *mut c_void {
    unsafe {
        let mut info: libc::Dl_info = std::mem::zeroed();
        let anchor = gtk::ffi::gtk_get_major_version as *const c_void;
        if libc::dladdr(anchor, &mut info) != 0 && !info.dli_fname.is_null() {
            let lib = libc::dlopen(info.dli_fname, libc::RTLD_LAZY | libc::RTLD_NOLOAD);
            if !lib.is_null() {
                let p = libc::dlsym(lib, sym.as_ptr());
                libc::dlclose(lib);
                if !p.is_null() {
                    return p;
                }
            }
        }
        libc::dlsym(libc::RTLD_DEFAULT, sym.as_ptr())
    }
}

#[cfg(windows)]
fn lookup(sym: &CStr) -> *mut c_void {
    unsafe extern "system" {
        fn GetModuleHandleA(name: *const std::ffi::c_char) -> *mut c_void;
        fn GetProcAddress(module: *mut c_void, name: *const std::ffi::c_char) -> *mut c_void;
    }
    unsafe {
        let m = GetModuleHandleA(c"gtk-4-1.dll".as_ptr());
        if m.is_null() {
            return std::ptr::null_mut();
        }
        GetProcAddress(m, sym.as_ptr())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_gate() {
        assert!(has((0, 0)));
        assert!(has(MIN));
        assert!(!has((99, 0)));
        let e = require((99, 0), "GtkFuture").unwrap_err();
        assert!(e.starts_with("GtkFuture needs GTK 99.0; the running GTK is 4."), "{e}");
    }

    #[test]
    fn runtime_lookup() {
        assert_ne!(call_get_type(c"gtk_widget_get_type"), 0);
        assert_eq!(call_get_type(c"gtk_no_such_thing_get_type"), 0);
    }
}
