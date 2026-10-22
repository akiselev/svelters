---
title: a
tag: a
kind: Anchor
category: display
description: "An inline link: the `link` colour, underlined on hover (DESIGN R18), the pointer cursor (R10); focusable, activated by click, Enter or Space."
status: stub
verified_at: ~
generated: partial   # regions between BEGIN/END GENERATED markers come from the spec
sources:
  - metadata/semantics/egui.json#kinds.Anchor
---

# `<a>`

<!-- BEGIN GENERATED: summary of kinds.Anchor (tools/docs/egui/gen_reference.py). Do not edit. -->
An inline link: the `link` colour, underlined on hover (DESIGN R18), the pointer cursor (R10); focusable, activated by click, Enter or Space.

**Base:** [`Run`](run.md) · **Children:** [`span`](span.md), [`b`](b.md), [`i`](i.md), [`code`](code.md), [`a`](a.md) · **Text content:** sets `text`
<!-- END GENERATED -->

<!-- TODO(DOC7): when to use it, and what to use instead. -->

## Example

<!-- TODO(DOC7): a checked example (a testkit case under docs/egui/cases/). -->

## Props

<!-- BEGIN GENERATED: props of kinds.Anchor (tools/docs/egui/gen_reference.py). Do not edit. -->
| Prop | Type | Default | `bind:` | Description |
|---|---|---|---|---|
| `accessible-name` | `string` | `""` | no | AccessKit name of the link; defaults to its text. |
| `href` | `string` | `""` | no | The link target. Without an `onclicked` handler a click opens it (as `hyperlink`'s `url`); with one, only the handler runs. |

From [`Run`](run.md):

| Prop | Type | Default | `bind:` | Description |
|---|---|---|---|---|
| `font-weight` | `number` | `0` | no | CSS font weight (100-900) of this run and the runs in it; 0 (the default) inherits the enclosing run's or label's weight. `b` has 700 from the theme's base sheet. |
| `text` | `string` | `""` | no | The run's own text, before its child runs (its text children fold into it). |
<!-- END GENERATED -->

## Events

<!-- BEGIN GENERATED: events of kinds.Anchor (tools/docs/egui/gen_reference.py). Do not edit. -->
| Attribute | Handler | When |
|---|---|---|
| `onclicked` | `() => void` | The link was activated (click, or Enter/Space while it has keyboard focus). |

`on:clicked` works too, and a bare `on:<event>` on a component forwards the component's `on<event>` prop.
<!-- END GENERATED -->

## Methods

<!-- BEGIN GENERATED: methods of kinds.Anchor (tools/docs/egui/gen_reference.py). Do not edit. -->
None.
<!-- END GENERATED -->

## Behaviour you need to know

<!-- TODO(DOC7): what the tables don't say: timing, edge cases, limits. -->

## CSS

<!-- BEGIN GENERATED: css of kinds.Anchor (tools/docs/egui/gen_reference.py). Do not edit. -->
**Selector:** `a`, plus `.class`, `#id` and attribute selectors on its props.

**Painted as:** nothing CSS reaches. No paint property applies to the element itself; style its children instead.

| Property | Applies | Theme default |
|---|---|---|
| `background-color` | no | — |
| `border-color` | no | — |
| `border-width` | no | — |
| `border-radius` | no | — |
| `color` | no | `var(--sv-link)` |
| `box-shadow` | no | — |
| `opacity` | no | — |

Theme rules for its states: `a:hover` sets `text-decoration-line: underline`.

Drawn as a section of its label's paragraph: `color`, `background-color`, `font-size`, `font-weight`, `font-style`, `font-family` (monospace when it names one) and `text-decoration-line` apply; box properties do not.

Full matrix: [css/properties](../../css/properties.md). States: below.
<!-- END GENERATED -->

## States

<!-- BEGIN GENERATED: states of kinds.Anchor (tools/docs/egui/gen_reference.py). Do not edit. -->
| Pseudo-class | Set when | Source |
|---|---|---|
| `:hover` | the pointer is over one of its glyphs | kind |
| `:focus` | it has keyboard focus | kind |

Never set on it (rules with them don't match): `:active`, `:focus-within`, `:disabled`, `:checked`, `:selected`, `:open`, `:expanded`, `:invalid`, `:read-only`, `:dragging`, `:exiting`, `:busy`.

States are tracked once the scene has a style input (a `<style>` block, `class`, `style=` or a theme).
<!-- END GENERATED -->

## Accessibility

<!-- BEGIN GENERATED: a11y of kinds.Anchor (tools/docs/egui/gen_reference.py). Do not edit. -->
| | |
|---|---|
| Role | `Link` |
| Name | `accessible-name`, else `text` |
| Actions | `Click`, `Focus` |
| `anchor`, `invalid` | not announced on this element (`anchor` still keys its ids) |

A `Link` node at its glyphs' bounds, a child of the label's node; Tab reaches it, Enter or Space activates it.
<!-- END GENERATED -->

## Testing

<!-- TODO(DOC7): how to find and drive it in a kittest harness. -->

## See also

<!-- TODO(DOC7) -->
