---
title: inner-window
tag: inner-window
kind: InnerWindow
category: overlays
description: "A floating window inside the viewport: movable, optionally resizable, collapsible and closable; its children are a flex box."
status: stub
verified_at: ~
generated: partial   # regions between BEGIN/END GENERATED markers come from the spec
sources:
  - metadata/semantics/egui.json#kinds.InnerWindow
---

# `<inner-window>`

<!-- BEGIN GENERATED: summary of kinds.InnerWindow (tools/docs/egui/gen_reference.py). Do not edit. -->
A floating window inside the viewport: movable, optionally resizable, collapsible and closable; its children are a flex box.

**Base:** [`box`](box.md) → [`Widget`](widget.md) · **Children:** any element · **Text content:** no
<!-- END GENERATED -->

<!-- TODO(DOC7): when to use it, and what to use instead. -->

## Example

<!-- TODO(DOC7): a checked example (a testkit case under docs/egui/cases/). -->

## Props

<!-- BEGIN GENERATED: props of kinds.InnerWindow (tools/docs/egui/gen_reference.py). Do not edit. -->
| Prop | Type | Default | `bind:` | Description |
|---|---|---|---|---|
| `closable` | `boolean` | `true` | no | Shows a close button in the title bar. |
| `collapsible` | `boolean` | `true` | no | The title bar can collapse it to the title. |
| `default-height` | `number` | `240` | no | Initial height in points. |
| `default-width` | `number` | `320` | no | Initial width in points (a resizable window's content fills it). |
| `default-x` | `number \| null` | `null` | no | Initial left edge in points (viewport coordinates); null: egui places it. |
| `default-y` | `number \| null` | `null` | no | Initial top edge in points (viewport coordinates); null: egui places it. |
| `open` | `boolean` | `true` | **yes**, on `changed` | Shown when true; the close button sets it false. |
| `resizable` | `boolean` | `true` | no | The user can resize it; the content is laid out at its size. Without it the window is as big as its content. |
| `title` | `string` | `""` | no | The title bar text. |

From [`box`](box.md):

| Prop | Type | Default | `bind:` | Description |
|---|---|---|---|---|
| `align-items` | `'start' \| 'end' \| 'center' \| 'stretch' \| 'baseline'` | `"stretch"` | no | Cross-axis alignment of the children. |
| `column-gap` | `number \| null` | `null` | no | Space between columns in points; null to use `gap`. |
| `flex-direction` | `'column' \| 'row' \| 'column-reverse' \| 'row-reverse'` | `"column"` | no | Main axis of the box. |
| `flex-wrap` | `'nowrap' \| 'wrap' \| 'wrap-reverse'` | `"nowrap"` | no | Whether children wrap onto more lines. |
| `focus-scope` | `'none' \| 'contain' \| 'dialog'` | `"none"` | no | `contain`: Tab and Shift+Tab cycle inside while focus is in it; `dialog`: also takes focus when it appears (`autofocus` node, else the first) and gives it back to the opener when it goes away. |
| `gap` | `number` | `0` | no | Space between children on both axes, in points (row-gap and column-gap override it). |
| `justify-content` | `'start' \| 'end' \| 'center' \| 'space-between' \| 'space-around' \| 'space-evenly'` | `"start"` | no | Main-axis distribution of the children. |
| `padding` | `number` | `0` | no | Inner spacing on all four sides, in points. |
| `row-gap` | `number \| null` | `null` | no | Space between rows in points; null to use `gap`. |

Plus the 26 props every element has, from [`Widget`](widget.md#props): `accessible-name`, `align-self`, `anchor`, `autofocus`, `bottom`, `drag-data`, `drag-type`, `drop-types`, `enabled`, `flex-basis`, `flex-grow`, `flex-shrink`, `height`, `invalid`, `left`, `margin`, `max-height`, `max-width`, `min-height`, `min-width`, `position`, `right`, `tooltip`, `top`, `visible`, `width`.
<!-- END GENERATED -->

## Events

<!-- BEGIN GENERATED: events of kinds.InnerWindow (tools/docs/egui/gen_reference.py). Do not edit. -->
| Attribute | Handler | When |
|---|---|---|
| `onchanged` | `(open: boolean) => void` | The user (or `popup`/`popdown`) opened or closed it; carries `open`. |
| `onclosed` | `() => void` | The user (or `popdown`) closed it. |

`on:changed` works too, and a bare `on:<event>` on a component forwards the component's `on<event>` prop.

Plus the events of [`Widget`](widget.md#events): `onanimationend`, `ondrag-begin`, `ondrag-end`, `ondrag-enter`, `ondrag-leave`, `ondrop`.
<!-- END GENERATED -->

## Methods

<!-- BEGIN GENERATED: methods of kinds.InnerWindow (tools/docs/egui/gen_reference.py). Do not edit. -->
None.
<!-- END GENERATED -->

## Behaviour you need to know

<!-- TODO(DOC7): what the tables don't say: timing, edge cases, limits. -->

## CSS

<!-- BEGIN GENERATED: css of kinds.InnerWindow (tools/docs/egui/gen_reference.py). Do not edit. -->
**Selector:** `inner-window`, plus `.class`, `#id` and attribute selectors on its props.

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

**Layout properties** (as attributes or in CSS; an explicit attribute wins): `align-items`, `align-self`, `bottom`, `column-gap`, `flex-basis`, `flex-direction`, `flex-grow`, `flex-shrink`, `flex-wrap`, `gap`, `height`, `justify-content`, `left`, `margin`, `max-height`, `max-width`, `min-height`, `min-width`, `padding`, `position`, `right`, `row-gap`, `top`, `visible`, `width`.
Also in CSS only: `display`, `grid-column`, `grid-row`, `grid-template-columns`, `grid-template-rows`, `margin-bottom`, `margin-left`, `margin-right`, `margin-top`, `padding-bottom`, `padding-left`, `padding-right`, `padding-top`.

The window frame follows the theme; style its content through a child `box`.

Full matrix: [css/properties](../../css/properties.md). States: below.
<!-- END GENERATED -->

## States

<!-- BEGIN GENERATED: states of kinds.InnerWindow (tools/docs/egui/gen_reference.py). Do not edit. -->
No built-in state is set on it.

Never set on it (rules with them don't match): `:hover`, `:active`, `:focus`, `:focus-within`, `:disabled`, `:checked`, `:selected`, `:open`, `:expanded`, `:invalid`, `:read-only`, `:dragging`, `:exiting`, `:busy`.
<!-- END GENERATED -->

## Accessibility

<!-- BEGIN GENERATED: a11y of kinds.InnerWindow (tools/docs/egui/gen_reference.py). Do not edit. -->
| | |
|---|---|
| Role | `Window` |
| Name | `title` |
| `anchor`, `invalid` | not announced on this element (`anchor` still keys its ids) |

Its title bar's collapse and close buttons are `Button` nodes.
<!-- END GENERATED -->

## Testing

<!-- TODO(DOC7): how to find and drive it in a kittest harness. -->

## See also

<!-- TODO(DOC7) -->
