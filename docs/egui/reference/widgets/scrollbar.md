---
title: scrollbar
tag: scrollbar
kind: Scrollbar
category: containers
description: "A standalone scroll bar (Qt's `QScrollBar`) for a view that scrolls itself (a canvas, a plot, cadatomic's viewport): `value` from `min` to `max`, the thumb sized by `page`."
status: stub
verified_at: ~
generated: partial   # regions between BEGIN/END GENERATED markers come from the spec
sources:
  - metadata/semantics/egui.json#kinds.Scrollbar
---

# `<scrollbar>`

<!-- BEGIN GENERATED: summary of kinds.Scrollbar (tools/docs/egui/gen_reference.py). Do not edit. -->
A standalone scroll bar (Qt's `QScrollBar`) for a view that scrolls itself (a canvas, a plot, cadatomic's viewport): `value` from `min` to `max`, the thumb sized by `page`. It looks like a scroll area's bar (overlay, VS Code's `scrollbarSlider.*`), always shown.

**Base:** [`Widget`](widget.md) · **Children:** none · **Text content:** no
<!-- END GENERATED -->

<!-- TODO(DOC7): when to use it, and what to use instead. -->

## Example

<!-- TODO(DOC7): a checked example (a testkit case under docs/egui/cases/). -->

## Props

<!-- BEGIN GENERATED: props of kinds.Scrollbar (tools/docs/egui/gen_reference.py). Do not edit. -->
| Prop | Type | Default | `bind:` | Description |
|---|---|---|---|---|
| `max` | `number` | `100` | no | The largest value (the content's size less `page`). |
| `min` | `number` | `0` | no | The smallest value. |
| `orientation` | `'horizontal' \| 'vertical'` | `"vertical"` | no | `vertical` or `horizontal`. |
| `page` | `number` | `10` | no | The visible part's size: the thumb's length in proportion, and what a press on the track scrolls. |
| `step` | `number` | `1` | no | What AccessKit's Increment and Decrement move. |
| `value` | `number` | `0` | **yes**, on `changed` | The position, from `min` to `max` (`bind:value`): the start of the visible part of what it scrolls. |

Plus the 31 props every element has, from [`Widget`](widget.md#props): `accessible-name`, `align-self`, `anchor`, `anchor-scope`, `autofocus`, `bottom`, `dialog-role`, `drag-data`, `drag-type`, `drop-types`, `enabled`, `flex-basis`, `flex-grow`, `flex-shrink`, `height`, `invalid`, `left`, `margin`, `max-height`, `max-width`, `min-height`, `min-width`, `position`, `right`, `source-loc`, `status-section`, `status-tip`, `tooltip`, `top`, `visible`, `width`.
<!-- END GENERATED -->

## Events

<!-- BEGIN GENERATED: events of kinds.Scrollbar (tools/docs/egui/gen_reference.py). Do not edit. -->
| Attribute | Handler | When |
|---|---|---|
| `onchanged` | `(value: number) => void` | The user moved it (thumb, track, wheel, AccessKit); carries the new value. |

`on:changed` works too, and a bare `on:<event>` on a component forwards the component's `on<event>` prop.

Plus the events of [`Widget`](widget.md#events): `onanimationend`, `onclick`, `oncontextmenu`, `ondblclick`, `ondrag-begin`, `ondrag-end`, `ondrag-enter`, `ondrag-leave`, `ondrop`, `onfocusin`, `onfocusout`, `onkeydown`, `onkeyup`, `onpointerdown`, `onpointermove`, `onpointerup`, `onwheel`.
<!-- END GENERATED -->

## Methods

<!-- BEGIN GENERATED: methods of kinds.Scrollbar (tools/docs/egui/gen_reference.py). Do not edit. -->
None.
<!-- END GENERATED -->

## Behaviour you need to know

<!-- TODO(DOC7): what the tables don't say: timing, edge cases, limits. -->

## CSS

<!-- BEGIN GENERATED: css of kinds.Scrollbar (tools/docs/egui/gen_reference.py). Do not edit. -->
**Selector:** `scrollbar`, plus `.class`, `#id` and attribute selectors on its props.

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

Its track and thumb (`::part(scrollbar)`, `::part(thumb)`) paint from VS Code's `scrollbarSlider.*` tokens until `::part()` reaches them; its thickness is `scrollbar-w`.

Full matrix: [css/properties](../../css/properties.md). States: below.
<!-- END GENERATED -->

## States

<!-- BEGIN GENERATED: states of kinds.Scrollbar (tools/docs/egui/gen_reference.py). Do not edit. -->
| Pseudo-class | Set when | Source |
|---|---|---|
| `:hover` | the pointer is over it | response |
| `:active` | the primary button is down on it | response |
| `:focus-within` | it, or an element inside it, has keyboard focus | scene |
| `:disabled` (`:enabled` is the opposite) | `enabled` is false | prop |
| `:exiting` | it plays an out transition before it is removed | scene |

Never set on it (rules with them don't match): `:focus`, `:checked`, `:selected`, `:open`, `:expanded`, `:invalid`, `:read-only`, `:dragging`, `:busy`, `:indeterminate`, `:placeholder-shown`, `:default`, `:drop-target`, `:current`, `:backdrop`, `:required`, `:user-invalid`, `:has-children`.

The kind's own states (`state_bits!`, `WidgetCx::set_state`) match as `:state(<namespace>.<name>)`.

States are tracked once the scene has a style input (a `<style>` block, `class`, `style=` or a theme).
<!-- END GENERATED -->

## Accessibility

<!-- BEGIN GENERATED: a11y of kinds.Scrollbar (tools/docs/egui/gen_reference.py). Do not edit. -->
| | |
|---|---|
| Role | `ScrollBar` |
| Name | `accessible-name` |
| Value | `value` in `min`..`max`, with its step |
| Actions | `Increment`, `Decrement`, `ScrollUp`, `ScrollDown` |
| `invalid` | the `invalid` prop sets AccessKit `invalid` |
| Author id | `anchor` (unique in its root) |
<!-- END GENERATED -->

## Testing

<!-- TODO(DOC7): how to find and drive it in a kittest harness. -->

## See also

<!-- TODO(DOC7) -->
