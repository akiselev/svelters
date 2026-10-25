//! The egui island (`plans/research/presto/architecture.md` §15; lane EGUI-ISLAND): an egui 0.37 context
//! hosted in a presto layer, for the migration. Every dock pane body can run in one unchanged, because
//! `PaneHost::ui(&PaneId, &mut egui::Ui)` is its only interface; egui-wgpu paint callbacks
//! (`CallbackTrait`: cadatomic's 3D viewport and canvases) run on the island's renderer, which runs on
//! presto's device.
//!
//! | Module | What |
//! |---|---|
//! | [`island`] | [`EguiIsland`]: input queue, egui pass, egui-wgpu render into a registered texture, the `Surface` display list |
//! | [`input`] | presto events → egui `Event`s (pointer routing and capture, keys, text, IME, wheel, files) |
//! | [`a11y`] | the AccessKit graft: nested trees, egui's root tree id rewritten, actions mapped back |
//! | [`dnd`] | the `egui::DragAndDrop` ↔ presto drag-and-drop bridge |
//! | [`shim`] | stand-ins for wave-1 APIs not published yet (texture registry, clipboard, presto drags) |
//!
//! The island is a layer kind ([`EguiIsland::layer_kind`]): the host node records
//! [`EguiIsland::display_list`] and places it in a `LayerKind::Texture` layer.

pub mod a11y;
pub mod dnd;
pub mod input;
pub mod island;
pub mod shim;

pub use a11y::{A11yGraft, island_tree_id};
pub use dnd::{DndBridge, DndOutput, IslandDrag, IslandDragCodec, PayloadCodec};
pub use island::{EguiIsland, ImeArea, IslandDesc, IslandOutput, TARGET_FORMAT};
pub use shim::{ClipboardText, PrestoDrag, TextureRegistry};
