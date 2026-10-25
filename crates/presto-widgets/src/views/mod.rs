//! Lane WIDGETS-VIEWS' kinds (`plans/cloud/presto-b2/LANE-widgets-views.md`): scrolling, panels and
//! splitters, virtual lists and trees, and the floating surfaces that live in a window's overlay stack
//! (architecture §14): popups, menus, selects, tooltips, toasts, modals and inner windows; plus the
//! status bar and the dialog button row.
//!
//! Each kind is a state struct that implements [`shim::Kind`] over plain logic the tests drive
//! without a tree: the logic (latching, row recycling, placement, menu navigation) is what the
//! consolidation keeps when `shim` gives way to `presto-core`.

pub mod shim;
pub(crate) mod draw;
pub mod placement;
pub mod menu_model;
pub mod virtual_list;
pub mod tree;
pub mod scroll;
pub mod menu;
pub mod overlay;
pub mod popup;
pub mod tooltip;
pub mod toast;
pub mod modal;
pub mod inner_window;
pub mod split;
pub mod select;
pub mod panel;
