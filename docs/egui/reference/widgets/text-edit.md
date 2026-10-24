---
title: text-edit
tag: text-edit
kind: TextEdit
category: inputs
description: "A single- or multi-line text field."
status: stub
verified_at: ~
generated: partial   # regions between BEGIN/END GENERATED markers come from the spec
sources:
  - metadata/semantics/egui.json#kinds.TextEdit
---

# `<text-edit>`

<!-- BEGIN GENERATED: summary of kinds.TextEdit (tools/docs/egui/gen_reference.py). Do not edit. -->
A single- or multi-line text field.

**Base:** [`Widget`](widget.md) · **Children:** none · **Text content:** no
<!-- END GENERATED -->

<!-- TODO(DOC7): when to use it, and what to use instead. -->

## Example

<!-- TODO(DOC7): a checked example (a testkit case under docs/egui/cases/). -->

## Props

<!-- BEGIN GENERATED: props of kinds.TextEdit (tools/docs/egui/gen_reference.py). Do not edit. -->
| Prop | Type | Default | `bind:` | Description |
|---|---|---|---|---|
| `char-limit` | `number` | `0` | no | Maximum number of characters; 0 for none. |
| `hint-text` | `string` | `""` | no | Shown while the text is empty; also the default accessible name. |
| `multiline` | `boolean` | `false` | no | Several lines; Enter inserts a newline. |
| `password` | `boolean` | `false` | no | Hides the characters. |
| `text` | `string` | `""` | **yes**, on `changed` | The edited text; typing changes it. |

Plus the 30 props every element has, from [`Widget`](widget.md#props): `accessible-name`, `align-self`, `anchor`, `anchor-scope`, `autofocus`, `bottom`, `dialog-role`, `drag-data`, `drag-type`, `drop-types`, `enabled`, `flex-basis`, `flex-grow`, `flex-shrink`, `height`, `invalid`, `left`, `margin`, `max-height`, `max-width`, `min-height`, `min-width`, `position`, `right`, `status-section`, `status-tip`, `tooltip`, `top`, `visible`, `width`.
<!-- END GENERATED -->

## Events

<!-- BEGIN GENERATED: events of kinds.TextEdit (tools/docs/egui/gen_reference.py). Do not edit. -->
| Attribute | Handler | When |
|---|---|---|
| `onchanged` | `(text: string) => void` | The user edited the text; carries the new text (once per frame). |
| `oncommit` | `(text: string) => void` | The user committed the text: Enter in a one-line field (also when a default button takes it), or losing focus after an edit since the last commit; carries the text. |
| `ongained-focus` | `() => void` | Took keyboard focus. |
| `onlost-focus` | `() => void` | Lost keyboard focus. |
| `onsubmit` | `(text: string) => void` | Enter in a one-line field (also when a default button takes it); carries the text. Fires after `commit`. |

`on:changed` works too, and a bare `on:<event>` on a component forwards the component's `on<event>` prop.

Plus the events of [`Widget`](widget.md#events): `onanimationend`, `onclick`, `oncontextmenu`, `ondblclick`, `ondrag-begin`, `ondrag-end`, `ondrag-enter`, `ondrag-leave`, `ondrop`, `onfocusin`, `onfocusout`, `onkeydown`, `onkeyup`, `onpointerdown`, `onpointermove`, `onpointerup`, `onwheel`.
<!-- END GENERATED -->

## Methods

<!-- BEGIN GENERATED: methods of kinds.TextEdit (tools/docs/egui/gen_reference.py). Do not edit. -->
| Method | Returns | Description |
|---|---|---|
| `request-focus` | `void` | Moves keyboard focus here on the next frame. |

Call them through `bind:this`: `el.call('request-focus')`.
<!-- END GENERATED -->

## Behaviour you need to know

<!-- TODO(DOC7): what the tables don't say: timing, edge cases, limits. -->

## CSS

<!-- BEGIN GENERATED: css of kinds.TextEdit (tools/docs/egui/gen_reference.py). Do not edit. -->
**Selector:** `text-edit`, plus `.class`, `#id` and attribute selectors on its props.

**Painted as:** a themed control. `background-color`, `border-*` and `color` set its look in each interaction state (normal, `:hover`, `:active`); outer `box-shadow` and `opacity` apply.

| Property | Applies | Theme default |
|---|---|---|
| `background-color` | yes, per state | — |
| `border-color` | yes | `var(--sv-input-border-color)` |
| `border-width` | yes | `var(--sv-stroke)` |
| `border-radius` | yes | `var(--sv-input-radius)` |
| `color` | yes (text and marks) | `var(--sv-input-fg)` |
| `box-shadow` | outer shadows only | — |
| `opacity` | yes | — |

**Layout properties** (as attributes or in CSS; an explicit attribute wins): `align-self`, `bottom`, `dialog-role`, `flex-basis`, `flex-grow`, `flex-shrink`, `height`, `left`, `max-height`, `max-width`, `min-height`, `min-width`, `position`, `right`, `status-section`, `top`, `visible`, `width`.
Also in CSS only: `display`, `grid-column`, `grid-row`, `margin-bottom`, `margin-left`, `margin-right`, `margin-top`.

The field's fill follows the `--sv-input-bg` token; prefer the input tokens to rules.

Full matrix: [css/properties](../../css/properties.md). States: below.
<!-- END GENERATED -->

## States

<!-- BEGIN GENERATED: states of kinds.TextEdit (tools/docs/egui/gen_reference.py). Do not edit. -->
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

<!-- BEGIN GENERATED: a11y of kinds.TextEdit (tools/docs/egui/gen_reference.py). Do not edit. -->
| | |
|---|---|
| Role | `TextInput`; `MultilineTextInput` when `multiline`; `PasswordInput` when `password` |
| Name | `accessible-name`, else `hint-text`, else `tooltip` |
| Value | the text |
| Actions | `Focus`, `SetValue`, `ReplaceSelectedText`, `SetTextSelection` |
| `invalid` | the `invalid` prop sets AccessKit `invalid` |
| Author id | `anchor` (unique in its root) |
<!-- END GENERATED -->

## Testing

<!-- TODO(DOC7): how to find and drive it in a kittest harness. -->

## See also

<!-- TODO(DOC7) -->
