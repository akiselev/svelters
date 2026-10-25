---
title: window
tag: window
kind: Window
category: containers
description: "A native window (the root viewport, or an immediate viewport); its children are laid out as a flex column."
status: stub
verified_at: ~
generated: partial   # regions between BEGIN/END GENERATED markers come from the spec
sources:
  - metadata/semantics/egui.json#kinds.Window
---

# `<window>`

<!-- BEGIN GENERATED: summary of kinds.Window (tools/docs/egui/gen_reference.py). Do not edit. -->
A native window (the root viewport, or an immediate viewport); its children are laid out as a flex column.

**Children:** any element · **Text content:** no · **Top level:** yes
<!-- END GENERATED -->

<!-- TODO(DOC7): when to use it, and what to use instead. -->

## Example

<!-- TODO(DOC7): a checked example (a testkit case under docs/egui/cases/). -->

## Props

<!-- BEGIN GENERATED: props of kinds.Window (tools/docs/egui/gen_reference.py). Do not edit. -->
| Prop | Type | Default | `bind:` | Description |
|---|---|---|---|---|
| `default-height` | `number` | `360` | no | Initial inner height in points. |
| `default-width` | `number` | `480` | no | Initial inner width in points. |
| `gap` | `number` | `0` | no | Space between the window's children, in points. |
| `padding` | `number` | `8` | no | Inner spacing of the content area, in points. |
| `source-loc` | `string` | `""` | no | Set by the runtime in dev builds: where the window was declared (`file#offset`, then the component call sites around it, U+001F-separated). Not for apps to set. |
| `title` | `string` | `""` | no | Window title. |
<!-- END GENERATED -->

## Events

<!-- BEGIN GENERATED: events of kinds.Window (tools/docs/egui/gen_reference.py). Do not edit. -->
| Attribute | Handler | When |
|---|---|---|
| `onclick` | `(e: SvlEvent<[number, number, number, number, number, number, number, number, number]>, target: number, phase: number, x: number, y: number, button: number, modifiers: number, ox: number, oy: number, buttons: number) => void` | The primary button was pressed and released; targets the innermost node that holds both points (EV1). `preventDefault` keeps the click from the widget. **Sync:** the handler runs before the frame's pass and gets an `SvlEvent` first. |
| `onclose-request` | `(e: SvlEvent<[]>) => void` | The user asked to close the window; return true to keep it open. Runs before the frame's pass. **Sync:** the handler runs before the frame's pass and gets an `SvlEvent` first. |
| `oncontextmenu` | `(e: SvlEvent<[number, number, number, number, number, number, number, number, number]>, target: number, phase: number, x: number, y: number, button: number, modifiers: number, ox: number, oy: number, buttons: number) => void` | The secondary button went down (EV1), after `pointerdown`. `preventDefault` keeps the press from egui (no context menu). **Sync:** the handler runs before the frame's pass and gets an `SvlEvent` first. |
| `ondblclick` | `(e: SvlEvent<[number, number, number, number, number, number, number, number, number]>, target: number, phase: number, x: number, y: number, button: number, modifiers: number, ox: number, oy: number, buttons: number) => void` | A second click on the same node in a short time, after its `click` (EV1). **Sync:** the handler runs before the frame's pass and gets an `SvlEvent` first. |
| `onfocusin` | `(e: SvlEvent<[number, number, number]>, target: number, phase: number, related: number) => void` | This node or a descendant took keyboard focus (EV1; after the frame that moved it). `related` is the node that lost it, 0 for none. **Sync:** the handler runs before the frame's pass and gets an `SvlEvent` first. |
| `onfocusout` | `(e: SvlEvent<[number, number, number]>, target: number, phase: number, related: number) => void` | This node or a descendant lost keyboard focus (EV1). `related` is the node that took it, 0 for none. **Sync:** the handler runs before the frame's pass and gets an `SvlEvent` first. |
| `onkeydown` | `(e: SvlEvent<[number, number, string, boolean, number]>, target: number, phase: number, key: string, repeat: boolean, modifiers: number) => void` | A key went down while this node or a descendant had focus (EV1); `key` is egui's key name. `preventDefault` keeps the key (and the text it types) from the focused widget. **Sync:** the handler runs before the frame's pass and gets an `SvlEvent` first. |
| `onkeyup` | `(e: SvlEvent<[number, number, string, boolean, number]>, target: number, phase: number, key: string, repeat: boolean, modifiers: number) => void` | A key went up while this node or a descendant had focus (EV1). **Sync:** the handler runs before the frame's pass and gets an `SvlEvent` first. |
| `onpointerdown` | `(e: SvlEvent<[number, number, number, number, number, number, number, number, number]>, target: number, phase: number, x: number, y: number, button: number, modifiers: number, ox: number, oy: number, buttons: number) => void` | A pointer button went down over this node or a descendant (EV1: capture, target, bubble). `preventDefault` keeps the press from egui (no click, drag or focus). **Sync:** the handler runs before the frame's pass and gets an `SvlEvent` first. |
| `onpointermove` | `(e: SvlEvent<[number, number, number, number, number, number, number, number, number]>, target: number, phase: number, x: number, y: number, button: number, modifiers: number, ox: number, oy: number, buttons: number) => void` | The pointer moved over this node or a descendant (EV1 propagation; CV1): one per frame at the last point, `button` -1, `buttons` the held ones, `offsetX`/`offsetY` relative to the target. Not cancelable: egui always sees the move. **Sync:** the handler runs before the frame's pass and gets an `SvlEvent` first. |
| `onpointerup` | `(e: SvlEvent<[number, number, number, number, number, number, number, number, number]>, target: number, phase: number, x: number, y: number, button: number, modifiers: number, ox: number, oy: number, buttons: number) => void` | A pointer button went up over this node or a descendant (EV1). `preventDefault` cancels the click. **Sync:** the handler runs before the frame's pass and gets an `SvlEvent` first. |
| `onwheel` | `(e: SvlEvent<[number, number, number, number, number, number, number, number, number]>, target: number, phase: number, x: number, y: number, dx: number, dy: number, modifiers: number, ox: number, oy: number) => void` | The mouse wheel or a trackpad scrolled over this node (EV1); `dx`, `dy` in points. `preventDefault` keeps it from scrolling. **Sync:** the handler runs before the frame's pass and gets an `SvlEvent` first. |

`on:click` works too, and a bare `on:<event>` on a component forwards the component's `on<event>` prop.
<!-- END GENERATED -->

## Methods

<!-- BEGIN GENERATED: methods of kinds.Window (tools/docs/egui/gen_reference.py). Do not edit. -->
| Method | Returns | Description |
|---|---|---|
| `close` | `void` | Closes the window as the user would (no close-request). |
| `present` | `void` | Shows the window (the first one presented is the root viewport). |

Call them through `bind:this`: `el.call('close')`.
<!-- END GENERATED -->

## Behaviour you need to know

<!-- TODO(DOC7): what the tables don't say: timing, edge cases, limits. -->

## CSS

<!-- BEGIN GENERATED: css of kinds.Window (tools/docs/egui/gen_reference.py). Do not edit. -->
**Selector:** `window`, plus `.class`, `#id` and attribute selectors on its props.

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

**Layout properties** (as attributes or in CSS; an explicit attribute wins): `gap`.
Also in CSS only: `display`, `grid-column`, `grid-row`, `margin-bottom`, `margin-left`, `margin-right`, `margin-top`.

The window's own background is the theme's panel colour; a `background-color` rule paints over it.

Full matrix: [css/properties](../../css/properties.md). States: below.
<!-- END GENERATED -->

## States

<!-- BEGIN GENERATED: states of kinds.Window (tools/docs/egui/gen_reference.py). Do not edit. -->
| Pseudo-class | Set when | Source |
|---|---|---|
| `:hover` | the pointer is over its box | response |
| `:active` | the primary button is down over its box | response |
| `:focus-within` | an element inside it has keyboard focus | scene |
| `:exiting` | it plays an out transition before it is removed | scene |

Never set on it (rules with them don't match): `:focus`, `:disabled`, `:checked`, `:selected`, `:open`, `:expanded`, `:invalid`, `:read-only`, `:dragging`, `:busy`, `:indeterminate`, `:placeholder-shown`, `:default`, `:drop-target`, `:current`, `:backdrop`, `:required`, `:user-invalid`, `:has-children`.

States are tracked once the scene has a style input (a `<style>` block, `class`, `style=` or a theme).
<!-- END GENERATED -->

## Accessibility

<!-- BEGIN GENERATED: a11y of kinds.Window (tools/docs/egui/gen_reference.py). Do not edit. -->
| | |
|---|---|
| Role | `Window` |
| `anchor`, `invalid` | not announced on this element (`anchor` still keys its ids) |

The root node of its viewport's accessibility tree.
<!-- END GENERATED -->

## Testing

<!-- TODO(DOC7): how to find and drive it in a kittest harness. -->

## See also

<!-- TODO(DOC7) -->
