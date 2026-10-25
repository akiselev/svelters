# Lane WIDGETS-VIEWS: presto-widgets (scrolling, layers, menus, popups, lists, trees)

Owns `crates/presto-widgets/src/views/**`. Port from svelters-egui `widgets/scroll_area.rs`, `views/`,
`kinds/virtuallist.rs`, `menus.rs`, `overlays.rs`, `windows.rs` (inner windows): scroll-area (a scroll layer
with a scroll property node; wheel/touchpad/fling through presto-anim's scroll physics; **latching per
gesture**; passive fast commits), panel, split-view (native splitter drag), virtual-list (RowPool,
`virtual_children`), tree-view, popup/popover placement, menu bar/menu/menu item/context menu, select
(combo), tooltip, toast-host/toast, modal, inner-window, status-bar, dialog-buttons. Overlays are layers in
the window's overlay stack (architecture §14). Tests (written, not run): scrolling and latching, virtual
list recycling, menu keyboard navigation, popup placement.
