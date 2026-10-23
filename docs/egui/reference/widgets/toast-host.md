---
title: toast-host
tag: toast-host
kind: ToastHost
category: overlays
description: "Shows transient notifications stacked in a corner of its viewport, over everything; takes no space itself."
status: stub
verified_at: ~
generated: partial   # regions between BEGIN/END GENERATED markers come from the spec
sources:
  - metadata/semantics/egui.json#kinds.ToastHost
---

# `<toast-host>`

<!-- BEGIN GENERATED: summary of kinds.ToastHost (tools/docs/egui/gen_reference.py). Do not edit. -->
Shows transient notifications stacked in a corner of its viewport, over everything; takes no space itself. `show` from JS (`bind:this`), `svelters_egui::toast::show` from Rust.

**Base:** [`Widget`](widget.md) · **Children:** none · **Text content:** no
<!-- END GENERATED -->

<!-- TODO(DOC7): when to use it, and what to use instead. -->

## Example

<!-- TODO(DOC7): a checked example (a testkit case under docs/egui/cases/). -->

## Props

<!-- BEGIN GENERATED: props of kinds.ToastHost (tools/docs/egui/gen_reference.py). Do not edit. -->
| Prop | Type | Default | `bind:` | Description |
|---|---|---|---|---|
| `corner` | `'bottom-right' \| 'bottom-left' \| 'top-right' \| 'top-left'` | `"bottom-right"` | no | The viewport corner the toasts stack in. |
| `duration` | `number` | `4` | no | Seconds a toast stays when `show` gives none; 0 keeps it until closed. |

Plus the 26 props every element has, from [`Widget`](widget.md#props): `accessible-name`, `align-self`, `anchor`, `autofocus`, `bottom`, `drag-data`, `drag-type`, `drop-types`, `enabled`, `flex-basis`, `flex-grow`, `flex-shrink`, `height`, `invalid`, `left`, `margin`, `max-height`, `max-width`, `min-height`, `min-width`, `position`, `right`, `tooltip`, `top`, `visible`, `width`.
<!-- END GENERATED -->

## Events

<!-- BEGIN GENERATED: events of kinds.ToastHost (tools/docs/egui/gen_reference.py). Do not edit. -->
| Attribute | Handler | When |
|---|---|---|
| `onaction` | `(id: number, index: number) => void` | One of a toast's action buttons was pressed (`index` into `show`'s `actions`); the toast then closes (`closed(id, 'action')`). |
| `onclicked` | `(id: number) => void` | A toast's body was clicked. |
| `onclosed` | `(id: number, reason: string) => void` | A toast went away: `reason` is `timeout`, `dismissed` (its close button), `action` (one of its actions), `method` (`dismiss`/`clear`). |

`on:action` works too, and a bare `on:<event>` on a component forwards the component's `on<event>` prop.

Plus the events of [`Widget`](widget.md#events): `onanimationend`, `onclick`, `oncontextmenu`, `ondblclick`, `ondrag-begin`, `ondrag-end`, `ondrag-enter`, `ondrag-leave`, `ondrop`, `onfocusin`, `onfocusout`, `onkeydown`, `onkeyup`, `onpointerdown`, `onpointerup`, `onwheel`.
<!-- END GENERATED -->

## Methods

<!-- BEGIN GENERATED: methods of kinds.ToastHost (tools/docs/egui/gen_reference.py). Do not edit. -->
| Method | Returns | Description |
|---|---|---|
| `clear` | `void` | Closes every toast. |
| `dismiss` | `void` | `dismiss(id)`: closes that toast (`closed(id, 'method')`). |
| `show` | `void` | `show(text, level = 'info', duration = the host's, actions = [])`: shows a toast (level: info, success, warning, error; duration in seconds, 0 = until closed; `actions`: button labels, see `action`) and returns its id. `text` may be a run list (RT2): strings and `{text, b, i, code, href}` objects, drawn as one paragraph with the theme's run defaults; a click on a run with an `href` opens it. |

Call them through `bind:this`: `el.call('clear')`.
<!-- END GENERATED -->

## Behaviour you need to know

<!-- TODO(DOC7): what the tables don't say: timing, edge cases, limits. -->

## CSS

<!-- BEGIN GENERATED: css of kinds.ToastHost (tools/docs/egui/gen_reference.py). Do not edit. -->
**Selector:** `toast-host`, plus `.class`, `#id` and attribute selectors on its props.

**Painted as:** nothing CSS reaches. No paint property applies to the element itself; style its children instead.

| Property | Applies | Theme default |
|---|---|---|
| `background-color` | no | — |
| `border-color` | no | — |
| `border-width` | no | — |
| `border-radius` | no | — |
| `color` | no | — |
| `box-shadow` | no | — |
| `opacity` | no | — |

**Layout properties** (as attributes or in CSS; an explicit attribute wins): `align-self`, `bottom`, `flex-basis`, `flex-grow`, `flex-shrink`, `height`, `left`, `max-height`, `max-width`, `min-height`, `min-width`, `position`, `right`, `top`, `visible`, `width`.
Also in CSS only: `display`, `grid-column`, `grid-row`, `margin-bottom`, `margin-left`, `margin-right`, `margin-top`.

The toasts are drawn over everything in the theme's look; CSS does not reach them.

Full matrix: [css/properties](../../css/properties.md). States: below.
<!-- END GENERATED -->

## States

<!-- BEGIN GENERATED: states of kinds.ToastHost (tools/docs/egui/gen_reference.py). Do not edit. -->
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

<!-- BEGIN GENERATED: a11y of kinds.ToastHost (tools/docs/egui/gen_reference.py). Do not edit. -->
| | |
|---|---|
| Role | none of its own; its content is announced in place |
| `anchor`, `invalid` | not announced on this element (`anchor` still keys its ids) |

Each toast is an `Alert` node named by its text, with a `Dismiss` button.
<!-- END GENERATED -->

## Testing

<!-- TODO(DOC7): how to find and drive it in a kittest harness. -->

## See also

<!-- TODO(DOC7) -->
