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

// ZU3: no blocking file, process or wait calls on the UI thread (list in clippy.toml). Unit tests
// are exempt; the legitimate I/O sites carry `#[allow(clippy::disallowed_methods)]` with a reason.
#![cfg_attr(not(test), deny(clippy::disallowed_methods))]

pub mod assets;
pub mod backend;
pub mod dialogs;
pub mod embed;
/// WG2b's Tier 0 kinds (payloads, paint, measure; the canvas replay and the PNG image loader).
pub mod elements;
/// SVP6: font weights and icon name tables the embedder registers (WG1).
pub mod fonts;
pub mod focus;
pub mod kinds;
pub mod layout;
/// W5: the Linux primary selection in the text kinds (select-to-copy, middle-click paste).
pub mod primary;
/// FX1: native focus policy and word rules (GTK on Linux).
pub mod native;
pub mod platform;
pub mod provider;
pub mod runner;
pub mod scene;
pub mod spec;
/// CS5e: the egui cascade (the Rust CSS engine on the scene).
pub mod style;
/// DS1: the theme (VS Code's base look as one token map: the `:root` sheet, the egui Style,
/// typed getters).
pub mod theme;

mod a11y;
/// AU1: the accessible name the automation selectors share with AccessKit.
pub use a11y::accessible_name;
pub mod anim;
mod events;
mod frame;
mod ids;
mod leaf;
pub mod numeric;
// WG2a: menus and overlays (plans/milestones/WG2a-menus-popups.md)
mod menus;
mod overlays;
pub mod rich;
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
        A11yCx, AvailableSpace, Behavior, BehaviorCx, Caps, Event, ExtKey, Hit, HitCx, IntoStateBits, KindDecl, LayoutCx, MeasureCx, PassCx, PassScope, PrepareCx, ProviderCx, ScenePass, StateBit, StateBits, StateProvider, Widget, WidgetCx,
    };
    pub use crate::scene::{NodeId, NodeMut, NodeRef, SceneMut, SceneRef};
    pub use egui;
}
pub use runner::run_app;

/// The CSS engine the scene is styled with (CS5e).
pub use svelters_style;

/// The egui this backend is built against (re-exported so apps and packs link the same copy).
pub use egui;
pub use egui_extras;
pub use kinds::dnd::DragPayload;
/// The taffy the scene is laid out with (SG4: `Widget::layout`, `LayoutCx`; unstable, ARCH-SCENE §16).
pub use taffy;
