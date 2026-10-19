//! GTK4 backend for svelters. Implements the `svl_backend` C ABI over tables generated from
//! `metadata/gtk/*.json` (see `plans/milestones/P3-gtk-lane.md`).
//!
//! `generated/` is produced by `tools/metadata/gtk/gen_backend.py`; everything else is hand-written.

mod backend;
pub mod compat;
mod convert;
mod gi;
pub mod generated;
mod lookup;
mod reflect;
pub mod testing;
pub mod timing;
mod types;
mod util;

pub use backend::canvas;
pub use backend::GtkBackend;
