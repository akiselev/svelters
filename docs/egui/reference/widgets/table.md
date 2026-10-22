---
title: table
tag: table
kind: Table
category: views
description: "A virtual table (`egui_table`): only the visible rows and columns are drawn."
status: stub
verified_at: ~
generated: partial   # regions between BEGIN/END GENERATED markers come from the spec
sources:
  - metadata/semantics/egui.json#kinds.Table
---

# `<table>`

<!-- BEGIN GENERATED: summary of kinds.Table (tools/docs/egui/gen_reference.py). Do not edit. -->
A virtual table (`egui_table`): only the visible rows and columns are drawn. Its children are `table-column`s; its rows are a JS data window (`range` asks, `set-rows` answers), keyed by `set-ids`. Keyboard navigation, type-ahead, keyed selection and cell editing (WG5a). Give it a size or let flex-grow size it.

**Base:** [`Widget`](widget.md) · **Children:** [`table-column`](table-column.md) · **Text content:** no
<!-- END GENERATED -->

<!-- TODO(DOC7): when to use it, and what to use instead. -->

## Example

<!-- TODO(DOC7): a checked example (a testkit case under docs/egui/cases/). -->

## Props

<!-- BEGIN GENERATED: props of kinds.Table (tools/docs/egui/gen_reference.py). Do not edit. -->
| Prop | Type | Default | `bind:` | Description |
|---|---|---|---|---|
| `current` | `number` | `-1` | **yes**, on `changed` | The cursor row's id (keyboard focus inside the table), -1 for none. |
| `current-column` | `number` | `0` | **yes**, on `changed` | The cursor column (Left/Right move it; F2 and typing edit that cell). |
| `overscan` | `number` | `20` | no | Rows beyond the visible ones that `range` asks for on each side (data ready before it scrolls into view). |
| `row-count` | `number` | `0` | no | Number of rows (the scroll extent). Rows without data yet are drawn empty. |
| `row-height` | `number` | `20` | no | Height of every row (and of the header row), in points. |
| `selected` | `number[]` | `null` | **yes**, on `changed` | The selected row ids (`bind:selected`); clicks (Ctrl/Shift with `selection="multi"`) and the keyboard change it. Kept by id across `set-ids` (sorts, filters). |
| `selection` | `'none' \| 'single' \| 'multi'` | `"single"` | no | `single`, `multi` (Ctrl/Shift clicks and keys, ranges) or `none` (a cursor only). |
| `sticky-columns` | `number` | `0` | no | Number of leading columns that do not scroll horizontally. |
| `striped` | `boolean` | `true` | no | Alternate rows get a faint background. |

Plus the 26 props every element has, from [`Widget`](widget.md#props): `accessible-name`, `align-self`, `anchor`, `autofocus`, `bottom`, `drag-data`, `drag-type`, `drop-types`, `enabled`, `flex-basis`, `flex-grow`, `flex-shrink`, `height`, `invalid`, `left`, `margin`, `max-height`, `max-width`, `min-height`, `min-width`, `position`, `right`, `tooltip`, `top`, `visible`, `width`.
<!-- END GENERATED -->

## Events

<!-- BEGIN GENERATED: events of kinds.Table (tools/docs/egui/gen_reference.py). Do not edit. -->
| Attribute | Handler | When |
|---|---|---|
| `onactivated` | `(row: number) => void` | A row was double-clicked (a cell of a column that is not `editable`) or Enter pressed on the cursor row; carries its position. |
| `onchanged` | `(selected: unknown[]) => void` | The user changed the selection; carries the selected row ids (`bind:selected`). |
| `oncommit` | `(e: SvlEvent<[number, number, unknown]>, row: number, column: number, value: unknown) => void` | An edit is committed: row id, column index, the editor's value. Accept by returning nothing; reject with `e.result = 'reason'` (shown under the cell) or `e.preventDefault()`: the cell reverts. **Sync:** the handler runs before the frame's pass and gets an `SvlEvent` first. |
| `onheader-clicked` | `(column: number) => void` | A column header was clicked (sort on it in JS and set the column's `sort`). |
| `onrange` | `(first: number, last: number) => void` | The rows the table needs data for changed: `first..=last` (the visible rows plus `overscan`); answer with `set-rows`. |
| `onselectionchange` | `(ids: unknown[]) => void` | The user changed the selection; carries the selected row ids. |

`on:activated` works too, and a bare `on:<event>` on a component forwards the component's `on<event>` prop.

Plus the events of [`Widget`](widget.md#events): `onanimationend`, `onclick`, `oncontextmenu`, `ondblclick`, `ondrag-begin`, `ondrag-end`, `ondrag-enter`, `ondrag-leave`, `ondrop`, `onfocusin`, `onfocusout`, `onkeydown`, `onkeyup`, `onpointerdown`, `onpointerup`, `onwheel`.
<!-- END GENERATED -->

## Methods

<!-- BEGIN GENERATED: methods of kinds.Table (tools/docs/egui/gen_reference.py). Do not edit. -->
| Method | Returns | Description |
|---|---|---|
| `clear-rows` | `void` | Drops the data window (every row draws empty until `set-rows`). |
| `edit` | `void` | `edit(rowId, column)`: starts editing that cell (its column must be `editable`). |
| `end-edit` | `void` | `end-edit(commit)`: ends the open edit; true commits (`commit`), false reverts. |
| `scroll-to-row` | `void` | `scroll-to-row(row)`: scrolls the row into view on the next frame. |
| `set-ids` | `void` | `set-ids(ids, labels?)`: the id of every row by position (and the text type-ahead matches); the row count is then `ids.length`. Selection, cursor and edits follow ids across sorts and filters. null: ids are positions again. |
| `set-rows` | `void` | `set-rows(first, rows)`: the data window, rows `first..first + rows.length`, each row a list of cell values (strings, numbers, booleans; null for empty). Replaces the previous window. |

Call them through `bind:this`: `el.call('clear-rows')`.
<!-- END GENERATED -->

## Behaviour you need to know

<!-- TODO(DOC7): what the tables don't say: timing, edge cases, limits. -->

## CSS

<!-- BEGIN GENERATED: css of kinds.Table (tools/docs/egui/gen_reference.py). Do not edit. -->
**Selector:** `table`, plus `.class`, `#id` and attribute selectors on its props.

**Painted as:** a box. The CSS engine paints its shadows, background and border at its rect, under its content; `opacity` fades it and its content.

| Property | Applies | Theme default |
|---|---|---|
| `background-color` | yes | — |
| `border-color` | yes | — |
| `border-width` | yes | — |
| `border-radius` | yes | — |
| `color` | inherited by its children | — |
| `box-shadow` | yes (outer and inset) | — |
| `opacity` | yes | — |

**Layout properties** (as attributes or in CSS; an explicit attribute wins): `align-self`, `bottom`, `flex-basis`, `flex-grow`, `flex-shrink`, `height`, `left`, `margin`, `max-height`, `max-width`, `min-height`, `min-width`, `position`, `right`, `top`, `visible`, `width`.
Also in CSS only: `display`, `grid-column`, `grid-row`, `margin-bottom`, `margin-left`, `margin-right`, `margin-top`.

Rows, cells and headers follow the theme; the box paints under the whole table.

Full matrix: [css/properties](../../css/properties.md). States: below.
<!-- END GENERATED -->

## States

<!-- BEGIN GENERATED: states of kinds.Table (tools/docs/egui/gen_reference.py). Do not edit. -->
| Pseudo-class | Set when | Source |
|---|---|---|
| `:hover` | the pointer is over its box | response |
| `:active` | the primary button is down over its box | response |
| `:focus-within` | an element inside it has keyboard focus | scene |
| `:disabled` (`:enabled` is the opposite) | `enabled` is false | prop |
| `:exiting` | it plays an out transition before it is removed | scene |

Never set on it (rules with them don't match): `:focus`, `:checked`, `:selected`, `:open`, `:expanded`, `:invalid`, `:read-only`, `:dragging`, `:busy`, `:indeterminate`, `:placeholder-shown`, `:default`, `:drop-target`, `:current`, `:backdrop`, `:required`, `:user-invalid`, `:has-children`.

States are tracked once the scene has a style input (a `<style>` block, `class`, `style=` or a theme).
<!-- END GENERATED -->

## Accessibility

<!-- BEGIN GENERATED: a11y of kinds.Table (tools/docs/egui/gen_reference.py). Do not edit. -->
| | |
|---|---|
| Role | `Table` |
| Name | `accessible-name` |
| Value | its row and column counts |
| Actions | `Focus` |
| `invalid` | the `invalid` prop sets AccessKit `invalid` |
| Author id | `anchor` (unique in its root) |

Its keyboard part is a `Grid` node whose active descendant is the cursor cell; rows are `Row` nodes and cells `Cell` nodes. Multi-row selection marks it multiselectable.
<!-- END GENERATED -->

## Testing

<!-- TODO(DOC7): how to find and drive it in a kittest harness. -->

## See also

<!-- TODO(DOC7) -->
