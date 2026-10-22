---
title: image
tag: image
kind: Image
category: media
description: "A picture from `source`, at its own size unless `width`/`height` say otherwise."
status: stub
verified_at: ~
generated: partial   # regions between BEGIN/END GENERATED markers come from the spec
sources:
  - metadata/semantics/egui.json#kinds.Image
---

# `<image>`

<!-- BEGIN GENERATED: summary of kinds.Image (tools/docs/egui/gen_reference.py). Do not edit. -->
A picture from `source`, at its own size unless `width`/`height` say otherwise. AccessKit role `Image`, named by `accessible-name` or `tooltip`.

**Base:** [`Widget`](widget.md) · **Children:** none · **Text content:** no
<!-- END GENERATED -->

<!-- TODO(DOC7): when to use it, and what to use instead. -->

## Example

<!-- TODO(DOC7): a checked example (a testkit case under docs/egui/cases/). -->

## Props

<!-- BEGIN GENERATED: props of kinds.Image (tools/docs/egui/gen_reference.py). Do not edit. -->
| Prop | Type | Default | `bind:` | Description |
|---|---|---|---|---|
| `fit` | `'contain' \| 'cover' \| 'fill' \| 'none' \| 'scale-down'` | `"contain"` | no | How the image fills its rect (CSS `object-fit`): `contain` keeps the aspect ratio inside it, `cover` fills it and crops, `fill` stretches, `none` keeps the image's size, `scale-down` is the smaller of `none` and `contain`. |
| `source` | `string` | `""` | no | What to show: a file path, a `file://` uri, or any uri an egui loader serves (`bytes://` from `Context::include_bytes`, `http(s)://` with egui_extras' loaders). PNG decodes built in. |

Plus the 26 props every element has, from [`Widget`](widget.md#props): `accessible-name`, `align-self`, `anchor`, `autofocus`, `bottom`, `drag-data`, `drag-type`, `drop-types`, `enabled`, `flex-basis`, `flex-grow`, `flex-shrink`, `height`, `invalid`, `left`, `margin`, `max-height`, `max-width`, `min-height`, `min-width`, `position`, `right`, `tooltip`, `top`, `visible`, `width`.
<!-- END GENERATED -->

## Events

<!-- BEGIN GENERATED: events of kinds.Image (tools/docs/egui/gen_reference.py). Do not edit. -->
| Attribute | Handler | When |
|---|---|---|
| `onclicked` | `() => void` | Pressed and released on it (it senses clicks only while listened). |

`on:clicked` works too, and a bare `on:<event>` on a component forwards the component's `on<event>` prop.

Plus the events of [`Widget`](widget.md#events): `onanimationend`, `ondrag-begin`, `ondrag-end`, `ondrag-enter`, `ondrag-leave`, `ondrop`.
<!-- END GENERATED -->

## Methods

<!-- BEGIN GENERATED: methods of kinds.Image (tools/docs/egui/gen_reference.py). Do not edit. -->
None.
<!-- END GENERATED -->

## Behaviour you need to know

<!-- TODO(DOC7): what the tables don't say: timing, edge cases, limits. -->

## CSS

<!-- BEGIN GENERATED: css of kinds.Image (tools/docs/egui/gen_reference.py). Do not edit. -->
**Selector:** `image`, plus `.class`, `#id` and attribute selectors on its props.

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

**Layout properties** (as attributes or in CSS; an explicit attribute wins): `align-self`, `bottom`, `flex-basis`, `flex-grow`, `flex-shrink`, `height`, `left`, `margin`, `max-height`, `max-width`, `min-height`, `min-width`, `position`, `right`, `top`, `visible`, `width`.
Also in CSS only: `display`, `grid-column`, `grid-row`, `margin-bottom`, `margin-left`, `margin-right`, `margin-top`.

Full matrix: [css/properties](../../css/properties.md). States: below.
<!-- END GENERATED -->

## States

<!-- BEGIN GENERATED: states of kinds.Image (tools/docs/egui/gen_reference.py). Do not edit. -->
| Pseudo-class | Set when | Source |
|---|---|---|
| `:hover` | the pointer is over it | response |
| `:active` | the primary button is down on it | response |
| `:focus` | it takes clicks (`onclicked` is set) and has keyboard focus | response |
| `:focus-within` | it, or an element inside it, has keyboard focus | scene |
| `:disabled` (`:enabled` is the opposite) | `enabled` is false | prop |
| `:exiting` | it plays an out transition before it is removed | scene |

Never set on it (rules with them don't match): `:checked`, `:selected`, `:open`, `:expanded`, `:invalid`, `:read-only`, `:dragging`, `:busy`, `:indeterminate`, `:placeholder-shown`, `:default`, `:drop-target`, `:current`, `:backdrop`, `:required`, `:user-invalid`, `:has-children`.

States are tracked once the scene has a style input (a `<style>` block, `class`, `style=` or a theme).
<!-- END GENERATED -->

## Accessibility

<!-- BEGIN GENERATED: a11y of kinds.Image (tools/docs/egui/gen_reference.py). Do not edit. -->
| | |
|---|---|
| Role | `Image` |
| Name | `accessible-name`, else `tooltip` |
| Actions | Click (with `onclicked`), Focus (with `onclicked`) |
| `invalid` | the `invalid` prop sets AccessKit `invalid` |
| Author id | `anchor` (unique in its root) |
<!-- END GENERATED -->

## Testing

<!-- TODO(DOC7): how to find and drive it in a kittest harness. -->

## See also

<!-- TODO(DOC7) -->
