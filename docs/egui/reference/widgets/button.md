---
title: button
tag: button
kind: Button
category: buttons
description: "A push button."
status: stub
verified_at: ~
generated: partial   # regions between BEGIN/END GENERATED markers come from the spec
sources:
  - metadata/semantics/egui.json#kinds.Button
---

# `<button>`

<!-- BEGIN GENERATED: summary of kinds.Button (tools/docs/egui/gen_reference.py). Do not edit. -->
A push button.

**Base:** [`Widget`](widget.md) · **Children:** none · **Text content:** sets `text`
<!-- END GENERATED -->

<!-- TODO(DOC7): when to use it, and what to use instead. -->

## Example

<!-- TODO(DOC7): a checked example (a testkit case under docs/egui/cases/). -->

## Props

<!-- BEGIN GENERATED: props of kinds.Button (tools/docs/egui/gen_reference.py). Do not edit. -->
| Prop | Type | Default | `bind:` | Description |
|---|---|---|---|---|
| `cancel` | `boolean` | `false` | no | Escape activates it while focus is in its focus scope (or root). |
| `default` | `boolean` | `false` | no | Enter activates it while focus is in its focus scope (or root) and the focused node does not take Enter itself. |
| `font-weight` | `number` | `400` | no | CSS font weight (100-900). Drawn with the font family the embedder registered for the nearest weight (`svelters_egui::fonts::register_weight`); 400, or no registered family, draws the regular face. |
| `frame` | `boolean` | `true` | no | Draws the button background and border. |
| `icon` | `string` | `""` | no | An icon before the caption: an icon key (`name` in the `icons` family, or `family:name`). Its size, place and visibility are CSS on `::part(icon)` (P3). |
| `repeat` | `boolean` | `false` | no | Held down, it activates again and again (`clicked` after 300 ms, then every 100 ms; QAbstractButton `autoRepeat`). |
| `selected` | `boolean` | `false` | no | Drawn as selected (a toggle look); clicking does not change it. |
| `small` | `boolean` | `false` | no | No vertical padding. |
| `text` | `string` | `""` | no | The caption. |

Plus the 26 props every element has, from [`Widget`](widget.md#props): `accessible-name`, `align-self`, `anchor`, `autofocus`, `bottom`, `drag-data`, `drag-type`, `drop-types`, `enabled`, `flex-basis`, `flex-grow`, `flex-shrink`, `height`, `invalid`, `left`, `margin`, `max-height`, `max-width`, `min-height`, `min-width`, `position`, `right`, `tooltip`, `top`, `visible`, `width`.
<!-- END GENERATED -->

## Events

<!-- BEGIN GENERATED: events of kinds.Button (tools/docs/egui/gen_reference.py). Do not edit. -->
| Attribute | Handler | When |
|---|---|---|
| `onclicked` | `() => void` | Pressed and released. |
| `ondouble-clicked` | `() => void` | Double click. |
| `onpress` | `() => void` | The primary button went down on it (QAbstractButton `pressed`). |
| `onrelease` | `() => void` | It came up again, over the button or not (`released`). |
| `onsecondary-clicked` | `() => void` | Right click. |

`on:clicked` works too, and a bare `on:<event>` on a component forwards the component's `on<event>` prop.

Plus the events of [`Widget`](widget.md#events): `onanimationend`, `onclick`, `oncontextmenu`, `ondblclick`, `ondrag-begin`, `ondrag-end`, `ondrag-enter`, `ondrag-leave`, `ondrop`, `onfocusin`, `onfocusout`, `onkeydown`, `onkeyup`, `onpointerdown`, `onpointerup`, `onwheel`.
<!-- END GENERATED -->

## Methods

<!-- BEGIN GENERATED: methods of kinds.Button (tools/docs/egui/gen_reference.py). Do not edit. -->
None.
<!-- END GENERATED -->

## Behaviour you need to know

<!-- TODO(DOC7): what the tables don't say: timing, edge cases, limits. -->

## CSS

<!-- BEGIN GENERATED: css of kinds.Button (tools/docs/egui/gen_reference.py). Do not edit. -->
**Selector:** `button`, plus `.class`, `#id` and attribute selectors on its props.

**Painted as:** a themed control. `background-color`, `border-*` and `color` set its look in each interaction state (normal, `:hover`, `:active`); outer `box-shadow` and `opacity` apply.

| Property | Applies | Theme default |
|---|---|---|
| `background-color` | yes, per state | `var(--sv-button-bg)` |
| `border-color` | yes | `var(--sv-button-border-color)` |
| `border-width` | yes | `var(--sv-stroke)` |
| `border-radius` | yes | `var(--sv-button-radius)` |
| `color` | yes (text and marks) | `var(--sv-button-fg)` |
| `box-shadow` | outer shadows only | — |
| `opacity` | yes | — |

Theme rules for its states: `button:hover` sets `background-color: var(--sv-button-bg-hover)`; `button:active` sets `background-color: var(--sv-button-bg-active)`.

**Layout properties** (as attributes or in CSS; an explicit attribute wins): `align-self`, `bottom`, `flex-basis`, `flex-grow`, `flex-shrink`, `height`, `left`, `margin`, `max-height`, `max-width`, `min-height`, `min-width`, `position`, `right`, `top`, `visible`, `width`.
Also in CSS only: `display`, `grid-column`, `grid-row`, `margin-bottom`, `margin-left`, `margin-right`, `margin-top`.

Full matrix: [css/properties](../../css/properties.md). States: below.
<!-- END GENERATED -->

## States

<!-- BEGIN GENERATED: states of kinds.Button (tools/docs/egui/gen_reference.py). Do not edit. -->
| Pseudo-class | Set when | Source |
|---|---|---|
| `:hover` | the pointer is over it | response |
| `:active` | the primary button is down on it | response |
| `:focus` | it has keyboard focus | response |
| `:focus-within` | it, or an element inside it, has keyboard focus | scene |
| `:disabled` (`:enabled` is the opposite) | `enabled` is false | prop |
| `:exiting` | it plays an out transition before it is removed | scene |
| `:selected` | `selected` is true | prop |

Never set on it (rules with them don't match): `:checked`, `:open`, `:expanded`, `:invalid`, `:read-only`, `:dragging`, `:busy`, `:indeterminate`, `:placeholder-shown`, `:default`, `:drop-target`, `:current`, `:backdrop`, `:required`, `:user-invalid`, `:has-children`.

States are tracked once the scene has a style input (a `<style>` block, `class`, `style=` or a theme).
<!-- END GENERATED -->

## Accessibility

<!-- BEGIN GENERATED: a11y of kinds.Button (tools/docs/egui/gen_reference.py). Do not edit. -->
| | |
|---|---|
| Role | `Button` |
| Name | `accessible-name`, else `text`, else tooltip (when `text` is empty) |
| Value | selected, when `selected` is true |
| Actions | `Click`, `Focus` |
| `invalid` | the `invalid` prop sets AccessKit `invalid` |
| Author id | `anchor` (unique in its root) |
<!-- END GENERATED -->

## Testing

<!-- TODO(DOC7): how to find and drive it in a kittest harness. -->

## See also

<!-- TODO(DOC7) -->
