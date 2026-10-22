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
| `layout` | `'list' \| 'grid'` | `"list"` | no | `list` (egui draws `grid` as a list for now). |
| `row-height` | `number` | `32` | no | The height a row is assumed to have before it is measured, and its minimum, in points. |
| `selected` | `number` | `-1` | **yes**, on `changed` | The selected position, -1 for none. Setting it never raises `select`. |
| `selected-ids` | `number[]` | `null` | **yes**, on `changed` | The selected item ids (`bind:selected-ids`); kept by id across splices, sorts and filters. |
| `selection` | `'none' \| 'single' \| 'multi'` | `"single"` | no | `single`: a click or the keyboard selects a row (`select`); `multi`: Ctrl/Shift clicks and keys select several (`selectionchange`); `none`: rows are not selectable. |

Plus the 26 props every element has, from [`Widget`](widget.md#props): `accessible-name`, `align-self`, `anchor`, `autofocus`, `bottom`, `drag-data`, `drag-type`, `drop-types`, `enabled`, `flex-basis`, `flex-grow`, `flex-shrink`, `height`, `invalid`, `left`, `margin`, `max-height`, `max-width`, `min-height`, `min-width`, `position`, `right`, `tooltip`, `top`, `visible`, `width`.
<!-- END GENERATED -->

## Events

<!-- BEGIN GENERATED: events of kinds.VirtualList (tools/docs/egui/gen_reference.py). Do not edit. -->
| Attribute | Handler | When |
|---|---|---|
| `onactivate` | `(pos: number) => void` | The user activated the item at `pos` (double click). |
| `onbind` | `(e: SvlEvent<[number, number, number, number]>, row: number, pos: number, id: number, gen: number) => void` | Slot `row` now shows item `id` at `pos`; `gen` counts the binds of the slot. **Sync:** the handler runs before the frame's pass and gets an `SvlEvent` first. |
| `onrow-clicked` | `(pos: number, id: number) => void` | OW5a: a click on the row at `pos` (item `id`), after the selection moved. |
| `onrow-context` | `(pos: number, id: number) => void` | OW5a: a secondary click on the row at `pos` (item `id`): open its context menu. |
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
| `scrollTo` | `void` | `scrollTo(pos)`: scrolls the item into view. |
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

**Layout properties** (as attributes or in CSS; an explicit attribute wins): `align-self`, `bottom`, `flex-basis`, `flex-grow`, `flex-shrink`, `height`, `left`, `max-height`, `max-width`, `min-height`, `min-width`, `position`, `right`, `top`, `visible`, `width`.
Also in CSS only: `display`, `grid-column`, `grid-row`, `margin-bottom`, `margin-left`, `margin-right`, `margin-top`.

Style the rows: each row is the element its row template renders.

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
