//! The runners. They differ in who owns the loop:
//!
//! - **eframe** (`eframe.rs`): a native window; [`run_app`] runs an app bundle on an [`Embed`]
//!   (`logic` from `App::logic`, the presented windows from `App::ui`), `svl_backend.run` runs a
//!   host's own backend ([`EguiBackend::ui`]).
//! - **embed** ([`Embed`] from any egui app): the embedder owns the loop and the frame (D1), creates
//!   slots and mounts roots into them (ARCH-EMBED §4).
//! - **kittest** ([`kittest`], also at the old path `crate::harness`): `egui_kittest::Harness` drives
//!   frames headless in `cargo test`: `TestApp` for app bundles, `EmbedHarness` for embed bundles.
//!
//! Writer lane: EMBED.

mod eframe;
#[cfg(feature = "kittest")]
pub mod kittest;

pub(crate) use self::eframe::run_eframe;

#[allow(unused_imports)]
use crate::backend::EguiBackend;
use crate::embed::{Embed, EmbedConfig};

/// The desktop order in one call: loads `bundle` (its `App` mounts its windows) on an [`Embed`] and
/// runs it in an eframe window until the app quits. Errors are load errors (mapped to the sources)
/// or the runner's.
pub fn run_app(bundle: &std::path::Path, cfg: svelters_host::HostConfig) -> Result<(), String> {
    // eframe creates the real context later: `logic` binds it at the first frame.
    let embed = Embed::new(&egui::Context::default(), EmbedConfig::app(cfg)).map_err(|e| e.to_string())?;
    embed.load_app(bundle).map_err(|e| e.to_string())?;
    self::eframe::run_desktop(embed)
}
