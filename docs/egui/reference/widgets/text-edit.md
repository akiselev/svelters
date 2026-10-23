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
| `can-copy` | `boolean` | `false` | no | Copy would copy something now: a selection outside a password field (read-only, observable). |
| `can-redo` | `boolean` | `false` | no | Redo would do something now (read-only, observable). |
| `can-undo` | `boolean` | `false` | no | Undo would do something now (read-only, observable). |
| `char-limit` | `number` | `0` | no | Maximum number of characters; 0 for none. |
| `echo` | `'normal' \| 'password' \| 'none'` | `"normal"` | no | `normal`, `password` or `none` (QLineEdit `echoMode`; `none` shows nothing). |
| `hint-text` | `string` | `""` | no | Shown while the text is empty; also the default accessible name. |
| `inputmode` | `string` | `"text"` | no | The input method's purpose: `text`, `decimal`, `numeric`, … (`password` fields always ask for a password IME). |
| `maxlength` | `number` | `0` | no | `char-limit` by its HTML name; wins when above 0. |
| `modified` | `boolean` | `false` | no | The user edited since the last `commit` (QLineEdit `isModified`); the app may clear it. |
| `multiline` | `boolean` | `false` | no | Several lines; Enter inserts a newline. |
| `password` | `boolean` | `false` | no | Hides the characters. |
| `placeholder` | `string` | `""` | no | `hint-text` by its HTML name (QLineEdit `placeholderText`); wins when set. |
| `read-only` | `boolean` | `false` | no | Motions, selection and copy only (QLineEdit `readOnly`, `:read-only`). |
| `rows` | `number` | `4` | no | A multi-line field's height in lines. |
| `selection-end` | `number` | `0` | no | The selection's end, in characters (equal to `selection-start` without a selection). |
| `selection-start` | `number` | `0` | no | The selection's start, in characters (a write selects). |
| `tab-focus` | `boolean` | `true` | no | A multi-line field's Tab moves focus (otherwise it inserts a tab; Ctrl+Tab always moves). |
| `text` | `string` | `""` | **yes**, on `changed` | The edited text; typing changes it. |
| `undo` | `boolean` | `true` | no | Undo and redo (`undo={false}` turns the history off). |
| `wrap` | `'word' \| 'char' \| 'none'` | `"word"` | no | A multi-line field's wrapping: `word`, `char` or `none`. |

Plus the 26 props every element has, from [`Widget`](widget.md#props): `accessible-name`, `align-self`, `anchor`, `autofocus`, `bottom`, `drag-data`, `drag-type`, `drop-types`, `enabled`, `flex-basis`, `flex-grow`, `flex-shrink`, `height`, `invalid`, `left`, `margin`, `max-height`, `max-width`, `min-height`, `min-width`, `position`, `right`, `tooltip`, `top`, `visible`, `width`.
<!-- END GENERATED -->

## Events

<!-- BEGIN GENERATED: events of kinds.TextEdit (tools/docs/egui/gen_reference.py). Do not edit. -->
| Attribute | Handler | When |
|---|---|---|
| `onchanged` | `(text: string) => void` | The user edited the text; carries the new text (once per frame). |
| `oncommit` | `(text: string) => void` | The user committed the text: Enter in a one-line field (also when a default button takes it), or losing focus after an edit since the last commit; carries the text. |
| `ongained-focus` | `() => void` | Took keyboard focus. |
| `onlost-focus` | `() => void` | Lost keyboard focus. |
| `onrejected` | `(value: string) => void` | An edit was refused or cut short: `max-chars`, `read-only`, `filtered`, `line-break`, `secret` (QLineEdit `inputRejected`). |
| `onselection-changed` | `() => void` | The selection or caret moved (read `selection-start` and `selection-end`). |
| `onsubmit` | `(text: string) => void` | Enter in a one-line field (also when a default button takes it); carries the text. Fires after `commit`. |

`on:changed` works too, and a bare `on:<event>` on a component forwards the component's `on<event>` prop.

Plus the events of [`Widget`](widget.md#events): `onanimationend`, `onclick`, `oncontextmenu`, `ondblclick`, `ondrag-begin`, `ondrag-end`, `ondrag-enter`, `ondrag-leave`, `ondrop`, `onfocusin`, `onfocusout`, `onkeydown`, `onkeyup`, `onpointerdown`, `onpointerup`, `onwheel`.
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

**Layout properties** (as attributes or in CSS; an explicit attribute wins): `align-self`, `bottom`, `flex-basis`, `flex-grow`, `flex-shrink`, `height`, `left`, `max-height`, `max-width`, `min-height`, `min-width`, `position`, `right`, `top`, `visible`, `width`.
Also in CSS only: `display`, `grid-column`, `grid-row`, `margin-bottom`, `margin-left`, `margin-right`, `margin-top`.

svelters' own field (OW4): the sunken look of DESIGN R4 from the input tokens; `::placeholder`, `::selection`, `::part(caret)` and `::part(preedit)` style its text.

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
| `:read-only` (`:read-write` is the opposite) | `read-only` is true | prop |
| `:invalid` (`:valid` is the opposite) | `invalid` is true | prop |
| `:user-invalid` | `invalid` after the first commit | kind |
| `:placeholder-shown` | the text is empty and there is a hint | kind |

Never set on it (rules with them don't match): `:checked`, `:selected`, `:open`, `:expanded`, `:dragging`, `:busy`, `:indeterminate`, `:default`, `:drop-target`, `:current`, `:backdrop`, `:required`, `:has-children`.

States are tracked once the scene has a style input (a `<style>` block, `class`, `style=` or a theme).
<!-- END GENERATED -->

## Accessibility

<!-- BEGIN GENERATED: a11y of kinds.TextEdit (tools/docs/egui/gen_reference.py). Do not edit. -->
| | |
|---|---|
| Role | `TextInput`; `MultilineTextInput` when `multiline`; `PasswordInput` when `password` or `echo` |
| Name | `accessible-name`, else `hint-text`, else `placeholder`, else `tooltip` |
| Value | the text (masked in a password field), its text runs, caret and selection |
| Actions | `Focus`, `SetValue`, `ReplaceSelectedText` |
| `invalid` | the `invalid` prop sets AccessKit `invalid` |
| Author id | `anchor` (unique in its root) |
<!-- END GENERATED -->

## Testing

<!-- TODO(DOC7): how to find and drive it in a kittest harness. -->

## See also

<!-- TODO(DOC7) -->
