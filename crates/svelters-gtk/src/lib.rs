//! GTK4 backend for svelters. Implements the `svl_backend` C ABI over tables generated from
//! `metadata/gtk/*.json` (see `plans/milestones/P3-gtk-lane.md`).
//!
//! `generated/` is produced by `tools/metadata/gtk/gen_backend.py`; everything else is hand-written.

mod backend;
mod convert;
mod gi;
pub mod generated;
mod lookup;
pub mod testing;
pub mod timing;
mod util;

pub use backend::GtkBackend;
