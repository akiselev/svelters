//! Native modules from a dlopen'd library (feature `dlopen`).
//!
//! The library exports `svl_modules_v1`, an `extern "C" fn() -> *const c_char` returning a static,
//! NUL-terminated UTF-8 JSON object that maps module ids to ES module sources:
//! `{"svelters:app/extra": "export const answer = 42;"}`. Ids must start with `svelters:app/`.

use crate::native::{NativeModules, APP_PREFIX};
use std::collections::BTreeMap;
use std::ffi::{c_char, CStr};
use std::path::Path;

/// The modules of one loaded library. The library stays loaded for the life of this value.
pub struct DylibModules {
    modules: BTreeMap<String, String>,
    _lib: libloading::Library,
}

impl DylibModules {
    /// Loads the library at `path` and reads its module table.
    pub fn open(path: &Path) -> Result<DylibModules, String> {
        // Safety: loading a library runs its initialisers. The caller names a library it trusts;
        // this type is behind the `dlopen` feature for that reason.
        let lib = unsafe { libloading::Library::new(path) }.map_err(|e| format!("{}: {e}", path.display()))?;
        let json = {
            // Safety: `svl_modules_v1` has the documented signature; the returned string is static.
            let entry: libloading::Symbol<unsafe extern "C" fn() -> *const c_char> =
                unsafe { lib.get(b"svl_modules_v1\0") }.map_err(|e| format!("{}: no svl_modules_v1: {e}", path.display()))?;
            let p = unsafe { entry() };
            if p.is_null() {
                return Err(format!("{}: svl_modules_v1 returned NULL", path.display()));
            }
            unsafe { CStr::from_ptr(p) }.to_str().map_err(|e| format!("{}: module table is not UTF-8: {e}", path.display()))?.to_string()
        };
        let modules: BTreeMap<String, String> =
            serde_json::from_str(&json).map_err(|e| format!("{}: module table is not a JSON object of strings: {e}", path.display()))?;
        if let Some(bad) = modules.keys().find(|id| !id.starts_with(APP_PREFIX)) {
            return Err(format!("{}: module id `{bad}` is outside `{APP_PREFIX}`", path.display()));
        }
        Ok(DylibModules { modules, _lib: lib })
    }
}

impl NativeModules for DylibModules {
    fn ids(&self) -> Vec<String> {
        self.modules.keys().cloned().collect()
    }

    fn source(&self, id: &str) -> Option<String> {
        self.modules.get(id).cloned()
    }
}
