---
title: color-editor
tag: color-editor
kind: ColorEditor
category: pickers
description: "An inline colour editor (the body of Qt's `QColorDialog`, GTK's `ColorChooserWidget`): a saturation/value square, hue and alpha strips, hex and channel fields, a palette and custom colours."
status: stub
verified_at: ~
generated: partial   # regions between BEGIN/END GENERATED markers come from the spec
sources:
  - metadata/semantics/egui.json#kinds.ColorEditor
---

# `<color-editor>`

<!-- BEGIN GENERATED: summary of kinds.ColorEditor (tools/docs/egui/gen_reference.py). Do not edit. -->
An inline colour editor (the body of Qt's `QColorDialog`, GTK's `ColorChooserWidget`): a saturation/value square, hue and alpha strips, hex and channel fields, a palette and custom colours. `color-picker` shows the same editor in its popup.

**Base:** [`Widget`](widget.md) · **Children:** none · **Text content:** no
<!-- END GENERATED -->

<!-- TODO(DOC7): when to use it, and what to use instead. -->

## Example

<!-- TODO(DOC7): a checked example (a testkit case under docs/egui/cases/). -->

## Props

<!-- BEGIN GENERATED: props of kinds.ColorEditor (tools/docs/egui/gen_reference.py). Do not edit. -->
| Prop | Type | Default | `bind:` | Description |
|---|---|---|---|---|
| `alpha` | `'opaque' \| 'blend' \| 'blend-or-additive'` | `"opaque"` | no | `opaque` hides the alpha strip; `blend` and `blend-or-additive` show it. |
| `custom-colors` | `string[]` | `null` | **yes**, on `changed` | The user's colours (`bind:custom-colors`, up to 16): the add cell appends the current colour. Persist it with `uiState()`. |
| `fields` | `string` | `"hex rgb"` | no | Which fields show under the strips, space-separated: `hex` (a hex field), `rgb` (R, G, B boxes, 0–255), `hsv` (H 0–360, S and V 0–100). |
| `swatches` | `string[]` | `null` | no | A palette of hex colours shown as cells (`::part(swatch-cell)`); a click picks one. |
| `value` | `string` | `"#000000"` | **yes**, on `changed` | The colour, CSS hex: `#rrggbb` when opaque, `#rrggbbaa` otherwise (`#rgb`, `#rgba` accepted). |

Plus the 30 props every element has, from [`Widget`](widget.md#props): `accessible-name`, `align-self`, `anchor`, `anchor-scope`, `autofocus`, `bottom`, `dialog-role`, `drag-data`, `drag-type`, `drop-types`, `enabled`, `flex-basis`, `flex-grow`, `flex-shrink`, `height`, `invalid`, `left`, `margin`, `max-height`, `max-width`, `min-height`, `min-width`, `position`, `right`, `status-section`, `status-tip`, `tooltip`, `top`, `visible`, `width`.
<!-- END GENERATED -->

## Events

<!-- BEGIN GENERATED: events of kinds.ColorEditor (tools/docs/egui/gen_reference.py). Do not edit. -->
| Attribute | Handler | When |
|---|---|---|
| `onchanged` | `(value: string) => void` | The user edited the colour (each drag frame, step, field or swatch); carries it (CSS hex). |
| `oncommit` | `(value: string) => void` | A commit point: a drag released, a key step, a field set, a swatch picked (§2.4). |

`on:changed` works too, and a bare `on:<event>` on a component forwards the component's `on<event>` prop.

Plus the events of [`Widget`](widget.md#events): `onanimationend`, `onclick`, `oncontextmenu`, `ondblclick`, `ondrag-begin`, `ondrag-end`, `ondrag-enter`, `ondrag-leave`, `ondrop`, `onfocusin`, `onfocusout`, `onkeydown`, `onkeyup`, `onpointerdown`, `onpointermove`, `onpointerup`, `onwheel`.
<!-- END GENERATED -->

## Methods

<!-- BEGIN GENERATED: methods of kinds.ColorEditor (tools/docs/egui/gen_reference.py). Do not edit. -->
None.
<!-- END GENERATED -->

## Behaviour you need to know

<!-- TODO(DOC7): what the tables don't say: timing, edge cases, limits. -->

## CSS

<!-- BEGIN GENERATED: css of kinds.ColorEditor (tools/docs/egui/gen_reference.py). Do not edit. -->
**Selector:** `color-editor`, plus `.class`, `#id` and attribute selectors on its props.

**Painted as:** a box under the kind's own drawing. The CSS engine paints shadows, background and border at its rect; the kind's `ui` draws the rest.

| Property | Applies | Theme default |
|---|---|---|
| `background-color` | yes | — |
| `border-color` | yes | — |
| `border-width` | yes | — |
| `border-radius` | yes | — |
| `color` | yes (its text) | — |
| `box-shadow` | yes (outer and inset) | — |
| `opacity` | yes | — |

**Layout properties** (as attributes or in CSS; an explicit attribute wins): `align-self`, `bottom`, `dialog-role`, `flex-basis`, `flex-grow`, `flex-shrink`, `height`, `left`, `max-height`, `max-width`, `min-height`, `min-width`, `position`, `right`, `status-section`, `top`, `visible`, `width`.
Also in CSS only: `display`, `grid-column`, `grid-row`, `margin-bottom`, `margin-left`, `margin-right`, `margin-top`.

Parts: `square`, `hue`, `alpha`, `handle` (`:dragging`, `:focus-visible`), `fields`, `field`, `swatches`, `swatch-cell`. The checkerboard under alpha is painted until `background-image` repeating gradients reach egui.

Full matrix: [css/properties](../../css/properties.md). States: below.
<!-- END GENERATED -->

## States

<!-- BEGIN GENERATED: states of kinds.ColorEditor (tools/docs/egui/gen_reference.py). Do not edit. -->
| Pseudo-class | Set when | Source |
|---|---|---|
| `:hover` | the pointer is over it | response |
| `:active` | the primary button is down on it | response |
| `:focus` | it has keyboard focus | response |
| `:focus-within` | it, or an element inside it, has keyboard focus | scene |
| `:disabled` (`:enabled` is the opposite) | `enabled` is false | prop |
| `:exiting` | it plays an out transition before it is removed | scene |

Never set on it (rules with them don't match): `:checked`, `:selected`, `:open`, `:expanded`, `:invalid`, `:read-only`, `:dragging`, `:busy`, `:indeterminate`, `:placeholder-shown`, `:default`, `:drop-target`, `:current`, `:backdrop`, `:required`, `:user-invalid`, `:has-children`.

The kind's own states (`state_bits!`, `WidgetCx::set_state`) match as `:state(<namespace>.<name>)`.

States are tracked once the scene has a style input (a `<style>` block, `class`, `style=` or a theme).
<!-- END GENERATED -->

## Accessibility

<!-- BEGIN GENERATED: a11y of kinds.ColorEditor (tools/docs/egui/gen_reference.py). Do not edit. -->
| | |
|---|---|
| Role | `Group` |
| Name | `accessible-name` |
| Value | the colour (hex) |
| Actions | `Focus` |
| `invalid` | the `invalid` prop sets AccessKit `invalid` |
| Author id | `anchor` (unique in its root) |

A `Group` holding a `Slider` each for saturation/value, hue and alpha, a `SpinButton` per channel box, a `TextInput` for hex (SetValue), and a `Button` per swatch and custom colour. Each slider and box is a tab stop; arrows step it (Shift ×10).
<!-- END GENERATED -->

## Testing

<!-- TODO(DOC7): how to find and drive it in a kittest harness. -->

## See also

<!-- TODO(DOC7) -->
