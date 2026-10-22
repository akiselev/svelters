---
title: checkbox
tag: checkbox
kind: Checkbox
category: inputs
description: "A check box with a caption."
status: stub
verified_at: ~
generated: partial   # regions between BEGIN/END GENERATED markers come from the spec
sources:
  - metadata/semantics/egui.json#kinds.Checkbox
---

# `<checkbox>`

<!-- BEGIN GENERATED: summary of kinds.Checkbox (tools/docs/egui/gen_reference.py). Do not edit. -->
A check box with a caption.

**Base:** [`Widget`](widget.md) · **Children:** none · **Text content:** sets `text`
<!-- END GENERATED -->

<!-- TODO(DOC7): when to use it, and what to use instead. -->

## Example

<!-- TODO(DOC7): a checked example (a testkit case under docs/egui/cases/). -->

## Props

<!-- BEGIN GENERATED: props of kinds.Checkbox (tools/docs/egui/gen_reference.py). Do not edit. -->
| Prop | Type | Default | `bind:` | Description |
|---|---|---|---|---|
| `checked` | `boolean` | `false` | **yes**, on `changed` | Checked state; clicking toggles it. |
| `text` | `string` | `""` | no | The caption. |

Plus the 26 props every element has, from [`Widget`](widget.md#props): `accessible-name`, `align-self`, `anchor`, `autofocus`, `bottom`, `drag-data`, `drag-type`, `drop-types`, `enabled`, `flex-basis`, `flex-grow`, `flex-shrink`, `height`, `invalid`, `left`, `margin`, `max-height`, `max-width`, `min-height`, `min-width`, `position`, `right`, `tooltip`, `top`, `visible`, `width`.
<!-- END GENERATED -->

## Events

<!-- BEGIN GENERATED: events of kinds.Checkbox (tools/docs/egui/gen_reference.py). Do not edit. -->
| Attribute | Handler | When |
|---|---|---|
| `onchanged` | `(checked: boolean) => void` | The user toggled it; carries the new state. |
| `onclicked` | `() => void` | Pressed and released. |

`on:changed` works too, and a bare `on:<event>` on a component forwards the component's `on<event>` prop.

Plus the events of [`Widget`](widget.md#events): `onanimationend`, `onclick`, `oncontextmenu`, `ondblclick`, `ondrag-begin`, `ondrag-end`, `ondrag-enter`, `ondrag-leave`, `ondrop`, `onfocusin`, `onfocusout`, `onkeydown`, `onkeyup`, `onpointerdown`, `onpointerup`, `onwheel`.
<!-- END GENERATED -->

## Methods

<!-- BEGIN GENERATED: methods of kinds.Checkbox (tools/docs/egui/gen_reference.py). Do not edit. -->
None.
<!-- END GENERATED -->

## Behaviour you need to know

<!-- TODO(DOC7): what the tables don't say: timing, edge cases, limits. -->

## CSS

<!-- BEGIN GENERATED: css of kinds.Checkbox (tools/docs/egui/gen_reference.py). Do not edit. -->
**Selector:** `checkbox`, plus `.class`, `#id` and attribute selectors on its props.

**Painted as:** a themed control. `background-color`, `border-*` and `color` set its look in each interaction state (normal, `:hover`, `:active`); outer `box-shadow` and `opacity` apply.

| Property | Applies | Theme default |
|---|---|---|
| `background-color` | yes, per state | `var(--sv-checkbox-bg)` |
| `border-color` | yes | `var(--sv-checkbox-border-color)` |
| `border-width` | yes | `var(--sv-stroke)` |
| `border-radius` | yes | `var(--sv-button-radius)` |
| `color` | yes (text and marks) | `var(--sv-button-fg)` |
| `box-shadow` | outer shadows only | — |
| `opacity` | yes | — |

Theme rules for its states: `checkbox:hover` sets `background-color: var(--sv-button-bg-hover)`; `checkbox:active` sets `background-color: var(--sv-button-bg-active)`.

**Layout properties** (as attributes or in CSS; an explicit attribute wins): `align-self`, `bottom`, `flex-basis`, `flex-grow`, `flex-shrink`, `height`, `left`, `margin`, `max-height`, `max-width`, `min-height`, `min-width`, `position`, `right`, `top`, `visible`, `width`.
Also in CSS only: `display`, `grid-column`, `grid-row`, `margin-bottom`, `margin-left`, `margin-right`, `margin-top`.

Full matrix: [css/properties](../../css/properties.md). States: below.
<!-- END GENERATED -->

## States

<!-- BEGIN GENERATED: states of kinds.Checkbox (tools/docs/egui/gen_reference.py). Do not edit. -->
| Pseudo-class | Set when | Source |
|---|---|---|
| `:hover` | the pointer is over it | response |
| `:active` | the primary button is down on it | response |
| `:focus` | it has keyboard focus | response |
| `:focus-within` | it, or an element inside it, has keyboard focus | scene |
| `:disabled` (`:enabled` is the opposite) | `enabled` is false | prop |
| `:exiting` | it plays an out transition before it is removed | scene |
| `:checked` | `checked` is true | prop |

Never set on it (rules with them don't match): `:selected`, `:open`, `:expanded`, `:invalid`, `:read-only`, `:dragging`, `:busy`, `:indeterminate`, `:placeholder-shown`, `:default`, `:drop-target`, `:current`, `:backdrop`, `:required`, `:user-invalid`, `:has-children`.

States are tracked once the scene has a style input (a `<style>` block, `class`, `style=` or a theme).
<!-- END GENERATED -->

## Accessibility

<!-- BEGIN GENERATED: a11y of kinds.Checkbox (tools/docs/egui/gen_reference.py). Do not edit. -->
| | |
|---|---|
| Role | `CheckBox` |
| Name | `accessible-name`, else `text`, else tooltip (when `text` is empty) |
| Value | checked or not (`checked`) |
| Actions | `Click`, `Focus` |
| `invalid` | the `invalid` prop sets AccessKit `invalid` |
| Author id | `anchor` (unique in its root) |
<!-- END GENERATED -->

## Testing

<!-- TODO(DOC7): how to find and drive it in a kittest harness. -->

## See also

<!-- TODO(DOC7) -->
