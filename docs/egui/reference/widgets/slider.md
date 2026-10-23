---
title: slider
tag: slider
kind: Slider
category: inputs
description: "A horizontal slider with an optional value box."
status: stub
verified_at: ~
generated: partial   # regions between BEGIN/END GENERATED markers come from the spec
sources:
  - metadata/semantics/egui.json#kinds.Slider
---

# `<slider>`

<!-- BEGIN GENERATED: summary of kinds.Slider (tools/docs/egui/gen_reference.py). Do not edit. -->
A horizontal slider with an optional value box.

**Base:** [`Widget`](widget.md) · **Children:** none · **Text content:** no
<!-- END GENERATED -->

<!-- TODO(DOC7): when to use it, and what to use instead. -->

## Example

<!-- TODO(DOC7): a checked example (a testkit case under docs/egui/cases/). -->

## Props

<!-- BEGIN GENERATED: props of kinds.Slider (tools/docs/egui/gen_reference.py). Do not edit. -->
| Prop | Type | Default | `bind:` | Description |
|---|---|---|---|---|
| `commit` | `boolean` | `false` | no | Commit-only mode (Qt `tracking=false`): `value` moves at the release, not while dragging. |
| `inverted` | `boolean` | `false` | no | Max at the start (Qt `invertedAppearance`). |
| `marks` | `string` | `""` | no | Marks on the rail, drawn as ticks (with labels) instead of the `tick-interval` ticks: values separated by commas, each optionally `value:label` (`"0:Low, 50, 100:High"`). |
| `max` | `number` | `100` | no | Upper bound. |
| `min` | `number` | `0` | no | Lower bound. |
| `orientation` | `'horizontal' \| 'vertical'` | `"horizontal"` | no | `horizontal` (default) or `vertical` (max at the top). |
| `page-step` | `number` | `0` | no | PageUp/PageDown and `track-click="page"` move this much (Qt `pageStep`); 0 for ten steps. |
| `scale` | `'linear' \| 'log'` | `"linear"` | no | `linear` (default) or `log` (both ends > 0). |
| `show-value` | `boolean` | `true` | no | Shows the value box next to the slider. |
| `snap-to-marks` | `boolean` | `false` | no | Dragging snaps to the nearest mark. |
| `step` | `number` | `0` | no | Value granularity; 0 for continuous. |
| `text` | `string` | `""` | no | Caption after the slider. |
| `tick-interval` | `number` | `0` | no | Between ticks; 0 for `page-step` (Qt `tickInterval`). |
| `ticks` | `'none' \| 'before' \| 'after' \| 'both'` | `"none"` | no | Tick marks: `none` (default), `before` (above or left), `after`, `both` (Qt `tickPosition`). |
| `track-click` | `'jump' \| 'page'` | `"jump"` | no | A press on the track off the thumb: `jump` there (default) or `page` toward it. |
| `value` | `number` | `0` | **yes**, on `changed` | The value; dragging or the arrow keys change it. |
| `wheel` | `'focused' \| 'hover' \| 'off'` | `"focused"` | no | When the wheel steps: `focused` (default), `hover`, `off`. |

Plus the 29 props every element has, from [`Widget`](widget.md#props): `accessible-name`, `align-self`, `anchor`, `autofocus`, `bottom`, `dialog-role`, `drag-data`, `drag-type`, `drop-types`, `enabled`, `flex-basis`, `flex-grow`, `flex-shrink`, `height`, `invalid`, `left`, `margin`, `max-height`, `max-width`, `min-height`, `min-width`, `position`, `right`, `status-section`, `status-tip`, `tooltip`, `top`, `visible`, `width`.
<!-- END GENERATED -->

## Events

<!-- BEGIN GENERATED: events of kinds.Slider (tools/docs/egui/gen_reference.py). Do not edit. -->
| Attribute | Handler | When |
|---|---|---|
| `onchanged` | `(value: number) => void` | The user moved it; carries the new value. |
| `oncommit` | `(value: number) => void` |  |
| `ondrag-started` | `() => void` | A drag on the slider began. |
| `ondrag-stopped` | `() => void` | A drag on the slider ended. |
| `oninput` | `(value: number) => void` |  |
| `onsubmit` | `(value: number) => void` |  |

`on:changed` works too, and a bare `on:<event>` on a component forwards the component's `on<event>` prop.

Plus the events of [`Widget`](widget.md#events): `onanimationend`, `onclick`, `oncontextmenu`, `ondblclick`, `ondrag-begin`, `ondrag-end`, `ondrag-enter`, `ondrag-leave`, `ondrop`, `onfocusin`, `onfocusout`, `onkeydown`, `onkeyup`, `onpointerdown`, `onpointerup`, `onwheel`.
<!-- END GENERATED -->

## Methods

<!-- BEGIN GENERATED: methods of kinds.Slider (tools/docs/egui/gen_reference.py). Do not edit. -->
None.
<!-- END GENERATED -->

## Behaviour you need to know

<!-- TODO(DOC7): what the tables don't say: timing, edge cases, limits. -->

## CSS

<!-- BEGIN GENERATED: css of kinds.Slider (tools/docs/egui/gen_reference.py). Do not edit. -->
**Selector:** `slider`, plus `.class`, `#id` and attribute selectors on its props.

**Painted as:** a themed control. `background-color`, `border-*` and `color` set its look in each interaction state (normal, `:hover`, `:active`); outer `box-shadow` and `opacity` apply.

| Property | Applies | Theme default |
|---|---|---|
| `background-color` | yes, per state | — |
| `border-color` | yes | — |
| `border-width` | yes | — |
| `border-radius` | yes | — |
| `color` | yes (text and marks) | — |
| `box-shadow` | outer shadows only | — |
| `opacity` | yes | — |

**Layout properties** (as attributes or in CSS; an explicit attribute wins): `align-self`, `bottom`, `dialog-role`, `flex-basis`, `flex-grow`, `flex-shrink`, `height`, `left`, `max-height`, `max-width`, `min-height`, `min-width`, `position`, `right`, `status-section`, `top`, `visible`, `width`.
Also in CSS only: `display`, `grid-column`, `grid-row`, `margin-bottom`, `margin-left`, `margin-right`, `margin-top`.

Full matrix: [css/properties](../../css/properties.md). States: below.
<!-- END GENERATED -->

## States

<!-- BEGIN GENERATED: states of kinds.Slider (tools/docs/egui/gen_reference.py). Do not edit. -->
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

<!-- BEGIN GENERATED: a11y of kinds.Slider (tools/docs/egui/gen_reference.py). Do not edit. -->
| | |
|---|---|
| Role | `Slider` |
| Name | `accessible-name`, else `text`, else `tooltip` |
| Value | `value`, within `min`..`max` |
| Actions | `Focus`, `SetValue`, `Increment`, `Decrement` |
| `invalid` | the `invalid` prop sets AccessKit `invalid` |
| Author id | `anchor` (unique in its root) |

With `show-value`, its value box is a separate `SpinButton` node.
<!-- END GENERATED -->

## Testing

<!-- TODO(DOC7): how to find and drive it in a kittest harness. -->

## See also

<!-- TODO(DOC7) -->
