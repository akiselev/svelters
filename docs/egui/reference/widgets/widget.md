---
title: widget
kind: Widget
category: base
description: "Common properties of everything placed in a window: state, tooltip, accessibility and its taffy item style."
status: stub
verified_at: ~
generated: partial   # regions between BEGIN/END GENERATED markers come from the spec
sources:
  - metadata/semantics/egui.json#kinds.Widget
---

# `Widget` (base)

<!-- BEGIN GENERATED: summary of kinds.Widget (tools/docs/egui/gen_reference.py). Do not edit. -->
Common properties of everything placed in a window: state, tooltip, accessibility and its taffy item style.

**Abstract:** documents the props its family shares; not written as a tag
<!-- END GENERATED -->

<!-- TODO(DOC7): when to use it, and what to use instead. -->

## Example

<!-- TODO(DOC7): a checked example (a testkit case under docs/egui/cases/). -->

## Props

<!-- BEGIN GENERATED: props of kinds.Widget (tools/docs/egui/gen_reference.py). Do not edit. -->
| Prop | Type | Default | `bind:` | Description |
|---|---|---|---|---|
| `accessible-name` | `string` | `""` | no | AccessKit name; inputs default to their hint or text. |
| `align-self` | `'auto' \| 'start' \| 'end' \| 'center' \| 'stretch' \| 'baseline'` | `"auto"` | no | Cross-axis alignment in the parent box, overriding its `align-items`. |
| `anchor` | `string` | `""` | no | A name unique in its embedded root: the node's egui id derives from it (stable across runs, not only across reorders), it is the AccessKit author id, and `Embed::anchor_rect` finds it. Changing it drops the node's egui memory. |
| `autofocus` | `boolean` | `false` | no | Takes keyboard focus when it first appears (in a `dialog` scope: when the dialog opens). |
| `bottom` | `number \| null` | `null` | no | Inset from the containing block's bottom edge in points; null for auto. |
| `drag-data` | `string` | `""` | no | The drag payload's data (JSON for structured data); `drop` delivers it. |
| `drag-type` | `string` | `""` | no | Makes the node a drag source: the payload's type, a MIME-like string (`cad/part`); empty for none. |
| `drop-types` | `string` | `""` | no | Makes the node a drop target: the drag types it accepts, separated by spaces or commas (`*` any, `cad/*` a prefix); empty for none. |
| `enabled` | `boolean` | `true` | no | Takes input when true; drawn greyed out otherwise. |
| `flex-basis` | `number \| null` | `null` | no | Main-axis size before growing or shrinking; null for auto. |
| `flex-grow` | `number` | `0` | no | Share of the parent's free main-axis space. |
| `flex-shrink` | `number` | `1` | no | Share of the parent's main-axis overflow to give up. |
| `height` | `number \| null` | `null` | no | Height in points; null for auto. |
| `invalid` | `boolean` | `false` | no | Marks the value as invalid: an outline in the error colour and AccessKit `invalid`. The app sets it; the widget never does. |
| `left` | `number \| null` | `null` | no | Inset from the containing block's left edge in points (`position: absolute` or `relative`); null for auto. |
| `margin` | `number` | `0` | no | Outer spacing on all four sides, in points. |
| `max-height` | `number \| null` | `null` | no | Maximum height in points; null for none. |
| `max-width` | `number \| null` | `null` | no | Maximum width in points; null for none. |
| `min-height` | `number \| null` | `null` | no | Minimum height in points; null for auto. |
| `min-width` | `number \| null` | `null` | no | Minimum width in points; null for auto. |
| `position` | `'static' \| 'relative' \| 'absolute'` | `"static"` | no | `absolute` takes the node out of the flow and places it by `left`/`top`/`right`/`bottom` inside its containing block: the nearest `relative` or `absolute` ancestor, else the root. |
| `right` | `number \| null` | `null` | no | Inset from the containing block's right edge in points; null for auto. |
| `tooltip` | `string` | `""` | no | Hover text; empty for none. |
| `top` | `number \| null` | `null` | no | Inset from the containing block's top edge in points; null for auto. |
| `visible` | `boolean` | `true` | no | Shown and laid out when true (`display: none` otherwise). |
| `width` | `number \| null` | `null` | no | Width in points; null for auto. |
<!-- END GENERATED -->

## Events

<!-- BEGIN GENERATED: events of kinds.Widget (tools/docs/egui/gen_reference.py). Do not edit. -->
| Attribute | Handler | When |
|---|---|---|
| `onanimationend` | `(id: number) => void` | A native animation (TR2) finished; carries its id. The runtime listens to it itself. |
| `ondrag-begin` | `() => void` | Drag source: the user started dragging this node (`drag-type` set). |
| `ondrag-end` | `(dropped: boolean) => void` | Drag source: the drag ended; `dropped` is true when a target took it. |
| `ondrag-enter` | `(type: string) => void` | Drop target: a drag it accepts entered it (`drop-types` set); carries the drag's type. |
| `ondrag-leave` | `() => void` | Drop target: the drag left, was cancelled, or was dropped here (before `drop`). |
| `ondrop` | `(type: string, data: string, x: number, y: number) => void` | Drop target: a drag it accepts was released over it; carries the type, the data and the point in the node's own points. |

`on:animationend` works too, and a bare `on:<event>` on a component forwards the component's `on<event>` prop.
<!-- END GENERATED -->

## Methods

<!-- BEGIN GENERATED: methods of kinds.Widget (tools/docs/egui/gen_reference.py). Do not edit. -->
None.
<!-- END GENERATED -->

## Behaviour you need to know

<!-- TODO(DOC7): what the tables don't say: timing, edge cases, limits. -->

## CSS

<!-- BEGIN GENERATED: css of kinds.Widget (tools/docs/egui/gen_reference.py). Do not edit. -->
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

What a given element paints is on its page: themed controls take their look per interaction state, other elements get a box.

Full matrix: [css/properties](../../css/properties.md). States: below.
<!-- END GENERATED -->

## States

<!-- BEGIN GENERATED: states of kinds.Widget (tools/docs/egui/gen_reference.py). Do not edit. -->
| Pseudo-class | Set when | Source |
|---|---|---|
| `:hover` | the pointer is over it | response |
| `:active` | the primary button is down on it | response |
| `:focus` | it has keyboard focus | response |
| `:focus-within` | it, or an element inside it, has keyboard focus | scene |
| `:disabled` (`:enabled` is the opposite) | `enabled` is false | prop |
| `:exiting` | it plays an out transition before it is removed | scene |

These are the states the walk can set on any element; each element's page lists the ones it gets.

States are tracked once the scene has a style input (a `<style>` block, `class`, `style=` or a theme).
<!-- END GENERATED -->

## Accessibility

<!-- BEGIN GENERATED: a11y of kinds.Widget (tools/docs/egui/gen_reference.py). Do not edit. -->
| | |
|---|---|
| Role | none of its own; its content is announced in place |
| Name | `accessible-name` |
| `invalid` | the `invalid` prop sets AccessKit `invalid` |
| Author id | `anchor` (unique in its root) |

Each element page says what its own node is. Containers that only lay out (`box`, `grid`, ...) have no node: `anchor` and `invalid` reach AccessKit only on elements that have one.
<!-- END GENERATED -->

## Testing

<!-- TODO(DOC7): how to find and drive it in a kittest harness. -->

## See also

<!-- TODO(DOC7) -->
