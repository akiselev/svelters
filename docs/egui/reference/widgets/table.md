---
title: table
tag: table
kind: Table
category: views
description: "svelters' own virtual table: only the visible rows and columns are drawn; rows from a JS window or a Rust row model."
status: stub
verified_at: ~
generated: partial   # regions between BEGIN/END GENERATED markers come from the spec
sources:
  - metadata/semantics/egui.json#kinds.Table
---

# `<table>`

<!-- BEGIN GENERATED: summary of kinds.Table (tools/docs/egui/gen_reference.py). Do not edit. -->
A virtual table, svelters' own (OW5b): only the visible rows and columns are drawn, under a frozen header; rows, cells, header cells, sort arrows and resize edges are CSS parts. Its children are `table-column`s; its rows are a JS data window (`range` asks, `set-rows` answers), keyed by `set-ids`, or a Rust row model. Keyboard navigation, type-ahead, keyed selection and cell editing (WG5a). Give it a size or let flex-grow size it.

**Base:** [`Widget`](widget.md) · **Children:** [`table-column`](table-column.md) · **Text content:** no
<!-- END GENERATED -->

<!-- TODO(DOC7): when to use it, and what to use instead. -->

## Example

<!-- TODO(DOC7): a checked example (a testkit case under docs/egui/cases/). -->

## Props

<!-- BEGIN GENERATED: props of kinds.Table (tools/docs/egui/gen_reference.py). Do not edit. -->
| Prop | Type | Default | `bind:` | Description |
|---|---|---|---|---|
| `column-menu` | `boolean` | `true` | no | A right click on the header opens a menu that hides and shows the `hideable` columns (after `header-context`, unless prevented). |
| `current` | `number` | `-1` | **yes**, on `changed` | The cursor row's id (keyboard focus inside the table), -1 for none. |
| `current-column` | `number` | `0` | **yes**, on `changed` | The cursor column (Left/Right move it; F2 and typing edit that cell). |
| `header-state` | `string` | `""` | **yes**, on `changed` | The header as JSON (Qt `saveState`): `{"widths":{name:points},"hidden":[name],"sort":{"column":name,"order":…}}`. The table writes it after the user resizes, hides or sorts; setting it restores them. Persist it with DX1's `uiState()` (`bind:header-state={ui.header}`): the table itself never writes files. |
| `hidden-columns` | `string[]` | `null` | **yes**, on `changed` | The hidden columns, by `name` (a column without one by its child index, `"2"`). The column menu changes it. |
| `overscan` | `number` | `20` | no | Rows beyond the visible ones that `range` asks for on each side (data ready before it scrolls into view). |
| `row-count` | `number` | `0` | no | Number of rows (the scroll extent). Rows without data yet are drawn empty. |
| `row-height` | `number` | `20` | no | Height of every row (and of the header row), in points. |
| `selected` | `number[]` | `null` | **yes**, on `changed` | The selected row ids (`bind:selected`); clicks (Ctrl/Shift with `selection="multi"`) and the keyboard change it. Kept by id across `set-ids` (sorts, filters). |
| `selection` | `'none' \| 'single' \| 'multi'` | `"single"` | no | `single`, `multi` (Ctrl/Shift clicks and keys, ranges) or `none` (a cursor only). |
| `sort-column` | `number` | `-1` | **yes**, on `changed` | The column (child index) the header shows sorted, -1 for none: then each column's own `sort` shows. |
| `sort-order` | `'none' \| 'ascending' \| 'descending'` | `"none"` | **yes**, on `changed` | The order shown on `sort-column`. |
| `sortable` | `boolean` | `false` | no | A header click sorts on that column: `sort-column` becomes it, `sort-order` ascending, or flips when it already was (the rows are still sorted by JS, on `sort-changed`). |
| `sticky-columns` | `number` | `0` | no | Number of leading columns that do not scroll horizontally. |
| `stretch-last` | `boolean` | `true` | no | The last shown column fills the width the columns leave (Qt `stretchLastSection`). |
| `striped` | `boolean` | `true` | no | Alternate rows get a faint background. |

Plus the 31 props every element has, from [`Widget`](widget.md#props): `accessible-name`, `align-self`, `anchor`, `anchor-scope`, `autofocus`, `bottom`, `dialog-role`, `drag-data`, `drag-type`, `drop-types`, `enabled`, `flex-basis`, `flex-grow`, `flex-shrink`, `height`, `invalid`, `left`, `margin`, `max-height`, `max-width`, `min-height`, `min-width`, `position`, `right`, `source-loc`, `status-section`, `status-tip`, `tooltip`, `top`, `visible`, `width`.
<!-- END GENERATED -->

## Events

<!-- BEGIN GENERATED: events of kinds.Table (tools/docs/egui/gen_reference.py). Do not edit. -->
| Attribute | Handler | When |
|---|---|---|
| `onactivated` | `(row: number) => void` | A row was double-clicked (a cell of a column that is not `editable`) or Enter pressed on the cursor row; carries its position. |
| `onchanged` | `(selected: unknown[]) => void` | The user changed the selection; carries the selected row ids (`bind:selected`). |
| `oncolumn-resized` | `(column: number, width: number) => void` | The user resized a column (drag released, a double-click fit, `fit-columns`); carries the column and its width. The column's `width` is bindable. |
| `oncolumns-changed` | `(hidden_columns: unknown[]) => void` | The column menu hid or showed a column; carries the hidden columns. |
| `oncommit` | `(e: SvlEvent<[number, number, unknown]>, row: number, column: number, value: unknown) => void` | An edit is committed: row id, column index, the editor's value. Accept by returning nothing; reject with `e.result = 'reason'` (shown under the cell) or `e.preventDefault()`: the cell reverts. **Sync:** the handler runs before the frame's pass and gets an `SvlEvent` first. |
| `oncopy` | `(ids: unknown[]) => void` | Ctrl+C (Cmd+C) on the focused table; carries the selected row ids (the app copies the data it holds). |
| `onheader-clicked` | `(column: number) => void` | A column header was clicked (sort on it in JS and set the column's `sort`). |
| `onheader-context` | `(column: number) => void` | A right click on the header (Menu key on the focused table); carries the column under the pointer. Unless prevented, `column-menu` opens. |
| `onheader-double-clicked` | `(column: number) => void` | A column header was double-clicked (not its resize edge); carries the column (child index). |
| `onheader-state-changed` | `(header_state: string) => void` | The user resized, hid or sorted; carries the new `header-state` (save it). |
| `onrange` | `(first: number, last: number) => void` | The rows the table needs data for changed: `first..=last` (the visible rows plus `overscan`); answer with `set-rows`. |
| `onrow-toggled` | `(id: number, expanded: boolean) => void` | A tree row of a Rust model was expanded or collapsed (its expander, Right/Left); carries its id and whether it is expanded now. |
| `onselectionchange` | `(ids: unknown[]) => void` | The user changed the selection; carries the selected row ids. |
| `onsort-changed` | `(sort_column: number, sort_order: string) => void` | A header click on a `sortable` table changed the sort; carries the column and the order (sort the rows in JS; a Rust model is sorted by the table). |

`on:activated` works too, and a bare `on:<event>` on a component forwards the component's `on<event>` prop.

Plus the events of [`Widget`](widget.md#events): `onanimationend`, `onclick`, `oncontextmenu`, `ondblclick`, `ondrag-begin`, `ondrag-end`, `ondrag-enter`, `ondrag-leave`, `ondrop`, `onfocusin`, `onfocusout`, `onkeydown`, `onkeyup`, `onpointerdown`, `onpointermove`, `onpointerup`, `onwheel`.
<!-- END GENERATED -->

## Methods

<!-- BEGIN GENERATED: methods of kinds.Table (tools/docs/egui/gen_reference.py). Do not edit. -->
| Method | Returns | Description |
|---|---|---|
| `clear-rows` | `void` | Drops the data window (every row draws empty until `set-rows`). |
| `edit` | `void` | `edit(rowId, column)`: starts editing that cell (its column must be `editable`). |
| `end-edit` | `void` | `end-edit(commit)`: ends the open edit; true commits (`commit`), false reverts. |
| `fit-columns` | `void` | `fit-columns(column?)`: sizes that column (child index), or every resizable shown column, to its header and the cells in the data window (Qt `resizeColumnsToContents`); `column-resized` for each. |
| `scroll-to-row` | `void` | `scroll-to-row(row)`: scrolls the row into view on the next frame. |
| `set-ids` | `void` | `set-ids(ids, labels?)`: the id of every row by position (and the text type-ahead matches); the row count is then `ids.length`. Selection, cursor and edits follow ids across sorts and filters. null: ids are positions again. |
| `set-rows` | `void` | `set-rows(first, rows)`: the data window, rows `first..first + rows.length`, each row a list of cell values (strings, numbers, booleans; null for empty). Replaces the previous window. |

Call them through `bind:this`: `el.call('clear-rows')`.
<!-- END GENERATED -->

## Behaviour you need to know

### Rows from a Rust model (LS2)

A table can read its rows from a Rust `RowModel` instead of the JS window: `SceneMut::set_table_model`
(see [Rust row models](../../rust/row-models.md)). Then:

- no `range` event fires, and `set-rows`, `set-ids` and `clear-rows` are not used;
- `sort-column`/`sort-order` (and header clicks on a `sortable` table) sort the rows in Rust;
- a filter set on the model's `RowView` hides rows; selection stays keyed by the model's row keys;
- tree rows (`RowModel::depth`) are indented in the first shown column, with a chevron
  (`::part(expander)`, `:checked` while expanded). A click on it, or Right/Left on the first column,
  expands and collapses the row and fires `row-toggled(id, expanded)`;
- a table without `table-column` children gets the model's columns.

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

**Layout properties** (as attributes or in CSS; an explicit attribute wins): `align-self`, `bottom`, `dialog-role`, `flex-basis`, `flex-grow`, `flex-shrink`, `height`, `left`, `max-height`, `max-width`, `min-height`, `min-width`, `position`, `right`, `status-section`, `top`, `visible`, `width`.
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
