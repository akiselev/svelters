---
title: status-bar
tag: status-bar
kind: StatusBar
category: containers
description: "A generic status bar (QStatusBar): a flex row whose children are its sections, placed by their `status-section`: `start` sections first, then the `temporary` ones, then the `end` sections pushed to the far end; `flex-grow` stretches a section."
status: stub
verified_at: ~
generated: partial   # regions between BEGIN/END GENERATED markers come from the spec
sources:
  - metadata/semantics/egui.json#kinds.StatusBar
---

# `<status-bar>`

<!-- BEGIN GENERATED: summary of kinds.StatusBar (tools/docs/egui/gen_reference.py). Do not edit. -->
A generic status bar (QStatusBar): a flex row whose children are its sections, placed by their `status-section`: `start` sections first, then the `temporary` ones, then the `end` sections pushed to the far end; `flex-grow` stretches a section. A message (`message`, `show-message`) or a status tip (a hovered or focused node's `status-tip`) is drawn in `::part(message)`, the free space after the start sections, and hides the temporary sections meanwhile. It holds no app-specific sections: compose your own from labels, buttons and the like.

**Base:** [`box`](box.md) → [`Widget`](widget.md) · **Children:** any element · **Text content:** no
<!-- END GENERATED -->

<!-- TODO(DOC7): when to use it, and what to use instead. -->

## Example

<!-- TODO(DOC7): a checked example (a testkit case under docs/egui/cases/). -->

## Props

<!-- BEGIN GENERATED: props of kinds.StatusBar (tools/docs/egui/gen_reference.py). Do not edit. -->
| Prop | Type | Default | `bind:` | Description |
|---|---|---|---|---|
| `message` | `string` | `""` | **yes**, on `changed` | The temporary message (Qt `currentMessage`), drawn in `::part(message)` over the temporary sections; empty for none. Setting it shows it until it is changed or cleared; `show-message` sets it with a timeout, after which it is empty again (`bind:message`). |

From [`box`](box.md):

| Prop | Type | Default | `bind:` | Description |
|---|---|---|---|---|
| `align-items` | `'start' \| 'end' \| 'center' \| 'stretch' \| 'baseline'` | `"stretch"` | no | Cross-axis alignment of the children. |
| `column-gap` | `number \| null` | `null` | no | Space between columns in points; null to use `gap`. |
| `flex-direction` | `'column' \| 'row' \| 'column-reverse' \| 'row-reverse'` | `"column"` | no | Main axis of the box. |
| `flex-wrap` | `'nowrap' \| 'wrap' \| 'wrap-reverse'` | `"nowrap"` | no | Whether children wrap onto more lines. |
| `focus-scope` | `'none' \| 'contain' \| 'dialog'` | `"none"` | no | `contain`: Tab and Shift+Tab cycle inside while focus is in it; `dialog`: also takes focus when it appears (`autofocus` node, else the first) and gives it back to the opener when it goes away. |
| `gap` | `number` | `0` | no | Space between children on both axes, in points (row-gap and column-gap override it). |
| `justify-content` | `'start' \| 'end' \| 'center' \| 'space-between' \| 'space-around' \| 'space-evenly'` | `"start"` | no | Main-axis distribution of the children. |
| `padding` | `number` | `0` | no | Inner spacing on all four sides, in points. |
| `row-gap` | `number \| null` | `null` | no | Space between rows in points; null to use `gap`. |

Plus the 30 props every element has, from [`Widget`](widget.md#props): `accessible-name`, `align-self`, `anchor`, `anchor-scope`, `autofocus`, `bottom`, `dialog-role`, `drag-data`, `drag-type`, `drop-types`, `enabled`, `flex-basis`, `flex-grow`, `flex-shrink`, `height`, `invalid`, `left`, `margin`, `max-height`, `max-width`, `min-height`, `min-width`, `position`, `right`, `status-section`, `status-tip`, `tooltip`, `top`, `visible`, `width`.
<!-- END GENERATED -->

## Events

<!-- BEGIN GENERATED: events of kinds.StatusBar (tools/docs/egui/gen_reference.py). Do not edit. -->
| Attribute | Handler | When |
|---|---|---|
| `onchanged` | `(message: string) => void` | Its message changed (Qt `messageChanged`): `show-message`, `clear-message` or a timeout; carries the new message. |

`on:changed` works too, and a bare `on:<event>` on a component forwards the component's `on<event>` prop.

Plus the events of [`Widget`](widget.md#events): `onanimationend`, `onclick`, `oncontextmenu`, `ondblclick`, `ondrag-begin`, `ondrag-end`, `ondrag-enter`, `ondrag-leave`, `ondrop`, `onfocusin`, `onfocusout`, `onkeydown`, `onkeyup`, `onpointerdown`, `onpointermove`, `onpointerup`, `onwheel`.
<!-- END GENERATED -->

## Methods

<!-- BEGIN GENERATED: methods of kinds.StatusBar (tools/docs/egui/gen_reference.py). Do not edit. -->
| Method | Returns | Description |
|---|---|---|
| `clear-message` | `void` | Clears the message (Qt `clearMessage`). |
| `show-message` | `void` | `show-message(text, timeout = 0)`: shows `text` in `::part(message)` for `timeout` seconds (0: until changed or cleared), hiding the temporary sections (Qt `showMessage`). |

Call them through `bind:this`: `el.call('clear-message')`.
<!-- END GENERATED -->

## Behaviour you need to know

<!-- TODO(DOC7): what the tables don't say: timing, edge cases, limits. -->

## CSS

<!-- BEGIN GENERATED: css of kinds.StatusBar (tools/docs/egui/gen_reference.py). Do not edit. -->
**Selector:** `status-bar`, plus `.class`, `#id` and attribute selectors on its props.

**Painted as:** a box. The CSS engine paints its shadows, background and border at its rect, under its content; `opacity` fades it and its content.

| Property | Applies | Theme default |
|---|---|---|
| `background-color` | yes | — |
| `border-color` | yes | — |
| `border-width` | yes | — |
| `border-radius` | yes | — |
| `color` | inherited by its children | — |
| `box-shadow` | yes (outer and inset) | — |
| `opacity` | yes | — |

**Layout properties** (as attributes or in CSS; an explicit attribute wins): `align-items`, `align-self`, `bottom`, `column-gap`, `dialog-role`, `flex-basis`, `flex-direction`, `flex-grow`, `flex-shrink`, `flex-wrap`, `gap`, `height`, `justify-content`, `left`, `max-height`, `max-width`, `min-height`, `min-width`, `position`, `right`, `row-gap`, `status-section`, `top`, `visible`, `width`.
Also in CSS only: `display`, `grid-column`, `grid-row`, `margin-bottom`, `margin-left`, `margin-right`, `margin-top`.

`status-bar` styles the bar (the `panel-bg` fill and a top `border-color` line by default); `status-bar > *` and `[status-section=…]` its sections; `::part(message)` the message and status-tip text (`color`, `font-size`, `padding-left`).

Full matrix: [css/properties](../../css/properties.md). States: below.
<!-- END GENERATED -->

## States

<!-- BEGIN GENERATED: states of kinds.StatusBar (tools/docs/egui/gen_reference.py). Do not edit. -->
| Pseudo-class | Set when | Source |
|---|---|---|
| `:hover` | the pointer is over its box | response |
| `:active` | the primary button is down over its box | response |
| `:focus-within` | an element inside it has keyboard focus | scene |
| `:disabled` (`:enabled` is the opposite) | `enabled` is false | prop |
| `:exiting` | it plays an out transition before it is removed | scene |
| `:hover` | the pointer is over it | kind |
| `:open` | a message or a status tip shows (its temporary sections are hidden) | kind |

Never set on it (rules with them don't match): `:focus`, `:checked`, `:selected`, `:expanded`, `:invalid`, `:read-only`, `:dragging`, `:busy`, `:indeterminate`, `:placeholder-shown`, `:default`, `:drop-target`, `:current`, `:backdrop`, `:required`, `:user-invalid`, `:has-children`.

States are tracked once the scene has a style input (a `<style>` block, `class`, `style=` or a theme).
<!-- END GENERATED -->

## Accessibility

<!-- BEGIN GENERATED: a11y of kinds.StatusBar (tools/docs/egui/gen_reference.py). Do not edit. -->
| | |
|---|---|
| Role | `Status` |
| Name | the message or status tip |
| `anchor`, `invalid` | not announced on this element (`anchor` still keys its ids) |

A polite live region: a new message or status tip is announced without interrupting.
<!-- END GENERATED -->

## Testing

<!-- TODO(DOC7): how to find and drive it in a kittest harness. -->

## See also

<!-- TODO(DOC7) -->
