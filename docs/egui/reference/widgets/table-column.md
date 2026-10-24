---
title: table-column
tag: table-column
kind: TableColumn
category: views
description: "A column of its parent `table` (data only: not laid out or drawn as a node)."
status: stub
verified_at: ~
generated: partial   # regions between BEGIN/END GENERATED markers come from the spec
sources:
  - metadata/semantics/egui.json#kinds.TableColumn
---

# `<table-column>`

<!-- BEGIN GENERATED: summary of kinds.TableColumn (tools/docs/egui/gen_reference.py). Do not edit. -->
A column of its parent `table` (data only: not laid out or drawn as a node). Its `width` (Widget's, in points; null for 100; `min-width`/`max-width` clamp it) is the column's width; a user resize writes it (`bind:width`). `visible` false hides it; `tooltip` shows on its header; `table-column::part(header)` styles its header cell.

**Base:** [`Widget`](widget.md) · **Children:** none · **Text content:** no
<!-- END GENERATED -->

<!-- TODO(DOC7): when to use it, and what to use instead. -->

## Example

<!-- TODO(DOC7): a checked example (a testkit case under docs/egui/cases/). -->

## Props

<!-- BEGIN GENERATED: props of kinds.TableColumn (tools/docs/egui/gen_reference.py). Do not edit. -->
| Prop | Type | Default | `bind:` | Description |
|---|---|---|---|---|
| `align` | `'start' \| 'center' \| 'end'` | `"start"` | no | Horizontal alignment of its cells. |
| `editable` | `boolean` | `false` | no | Its cells can be edited (F2, typing, double click); the editor is `editor`, else a text field. |
| `editor` | `unknown` | `false` | no | The cell editor, a snippet taking the edit (`{ row, column, value (bindable), typed, commit(), cancel() }`); realised once and rebound per edit. |
| `elide` | `'end' \| 'middle' \| 'start' \| 'none'` | `"end"` | no | Where text too wide for a cell or the header is cut with `…` (`none`: clipped). |
| `header-align` | `'auto' \| 'start' \| 'center' \| 'end'` | `"auto"` | no | Alignment of its header text (`auto`: its `align`). |
| `hideable` | `boolean` | `true` | no | The column menu lists it (it can be hidden). |
| `name` | `string` | `""` | no | Its name in `hidden-columns` and `header-state` (empty: its child index). |
| `resizable` | `boolean` | `true` | no | Its right edge can be dragged (double-click: fit the content). |
| `sort` | `'none' \| 'ascending' \| 'descending'` | `"none"` | no | The sort arrow its header shows (the data is sorted by JS) while the table's `sort-column` is -1. |
| `title` | `string` | `""` | no | The header text. |

Plus the 30 props every element has, from [`Widget`](widget.md#props): `accessible-name`, `align-self`, `anchor`, `anchor-scope`, `autofocus`, `bottom`, `dialog-role`, `drag-data`, `drag-type`, `drop-types`, `enabled`, `flex-basis`, `flex-grow`, `flex-shrink`, `height`, `invalid`, `left`, `margin`, `max-height`, `max-width`, `min-height`, `min-width`, `position`, `right`, `status-section`, `status-tip`, `tooltip`, `top`, `visible`, `width`.
<!-- END GENERATED -->

## Events

<!-- BEGIN GENERATED: events of kinds.TableColumn (tools/docs/egui/gen_reference.py). Do not edit. -->
| Attribute | Handler | When |
|---|---|---|
| `oneditor-bind` | `(e: SvlEvent<[number, string, string]>, row: number, value: string, typed: string) => void` | Runtime protocol: an edit of this column's cell starts (the editor snippet's outlet is `set-editor`). **Sync:** the handler runs before the frame's pass and gets an `SvlEvent` first. |
| `oneditor-unbind` | `(e: SvlEvent<[]>) => void` | Runtime protocol: the edit ended; the editor stays realised for the next one. **Sync:** the handler runs before the frame's pass and gets an `SvlEvent` first. |
| `oneditor-value` | `(e: SvlEvent<[]>) => void` | Runtime protocol: the editor's value to commit (the handler sets `e.result`). **Sync:** the handler runs before the frame's pass and gets an `SvlEvent` first. |

`on:editor-bind` works too, and a bare `on:<event>` on a component forwards the component's `on<event>` prop.

Plus the events of [`Widget`](widget.md#events): `onanimationend`, `onclick`, `oncontextmenu`, `ondblclick`, `ondrag-begin`, `ondrag-end`, `ondrag-enter`, `ondrag-leave`, `ondrop`, `onfocusin`, `onfocusout`, `onkeydown`, `onkeyup`, `onpointerdown`, `onpointermove`, `onpointerup`, `onwheel`.
<!-- END GENERATED -->

## Methods

<!-- BEGIN GENERATED: methods of kinds.TableColumn (tools/docs/egui/gen_reference.py). Do not edit. -->
| Method | Returns | Description |
|---|---|---|
| `end-edit` | `void` | `end-edit(commit)`: ends this column's open edit; true commits, false reverts. |
| `set-editor` | `void` | Runtime protocol: `set-editor(outlet)`, the realised editor snippet of this column (null drops it). |

Call them through `bind:this`: `el.call('end-edit')`.
<!-- END GENERATED -->

## Behaviour you need to know

<!-- TODO(DOC7): what the tables don't say: timing, edge cases, limits. -->

## CSS

<!-- BEGIN GENERATED: css of kinds.TableColumn (tools/docs/egui/gen_reference.py). Do not edit. -->
**Selector:** `table-column`, plus `.class`, `#id` and attribute selectors on its props.

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

Data only: the table draws it.

Full matrix: [css/properties](../../css/properties.md). States: below.
<!-- END GENERATED -->

## States

<!-- BEGIN GENERATED: states of kinds.TableColumn (tools/docs/egui/gen_reference.py). Do not edit. -->
No built-in state is set on it.

Never set on it (rules with them don't match): `:hover`, `:active`, `:focus`, `:focus-within`, `:disabled`, `:checked`, `:selected`, `:open`, `:expanded`, `:invalid`, `:read-only`, `:dragging`, `:exiting`, `:busy`, `:indeterminate`, `:placeholder-shown`, `:default`, `:drop-target`, `:current`, `:backdrop`, `:required`, `:user-invalid`, `:has-children`.
<!-- END GENERATED -->

## Accessibility

<!-- BEGIN GENERATED: a11y of kinds.TableColumn (tools/docs/egui/gen_reference.py). Do not edit. -->
| | |
|---|---|
| Role | `ColumnHeader` |
| Name | `title` |
| Value | the sort direction (`sort`) |
| `anchor`, `invalid` | not announced on this element (`anchor` still keys its ids) |

The table draws the header; the column has no node of its own.
<!-- END GENERATED -->

## Testing

<!-- TODO(DOC7): how to find and drive it in a kittest harness. -->

## See also

<!-- TODO(DOC7) -->
