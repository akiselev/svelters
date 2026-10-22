---
title: color-picker
tag: color-picker
kind: ColorPicker
category: pickers
description: "A colour swatch; clicking it opens egui's colour picker popup."
status: stub
verified_at: ~
generated: partial   # regions between BEGIN/END GENERATED markers come from the spec
sources:
  - metadata/semantics/egui.json#kinds.ColorPicker
---

# `<color-picker>`

<!-- BEGIN GENERATED: summary of kinds.ColorPicker (tools/docs/egui/gen_reference.py). Do not edit. -->
A colour swatch; clicking it opens egui's colour picker popup.

**Base:** [`Widget`](widget.md) · **Children:** none · **Text content:** no
<!-- END GENERATED -->

<!-- TODO(DOC7): when to use it, and what to use instead. -->

## Example

<!-- TODO(DOC7): a checked example (a testkit case under docs/egui/cases/). -->

## Props

<!-- BEGIN GENERATED: props of kinds.ColorPicker (tools/docs/egui/gen_reference.py). Do not edit. -->
| Prop | Type | Default | `bind:` | Description |
|---|---|---|---|---|
| `alpha` | `'opaque' \| 'blend' \| 'blend-or-additive'` | `"opaque"` | no | Whether the popup edits alpha: `opaque` hides it, `blend` edits it, `blend-or-additive` also offers additive. |
| `value` | `string` | `"#000000"` | **yes**, on `changed` | The colour, CSS hex: `#rrggbb` when opaque, `#rrggbbaa` otherwise (`#rgb`, `#rgba` accepted). Editing it in the popup changes it. |

Plus the 26 props every element has, from [`Widget`](widget.md#props): `accessible-name`, `align-self`, `anchor`, `autofocus`, `bottom`, `drag-data`, `drag-type`, `drop-types`, `enabled`, `flex-basis`, `flex-grow`, `flex-shrink`, `height`, `invalid`, `left`, `margin`, `max-height`, `max-width`, `min-height`, `min-width`, `position`, `right`, `tooltip`, `top`, `visible`, `width`.
<!-- END GENERATED -->

## Events

<!-- BEGIN GENERATED: events of kinds.ColorPicker (tools/docs/egui/gen_reference.py). Do not edit. -->
| Attribute | Handler | When |
|---|---|---|
| `onchanged` | `(value: string) => void` | The user edited the colour; carries it (CSS hex, once per frame while dragging). |

`on:changed` works too, and a bare `on:<event>` on a component forwards the component's `on<event>` prop.

Plus the events of [`Widget`](widget.md#events): `onanimationend`, `ondrag-begin`, `ondrag-end`, `ondrag-enter`, `ondrag-leave`, `ondrop`.
<!-- END GENERATED -->

## Methods

<!-- BEGIN GENERATED: methods of kinds.ColorPicker (tools/docs/egui/gen_reference.py). Do not edit. -->
None.
<!-- END GENERATED -->

## Behaviour you need to know

<!-- TODO(DOC7): what the tables don't say: timing, edge cases, limits. -->

## CSS

<!-- BEGIN GENERATED: css of kinds.ColorPicker (tools/docs/egui/gen_reference.py). Do not edit. -->
**Selector:** `color-picker`, plus `.class`, `#id` and attribute selectors on its props.

**Painted as:** a box. The CSS engine paints its shadows, background and border at its rect, under its content; `opacity` fades it and its content.

| Property | Applies | Theme default |
|---|---|---|
| `background-color` | yes | — |
| `border-color` | yes | — |
| `border-width` | yes | — |
| `border-radius` | yes | — |
| `color` | yes (its text) | — |
| `box-shadow` | yes (outer and inset) | — |
| `opacity` | yes | — |

**Layout properties** (as attributes or in CSS; an explicit attribute wins): `align-self`, `bottom`, `flex-basis`, `flex-grow`, `flex-shrink`, `height`, `left`, `margin`, `max-height`, `max-width`, `min-height`, `min-width`, `position`, `right`, `top`, `visible`, `width`.
Also in CSS only: `display`, `grid-column`, `grid-row`, `margin-bottom`, `margin-left`, `margin-right`, `margin-top`.

The swatch shows the colour; the picker popup follows the theme.

Full matrix: [css/properties](../../css/properties.md). States: below.
<!-- END GENERATED -->

## States

<!-- BEGIN GENERATED: states of kinds.ColorPicker (tools/docs/egui/gen_reference.py). Do not edit. -->
| Pseudo-class | Set when | Source |
|---|---|---|
| `:hover` | the pointer is over it | response |
| `:active` | the primary button is down on it | response |
| `:focus` | it has keyboard focus | response |
| `:focus-within` | it, or an element inside it, has keyboard focus | scene |
| `:disabled` (`:enabled` is the opposite) | `enabled` is false | prop |
| `:exiting` | it plays an out transition before it is removed | scene |

Never set on it (rules with them don't match): `:checked`, `:selected`, `:open`, `:expanded`, `:invalid`, `:read-only`, `:dragging`, `:busy`.

States are tracked once the scene has a style input (a `<style>` block, `class`, `style=` or a theme).
<!-- END GENERATED -->

## Accessibility

<!-- BEGIN GENERATED: a11y of kinds.ColorPicker (tools/docs/egui/gen_reference.py). Do not edit. -->
| | |
|---|---|
| Role | `ColorWell` |
| Name | `accessible-name`, else `tooltip`, else "Color" |
| Value | the colour |
| Actions | `Click`, `Focus` |
| `invalid` | the `invalid` prop sets AccessKit `invalid` |
| Author id | `anchor` (unique in its root) |
<!-- END GENERATED -->

## Testing

<!-- TODO(DOC7): how to find and drive it in a kittest harness. -->

## See also

<!-- TODO(DOC7) -->
