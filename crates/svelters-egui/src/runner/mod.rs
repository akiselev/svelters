//! The runners. All three call [`EguiBackend::ui`] once per pass; they differ in who owns the loop:
//!
//! - **eframe** (`svl_backend.run`, `eframe.rs`): a native window; the backend owns the loop.
//! - **embed** ([`EguiBackend::ui`] from any egui app): the embedder owns the loop and the frame
//!   (D1); attach the host with `Host::start_with` and call `backend.ui(ui)` where the app should be.
//! - **kittest** ([`kittest`], also at the old path `crate::harness`): `egui_kittest::Harness` drives
//!   frames headless in `cargo test`.
//!
//! EM1 rebuilds both on `Embed` (one root, the bundle's `window`; ARCH-EMBED §9). Writer lane: EMBED.

mod eframe;
#[cfg(feature = "kittest")]
pub mod kittest;

pub(crate) use self::eframe::run_eframe;

use crate::backend::EguiBackend;

/// The desktop order in one call: loads `bundle` and runs it in an eframe window until the app
/// quits. Errors are load errors (mapped to the sources) or the runner's.
pub fn run_app(bundle: &std::path::Path, cfg: svelters_host::HostConfig) -> Result<(), String> {
    let backend = EguiBackend::new();
    let vt = backend.vtable();
    let host = svelters_host::Host::new(vt.backend_ref(), cfg).map_err(|e| e.to_string())?;
    host.load_bundle(bundle).map_err(|e| e.to_string())?;
    host.run().map_err(|e| e.to_string())
}
