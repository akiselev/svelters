---
title: combo-box
tag: combo-box
kind: ComboBox
category: inputs
description: "A drop-down list: shows the selected option, picks one from a popup."
status: stub
verified_at: ~
generated: partial   # regions between BEGIN/END GENERATED markers come from the spec
sources:
  - metadata/semantics/egui.json#kinds.ComboBox
---

# `<combo-box>`

<!-- BEGIN GENERATED: summary of kinds.ComboBox (tools/docs/egui/gen_reference.py). Do not edit. -->
A drop-down list: shows the selected option, picks one from a popup. Name it with `accessible-name` (it has no caption).

**Base:** [`Widget`](widget.md) · **Children:** none · **Text content:** no
<!-- END GENERATED -->

<!-- TODO(DOC7): when to use it, and what to use instead. -->

## Example

<!-- TODO(DOC7): a checked example (a testkit case under docs/egui/cases/). -->

## Props

<!-- BEGIN GENERATED: props of kinds.ComboBox (tools/docs/egui/gen_reference.py). Do not edit. -->
| Prop | Type | Default | `bind:` | Description |
|---|---|---|---|---|
| `hint-text` | `string` | `""` | no | Shown while `selected` is out of range. |
| `max-visible` | `number` | `30` | no | Rows shown before the list scrolls (QComboBox `maxVisibleItems`). |
| `open` | `boolean` | `false` | **yes**, on `changed` | The list is open; the user opening or closing it changes it, setting it opens or closes it (QComboBox `showPopup`/`hidePopup`). |
| `options` | `string[]` | `null` | no | The choices, in order (the popup's rows). |
| `selected` | `number` | `0` | **yes**, on `changed` | Index of the chosen option; picking a row changes it. Out of range shows no text. |
| `wheel` | `'off' \| 'hover' \| 'focused'` | `"off"` | no | The wheel over the closed select changes the pick: `off` (default), `hover`, `focused`. |

Plus the 31 props every element has, from [`Widget`](widget.md#props): `accessible-name`, `align-self`, `anchor`, `anchor-scope`, `autofocus`, `bottom`, `dialog-role`, `drag-data`, `drag-type`, `drop-types`, `enabled`, `flex-basis`, `flex-grow`, `flex-shrink`, `height`, `invalid`, `left`, `margin`, `max-height`, `max-width`, `min-height`, `min-width`, `position`, `right`, `source-loc`, `status-section`, `status-tip`, `tooltip`, `top`, `visible`, `width`.
<!-- END GENERATED -->

## Events

<!-- BEGIN GENERATED: events of kinds.ComboBox (tools/docs/egui/gen_reference.py). Do not edit. -->
| Attribute | Handler | When |
|---|---|---|
| `onactivate` | `(value: number) => void` | The user picked an option, the same one or not (QComboBox `activated`); carries its index. |
| `onchanged` | `(selected: number) => void` | The user picked a row; carries its index. |
| `oncommit` | `(value: number) => void` | A pick finished the edit (§2.4); carries the index. |
| `onhighlight` | `(value: number) => void` | The current row of the open list moved (keyboard or pointer; `highlighted`). |

`on:activate` works too, and a bare `on:<event>` on a component forwards the component's `on<event>` prop.

Plus the events of [`Widget`](widget.md#events): `onanimationend`, `onclick`, `oncontextmenu`, `ondblclick`, `ondrag-begin`, `ondrag-end`, `ondrag-enter`, `ondrag-leave`, `ondrop`, `onfocusin`, `onfocusout`, `onkeydown`, `onkeyup`, `onpointerdown`, `onpointermove`, `onpointerup`, `onwheel`.
<!-- END GENERATED -->

## Methods

<!-- BEGIN GENERATED: methods of kinds.ComboBox (tools/docs/egui/gen_reference.py). Do not edit. -->
None.
<!-- END GENERATED -->

## Behaviour you need to know

<!-- TODO(DOC7): what the tables don't say: timing, edge cases, limits. -->

## CSS

<!-- BEGIN GENERATED: css of kinds.ComboBox (tools/docs/egui/gen_reference.py). Do not edit. -->
**Selector:** `combo-box`, plus `.class`, `#id` and attribute selectors on its props.

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

`:focus` does not match it yet; use `:focus-within` on its container.

Full matrix: [css/properties](../../css/properties.md). States: below.
<!-- END GENERATED -->

## States

<!-- BEGIN GENERATED: states of kinds.ComboBox (tools/docs/egui/gen_reference.py). Do not edit. -->
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

<!-- BEGIN GENERATED: a11y of kinds.ComboBox (tools/docs/egui/gen_reference.py). Do not edit. -->
| | |
|---|---|
| Role | `ComboBox` |
| Name | `accessible-name`, else `tooltip` |
| Value | the selected option's text |
| Actions | `Click`, `Focus` |
| `invalid` | the `invalid` prop sets AccessKit `invalid` |
| Author id | `anchor` (unique in its root) |

Its open list is a `ListBox`. It shows no caption of its own: name it with `accessible-name` (or a `tooltip`).
<!-- END GENERATED -->

## Testing

<!-- TODO(DOC7): how to find and drive it in a kittest harness. -->

## See also

<!-- TODO(DOC7) -->
