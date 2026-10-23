---
title: label
tag: label
kind: Label
category: display
description: "Static text."
status: stub
verified_at: ~
generated: partial   # regions between BEGIN/END GENERATED markers come from the spec
sources:
  - metadata/semantics/egui.json#kinds.Label
---

# `<label>`

<!-- BEGIN GENERATED: summary of kinds.Label (tools/docs/egui/gen_reference.py). Do not edit. -->
Static text; with inline run children (`<b>`, `<i>`, `<code>`, `<a>`, `<span>`) one paragraph of styled runs (RT2).

**Base:** [`Widget`](widget.md) · **Children:** [`span`](span.md), [`b`](b.md), [`i`](i.md), [`code`](code.md), [`a`](a.md) · **Text content:** sets `text`
<!-- END GENERATED -->

<!-- TODO(DOC7): when to use it, and what to use instead. -->

## Example

<!-- TODO(DOC7): a checked example (a testkit case under docs/egui/cases/). -->

## Props

<!-- BEGIN GENERATED: props of kinds.Label (tools/docs/egui/gen_reference.py). Do not edit. -->
| Prop | Type | Default | `bind:` | Description |
|---|---|---|---|---|
| `elide` | `'none' \| 'start' \| 'middle' \| 'end'` | `"none"` | no | WG6 QW13: cut one line with `…` to the width: `none`, `start`, `middle`, `end`. |
| `font-weight` | `number` | `400` | no | CSS font weight (100-900). Drawn with the font family the embedder registered for the nearest weight (`svelters_egui::fonts::register_weight`); 400, or no registered family, draws the regular face. |
| `for` | `string` | `""` | no | The `anchor` of the control this label names (Qt's buddy): a click focuses it. |
| `selectable` | `boolean` | `false` | no | The text can be selected and copied. |
| `selection-end` | `number` | `0` | **yes**, on `changed` | The selection's end, in chars (`bind:`); a negative end is the end of the text. |
| `selection-start` | `number` | `0` | **yes**, on `changed` | The selection's start, in chars of the shown text (`bind:`). |
| `text` | `string` | `""` | no | The text. |
| `wrap-mode` | `'wrap' \| 'truncate' \| 'extend'` | `"wrap"` | no | What a line longer than the width does. |

Plus the 27 props every element has, from [`Widget`](widget.md#props): `accessible-name`, `align-self`, `anchor`, `autofocus`, `bottom`, `dialog-role`, `drag-data`, `drag-type`, `drop-types`, `enabled`, `flex-basis`, `flex-grow`, `flex-shrink`, `height`, `invalid`, `left`, `margin`, `max-height`, `max-width`, `min-height`, `min-width`, `position`, `right`, `tooltip`, `top`, `visible`, `width`.
<!-- END GENERATED -->

## Events

<!-- BEGIN GENERATED: events of kinds.Label (tools/docs/egui/gen_reference.py). Do not edit. -->
| Attribute | Handler | When |
|---|---|---|
| `onclicked` | `() => void` | Pressed and released. |

`on:clicked` works too, and a bare `on:<event>` on a component forwards the component's `on<event>` prop.

Plus the events of [`Widget`](widget.md#events): `onanimationend`, `onclick`, `oncontextmenu`, `ondblclick`, `ondrag-begin`, `ondrag-end`, `ondrag-enter`, `ondrag-leave`, `ondrop`, `onfocusin`, `onfocusout`, `onkeydown`, `onkeyup`, `onpointerdown`, `onpointerup`, `onwheel`.
<!-- END GENERATED -->

## Methods

<!-- BEGIN GENERATED: methods of kinds.Label (tools/docs/egui/gen_reference.py). Do not edit. -->
None.
<!-- END GENERATED -->

## Behaviour you need to know

<!-- TODO(DOC7): what the tables don't say: timing, edge cases, limits. -->

## CSS

<!-- BEGIN GENERATED: css of kinds.Label (tools/docs/egui/gen_reference.py). Do not edit. -->
**Selector:** `label`, plus `.class`, `#id` and attribute selectors on its props.

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

**Layout properties** (as attributes or in CSS; an explicit attribute wins): `align-self`, `bottom`, `dialog-role`, `flex-basis`, `flex-grow`, `flex-shrink`, `height`, `left`, `max-height`, `max-width`, `min-height`, `min-width`, `position`, `right`, `top`, `visible`, `width`.
Also in CSS only: `display`, `grid-column`, `grid-row`, `margin-bottom`, `margin-left`, `margin-right`, `margin-top`.

Full matrix: [css/properties](../../css/properties.md). States: below.
<!-- END GENERATED -->

## States

<!-- BEGIN GENERATED: states of kinds.Label (tools/docs/egui/gen_reference.py). Do not edit. -->
| Pseudo-class | Set when | Source |
|---|---|---|
| `:hover` | the pointer is over it | response |
| `:active` | the primary button is down on it | response |
| `:focus` | it takes clicks (`onclicked` is set) and has keyboard focus | response |
| `:focus-within` | it, or an element inside it, has keyboard focus | scene |
| `:disabled` (`:enabled` is the opposite) | `enabled` is false | prop |
| `:exiting` | it plays an out transition before it is removed | scene |

Never set on it (rules with them don't match): `:checked`, `:selected`, `:open`, `:expanded`, `:invalid`, `:read-only`, `:dragging`, `:busy`, `:indeterminate`, `:placeholder-shown`, `:default`, `:drop-target`, `:current`, `:backdrop`, `:required`, `:user-invalid`, `:has-children`.

States are tracked once the scene has a style input (a `<style>` block, `class`, `style=` or a theme).
<!-- END GENERATED -->

## Accessibility

<!-- BEGIN GENERATED: a11y of kinds.Label (tools/docs/egui/gen_reference.py). Do not edit. -->
| | |
|---|---|
| Role | `Label` |
| Name | `accessible-name`, else `text` |
| Actions | Click (with `onclicked`), Focus (with `onclicked`) |
| `invalid` | the `invalid` prop sets AccessKit `invalid` |
| Author id | `anchor` (unique in its root) |
<!-- END GENERATED -->

## Testing

<!-- TODO(DOC7): how to find and drive it in a kittest harness. -->

## See also

<!-- TODO(DOC7) -->
