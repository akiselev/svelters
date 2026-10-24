---
title: progress-bar
tag: progress-bar
kind: ProgressBar
category: display
description: "A horizontal bar filled to `fraction`, as wide as its rect."
status: stub
verified_at: ~
generated: partial   # regions between BEGIN/END GENERATED markers come from the spec
sources:
  - metadata/semantics/egui.json#kinds.ProgressBar
---

# `<progress-bar>`

<!-- BEGIN GENERATED: summary of kinds.ProgressBar (tools/docs/egui/gen_reference.py). Do not edit. -->
A horizontal bar filled to `fraction`, as wide as its rect.

**Base:** [`Widget`](widget.md) · **Children:** none · **Text content:** no
<!-- END GENERATED -->

<!-- TODO(DOC7): when to use it, and what to use instead. -->

## Example

<!-- TODO(DOC7): a checked example (a testkit case under docs/egui/cases/). -->

## Props

<!-- BEGIN GENERATED: props of kinds.ProgressBar (tools/docs/egui/gen_reference.py). Do not edit. -->
| Prop | Type | Default | `bind:` | Description |
|---|---|---|---|---|
| `animate` | `boolean` | `false` | no | Animates the bar and shows a spinner in it (busy look; repaints while shown). |
| `format` | `string` | `""` | no | The label's text when `text` is empty: `%p` percent, `%v` value, `%m` maximum (`%%` a percent sign); shown when set or with `show-percentage` (`%p%`). |
| `fraction` | `number` | `0` | no | Progress from 0 to 1 (GTK's `fraction`). |
| `indeterminate` | `boolean` | `false` | no | Busy, as `animate` (and `:indeterminate`); also when `value` is set and `min == max`. |
| `inverted` | `boolean` | `false` | no | Fills from the end (right, or top when vertical) instead. |
| `max` | `number` | `100` | no |  |
| `min` | `number` | `0` | no |  |
| `orientation` | `'horizontal' \| 'vertical'` | `"horizontal"` | no | The bar's axis (P7: `[orientation="vertical"]` styles it). |
| `show-percentage` | `boolean` | `false` | no | Shows the percentage inside the bar when `text` is empty. |
| `text` | `string` | `""` | no | Text inside the bar; empty for none. |
| `value` | `number \| null` | `null` | no | WIDGET-PARITY §2.9: the value in `min..max`, the alternative to `fraction` (Qt's form). |

Plus the 30 props every element has, from [`Widget`](widget.md#props): `accessible-name`, `align-self`, `anchor`, `anchor-scope`, `autofocus`, `bottom`, `dialog-role`, `drag-data`, `drag-type`, `drop-types`, `enabled`, `flex-basis`, `flex-grow`, `flex-shrink`, `height`, `invalid`, `left`, `margin`, `max-height`, `max-width`, `min-height`, `min-width`, `position`, `right`, `status-section`, `status-tip`, `tooltip`, `top`, `visible`, `width`.
<!-- END GENERATED -->

## Events

<!-- BEGIN GENERATED: events of kinds.ProgressBar (tools/docs/egui/gen_reference.py). Do not edit. -->
No events of its own.

Plus the events of [`Widget`](widget.md#events): `onanimationend`, `onclick`, `oncontextmenu`, `ondblclick`, `ondrag-begin`, `ondrag-end`, `ondrag-enter`, `ondrag-leave`, `ondrop`, `onfocusin`, `onfocusout`, `onkeydown`, `onkeyup`, `onpointerdown`, `onpointermove`, `onpointerup`, `onwheel`.
<!-- END GENERATED -->

## Methods

<!-- BEGIN GENERATED: methods of kinds.ProgressBar (tools/docs/egui/gen_reference.py). Do not edit. -->
None.
<!-- END GENERATED -->

## Behaviour you need to know

<!-- TODO(DOC7): what the tables don't say: timing, edge cases, limits. -->

## CSS

<!-- BEGIN GENERATED: css of kinds.ProgressBar (tools/docs/egui/gen_reference.py). Do not edit. -->
**Selector:** `progress-bar`, plus `.class`, `#id` and attribute selectors on its props.

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

**Layout properties** (as attributes or in CSS; an explicit attribute wins): `align-self`, `bottom`, `dialog-role`, `flex-basis`, `flex-grow`, `flex-shrink`, `height`, `left`, `max-height`, `max-width`, `min-height`, `min-width`, `position`, `right`, `status-section`, `top`, `visible`, `width`.
Also in CSS only: `display`, `grid-column`, `grid-row`, `margin-bottom`, `margin-left`, `margin-right`, `margin-top`.

The fill follows the theme's selection colour.

Full matrix: [css/properties](../../css/properties.md). States: below.
<!-- END GENERATED -->

## States

<!-- BEGIN GENERATED: states of kinds.ProgressBar (tools/docs/egui/gen_reference.py). Do not edit. -->
| Pseudo-class | Set when | Source |
|---|---|---|
| `:hover` | the pointer is over it | response |
| `:active` | the primary button is down on it | response |
| `:focus-within` | it, or an element inside it, has keyboard focus | scene |
| `:disabled` (`:enabled` is the opposite) | `enabled` is false | prop |
| `:exiting` | it plays an out transition before it is removed | scene |

Never set on it (rules with them don't match): `:focus`, `:checked`, `:selected`, `:open`, `:expanded`, `:invalid`, `:read-only`, `:dragging`, `:busy`, `:indeterminate`, `:placeholder-shown`, `:default`, `:drop-target`, `:current`, `:backdrop`, `:required`, `:user-invalid`, `:has-children`.

States are tracked once the scene has a style input (a `<style>` block, `class`, `style=` or a theme).
<!-- END GENERATED -->

## Accessibility

<!-- BEGIN GENERATED: a11y of kinds.ProgressBar (tools/docs/egui/gen_reference.py). Do not edit. -->
| | |
|---|---|
| Role | `ProgressIndicator` |
| Name | `accessible-name`, else `text`, else `tooltip` |
| Value | `fraction` |
| `invalid` | the `invalid` prop sets AccessKit `invalid` |
| Author id | `anchor` (unique in its root) |
<!-- END GENERATED -->

## Testing

<!-- TODO(DOC7): how to find and drive it in a kittest harness. -->

## See also

<!-- TODO(DOC7) -->
