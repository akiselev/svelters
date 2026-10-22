//! svelters-egui: the egui backend (milestone G1, plans/research/EGUI.md, plans/milestones/G1-egui-skeleton.md).
//!
//! Unlike the GTK/Qt/AppKit/WinUI backends, the backend owns the tree: host ops edit a retained
//! scene ([`scene`]), taffy lays it out ([`layout`]), and each egui frame walks the visible part and
//! paints it with egui widgets (`walk.rs`, `leaf.rs`). Events and widget changes are queued during the
//! pass and delivered after it; JS runs only before the pass (`frame.rs`). The vocabulary comes from
//! one Rust kind table ([`kinds`]), which also generates the compiler's spec ([`spec`]).
//!
//! Runners ([`runner`]): eframe (`svl_backend.run`, [`run_app`]), embedding into a host's egui app
//! ([`Embed`]: slots and roots, ARCH-EMBED §4; EM1), and kittest ([`harness`], feature `kittest`).
//!
//! The module layout is ARCH-EMBED §9 with ARCH-SCENE §13's `scene/*` cut (EG0,
//! plans/milestones/EG0-module-split.md); each file's doc names its writer lane. The old paths
//! (`render`, `harness`, `layout::invalidate`) are kept as re-exports.

pub mod backend;
pub mod embed;
/// SVP6: font weights and icon name tables the embedder registers (WG1).
pub mod fonts;
pub mod kinds;
pub mod layout;
pub mod provider;
pub mod runner;
pub mod scene;
pub mod spec;
/// CS5e: the egui cascade (the Rust CSS engine on the scene).
pub mod style;

mod a11y;
mod events;
mod frame;
mod ids;
mod leaf;
mod ops;
mod walk;
// WG4a: state, layout and paint of the Tier 1 views (`kinds/views.rs`); `toast` is public
mod views;
pub use views::toast;
mod windows;

/// The walk's outbox and paint entry point, at their pre-EG0 path (now `walk.rs`).
pub mod render {
    pub use crate::walk::{Out, paint};
}

#[cfg(feature = "kittest")]
pub use runner::kittest as harness;

pub use backend::{EguiBackend, Stats};
pub use embed::Embed;
pub use spec::dump;

/// `use svelters_egui::prelude::*;`: what a provider kind (`#[svelters::widget]`) or a behaviour
/// (`#[svelters::behavior]`) needs.
pub mod prelude {
    pub use crate::provider::{
        A11yCx, AvailableSpace, Behavior, BehaviorCx, Caps, Event, ExtKey, Hit, HitCx, IntoStateBits, KindDecl, MeasureCx, StateBit, StateBits, Widget, WidgetCx,
    };
    pub use crate::scene::{NodeMut, NodeRef};
    pub use egui;
}
pub use runner::run_app;

/// The CSS engine the scene is styled with (CS5e).
pub use svelters_style;

/// The egui this backend is built against (re-exported so apps and packs link the same copy).
pub use egui;
