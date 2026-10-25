# Lane WIDGETS-TIER1: presto-widgets (tables, tabs, pickers, DnD, dialogs, zoom)

Owns `crates/presto-widgets/src/{table/**,tabs/**,pickers/**,dnd/**,dialogs/**}`. Port from svelters-egui
`views/table*`, `tab-view` (write a presto-drawn tab-view: svelters-egui's still uses egui widgets),
the colour and date pickers, `dialogs.rs`, the DnD code (`views/dnd`), `zoom.rs`. In-app DnD carries a JS-side
id payload; an `egui::DragAndDrop` bridge is lane EGUI-ISLAND's. Tests (written, not run): table virtual rows
and column resize, tab selection and close, DnD hover/drop, dialog button order per OS.
