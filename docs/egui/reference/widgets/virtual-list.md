---
title: virtual-list
tag: virtual-list
kind: VirtualList
category: views
description: "A virtualized list (the M5 model-view protocol): JS keeps the items, the list asks for the rows it shows."
status: stub
verified_at: ~
generated: partial   # regions between BEGIN/END GENERATED markers come from the spec
sources:
  - metadata/semantics/egui.json#kinds.VirtualList
---

# `<virtual-list>`

<!-- BEGIN GENERATED: summary of kinds.VirtualList (tools/docs/egui/gen_reference.py). Do not edit. -->
A virtualized list (the M5 model-view protocol): JS keeps the items, the list asks for the rows it shows.

**Base:** [`Widget`](widget.md) · **Children:** none · **Text content:** no
<!-- END GENERATED -->

<!-- TODO(DOC7): when to use it, and what to use instead. -->

## Example

<!-- TODO(DOC7): a checked example (a testkit case under docs/egui/cases/). -->

## Props

<!-- BEGIN GENERATED: props of kinds.VirtualList (tools/docs/egui/gen_reference.py). Do not edit. -->
| Prop | Type | Default | `bind:` | Description |
|---|---|---|---|---|
| `current` | `number` | `-1` | **yes**, on `changed` | OW5a1b: the cursor's item id (`bind:current`), -1 for none. Setting it moves the cursor without selecting. |
| `internal-move` | `boolean` | `false` | no | OW5a1b: rows reorder by drag within the list (Qt `InternalMove`): a drop raises `move(from, to)`. |
| `layout` | `'list' \| 'grid'` | `"list"` | no | `list` (egui draws `grid` as a list for now). |
| `row-drag-type` | `string` | `""` | no | OW5a1b: non-empty: rows drag, as a payload of this type (the dragged item ids, a line each) plus `text/plain` (their labels). |
| `row-drop-types` | `string` | `""` | no | OW5a1b: non-empty: rows take drops of these types (WG4b patterns) and raise `drop-row`. |
| `row-height` | `number` | `32` | no | The height a row is assumed to have before it is measured, and its minimum, in points. |
| `selected` | `number` | `-1` | **yes**, on `changed` | The selected position, -1 for none. Setting it never raises `select`. |
| `selected-ids` | `number[]` | `null` | **yes**, on `changed` | The selected item ids (`bind:selected-ids`); kept by id across splices, sorts and filters. |
| `selection` | `'none' \| 'single' \| 'multi' \| 'toggle' \| 'range'` | `"single"` | no | `single`: a click or the keyboard selects a row (`select`); `multi`: Ctrl/Shift clicks and keys select several (`selectionchange`); `toggle`: a click toggles its row, no modifier (checklists); `range`: one contiguous run, Shift extends; `none`: rows are not selectable. |
| `uniform-rows` | `boolean` | `false` | no | OW5a1b: every row is `row-height` tall; rows are not measured (Qt `uniformRowHeights`). |

Plus the 27 props every element has, from [`Widget`](widget.md#props): `accessible-name`, `align-self`, `anchor`, `autofocus`, `bottom`, `dialog-role`, `drag-data`, `drag-type`, `drop-types`, `enabled`, `flex-basis`, `flex-grow`, `flex-shrink`, `height`, `invalid`, `left`, `margin`, `max-height`, `max-width`, `min-height`, `min-width`, `position`, `right`, `tooltip`, `top`, `visible`, `width`.
<!-- END GENERATED -->

## Events

<!-- BEGIN GENERATED: events of kinds.VirtualList (tools/docs/egui/gen_reference.py). Do not edit. -->
| Attribute | Handler | When |
|---|---|---|
| `onactivate` | `(pos: number) => void` | The user activated the item at `pos` (double click). |
| `onbind` | `(e: SvlEvent<[number, number, number, number]>, row: number, pos: number, id: number, gen: number) => void` | Slot `row` now shows item `id` at `pos`; `gen` counts the binds of the slot. **Sync:** the handler runs before the frame's pass and gets an `SvlEvent` first. |
| `oncopy` | `(ids: unknown[]) => void` | OW5a1b: Ctrl+C on the focused list: copy the selected items (their ids). |
| `ondrop-row` | `(id: number, where: string, type: string, data: string) => void` | OW5a1b: a drag was dropped `before`, `after` or `into` item `id` (-1: below the last row), as the entry of `type` it took. |
| `onmove` | `(from: number, to: number) => void` | OW5a1b (`internal-move`): the row at `from` was dragged to the gap before position `to` (`count()`: the end); move it. |
| `onrow-clicked` | `(pos: number, id: number) => void` | OW5a: a click on the row at `pos` (item `id`), after the selection moved. |
| `onrow-context` | `(pos: number, id: number) => void` | OW5a: a secondary click on the row at `pos` (item `id`): open its context menu. The selection follows first (an unselected row is selected, per platform Policy). |
| `onrow-hovered` | `(pos: number, id: number) => void` | OW5a1b: the pointer entered the row at `pos` (item `id`; Qt `entered`). |
| `onselect` | `(pos: number) => void` | The user selected the item at `pos`. |
| `onselectionchange` | `(ids: unknown[]) => void` | The user changed the selection; carries the selected ids. |
| `onsetup` | `(e: SvlEvent<[number]>, row: number) => void` | Row slot `row` was created: give it content with `setRow`. **Sync:** the handler runs before the frame's pass and gets an `SvlEvent` first. |
| `onteardown` | `(e: SvlEvent<[number]>, row: number) => void` | Slot `row` is gone: release its widget. **Sync:** the handler runs before the frame's pass and gets an `SvlEvent` first. |
| `onunbind` | `(e: SvlEvent<[number, number]>, row: number, gen: number) => void` | Slot `row` no longer shows binding `gen`. **Sync:** the handler runs before the frame's pass and gets an `SvlEvent` first. |

`on:activate` works too, and a bare `on:<event>` on a component forwards the component's `on<event>` prop.

Plus the events of [`Widget`](widget.md#events): `onanimationend`, `onclick`, `oncontextmenu`, `ondblclick`, `ondrag-begin`, `ondrag-end`, `ondrag-enter`, `ondrag-leave`, `ondrop`, `onfocusin`, `onfocusout`, `onkeydown`, `onkeyup`, `onpointerdown`, `onpointerup`, `onwheel`.
<!-- END GENERATED -->

## Methods

<!-- BEGIN GENERATED: methods of kinds.VirtualList (tools/docs/egui/gen_reference.py). Do not edit. -->
| Method | Returns | Description |
|---|---|---|
| `count` | `void` | The number of items. |
| `rowAt` | `void` | `rowAt(pos)`: the slot bound to `pos`, or -1. |
| `scrollTo` | `void` | `scrollTo(pos, hint?)`: scrolls the item into view; `hint` (Qt `ScrollHint`): `nearest` (the default, the least scroll), `top`, `bottom` or `center`. |
| `setRow` | `void` | `setRow(row, widget)`: the content of row slot `row` (called from `setup`); null clears it. |
| `splice` | `void` | `splice(pos, removed, ids, labels?)`: replaces `removed` ids at `pos` with `ids` (unique integers); `labels` (strings, one per id) are what type-ahead matches. |

Call them through `bind:this`: `el.call('count')`.
<!-- END GENERATED -->

## Behaviour you need to know

<!-- TODO(DOC7): what the tables don't say: timing, edge cases, limits. -->

## CSS

<!-- BEGIN GENERATED: css of kinds.VirtualList (tools/docs/egui/gen_reference.py). Do not edit. -->
**Selector:** `virtual-list`, plus `.class`, `#id` and attribute selectors on its props.

**Painted as:** nothing CSS reaches. No paint property applies to the element itself; style its children instead.

| Property | Applies | Theme default |
|---|---|---|
| `background-color` | no | — |
| `border-color` | no | — |
| `border-width` | no | — |
| `border-radius` | no | — |
| `color` | no | — |
| `box-shadow` | no | — |
| `opacity` | no | — |

**Layout properties** (as attributes or in CSS; an explicit attribute wins): `align-self`, `bottom`, `dialog-role`, `flex-basis`, `flex-grow`, `flex-shrink`, `height`, `left`, `max-height`, `max-width`, `min-height`, `min-width`, `position`, `right`, `top`, `visible`, `width`.
Also in CSS only: `display`, `grid-column`, `grid-row`, `margin-bottom`, `margin-left`, `margin-right`, `margin-top`.

Each row is `::part(row)` in its own state (`:hover`, `:active`, `:selected`, `:current`, `:focus-visible`, `:drop-target`, `:nth-child()`; `:focus-within` stays the list's) and the element its row template renders. Other parts: `placeholder`, `drop-line`, `ghost` (the drag label), `scrollbar-y`, `thumb-y`. The list is `:drop-target` while a row drag it takes is over it.

Full matrix: [css/properties](../../css/properties.md). States: below.
<!-- END GENERATED -->

## States

<!-- BEGIN GENERATED: states of kinds.VirtualList (tools/docs/egui/gen_reference.py). Do not edit. -->
No built-in state is set on it.

Never set on it (rules with them don't match): `:hover`, `:active`, `:focus`, `:focus-within`, `:disabled`, `:checked`, `:selected`, `:open`, `:expanded`, `:invalid`, `:read-only`, `:dragging`, `:exiting`, `:busy`, `:indeterminate`, `:placeholder-shown`, `:default`, `:drop-target`, `:current`, `:backdrop`, `:required`, `:user-invalid`, `:has-children`.
<!-- END GENERATED -->

## Accessibility

<!-- BEGIN GENERATED: a11y of kinds.VirtualList (tools/docs/egui/gen_reference.py). Do not edit. -->
| | |
|---|---|
| Role | `List`; `ListBox` when `selection` is not `none` |
| Name | `accessible-name` |
| Value | its row count (size of set) |
| Actions | `Focus` |
| `anchor`, `invalid` | not announced on this element (`anchor` still keys its ids) |

Rows are `ListItem` nodes (`ListBoxOption` when selectable) with their position in the set; the cursor row is the active descendant while the list has focus. Multi selection marks it multiselectable.
<!-- END GENERATED -->

## Testing

<!-- TODO(DOC7): how to find and drive it in a kittest harness. -->

## See also

<!-- TODO(DOC7) -->
