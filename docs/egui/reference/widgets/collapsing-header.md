---
title: collapsing-header
tag: collapsing-header
kind: CollapsingHeader
category: containers
description: "A clickable header with a caret; its children (a flex box, indented) are shown and laid out only while `open`."
status: stub
verified_at: ~
generated: partial   # regions between BEGIN/END GENERATED markers come from the spec
sources:
  - metadata/semantics/egui.json#kinds.CollapsingHeader
---

# `<collapsing-header>`

<!-- BEGIN GENERATED: summary of kinds.CollapsingHeader (tools/docs/egui/gen_reference.py). Do not edit. -->
A clickable header with a caret; its children (a flex box, indented) are shown and laid out only while `open`.

**Base:** [`box`](box.md) → [`Widget`](widget.md) · **Children:** any element · **Text content:** sets `text`
<!-- END GENERATED -->

<!-- TODO(DOC7): when to use it, and what to use instead. -->

## Example

<!-- TODO(DOC7): a checked example (a testkit case under docs/egui/cases/). -->

## Props

<!-- BEGIN GENERATED: props of kinds.CollapsingHeader (tools/docs/egui/gen_reference.py). Do not edit. -->
| Prop | Type | Default | `bind:` | Description |
|---|---|---|---|---|
| `accordion` | `string` | `""` | no | The group of sibling headers of which one is open at a time (QToolBox); empty: none. |
| `icon` | `string` | `""` | no | An icon before the caption (a key, or `family:name`). |
| `open` | `boolean` | `false` | **yes**, on `changed` | Shows the children; a click on the header (or Enter/Space on it) toggles it. |
| `text` | `string` | `""` | no | The header's caption (also its accessible name). |

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

Plus the 30 props every element has, from [`Widget`](widget.md#props): `accessible-name`, `align-self`, `anchor`, `anchor-scope`, `autofocus`, `bottom`, `dialog-role`, `drag-data`, `drag-type`, `drop-types`, `enabled`, `flex-basis`, `flex-grow`, `flex-shrink`, `height`, `invalid`, `left`, `margin`, `max-height`, `max-width`, `min-height`, `min-width`, `position`, `right`, `status-section`, `status-tip`, `tooltip`, `top`, `visible`, `width`.
<!-- END GENERATED -->

## Events

<!-- BEGIN GENERATED: events of kinds.CollapsingHeader (tools/docs/egui/gen_reference.py). Do not edit. -->
| Attribute | Handler | When |
|---|---|---|
| `onchanged` | `(open: boolean) => void` | The user opened or closed it; carries the new state. |

`on:changed` works too, and a bare `on:<event>` on a component forwards the component's `on<event>` prop.

Plus the events of [`Widget`](widget.md#events): `onanimationend`, `onclick`, `oncontextmenu`, `ondblclick`, `ondrag-begin`, `ondrag-end`, `ondrag-enter`, `ondrag-leave`, `ondrop`, `onfocusin`, `onfocusout`, `onkeydown`, `onkeyup`, `onpointerdown`, `onpointermove`, `onpointerup`, `onwheel`.
<!-- END GENERATED -->

## Methods

<!-- BEGIN GENERATED: methods of kinds.CollapsingHeader (tools/docs/egui/gen_reference.py). Do not edit. -->
None.
<!-- END GENERATED -->

## Behaviour you need to know

<!-- TODO(DOC7): what the tables don't say: timing, edge cases, limits. -->

## CSS

<!-- BEGIN GENERATED: css of kinds.CollapsingHeader (tools/docs/egui/gen_reference.py). Do not edit. -->
**Selector:** `collapsing-header`, plus `.class`, `#id` and attribute selectors on its props.

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

**Layout properties** (as attributes or in CSS; an explicit attribute wins): `align-items`, `align-self`, `bottom`, `column-gap`, `dialog-role`, `flex-basis`, `flex-direction`, `flex-grow`, `flex-shrink`, `flex-wrap`, `gap`, `height`, `justify-content`, `left`, `max-height`, `max-width`, `min-height`, `min-width`, `position`, `right`, `row-gap`, `status-section`, `top`, `visible`, `width`.
Also in CSS only: `display`, `grid-column`, `grid-row`, `margin-bottom`, `margin-left`, `margin-right`, `margin-top`.

`:open` and `:expanded` do not match it yet: use `class:open={open}` with `bind:open`.

Full matrix: [css/properties](../../css/properties.md). States: below.
<!-- END GENERATED -->

## States

<!-- BEGIN GENERATED: states of kinds.CollapsingHeader (tools/docs/egui/gen_reference.py). Do not edit. -->
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

<!-- BEGIN GENERATED: a11y of kinds.CollapsingHeader (tools/docs/egui/gen_reference.py). Do not edit. -->
| | |
|---|---|
| Role | `DisclosureTriangle` |
| Name | `accessible-name`, else `text` |
| Value | expanded while `open` |
| Actions | `Click`, `Focus` |
| `invalid` | the `invalid` prop sets AccessKit `invalid` |
| Author id | `anchor` (unique in its root) |

The node is the header row; the body's children are announced in place while it is open.
<!-- END GENERATED -->

## Testing

<!-- TODO(DOC7): how to find and drive it in a kittest harness. -->

## See also

<!-- TODO(DOC7) -->
