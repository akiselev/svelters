//! svelters-egui: the egui backend (milestone G1, plans/research/EGUI.md, plans/milestones/G1-egui-skeleton.md).
//!
//! Unlike the GTK/Qt/AppKit/WinUI backends, the backend owns the tree: host ops edit a retained
//! scene ([`scene`]), taffy lays it out ([`layout`]), and each egui frame walks the visible part and
//! paints it with egui widgets ([`render`]). Events and widget changes are queued during the pass and
//! delivered after it; JS runs only before the pass ([`backend`]). The vocabulary comes from one Rust
//! kind table ([`kinds`]), which also generates the compiler's spec ([`spec`]).
//!
//! Runners ([`runner`]): eframe (`svl_backend.run`), embedding into any egui `Ui`
//! ([`EguiBackend::ui`] with `Host::start_with`), and kittest ([`harness`], feature `kittest`).

pub mod backend;
pub mod kinds;
pub mod layout;
pub mod render;
pub mod runner;
pub mod scene;
pub mod spec;

#[cfg(feature = "kittest")]
pub mod harness;

pub use backend::{EguiBackend, Stats};
pub use runner::run_app;

/// The egui this backend is built against (re-exported so apps and packs link the same copy).
pub use egui;
