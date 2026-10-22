//! Component tests in plain `cargo test`: a compiled Svelte bundle on the real QuickJS host and the
//! egui backend, driven by `egui_kittest` (headless, no renderer: no GPU and no display).
//!
//! ```ignore
//! let mut app = TestApp::load("fixtures/counter-egui/dist/app.js")?;
//! app.harness.get_by_label("Increment").click();
//! app.run();
//! assert!(app.harness.query_by_label("Count: 1").is_some());
//! ```

use crate::backend::EguiBackend;
use egui_kittest::Harness;
use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use svelters_abi::BackendVtable;
use svelters_host::{Host, HostConfig};

pub use egui_kittest;
pub use egui_kittest::kittest::{NodeT, Queryable};

/// A bundle mounted on a [`Host`] attached through ABI v2.7 `start` (D1: the harness drives the
/// frames), with a kittest [`Harness`] whose frames run the backend.
pub struct TestApp {
    pub harness: Harness<'static, EguiBackend>,
    pub host: Host<'static>,
    pub backend: EguiBackend,
}

impl TestApp {
    /// Loads `bundle` with a quiet host config (console and errors are captured, not echoed).
    pub fn load(bundle: impl AsRef<Path>) -> Result<TestApp, String> {
        TestApp::with_config(bundle, HostConfig { echo_console: false, echo_errors: false, ..HostConfig::default() })
    }

    pub fn with_config(bundle: impl AsRef<Path>, cfg: HostConfig) -> Result<TestApp, String> {
        let backend = EguiBackend::new();
        // The host borrows the vtable for its lifetime; a test app lives as long as the test.
        let vt: &'static BackendVtable = Box::leak(Box::new(backend.vtable()));
        let host = Host::new(vt.backend_ref(), cfg).map_err(|e| e.to_string())?;
        let err: Rc<RefCell<Option<String>>> = Rc::default();
        let (path, e2) = (bundle.as_ref().to_path_buf(), err.clone());
        host.start_with(move |h| {
            if let Err(e) = h.load_bundle(&path) {
                *e2.borrow_mut() = Some(e.to_string());
            }
        })
        .map_err(|e| e.to_string())?;
        if let Some(e) = err.borrow_mut().take() {
            return Err(e);
        }
        let size = backend
            .root_window()
            .and_then(|w| match backend.prop(w, "default-width").ok().zip(backend.prop(w, "default-height").ok()) {
                Some((svelters_abi::Value::F64(w), svelters_abi::Value::F64(h))) => Some(egui::vec2(w as f32, h as f32)),
                _ => None,
            })
            .unwrap_or(egui::vec2(480.0, 360.0));
        let harness = Harness::builder().with_size(size).build_ui_state(|ui, b: &mut EguiBackend| b.ui(ui), backend.clone());
        Ok(TestApp { harness, host, backend })
    }

    /// Runs frames until nothing asks for another one; returns how many ran.
    pub fn run(&mut self) -> u64 {
        self.harness.run()
    }

    /// Errors JS reported so far (uncaught exceptions, rejected promises).
    pub fn errors(&self) -> Vec<String> {
        self.host.take_errors().into_iter().map(|e| e.to_string()).collect()
    }
}
