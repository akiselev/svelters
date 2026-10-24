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
A colour swatch drop button; clicking it opens the colour editor (the `color-editor` kind's) in a popup.

**Base:** [`Widget`](widget.md) · **Children:** none · **Text content:** no
<!-- END GENERATED -->

<!-- TODO(DOC7): when to use it, and what to use instead. -->

## Example

<!-- TODO(DOC7): a checked example (a testkit case under docs/egui/cases/). -->

## Props

<!-- BEGIN GENERATED: props of kinds.ColorPicker (tools/docs/egui/gen_reference.py). Do not edit. -->
| Prop | Type | Default | `bind:` | Description |
|---|---|---|---|---|
| `alpha` | `'opaque' \| 'blend' \| 'blend-or-additive'` | `"opaque"` | no | Whether the popup edits alpha: `opaque` hides the alpha strip, `blend` and `blend-or-additive` show it (additive colours are not offered by the own editor). |
| `commit-mode` | `'live' \| 'confirm'` | `"live"` | no | `live`: each edit changes `value` (closing the popup commits); `confirm`: edits are pending until Save, Cancel or Escape drops them (Qt's dialog). |
| `custom-colors` | `string[]` | `null` | **yes**, on `changed` | The user's colours (`bind:custom-colors`, up to 16): the add cell appends the current colour. Persist it with `uiState()`. |
| `fields` | `string` | `"hex rgb"` | no | Which fields show under the strips, space-separated: `hex` (a hex field), `rgb` (R, G, B boxes, 0–255), `hsv` (H 0–360, S and V 0–100). |
| `swatches` | `string[]` | `null` | no | A palette of hex colours shown as cells (`::part(swatch-cell)`); a click picks one. |
| `value` | `string` | `"#000000"` | **yes**, on `changed` | The colour, CSS hex: `#rrggbb` when opaque, `#rrggbbaa` otherwise (`#rgb`, `#rgba` accepted). Editing it in the popup changes it. |

Plus the 30 props every element has, from [`Widget`](widget.md#props): `accessible-name`, `align-self`, `anchor`, `anchor-scope`, `autofocus`, `bottom`, `dialog-role`, `drag-data`, `drag-type`, `drop-types`, `enabled`, `flex-basis`, `flex-grow`, `flex-shrink`, `height`, `invalid`, `left`, `margin`, `max-height`, `max-width`, `min-height`, `min-width`, `position`, `right`, `status-section`, `status-tip`, `tooltip`, `top`, `visible`, `width`.
<!-- END GENERATED -->

## Events

<!-- BEGIN GENERATED: events of kinds.ColorPicker (tools/docs/egui/gen_reference.py). Do not edit. -->
| Attribute | Handler | When |
|---|---|---|
| `onchanged` | `(value: string) => void` | The user edited the colour; carries it (CSS hex, once per frame while dragging). |
| `oncommit` | `(value: string) => void` | A commit point (§2.4): `live`, a drag released, a step, a field or a swatch, and the popup closing after edits; `confirm`, Save. |

`on:changed` works too, and a bare `on:<event>` on a component forwards the component's `on<event>` prop.

Plus the events of [`Widget`](widget.md#events): `onanimationend`, `onclick`, `oncontextmenu`, `ondblclick`, `ondrag-begin`, `ondrag-end`, `ondrag-enter`, `ondrag-leave`, `ondrop`, `onfocusin`, `onfocusout`, `onkeydown`, `onkeyup`, `onpointerdown`, `onpointermove`, `onpointerup`, `onwheel`.
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

The button shows `::part(swatch)` (a checkerboard under alpha) and R14's `::part(chevron)`; the popup is `::part(popup)` holding the `color-editor` kind's parts and `::part(save)`, `::part(cancel)`.

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
| `:open` | its editor popup is shown | kind |

Never set on it (rules with them don't match): `:checked`, `:selected`, `:expanded`, `:invalid`, `:read-only`, `:dragging`, `:busy`, `:indeterminate`, `:placeholder-shown`, `:default`, `:drop-target`, `:current`, `:backdrop`, `:required`, `:user-invalid`, `:has-children`.

The kind's own states (`state_bits!`, `WidgetCx::set_state`) match as `:state(<namespace>.<name>)`.

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

A `ColorWell` named "Color" (or its accessible name) with the hex as its value, `expanded` while open. The popup holds the `color-editor` kind's sliders, boxes and swatches and, with `commit-mode="confirm"`, Save and Cancel; Escape closes it and focus returns.
<!-- END GENERATED -->

## Testing

<!-- TODO(DOC7): how to find and drive it in a kittest harness. -->

## See also

<!-- TODO(DOC7) -->
