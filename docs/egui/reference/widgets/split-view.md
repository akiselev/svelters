---
title: split-view
tag: split-view
kind: SplitView
category: containers
description: "Two panes (its first two children; more are not shown) split by a divider the user drags."
status: stub
verified_at: ~
generated: partial   # regions between BEGIN/END GENERATED markers come from the spec
sources:
  - metadata/semantics/egui.json#kinds.SplitView
---

# `<split-view>`

<!-- BEGIN GENERATED: summary of kinds.SplitView (tools/docs/egui/gen_reference.py). Do not edit. -->
Panes (its children) side by side or stacked, split by sashes the user drags or moves with the keys (R11: a 1 px line, a 4 px hit area, a focus-coloured bar and grip after 300 ms of hover). `divider-position` is the first pane's size; `sizes` every pane's.

**Base:** [`Widget`](widget.md) · **Children:** any element · **Text content:** no
<!-- END GENERATED -->

<!-- TODO(DOC7): when to use it, and what to use instead. -->

## Example

<!-- TODO(DOC7): a checked example (a testkit case under docs/egui/cases/). -->

## Props

<!-- BEGIN GENERATED: props of kinds.SplitView (tools/docs/egui/gen_reference.py). Do not edit. -->
| Prop | Type | Default | `bind:` | Description |
|---|---|---|---|---|
| `collapsible` | `boolean` | `false` | no | OW2: dragging a sash below half a pane's minimum collapses the pane to 0 (Qt's `childrenCollapsible`); Enter on a focused sash collapses or restores. |
| `commit-mode` | `'live' \| 'release'` | `"live"` | no | OW2: `live` moves the panes while dragging; `release` draws a guide line and moves them once, on release (Qt's `opaqueResize = false`). |
| `divider-position` | `number \| null` | `null` | **yes**, on `changed` | Size of the first pane in points along the split; null for half. Dragging the divider changes it (`bind:divider-position`); it is clamped by `min-size` when laid out. |
| `min-size` | `number` | `32` | no | Smallest size of either pane, in points. |
| `orientation` | `'horizontal' \| 'vertical'` | `"horizontal"` | no | `horizontal`: the panes side by side (a vertical divider); `vertical`: one above the other. |
| `resizable` | `boolean` | `true` | no | The divider can be dragged. |
| `restore-on-double-click` | `boolean` | `true` | no | OW2: a double click on a sash restores the pane before it to its size before a collapse, or collapses it (`collapsible`). |
| `sizes` | `unknown[] \| null` | `null` | **yes**, on `changed` | OW2: every pane's size in points along the split (`bind:sizes`, Qt's `sizes`); null: from `divider-position` (two panes) or equal shares. When the split view grows or shrinks, the difference goes to the panes by their `flex-grow` (Qt's stretch factors; the last pane when none has one). Each pane keeps `min-size` and its own `min-width`/`max-width` (or heights). |

Plus the 30 props every element has, from [`Widget`](widget.md#props): `accessible-name`, `align-self`, `anchor`, `anchor-scope`, `autofocus`, `bottom`, `dialog-role`, `drag-data`, `drag-type`, `drop-types`, `enabled`, `flex-basis`, `flex-grow`, `flex-shrink`, `height`, `invalid`, `left`, `margin`, `max-height`, `max-width`, `min-height`, `min-width`, `position`, `right`, `status-section`, `status-tip`, `tooltip`, `top`, `visible`, `width`.
<!-- END GENERATED -->

## Events

<!-- BEGIN GENERATED: events of kinds.SplitView (tools/docs/egui/gen_reference.py). Do not edit. -->
| Attribute | Handler | When |
|---|---|---|
| `onchanged` | `(divider_position: number) => void` | The user dragged the divider; carries the first pane's new size. |
| `oncommit` | `(sizes: unknown[]) => void` | OW2: the user finished a resize (drag released, a key step, a restore); carries every pane's size. |
| `ondrag-started` | `() => void` | A divider drag began. |
| `ondrag-stopped` | `() => void` | A divider drag ended. |

`on:changed` works too, and a bare `on:<event>` on a component forwards the component's `on<event>` prop.

Plus the events of [`Widget`](widget.md#events): `onanimationend`, `onclick`, `oncontextmenu`, `ondblclick`, `ondrag-begin`, `ondrag-end`, `ondrag-enter`, `ondrag-leave`, `ondrop`, `onfocusin`, `onfocusout`, `onkeydown`, `onkeyup`, `onpointerdown`, `onpointermove`, `onpointerup`, `onwheel`.
<!-- END GENERATED -->

## Methods

<!-- BEGIN GENERATED: methods of kinds.SplitView (tools/docs/egui/gen_reference.py). Do not edit. -->
None.
<!-- END GENERATED -->

## Behaviour you need to know

<!-- TODO(DOC7): what the tables don't say: timing, edge cases, limits. -->

## CSS

<!-- BEGIN GENERATED: css of kinds.SplitView (tools/docs/egui/gen_reference.py). Do not edit. -->
**Selector:** `split-view`, plus `.class`, `#id` and attribute selectors on its props.

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

**Layout properties** (as attributes or in CSS; an explicit attribute wins): `align-self`, `bottom`, `dialog-role`, `flex-basis`, `flex-grow`, `flex-shrink`, `height`, `left`, `max-height`, `max-width`, `min-height`, `min-width`, `position`, `right`, `status-section`, `top`, `visible`, `width`.
Also in CSS only: `display`, `grid-column`, `grid-row`, `margin-bottom`, `margin-left`, `margin-right`, `margin-top`.

The sashes (`::part(sash)`, `::part(grip)`) paint from the theme's `border` (the 1 px line), `sash-w` (the hit area and hover bar) and `focus` (R11's hover bar after 300 ms) until `::part()` reaches them.

Full matrix: [css/properties](../../css/properties.md). States: below.
<!-- END GENERATED -->

## States

<!-- BEGIN GENERATED: states of kinds.SplitView (tools/docs/egui/gen_reference.py). Do not edit. -->
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

<!-- BEGIN GENERATED: a11y of kinds.SplitView (tools/docs/egui/gen_reference.py). Do not edit. -->
| | |
|---|---|
| Role | `Group` |
| Name | `accessible-name` |
| `invalid` | the `invalid` prop sets AccessKit `invalid` |
| Author id | `anchor` (unique in its root) |

svelters' own (OW2): each sash between two panes is a `Splitter` node named by the split view (else `Resize pane <n>`), in the tab order, with the size of the pane before it, its bounds and step; arrows (Shift: ×5), Home/End and Enter (collapse) work while it is focused.
<!-- END GENERATED -->

## Testing

<!-- TODO(DOC7): how to find and drive it in a kittest harness. -->

## See also

<!-- TODO(DOC7) -->
