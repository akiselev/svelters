//! [`Toolkit`] over Qt: the generated backend's tooling hooks (see `counter_qt::hooks`) and the
//! backend's own `click` method on buttons.

use crate::driver::Toolkit;
use counter_qt::hooks;
use serde_json::Value as Json;
use svelters_abi::{BackendRef, NameKind};

pub struct QtToolkit {
    pub backend: BackendRef<'static>,
}

impl Toolkit for QtToolkit {
    fn click(&self, h: u64) -> Result<(), String> {
        let kind = self.backend.resolve(NameKind::Kind, 0, "button");
        let method = self.backend.resolve(NameKind::Method, kind, "click");
        if method == 0 {
            return Err("button has no click method".into());
        }
        self.backend.call(h, method, &[]).map(|_| ()).map_err(|e| e.message)
    }

    fn type_text(&self, h: u64, text: &str) -> Result<(), String> {
        hooks::type_text(h, text)
    }

    fn dump(&self, h: u64) -> Json {
        serde_json::from_str(&hooks::dump(h)).unwrap_or(Json::Null)
    }

    fn screenshot(&self, h: u64, path: &str) -> Result<(), String> {
        hooks::grab_png(h, Some(path))
    }

    fn render(&self, h: u64) -> Result<(), String> {
        hooks::grab_png(h, None)
    }

    fn close(&self, h: u64) -> Result<(), String> {
        hooks::close(h)
    }

    fn native_toplevels(&self) -> usize {
        hooks::toplevel_count()
    }

    fn schedule(&self, ms: u32, f: Box<dyn FnOnce()>) {
        hooks::run_later(ms as i32, f);
    }

    fn take_toolkit_timing(&self) -> (u64, u64) {
        hooks::take_timing()
    }
}
