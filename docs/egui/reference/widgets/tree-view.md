---
title: tree-view
tag: tree-view
kind: TreeView
category: views
description: "A virtualized tree: JS keeps the tree and flattens its visible rows into the list; children load when a node expands."
status: stub
verified_at: ~
generated: partial   # regions between BEGIN/END GENERATED markers come from the spec
sources:
  - metadata/semantics/egui.json#kinds.TreeView
---

# `<tree-view>`

<!-- BEGIN GENERATED: summary of kinds.TreeView (tools/docs/egui/gen_reference.py). Do not edit. -->
A virtualized tree: JS keeps the tree and flattens its visible rows into the list; children load when a node expands.

**Base:** [`Widget`](widget.md) · **Children:** none · **Text content:** no
<!-- END GENERATED -->

<!-- TODO(DOC7): when to use it, and what to use instead. -->

## Example

<!-- TODO(DOC7): a checked example (a testkit case under docs/egui/cases/). -->

## Props

<!-- BEGIN GENERATED: props of kinds.TreeView (tools/docs/egui/gen_reference.py). Do not edit. -->
| Prop | Type | Default | `bind:` | Description |
|---|---|---|---|---|
| `current` | `number` | `-1` | **yes**, on `changed` | OW5a1b: the cursor's row id (`bind:current`), -1 for none. Setting it moves the cursor without selecting. |
| `expand-on` | `'twistie' \| 'click' \| 'double-click'` | `"twistie"` | no | What toggles a row besides its twistie, Left/Right and AccessKit: `twistie` (nothing more; a double click activates), `click` (a click on the row), `double-click` (Qt's `expandsOnDoubleClick`). |
| `indent` | `number` | `16` | no | Points per level; the disclosure triangle sits in the last one. |
| `root-decorated` | `boolean` | `true` | no | Level-0 rows show their twistie (Qt's `rootIsDecorated`). |
| `row-drag-type` | `string` | `""` | no | Non-empty: rows drag, as a payload of this type (their `data`) plus `text/plain` (their labels). |
| `row-drop-types` | `string` | `""` | no | Non-empty: rows take drops of these types (WG4b patterns) and raise `drop-row`. |
| `row-height` | `number` | `24` | no | The height a row is assumed to have before it is measured, and its minimum, in points. |
| `selection` | `'none' \| 'single' \| 'multi' \| 'toggle' \| 'range'` | `"single"` | no | `single`, `multi` (Shift ranges, Ctrl toggles), `toggle` (a click toggles, no modifier), `range` (one contiguous run, Shift extends); `none`: rows are not selectable. |
| `uniform-rows` | `boolean` | `false` | no | OW5a1b: every row is `row-height` tall; rows are not measured (Qt `uniformRowHeights`). |

Plus the 30 props every element has, from [`Widget`](widget.md#props): `accessible-name`, `align-self`, `anchor`, `anchor-scope`, `autofocus`, `bottom`, `dialog-role`, `drag-data`, `drag-type`, `drop-types`, `enabled`, `flex-basis`, `flex-grow`, `flex-shrink`, `height`, `invalid`, `left`, `margin`, `max-height`, `max-width`, `min-height`, `min-width`, `position`, `right`, `status-section`, `status-tip`, `tooltip`, `top`, `visible`, `width`.
<!-- END GENERATED -->

## Events

<!-- BEGIN GENERATED: events of kinds.TreeView (tools/docs/egui/gen_reference.py). Do not edit. -->
| Attribute | Handler | When |
|---|---|---|
| `onactivate` | `(pos: number) => void` | The user activated the row at `pos` (double click, Enter). |
| `onbind` | `(e: SvlEvent<[number, number, number, number]>, row: number, pos: number, id: number, gen: number) => void` | Slot `row` now shows item `id` at `pos`; `gen` counts the binds of the slot. **Sync:** the handler runs before the frame's pass and gets an `SvlEvent` first. |
| `oncollapse` | `(id: number) => void` | The user collapsed node `id` (disclosure click, Left): splice its children out. |
| `oncopy` | `() => void` | Ctrl+C on the focused tree: copy the selected rows. |
| `ondrop-row` | `(id: number, where: string, type: string, data: string) => void` | A drag was dropped `before`, `after` or `into` row `id` (-1: below the last row), as the entry of `type` it took. |
| `onexpand` | `(id: number) => void` | The user expanded node `id` (disclosure click, Right): splice its children in. |
| `onrow-clicked` | `(pos: number, id: number) => void` | OW5a: a click on the row at `pos` (item `id`), after the selection moved. |
| `onrow-context` | `(pos: number, id: number) => void` | OW5a: a secondary click on the row at `pos` (item `id`): open its context menu. The selection follows first (an unselected row is selected, per platform Policy). |
| `onrow-hovered` | `(pos: number, id: number) => void` | OW5a1b: the pointer entered the row at `pos` (item `id`; Qt `entered`). |
| `onselection-changed` | `() => void` | The user changed the selection: read it with `selection()`. |
| `onsetup` | `(e: SvlEvent<[number]>, row: number) => void` | Row slot `row` was created: give it content with `setRow`. **Sync:** the handler runs before the frame's pass and gets an `SvlEvent` first. |
| `onteardown` | `(e: SvlEvent<[number]>, row: number) => void` | Slot `row` is gone: release its widget. **Sync:** the handler runs before the frame's pass and gets an `SvlEvent` first. |
| `onunbind` | `(e: SvlEvent<[number, number]>, row: number, gen: number) => void` | Slot `row` no longer shows binding `gen`. **Sync:** the handler runs before the frame's pass and gets an `SvlEvent` first. |

`on:activate` works too, and a bare `on:<event>` on a component forwards the component's `on<event>` prop.

Plus the events of [`Widget`](widget.md#events): `onanimationend`, `onclick`, `oncontextmenu`, `ondblclick`, `ondrag-begin`, `ondrag-end`, `ondrag-enter`, `ondrag-leave`, `ondrop`, `onfocusin`, `onfocusout`, `onkeydown`, `onkeyup`, `onpointerdown`, `onpointermove`, `onpointerup`, `onwheel`.
<!-- END GENERATED -->

## Methods

<!-- BEGIN GENERATED: methods of kinds.TreeView (tools/docs/egui/gen_reference.py). Do not edit. -->
| Method | Returns | Description |
|---|---|---|
| `count` | `void` | The number of visible rows. |
| `rowAt` | `void` | `rowAt(pos)`: the slot bound to `pos`, or -1. |
| `scrollTo` | `void` | `scrollTo(pos, hint?)`: scrolls the row into view; `hint`: `nearest` (default), `top`, `bottom`, `center`. |
| `selection` | `void` | The selected ids, hidden ones included. |
| `setNodes` | `void` | `setNodes(ids, levels, flags, labels, data)`: the tree facts of rows by id (flags: 1 expandable, 2 expanded; a level < 0 forgets the id). |
| `setRow` | `void` | `setRow(row, widget)`: the content of row slot `row` (called from `setup`); null clears it. |
| `setSelection` | `void` | `setSelection(ids)`: replaces the selection (single keeps the first); raises nothing. |
| `splice` | `void` | `splice(pos, removed, ids)`: replaces `removed` rows at `pos` with `ids` (the visible rows, flattened). |

Call them through `bind:this`: `el.call('count')`.
<!-- END GENERATED -->

## Behaviour you need to know

<!-- TODO(DOC7): what the tables don't say: timing, edge cases, limits. -->

## CSS

<!-- BEGIN GENERATED: css of kinds.TreeView (tools/docs/egui/gen_reference.py). Do not edit. -->
**Selector:** `tree-view`, plus `.class`, `#id` and attribute selectors on its props.

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

**Layout properties** (as attributes or in CSS; an explicit attribute wins): `align-self`, `bottom`, `dialog-role`, `flex-basis`, `flex-grow`, `flex-shrink`, `height`, `left`, `max-height`, `max-width`, `min-height`, `min-width`, `position`, `right`, `status-section`, `top`, `visible`, `width`.
Also in CSS only: `display`, `grid-column`, `grid-row`, `margin-bottom`, `margin-left`, `margin-right`, `margin-top`.

As `virtual-list` (`::part(row)` per row, `placeholder`, `drop-line`, `ghost`, the bar), plus `:has-children` and `:expanded` on rows and the `::part(twistie)` chevron.

Full matrix: [css/properties](../../css/properties.md). States: below.
<!-- END GENERATED -->

## States

<!-- BEGIN GENERATED: states of kinds.TreeView (tools/docs/egui/gen_reference.py). Do not edit. -->
No built-in state is set on it.

Never set on it (rules with them don't match): `:hover`, `:active`, `:focus`, `:focus-within`, `:disabled`, `:checked`, `:selected`, `:open`, `:expanded`, `:invalid`, `:read-only`, `:dragging`, `:exiting`, `:busy`, `:indeterminate`, `:placeholder-shown`, `:default`, `:drop-target`, `:current`, `:backdrop`, `:required`, `:user-invalid`, `:has-children`.
<!-- END GENERATED -->

## Accessibility

<!-- BEGIN GENERATED: a11y of kinds.TreeView (tools/docs/egui/gen_reference.py). Do not edit. -->
| | |
|---|---|
| Role | `Tree` |
| Name | `accessible-name` |
| Actions | `Focus` |
| `anchor`, `invalid` | not announced on this element (`anchor` still keys its ids) |

Rows are `TreeItem` nodes with their level, `expanded` on expandable rows, `selected`, and their position and set size over the visible rows. The tree is one tab stop; the cursor row is its active descendant.
<!-- END GENERATED -->

## Testing

<!-- TODO(DOC7): how to find and drive it in a kittest harness. -->

## See also

<!-- TODO(DOC7) -->
