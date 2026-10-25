---
title: range-slider
tag: range-slider
kind: RangeSlider
category: inputs
description: "A horizontal slider with two handles selecting `low`..`high`; the handles meet but never cross."
status: stub
verified_at: ~
generated: partial   # regions between BEGIN/END GENERATED markers come from the spec
sources:
  - metadata/semantics/egui.json#kinds.RangeSlider
---

# `<range-slider>`

<!-- BEGIN GENERATED: summary of kinds.RangeSlider (tools/docs/egui/gen_reference.py). Do not edit. -->
A horizontal slider with two handles selecting `low`..`high`; the handles meet but never cross.

**Base:** [`Widget`](widget.md) · **Children:** none · **Text content:** no
<!-- END GENERATED -->

<!-- TODO(DOC7): when to use it, and what to use instead. -->

## Example

<!-- TODO(DOC7): a checked example (a testkit case under docs/egui/cases/). -->

## Props

<!-- BEGIN GENERATED: props of kinds.RangeSlider (tools/docs/egui/gen_reference.py). Do not edit. -->
| Prop | Type | Default | `bind:` | Description |
|---|---|---|---|---|
| `commit` | `boolean` | `false` | no | Commit-only mode: `low`/`high` move at the release. |
| `high` | `number` | `100` | **yes**, on `changed` | The upper end; dragging its handle changes it. Kept within `low` and `max`. |
| `inverted` | `boolean` | `false` | no | Max at the start. |
| `low` | `number` | `0` | **yes**, on `changed` | The lower end; dragging its handle (or the arrow keys on it) changes it. Kept within `min` and `high`. |
| `max` | `number` | `100` | no | Upper bound. |
| `min` | `number` | `0` | no | Lower bound. |
| `orientation` | `'horizontal' \| 'vertical'` | `"horizontal"` | no | `horizontal` (default) or `vertical`. |
| `page-step` | `number` | `0` | no | PageUp/PageDown; 0 for ten steps. |
| `scale` | `'linear' \| 'log'` | `"linear"` | no | `linear` (default) or `log`. |
| `show-value` | `boolean` | `true` | no | Shows an editable value box at each end. |
| `step` | `number` | `0` | no | Value granularity; 0 for continuous. |
| `text` | `string` | `""` | no | Caption after the slider (each handle is named `<text> low`/`<text> high`). |
| `tick-interval` | `number` | `0` | no | Between ticks; 0 for `page-step`. |
| `ticks` | `'none' \| 'before' \| 'after' \| 'both'` | `"none"` | no | Tick marks: `none`, `before`, `after`, `both`. |
| `wheel` | `'focused' \| 'hover' \| 'off'` | `"focused"` | no | When the wheel steps: `focused` (default), `hover`, `off`. |

Plus the 31 props every element has, from [`Widget`](widget.md#props): `accessible-name`, `align-self`, `anchor`, `anchor-scope`, `autofocus`, `bottom`, `dialog-role`, `drag-data`, `drag-type`, `drop-types`, `enabled`, `flex-basis`, `flex-grow`, `flex-shrink`, `height`, `invalid`, `left`, `margin`, `max-height`, `max-width`, `min-height`, `min-width`, `position`, `right`, `source-loc`, `status-section`, `status-tip`, `tooltip`, `top`, `visible`, `width`.
<!-- END GENERATED -->

## Events

<!-- BEGIN GENERATED: events of kinds.RangeSlider (tools/docs/egui/gen_reference.py). Do not edit. -->
| Attribute | Handler | When |
|---|---|---|
| `onchanged` | `(low: number, high: number) => void` | The user moved a handle (or the widget clamped a written value); carries both ends. |
| `oncommit` | `() => void` |  |
| `ondrag-started` | `() => void` | A drag on the rail began. |
| `ondrag-stopped` | `() => void` | A drag on the rail ended. |
| `oninput` | `() => void` |  |

`on:changed` works too, and a bare `on:<event>` on a component forwards the component's `on<event>` prop.

Plus the events of [`Widget`](widget.md#events): `onanimationend`, `onclick`, `oncontextmenu`, `ondblclick`, `ondrag-begin`, `ondrag-end`, `ondrag-enter`, `ondrag-leave`, `ondrop`, `onfocusin`, `onfocusout`, `onkeydown`, `onkeyup`, `onpointerdown`, `onpointermove`, `onpointerup`, `onwheel`.
<!-- END GENERATED -->

## Methods

<!-- BEGIN GENERATED: methods of kinds.RangeSlider (tools/docs/egui/gen_reference.py). Do not edit. -->
None.
<!-- END GENERATED -->

## Behaviour you need to know

<!-- TODO(DOC7): what the tables don't say: timing, edge cases, limits. -->

## CSS

<!-- BEGIN GENERATED: css of kinds.RangeSlider (tools/docs/egui/gen_reference.py). Do not edit. -->
**Selector:** `range-slider`, plus `.class`, `#id` and attribute selectors on its props.

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

`:focus` follows its first focusable part (the low value box with `show-value`, else the rail).

Full matrix: [css/properties](../../css/properties.md). States: below.
<!-- END GENERATED -->

## States

<!-- BEGIN GENERATED: states of kinds.RangeSlider (tools/docs/egui/gen_reference.py). Do not edit. -->
| Pseudo-class | Set when | Source |
|---|---|---|
| `:hover` | the pointer is over it | response |
| `:active` | the primary button is down on it | response |
| `:focus` | it has keyboard focus | response |
| `:focus-within` | it, or an element inside it, has keyboard focus | scene |
| `:disabled` (`:enabled` is the opposite) | `enabled` is false | prop |
| `:exiting` | it plays an out transition before it is removed | scene |

Never set on it (rules with them don't match): `:checked`, `:selected`, `:open`, `:expanded`, `:invalid`, `:read-only`, `:dragging`, `:busy`, `:indeterminate`, `:placeholder-shown`, `:default`, `:drop-target`, `:current`, `:backdrop`, `:required`, `:user-invalid`, `:has-children`.

States are tracked once the scene has a style input (a `<style>` block, `class`, `style=` or a theme).
<!-- END GENERATED -->

## Accessibility

<!-- BEGIN GENERATED: a11y of kinds.RangeSlider (tools/docs/egui/gen_reference.py). Do not edit. -->
| | |
|---|---|
| Role | `Slider` |
| Name | `accessible-name`, else `text`, else `tooltip` |
| Value | `low` and `high`, one per handle |
| Actions | `Focus`, `SetValue`, `Increment`, `Decrement` |
| `invalid` | the `invalid` prop sets AccessKit `invalid` |
| Author id | `anchor` (unique in its root) |

Each handle is a `Slider` node of its own; with `show-value`, the two value boxes are `SpinButton` nodes.
<!-- END GENERATED -->

## Testing

<!-- TODO(DOC7): how to find and drive it in a kittest harness. -->

## See also

<!-- TODO(DOC7) -->
