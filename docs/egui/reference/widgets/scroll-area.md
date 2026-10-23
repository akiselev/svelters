---
title: scroll-area
tag: scroll-area
kind: ScrollArea
category: containers
description: "A flex box whose content scrolls when it overflows the area's size (taffy `overflow: scroll`); give it a size or let flex-grow size it."
status: stub
verified_at: ~
generated: partial   # regions between BEGIN/END GENERATED markers come from the spec
sources:
  - metadata/semantics/egui.json#kinds.ScrollArea
---

# `<scroll-area>`

<!-- BEGIN GENERATED: summary of kinds.ScrollArea (tools/docs/egui/gen_reference.py). Do not edit. -->
A flex box whose content scrolls when it overflows the area's size (taffy `overflow: scroll`); give it a size or let flex-grow size it.

**Base:** [`box`](box.md) → [`Widget`](widget.md) · **Children:** any element · **Text content:** no
<!-- END GENERATED -->

<!-- TODO(DOC7): when to use it, and what to use instead. -->

## Example

<!-- TODO(DOC7): a checked example (a testkit case under docs/egui/cases/). -->

## Props

<!-- BEGIN GENERATED: props of kinds.ScrollArea (tools/docs/egui/gen_reference.py). Do not edit. -->
| Prop | Type | Default | `bind:` | Description |
|---|---|---|---|---|
| `direction` | `'vertical' \| 'horizontal' \| 'both'` | `"vertical"` | no | The axes that scroll (taffy `overflow: scroll` on them). |
| `scroll-x` | `number` | `0` | **yes**, on `changed` | Horizontal offset in points; scrolling changes it, and a write scrolls (clamped to the content). |
| `scroll-y` | `number` | `0` | **yes**, on `changed` | Vertical offset in points; scrolling changes it, and a write scrolls (clamped to the content). |

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

Plus the 27 props every element has, from [`Widget`](widget.md#props): `accessible-name`, `align-self`, `anchor`, `autofocus`, `bottom`, `dialog-role`, `drag-data`, `drag-type`, `drop-types`, `enabled`, `flex-basis`, `flex-grow`, `flex-shrink`, `height`, `invalid`, `left`, `margin`, `max-height`, `max-width`, `min-height`, `min-width`, `position`, `right`, `tooltip`, `top`, `visible`, `width`.
<!-- END GENERATED -->

## Events

<!-- BEGIN GENERATED: events of kinds.ScrollArea (tools/docs/egui/gen_reference.py). Do not edit. -->
| Attribute | Handler | When |
|---|---|---|
| `onchanged` | `(scroll_x: number, scroll_y: number) => void` | The user scrolled (or a write was clamped); carries the new offsets. |

`on:changed` works too, and a bare `on:<event>` on a component forwards the component's `on<event>` prop.

Plus the events of [`Widget`](widget.md#events): `onanimationend`, `onclick`, `oncontextmenu`, `ondblclick`, `ondrag-begin`, `ondrag-end`, `ondrag-enter`, `ondrag-leave`, `ondrop`, `onfocusin`, `onfocusout`, `onkeydown`, `onkeyup`, `onpointerdown`, `onpointerup`, `onwheel`.
<!-- END GENERATED -->

## Methods

<!-- BEGIN GENERATED: methods of kinds.ScrollArea (tools/docs/egui/gen_reference.py). Do not edit. -->
None.
<!-- END GENERATED -->

## Behaviour you need to know

<!-- TODO(DOC7): what the tables don't say: timing, edge cases, limits. -->

## CSS

<!-- BEGIN GENERATED: css of kinds.ScrollArea (tools/docs/egui/gen_reference.py). Do not edit. -->
**Selector:** `scroll-area`, plus `.class`, `#id` and attribute selectors on its props.

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

**Layout properties** (as attributes or in CSS; an explicit attribute wins): `align-items`, `align-self`, `bottom`, `column-gap`, `dialog-role`, `flex-basis`, `flex-direction`, `flex-grow`, `flex-shrink`, `flex-wrap`, `gap`, `height`, `justify-content`, `left`, `max-height`, `max-width`, `min-height`, `min-width`, `position`, `right`, `row-gap`, `top`, `visible`, `width`.
Also in CSS only: `display`, `grid-column`, `grid-row`, `margin-bottom`, `margin-left`, `margin-right`, `margin-top`.

`overflow-x`/`overflow-y` (`hidden`, `scroll`, `auto`) override `direction` per axis; `overscroll-behavior: contain` keeps the wheel from chaining to an outer area; `scroll-margin` is the gap `scroll-into-view` and focus keep; `scrollbar-gutter: stable` (or `--sv-scrollbar-mode: classic`) gives classic bars in a gutter. Provider states `:state(sv.overflow-top)`, `-bottom`, `-left`, `-right` match while content lies beyond that edge; the theme draws R15's edge shadows as an inset `box-shadow` on `:state(sv.overflow-top)`/`(sv.overflow-left)`, painted over the content. The bars and thumbs are parts (`scrollbar-x`/`-y`, `thumb-x`/`-y`, `corner`, `shadow-*`), painted from VS Code's `scrollbarSlider.*` tokens until `::part()` reaches them.

Full matrix: [css/properties](../../css/properties.md). States: below.
<!-- END GENERATED -->

## States

<!-- BEGIN GENERATED: states of kinds.ScrollArea (tools/docs/egui/gen_reference.py). Do not edit. -->
| Pseudo-class | Set when | Source |
|---|---|---|
| `:hover` | the pointer is over its box | response |
| `:active` | the primary button is down over its box | response |
| `:focus-within` | an element inside it has keyboard focus | scene |
| `:disabled` (`:enabled` is the opposite) | `enabled` is false | prop |
| `:exiting` | it plays an out transition before it is removed | scene |

Never set on it (rules with them don't match): `:focus`, `:checked`, `:selected`, `:open`, `:expanded`, `:invalid`, `:read-only`, `:dragging`, `:busy`, `:indeterminate`, `:placeholder-shown`, `:default`, `:drop-target`, `:current`, `:backdrop`, `:required`, `:user-invalid`, `:has-children`.

States are tracked once the scene has a style input (a `<style>` block, `class`, `style=` or a theme).
<!-- END GENERATED -->

## Accessibility

<!-- BEGIN GENERATED: a11y of kinds.ScrollArea (tools/docs/egui/gen_reference.py). Do not edit. -->
| | |
|---|---|
| Role | `ScrollView` |
| Name | `accessible-name` |
| Value | `scroll-x`, `scroll-y` and their ranges |
| Actions | `ScrollUp`, `ScrollDown`, `ScrollLeft`, `ScrollRight`, `SetScrollOffset` |
| `anchor`, `invalid` | not announced on this element (`anchor` still keys its ids) |

svelters' own (OW2): the node carries the scroll offsets and their ranges and takes ScrollUp/Down/Left/Right and SetScrollOffset; its bars are `ScrollBar` nodes with their value; ScrollIntoView on a descendant scrolls it into view.
<!-- END GENERATED -->

## Testing

<!-- TODO(DOC7): how to find and drive it in a kittest harness. -->

## See also

<!-- TODO(DOC7) -->
