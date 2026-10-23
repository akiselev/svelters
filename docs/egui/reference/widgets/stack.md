---
title: stack
tag: stack
kind: Stack
category: layout
description: "Shows one of its children (GTK's `stack`, Qt's `stacked-widget`): the one anchored `visible-child-name`, else the one at `visible-index`."
status: stub
verified_at: ~
generated: partial   # regions between BEGIN/END GENERATED markers come from the spec
sources:
  - metadata/semantics/egui.json#kinds.Stack
---

# `<stack>`

<!-- BEGIN GENERATED: summary of kinds.Stack (tools/docs/egui/gen_reference.py). Do not edit. -->
Shows one of its children (GTK's `stack`, Qt's `stacked-widget`): the one anchored `visible-child-name`, else the one at `visible-index`. The others are neither drawn nor hit.

**Base:** [`Widget`](widget.md) · **Children:** any element · **Text content:** no
<!-- END GENERATED -->

<!-- TODO(DOC7): when to use it, and what to use instead. -->

## Example

<!-- TODO(DOC7): a checked example (a testkit case under docs/egui/cases/). -->

## Props

<!-- BEGIN GENERATED: props of kinds.Stack (tools/docs/egui/gen_reference.py). Do not edit. -->
| Prop | Type | Default | `bind:` | Description |
|---|---|---|---|---|
| `homogeneous` | `boolean` | `true` | no | Sized to the largest child (GTK's `hhomogeneous`/`vhomogeneous`); false: to the visible child. |
| `padding` | `number` | `0` | no | Inner spacing on all four sides, in points. |
| `visible-child-name` | `string` | `""` | no | Shows the child whose `anchor` is this name (GTK's `visible-child-name`; the child names itself with `anchor`). Empty: `visible-index` decides. |
| `visible-index` | `number` | `0` | no | Shows the child at this index when `visible-child-name` is empty or names no child (Qt's `current-index`). |

Plus the 29 props every element has, from [`Widget`](widget.md#props): `accessible-name`, `align-self`, `anchor`, `autofocus`, `bottom`, `dialog-role`, `drag-data`, `drag-type`, `drop-types`, `enabled`, `flex-basis`, `flex-grow`, `flex-shrink`, `height`, `invalid`, `left`, `margin`, `max-height`, `max-width`, `min-height`, `min-width`, `position`, `right`, `status-section`, `status-tip`, `tooltip`, `top`, `visible`, `width`.
<!-- END GENERATED -->

## Events

<!-- BEGIN GENERATED: events of kinds.Stack (tools/docs/egui/gen_reference.py). Do not edit. -->
No events of its own.

Plus the events of [`Widget`](widget.md#events): `onanimationend`, `onclick`, `oncontextmenu`, `ondblclick`, `ondrag-begin`, `ondrag-end`, `ondrag-enter`, `ondrag-leave`, `ondrop`, `onfocusin`, `onfocusout`, `onkeydown`, `onkeyup`, `onpointerdown`, `onpointerup`, `onwheel`.
<!-- END GENERATED -->

## Methods

<!-- BEGIN GENERATED: methods of kinds.Stack (tools/docs/egui/gen_reference.py). Do not edit. -->
None.
<!-- END GENERATED -->

## Behaviour you need to know

<!-- TODO(DOC7): what the tables don't say: timing, edge cases, limits. -->

## CSS

<!-- BEGIN GENERATED: css of kinds.Stack (tools/docs/egui/gen_reference.py). Do not edit. -->
**Selector:** `stack`, plus `.class`, `#id` and attribute selectors on its props.

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

Full matrix: [css/properties](../../css/properties.md). States: below.
<!-- END GENERATED -->

## States

<!-- BEGIN GENERATED: states of kinds.Stack (tools/docs/egui/gen_reference.py). Do not edit. -->
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

<!-- BEGIN GENERATED: a11y of kinds.Stack (tools/docs/egui/gen_reference.py). Do not edit. -->
| | |
|---|---|
| Role | none of its own; its content is announced in place |
| `anchor`, `invalid` | not announced on this element (`anchor` still keys its ids) |
<!-- END GENERATED -->

## Testing

<!-- TODO(DOC7): how to find and drive it in a kittest harness. -->

## See also

<!-- TODO(DOC7) -->
