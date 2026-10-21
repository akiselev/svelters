//! The macOS AppKit backend for svelters (`docs/notes/mac1-appkit.md`).
//!
//! - [`tables`] / `generated`: the kind tables generated from `metadata/appkit` and
//!   `metadata/overrides/appkit` by `tools/metadata/appkit/gen_backend.py`.
//! - [`lookup`]: name resolution over the tables (pure Rust; unit-tested on Linux too).
//! - `backend` (macOS only): [`AppKitBackend`], the `svl_backend` implementation.
//! - `testing` (macOS only): the harness the generated verification tests use.
//!
//! On other platforms the crate builds the tables and lookup only, so the Linux workspace gate
//! compiles and tests it without an SDK.

pub mod generated;
pub mod lookup;
pub mod tables;
pub mod types_table;

#[cfg(target_os = "macos")]
mod backend;
#[cfg(target_os = "macos")]
pub mod testing;

#[cfg(target_os = "macos")]
pub use backend::AppKitBackend;
