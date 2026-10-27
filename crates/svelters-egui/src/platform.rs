//! DX2: the desktop clipboard and shell for `HostConfig::platform` (svelters-host `platform.rs`).
//!
//! SV75: the implementation lives in svelters-host (feature `system-clipboard`, which svelters-egui
//! turns on), so an embedder without egui (svelters-skelter) builds the same desktop platform with
//! `svelters_host::platform::Platform::system()`. These are re-exports under the old paths.
//!
//! [`system`] is what a desktop app installs: [`SystemClipboard`] (arboard, the same library egui's
//! winit integration copies and pastes with, so `text-edit` and `svelters:clipboard` see one
//! clipboard) and svelters-host's `SystemShell`. [`run_app`](crate::run_app) and
//! `svelters-egui-run` install it; an embedder (cadatomic) sets it on its own `HostConfig`:
//!
//! ```ignore
//! let mut host = HostConfig::default();
//! host.platform = svelters_egui::platform::system();
//! ```
//!
//! The kittest harness keeps the default (`Platform::headless`: an in-process clipboard, no shell),
//! so tests never touch the system clipboard or start a file manager.
//!
//! Writer lane: DX2 (plans/milestones/DX2-clipboard-files.md).

pub use svelters_host::platform::{system, SystemClipboard};
