---
title: icon
tag: icon
kind: Icon
category: display
description: "A glyph from an icon font the embedder registered (SVP6), by name or as text."
status: stub
verified_at: ~
generated: partial   # regions between BEGIN/END GENERATED markers come from the spec
sources:
  - metadata/semantics/egui.json#kinds.Icon
---

# `<icon>`

<!-- BEGIN GENERATED: summary of kinds.Icon (tools/docs/egui/gen_reference.py). Do not edit. -->
A glyph from an icon font the embedder registered (SVP6), by name or as text. With `frame` it is an icon button. Its accessible name is `accessible-name`, else `tooltip`, else `name`.

**Base:** [`Widget`](widget.md) · **Children:** none · **Text content:** no
<!-- END GENERATED -->

<!-- TODO(DOC7): when to use it, and what to use instead. -->

## Example

<!-- TODO(DOC7): a checked example (a testkit case under docs/egui/cases/). -->

## Props

<!-- BEGIN GENERATED: props of kinds.Icon (tools/docs/egui/gen_reference.py). Do not edit. -->
| Prop | Type | Default | `bind:` | Description |
|---|---|---|---|---|
| `family` | `string` | `"icons"` | no | The registered font family holding the glyphs; empty for the proportional family. A family with no fonts, a name not in its table or a glyph no font has draws an outlined placeholder box (`svelters_egui::fonts::FontWarning`). |
| `font-weight` | `number` | `400` | no | CSS font weight (100-900). Drawn with the font family the embedder registered for the nearest weight (`svelters_egui::fonts::register_weight`); 400, or no registered family, draws the regular face. |
| `frame` | `boolean` | `false` | no | Draws it as a button (an icon button: role `Button`, takes focus). |
| `glyph` | `string` | `""` | no | The glyph itself (text), when `name` is empty or not in the table. |
| `name` | `string` | `""` | no | The glyph's name in the family's name table (`svelters_egui::fonts::register_icons`); also the default accessible name. |
| `selected` | `boolean` | `false` | no | With `frame`: drawn as selected (a toggle look). |
| `size` | `number \| null` | `null` | no | Font size in points; null for the body size. |
| `source` | `string \| null` | `null` | no | An image instead of the glyph: an `asset://` ref (an imported SVG or PNG), a file path or any uri an egui loader serves. Measured `size` (else the body size) square and fitted inside; when it does not load the placeholder box is drawn. |
| `tint` | `boolean` | `true` | no | With `source`: tints the image with the text colour (SVG `currentColor` icons render white, then tint like a glyph); false keeps its own colours. |

Plus the 26 props every element has, from [`Widget`](widget.md#props): `accessible-name`, `align-self`, `anchor`, `autofocus`, `bottom`, `drag-data`, `drag-type`, `drop-types`, `enabled`, `flex-basis`, `flex-grow`, `flex-shrink`, `height`, `invalid`, `left`, `margin`, `max-height`, `max-width`, `min-height`, `min-width`, `position`, `right`, `tooltip`, `top`, `visible`, `width`.
<!-- END GENERATED -->

## Events

<!-- BEGIN GENERATED: events of kinds.Icon (tools/docs/egui/gen_reference.py). Do not edit. -->
| Attribute | Handler | When |
|---|---|---|
| `onclicked` | `() => void` | Pressed and released. |
| `ondouble-clicked` | `() => void` | Double click. |
| `onsecondary-clicked` | `() => void` | Right click. |

`on:clicked` works too, and a bare `on:<event>` on a component forwards the component's `on<event>` prop.

Plus the events of [`Widget`](widget.md#events): `onanimationend`, `ondrag-begin`, `ondrag-end`, `ondrag-enter`, `ondrag-leave`, `ondrop`.
<!-- END GENERATED -->

## Methods

<!-- BEGIN GENERATED: methods of kinds.Icon (tools/docs/egui/gen_reference.py). Do not edit. -->
None.
<!-- END GENERATED -->

## Behaviour you need to know

<!-- TODO(DOC7): what the tables don't say: timing, edge cases, limits. -->

## CSS

<!-- BEGIN GENERATED: css of kinds.Icon (tools/docs/egui/gen_reference.py). Do not edit. -->
**Selector:** `icon`, plus `.class`, `#id` and attribute selectors on its props.

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

The glyph takes `color`. With `frame`, it is drawn as a button and `:focus` matches.

Full matrix: [css/properties](../../css/properties.md). States: below.
<!-- END GENERATED -->

## States

<!-- BEGIN GENERATED: states of kinds.Icon (tools/docs/egui/gen_reference.py). Do not edit. -->
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

<!-- BEGIN GENERATED: a11y of kinds.Icon (tools/docs/egui/gen_reference.py). Do not edit. -->
| | |
|---|---|
| Role | `Image`; `Button` when `frame`, or an `onclicked` handler |
| Name | `accessible-name`, else `tooltip`, else `name` |
| Actions | Click (when it is a button), Focus (when it is a button) |
| `invalid` | the `invalid` prop sets AccessKit `invalid` |
| Author id | `anchor` (unique in its root) |
<!-- END GENERATED -->

## Testing

<!-- TODO(DOC7): how to find and drive it in a kittest harness. -->

## See also

<!-- TODO(DOC7) -->
