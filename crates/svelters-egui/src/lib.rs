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
// OW0: no egui widget wrappers beyond the allow-listed ones (clippy.toml `disallowed-types`).
#![cfg_attr(not(test), deny(clippy::disallowed_types))]

// OW0: the own widgets (`widgets/`) are `#[svelters::widget]` kinds of this crate, whose generated
// code names `::svelters_egui`.
extern crate self as svelters_egui;

pub mod assets;
pub mod backend;
pub mod dialogs;
pub mod embed;
/// EV1: capture and bubbling of the DOM-style events, Rust event filters (SV6), overlays (SV7).
pub mod events;
/// WG2b's Tier 0 kinds (payloads, paint, measure; the canvas replay and the PNG image loader).
pub mod elements;
/// SVP6: font weights and icon name tables the embedder registers (WG1).
pub mod fonts;
pub mod focus;
// ICR1: the icon registry and how icons are drawn.
pub mod icons;
/// OW0: behaviour units of the own widgets (hover, press, focus; ARCH-WIDGETS §1.2).
pub mod interact;
pub mod kinds;
pub mod layout;
/// W5: the Linux primary selection in the text kinds (select-to-copy, middle-click paste).
pub mod primary;
/// FX1: native focus policy and word rules (GTK on Linux).
pub mod native;
/// OW0: paint primitives of the own widgets (box, text, icon, image; ARCH-WIDGETS §1.1).
pub mod paint;
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
/// OW0: svelters' own widgets (ARCH-WIDGETS §1.3) and the own-widget switch (§6.2).
pub mod widgets;
/// OW-RUST: the own widgets in immediate mode, for Rust chrome with no scene node (ARCH-WIDGETS Q5).
pub mod imm;
/// ZM1: UI zoom (`set_zoom`, VS Code's 1.2^n levels, who handles Ctrl+= / Ctrl+- / Ctrl+0).
pub mod zoom;
pub mod use_units;

mod a11y;
/// AU1: the accessible name the automation selectors share with AccessKit.
pub use a11y::accessible_name;
pub mod anim;
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
pub use views::status_bar::status_tip;
mod windows;

/// The walk's outbox and paint entry point, at their pre-EG0 path (now `walk.rs`).
pub mod render {
    pub use crate::walk::{Out, paint};
}

#[cfg(feature = "kittest")]
pub use runner::kittest as harness;

pub use backend::{EguiBackend, Stats};
pub use embed::Embed;
pub use style::Utilities;
pub use spec::dump;

/// `use svelters_egui::prelude::*;`: what a provider kind (`#[svelters::widget]`) or a behaviour
/// (`#[svelters::behavior]`) needs.
pub mod prelude {
    pub use crate::provider::{
        A11yCx, AvailableSpace, Behavior, BehaviorCx, Caps, Dispatched, Event, ExtKey, Hit, HitCx, IntoStateBits, KindDecl, LayoutCx, MeasureCx, MethodCx, Part, PassCx, PassScope, PrepareCx, ProviderCx, ScenePass, StateBit, StateBits, StateProvider, Widget, WidgetCx,
    };
    /// OW0: units and primitives for own widgets.
    pub use crate::interact::{Focusable, Hoverable, Intents, Pressable, Unit};
    pub use crate::paint::{StyleView, Ua, UaRule, UaValue};
    pub use crate::events::{At, EventFilter, EventKind, EventPhase, FilterScope, Flow, Overlay, OverlayCx, SceneEvent};
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
