//! The egui island (`plans/research/skelter/architecture.md` §15; lane EGUI-ISLAND): an egui 0.37 context
//! hosted in a skelter layer, for the migration. Every dock pane body can run in one unchanged, because
//! `PaneHost::ui(&PaneId, &mut egui::Ui)` is its only interface; egui-wgpu paint callbacks
//! (`CallbackTrait`: cadatomic's 3D viewport and canvases) run on the island's renderer, which runs on
//! skelter's device.
//!
//! | Module | What |
//! |---|---|
//! | [`island`] | [`EguiIsland`]: input queue, egui pass, egui-wgpu render into a registered texture, the `Surface` display list |
//! | [`input`] | skelter events → egui `Event`s (pointer routing and capture, keys, text, IME, wheel, files) |
//! | [`a11y`] | the AccessKit graft: nested trees, egui's root tree id rewritten, actions mapped back |
//! | [`dnd`] | the `egui::DragAndDrop` ↔ skelter drag-and-drop bridge |
//! | [`host`] | the `egui-island` kind and the document's table of mounted islands (mount, detach, run, render) |
//!
//! In a skelter document an island is mounted on a node of the `egui-island` kind ([`host`]); without a
//! document (tests, a custom host) [`EguiIsland`] is driven directly.

pub mod a11y;
pub mod dnd;
pub mod host;
pub mod input;
pub mod island;

pub use a11y::{A11yGraft, island_tree_id};
pub use dnd::{DndBridge, DndOutput, DragData, IslandDrag, IslandDragCodec, PayloadCodec};
pub use island::{EguiIsland, ImeArea, IslandDesc, IslandOutput, TARGET_FORMAT, TextureRegistry};
pub use host::{EGUI_ISLAND, EguiIslandKind, IslandHost, MountError, MountRejected};
