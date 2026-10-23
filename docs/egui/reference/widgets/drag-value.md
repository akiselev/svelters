---
title: drag-value
tag: drag-value
kind: DragValue
category: inputs
description: "A number edited by dragging horizontally, the arrow keys, or typing after a click."
status: stub
verified_at: ~
generated: partial   # regions between BEGIN/END GENERATED markers come from the spec
sources:
  - metadata/semantics/egui.json#kinds.DragValue
---

# `<drag-value>`

<!-- BEGIN GENERATED: summary of kinds.DragValue (tools/docs/egui/gen_reference.py). Do not edit. -->
A number edited by dragging horizontally, the arrow keys, or typing after a click. Name it with `accessible-name` or `tooltip`.

**Base:** [`Widget`](widget.md) · **Children:** none · **Text content:** no
<!-- END GENERATED -->

<!-- TODO(DOC7): when to use it, and what to use instead. -->

## Example

<!-- TODO(DOC7): a checked example (a testkit case under docs/egui/cases/). -->

## Props

<!-- BEGIN GENERATED: props of kinds.DragValue (tools/docs/egui/gen_reference.py). Do not edit. -->
| Prop | Type | Default | `bind:` | Description |
|---|---|---|---|---|
| `accelerated` | `boolean` | `false` | no | Holding a step button speeds it up (Qt `accelerated`). |
| `buttons` | `boolean` | `false` | no | Shows the `dec`/`inc` step buttons at the right end (Qt `buttonSymbols`). |
| `commit` | `boolean` | `false` | no | Commit-only mode (Qt `keyboardTracking=false`): `value` moves only at a commit point. |
| `correction` | `'nearest' \| 'previous'` | `"nearest"` | no | An out-of-range typed value commits to the `nearest` bound (default) or the `previous` value. |
| `decimals` | `number \| null` | `null` | no | Decimals shown; null for egui's automatic precision. |
| `formatter` | `string` | `""` | no | A registered formatter. |
| `group-separator` | `boolean` | `true` | no | The locale's group separators (Qt `showGroupSeparator`). |
| `max` | `number \| null` | `null` | no | Upper bound; null for none. |
| `min` | `number \| null` | `null` | no | Lower bound; null for none. |
| `page-step` | `number` | `0` | no | PageUp/PageDown in value units; 0 for ten steps. |
| `parser` | `string` | `""` | no | A registered parser. |
| `prefix` | `string` | `""` | no | Text before the number (`x: `). |
| `read-only` | `boolean` | `false` | no | Shown and focusable, not changed by the user (Qt `readOnly`). |
| `special-value-text` | `string` | `""` | no | Shown instead of the number at `min` (Qt `specialValueText`, e.g. "Auto"). |
| `speed` | `number` | `1` | no | Value change per point dragged. |
| `step` | `number` | `1` | no | One step (Up/Down, a step button, a wheel notch; Qt `singleStep`). |
| `step-type` | `'default' \| 'adaptive'` | `"default"` | no | `default`, or `adaptive` (Qt AdaptiveDecimalStepType: one decade below the value). |
| `suffix` | `string` | `""` | no | Text after the number (` mm`). |
| `value` | `number` | `0` | **yes**, on `changed` | The value; dragging, the arrow keys or typing change it. Kept within `min` and `max`. |
| `wheel` | `'focused' \| 'hover' \| 'off'` | `"focused"` | no | When the wheel steps: `focused` (default: over the field while it has focus), `hover`, `off`. |
| `wrap` | `boolean` | `false` | no | Stepping past one end goes to the other (Qt `wrapping`). |

Plus the 29 props every element has, from [`Widget`](widget.md#props): `accessible-name`, `align-self`, `anchor`, `autofocus`, `bottom`, `dialog-role`, `drag-data`, `drag-type`, `drop-types`, `enabled`, `flex-basis`, `flex-grow`, `flex-shrink`, `height`, `invalid`, `left`, `margin`, `max-height`, `max-width`, `min-height`, `min-width`, `position`, `right`, `status-section`, `status-tip`, `tooltip`, `top`, `visible`, `width`.
<!-- END GENERATED -->

## Events

<!-- BEGIN GENERATED: events of kinds.DragValue (tools/docs/egui/gen_reference.py). Do not edit. -->
| Attribute | Handler | When |
|---|---|---|
| `onchanged` | `(value: number) => void` | The user changed it; carries the new value. |
| `oncommit` | `(value: number) => void` | The user committed a value: a drag ended, or a change made otherwise (arrow keys, a typed value applied on Enter or blur); carries the value. |
| `ondrag-started` | `() => void` | A drag on it began. |
| `ondrag-stopped` | `() => void` | A drag on it ended. |
| `oninput` | `(value: number) => void` | §2.4's live signal (as `changed`). |
| `onsubmit` | `(value: number) => void` | Enter in the field; carries the value. |

`on:changed` works too, and a bare `on:<event>` on a component forwards the component's `on<event>` prop.

Plus the events of [`Widget`](widget.md#events): `onanimationend`, `onclick`, `oncontextmenu`, `ondblclick`, `ondrag-begin`, `ondrag-end`, `ondrag-enter`, `ondrag-leave`, `ondrop`, `onfocusin`, `onfocusout`, `onkeydown`, `onkeyup`, `onpointerdown`, `onpointerup`, `onwheel`.
<!-- END GENERATED -->

## Methods

<!-- BEGIN GENERATED: methods of kinds.DragValue (tools/docs/egui/gen_reference.py). Do not edit. -->
None.
<!-- END GENERATED -->

## Behaviour you need to know

<!-- TODO(DOC7): what the tables don't say: timing, edge cases, limits. -->

## CSS

<!-- BEGIN GENERATED: css of kinds.DragValue (tools/docs/egui/gen_reference.py). Do not edit. -->
**Selector:** `drag-value`, plus `.class`, `#id` and attribute selectors on its props.

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

Full matrix: [css/properties](../../css/properties.md). States: below.
<!-- END GENERATED -->

## States

<!-- BEGIN GENERATED: states of kinds.DragValue (tools/docs/egui/gen_reference.py). Do not edit. -->
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

<!-- BEGIN GENERATED: a11y of kinds.DragValue (tools/docs/egui/gen_reference.py). Do not edit. -->
| | |
|---|---|
| Role | `SpinButton` |
| Name | `accessible-name`, else `tooltip` |
| Value | `value` |
| Actions | `Focus`, `SetValue`, `Increment`, `Decrement` |
| `invalid` | the `invalid` prop sets AccessKit `invalid` |
| Author id | `anchor` (unique in its root) |
<!-- END GENERATED -->

## Testing

<!-- TODO(DOC7): how to find and drive it in a kittest harness. -->

## See also

<!-- TODO(DOC7) -->
