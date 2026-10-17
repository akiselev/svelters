//! GTK4 backend for svelters. Implements the `svl_backend` C ABI over tables generated from
//! `metadata/gtk/*.json` (see `plans/milestones/P3-gtk-lane.md`).
//!
//! `generated/` is produced by `tools/metadata/gtk/gen_backend.py`; everything else is hand-written.

mod backend;
mod convert;
pub mod generated;
mod lookup;
pub mod testing;
mod util;

pub use backend::GtkBackend;
