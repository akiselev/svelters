# Lane EGUI-ISLAND: presto-egui-island

Owns `crates/presto-egui-island/**`. A layer kind hosting an `egui::Context` (egui 0.37): egui-wgpu's
`Renderer` on presto's device rendering into an `Rgba8Unorm` texture registered with the compositor
(`register_texture`), composited as a `Surface` primitive; input inside the island's rect translated from
presto events to egui `RawInput` (no egui-winit); repaint only when egui asks; AccessKit grafting with nested
trees (`TreeId`): rewrite egui's root tree id, map actions back; an `egui::DragAndDrop` ↔ presto DnD bridge;
`egui_wgpu::CallbackTrait` paint callbacks work (cadatomic's viewport and canvases use them). Tests (written,
not run): an egui label and button inside an island are clickable through presto input; the AccessKit graft
is valid.
