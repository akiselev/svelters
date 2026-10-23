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
| `onclick` | `(e: SvlEvent<[number, number, number, number, number, number]>, target: number, phase: number, x: number, y: number, button: number, modifiers: number) => void` | The primary button was pressed and released; targets the innermost node that holds both points (EV1). `preventDefault` keeps the click from the widget. **Sync:** the handler runs before the frame's pass and gets an `SvlEvent` first. |
| `oncontextmenu` | `(e: SvlEvent<[number, number, number, number, number, number]>, target: number, phase: number, x: number, y: number, button: number, modifiers: number) => void` | The secondary button went down (EV1), after `pointerdown`. `preventDefault` keeps the press from egui (no context menu). **Sync:** the handler runs before the frame's pass and gets an `SvlEvent` first. |
| `ondblclick` | `(e: SvlEvent<[number, number, number, number, number, number]>, target: number, phase: number, x: number, y: number, button: number, modifiers: number) => void` | A second click on the same node in a short time, after its `click` (EV1). **Sync:** the handler runs before the frame's pass and gets an `SvlEvent` first. |
| `ondrag-begin` | `() => void` | Drag source: the user started dragging this node (`drag-type` set). |
| `ondrag-end` | `(dropped: boolean, action: string) => void` | Drag source: the drag ended; `dropped` is true when a target took it, `action` how (`move`, `copy`, `link`; `none` when not dropped). |
| `ondrag-enter` | `(type: string) => void` | Drop target: a drag it accepts entered it (`drop-types` set); carries the drag's type. |
| `ondrag-leave` | `() => void` | Drop target: the drag left, was cancelled, or was dropped here (before `drop`). |
| `ondrop` | `(type: string, data: string, x: number, y: number, action: string) => void` | Drop target: a drag it accepts was released over it; carries the type, the data, the point in the node's own points and the drop action from the modifiers (`move`; `copy` with Ctrl, Option on macOS; `link` with Ctrl+Shift, Cmd+Option on macOS). Files dragged in from the OS arrive as `text/uri-list` (one `file://` URI per line) with `copy` on a target that accepts that type. |
| `onfocusin` | `(e: SvlEvent<[number, number, number]>, target: number, phase: number, related: number) => void` | This node or a descendant took keyboard focus (EV1; after the frame that moved it). `related` is the node that lost it, 0 for none. **Sync:** the handler runs before the frame's pass and gets an `SvlEvent` first. |
| `onfocusout` | `(e: SvlEvent<[number, number, number]>, target: number, phase: number, related: number) => void` | This node or a descendant lost keyboard focus (EV1). `related` is the node that took it, 0 for none. **Sync:** the handler runs before the frame's pass and gets an `SvlEvent` first. |
| `onkeydown` | `(e: SvlEvent<[number, number, string, boolean, number]>, target: number, phase: number, key: string, repeat: boolean, modifiers: number) => void` | A key went down while this node or a descendant had focus (EV1); `key` is egui's key name. `preventDefault` keeps the key (and the text it types) from the focused widget. **Sync:** the handler runs before the frame's pass and gets an `SvlEvent` first. |
| `onkeyup` | `(e: SvlEvent<[number, number, string, boolean, number]>, target: number, phase: number, key: string, repeat: boolean, modifiers: number) => void` | A key went up while this node or a descendant had focus (EV1). **Sync:** the handler runs before the frame's pass and gets an `SvlEvent` first. |
| `onpointerdown` | `(e: SvlEvent<[number, number, number, number, number, number]>, target: number, phase: number, x: number, y: number, button: number, modifiers: number) => void` | A pointer button went down over this node or a descendant (EV1: capture, target, bubble). `preventDefault` keeps the press from egui (no click, drag or focus). **Sync:** the handler runs before the frame's pass and gets an `SvlEvent` first. |
| `onpointerup` | `(e: SvlEvent<[number, number, number, number, number, number]>, target: number, phase: number, x: number, y: number, button: number, modifiers: number) => void` | A pointer button went up over this node or a descendant (EV1). `preventDefault` cancels the click. **Sync:** the handler runs before the frame's pass and gets an `SvlEvent` first. |
| `onwheel` | `(e: SvlEvent<[number, number, number, number, number, number, number]>, target: number, phase: number, x: number, y: number, dx: number, dy: number, modifiers: number) => void` | The mouse wheel or a trackpad scrolled over this node (EV1); `dx`, `dy` in points. `preventDefault` keeps it from scrolling. **Sync:** the handler runs before the frame's pass and gets an `SvlEvent` first. |

`on:animationend` works too, and a bare `on:<event>` on a component forwards the component's `on<event>` prop.
<!-- END GENERATED -->

## Methods

<!-- BEGIN GENERATED: methods of kinds.Widget (tools/docs/egui/gen_reference.py). Do not edit. -->
| Method | Returns | Description |
|---|---|---|
| `scroll-into-view` | `void` | `scroll-into-view(align?)`: scrolls the scroll areas around the node so it is visible on the next frame, innermost first; `align` is `nearest` (the default: the least scrolling, keeping the area's `scroll-margin`) or `center`. Qt's `ensureWidgetVisible`, the DOM's `scrollIntoView`. |

Call them through `bind:this`: `el.call('scroll-into-view')`.
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
