---
title: Widget reference
description: "Every element: props, events, methods, CSS, states and accessibility, by category."
status: stub
verified_at: ~
generated: partial
---

# Widget reference

One page per element. The tables on each page are generated from the kind table; the prose around them is
written by hand. Regenerate with `python3 tools/docs/egui/gen_reference.py`.

<!-- BEGIN GENERATED: index (tools/docs/egui/gen_reference.py). Do not edit. -->
## Inputs

Fields and controls that edit a value.

| Element | What it is |
|---|---|
| [`<checkbox>`](checkbox.md) | A check box with a caption. |
| [`<combo-box>`](combo-box.md) | A drop-down list: shows the selected option, picks one from a popup. |
| [`<drag-value>`](drag-value.md) | A number edited by dragging horizontally, the arrow keys, or typing after a click. |
| [`<radio-button>`](radio-button.md) | One choice of several. |
| [`<range-slider>`](range-slider.md) | A horizontal slider with two handles selecting `low`..`high`; the handles meet but never cross. |
| [`<slider>`](slider.md) | A horizontal slider with an optional value box. |
| [`<switch>`](switch.md) | An on/off switch with a sliding knob (GTK's `switch`; WinUI's `toggle-switch`). |
| [`<text-edit>`](text-edit.md) | A single- or multi-line text field. |

## Buttons

Things the user clicks to act.

| Element | What it is |
|---|---|
| [`<button>`](button.md) | A push button. |
| [`<hyperlink>`](hyperlink.md) | A link: text that opens `url` when clicked (AccessKit role `Link`). |
| [`<toggle-button>`](toggle-button.md) | A button that stays pressed (`checked`). |

## Display

Text, icons and indicators that show something.

| Element | What it is |
|---|---|
| [`<icon>`](icon.md) | A glyph from an icon font the embedder registered (SVP6), by name or as text. |
| [`<label>`](label.md) | Static text. |
| [`<progress-bar>`](progress-bar.md) | A horizontal bar filled to `fraction`, as wide as its rect. |
| [`<separator>`](separator.md) | A thin line between items; fills its rect along its orientation. |
| [`<spinner>`](spinner.md) | A busy indicator. |

## Containers

Elements that hold other elements and add behaviour or chrome.

| Element | What it is |
|---|---|
| [`<collapsing-header>`](collapsing-header.md) | A clickable header with a caret; its children (a flex box, indented) are shown and laid out only while `open`. |
| [`<frame>`](frame.md) | A flex box drawn inside an egui frame (background, border, rounding, inner margin), with an optional caption. |
| [`<panel>`](panel.md) | A side or top/bottom panel: a flex box with the panel background and a separator on its inner edge, optionally resizable. |
| [`<scroll-area>`](scroll-area.md) | A flex box whose content scrolls when it overflows the area's size (taffy `overflow: scroll`); give it a size or let flex-grow size it. |
| [`<split-view>`](split-view.md) | Two panes (its first two children; more are not shown) split by a divider the user drags. |
| [`<tab>`](tab.md) | A page of a `tab-view`: a flex box whose `title` is its tab's caption. |
| [`<tab-view>`](tab-view.md) | Pages under a tab bar: each child (a `tab`) is a page titled by its `title`; only the `selected` one is laid out and drawn. |
| [`<window>`](window.md) | A native window (the root viewport, or an immediate viewport); its children are laid out as a flex column. |

## Layout

Containers that only arrange their children.

| Element | What it is |
|---|---|
| [`<box>`](box.md) | A flexbox container laid out by taffy; draws nothing itself. |
| [`<grid>`](grid.md) | A CSS grid container laid out by taffy: children fill the cells row by row (auto-placement); draws nothing itself. |
| [`<sides>`](sides.md) | A row with its first child at the start and its second at the end (wrap several items in a `box`); with more children, the space is shared between them. |
| [`<stack>`](stack.md) | Shows one of its children (GTK's `stack`, Qt's `stacked-widget`): the one anchored `visible-child-name`, else the one at `visible-index`. |

## Views

Lists and tables over app data.

| Element | What it is |
|---|---|
| [`<table>`](table.md) | A virtual table (`egui_table`): only the visible rows and columns are drawn. |
| [`<table-column>`](table-column.md) | A column of its parent `table` (data only: not laid out or drawn as a node). |
| [`<virtual-list>`](virtual-list.md) | A virtualized list (the M5 model-view protocol): JS keeps the items, the list asks for the rows it shows. |

## Menus

Menu bars, menu buttons, menus and their items.

| Element | What it is |
|---|---|
| [`<action>`](action.md) | A menu item: plain, a check item (`checkable`) or a radio item (`group`), with an optional `shortcut`. |
| [`<context-menu>`](context-menu.md) | A menu that attaches to the element it is declared in and opens at the pointer on a secondary click on it (the innermost owner wins). |
| [`<menu>`](menu.md) | A menu: a menu-bar button, or a submenu row inside another menu. |
| [`<menu-bar>`](menu-bar.md) | A horizontal menu bar of `menu` children (and plain `action`s). |
| [`<menu-button>`](menu-button.md) | A button that opens a menu of its children (`action`, `menu`, `menu-separator`; a `menu` child's items are shown inline, as GTK's menu-button takes a menu). |
| [`<menu-separator>`](menu-separator.md) | A line between menu items (a vertical one in a menu bar). |

## Overlays

Popups, dialogs, inner windows and notifications.

| Element | What it is |
|---|---|
| [`<inner-window>`](inner-window.md) | A floating window inside the viewport: movable, optionally resizable, collapsible and closable; its children are a flex box. |
| [`<modal>`](modal.md) | An in-window modal dialog: a backdrop over the viewport that blocks the rest of the UI, its content centred, sized to fit (or `width`/`height`). |
| [`<popup>`](popup.md) | A popover anchored to its owner: the element it is declared in (attached), or the box it is placed in. |
| [`<toast-host>`](toast-host.md) | Shows transient notifications stacked in a corner of its viewport, over everything; takes no space itself. |

## Pickers

Buttons that open a picker for a date or colour.

| Element | What it is |
|---|---|
| [`<color-picker>`](color-picker.md) | A colour swatch; clicking it opens egui's colour picker popup. |
| [`<date-picker>`](date-picker.md) | A button showing a date; clicking it opens a calendar popup (Save/Cancel). |

## Media

Images and drawing surfaces.

| Element | What it is |
|---|---|
| [`<canvas>`](canvas.md) | A drawing surface replaying the X8 display list (`ops`) that `@svelters/canvas` records, as GTK's and Qt's `canvas` do. |
| [`<image>`](image.md) | A picture from `source`, at its own size unless `width`/`height` say otherwise. |

## Bases

Abstract kinds whose props every element of a family has.

| Element | What it is |
|---|---|
| [`MenuEntry`](menu-entry.md) | Common properties of the menu entries (`menu`, `action`, `menu-separator`, `context-menu`): drawn inside a menu, never laid out by taffy. |
| [`Widget`](widget.md) | Common properties of everything placed in a window: state, tooltip, accessibility and its taffy item style. |
<!-- END GENERATED -->
