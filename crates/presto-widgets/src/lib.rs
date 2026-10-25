//! presto's built-in kinds (`plans/research/presto/architecture.md` §4.2; EGUI.md §5 for the catalogue).
//!
//! Wave-2 lanes own one module group each (`plans/cloud/presto-b2/`): WIDGETS-BASIC `interact`, `paint`,
//! `theme`, `controls`; WIDGETS-VIEWS `views`; TEXT-INPUT `text`; WIDGETS-TIER1 `table`, `tabs`, `pickers`,
//! `dnd`, `dialogs`, `zoom`.

pub mod controls;
pub mod dialogs;
pub mod dnd;
pub mod interact;
pub mod paint;
pub mod pickers;
pub mod table;
pub mod tabs;
pub mod text;
pub mod theme;
pub mod views;
pub mod zoom;
