---
title: i
tag: i
kind: Italic
category: display
description: "Italic inline text (`font-style: italic` from the theme's base sheet)."
status: stub
verified_at: ~
generated: partial   # regions between BEGIN/END GENERATED markers come from the spec
sources:
  - metadata/semantics/egui.json#kinds.Italic
---

# `<i>`

<!-- BEGIN GENERATED: summary of kinds.Italic (tools/docs/egui/gen_reference.py). Do not edit. -->
Italic inline text (`font-style: italic` from the theme's base sheet).

**Base:** [`Run`](run.md) · **Children:** [`span`](span.md), [`b`](b.md), [`i`](i.md), [`code`](code.md), [`a`](a.md) · **Text content:** sets `text`
<!-- END GENERATED -->

<!-- TODO(DOC7): when to use it, and what to use instead. -->

## Example

<!-- TODO(DOC7): a checked example (a testkit case under docs/egui/cases/). -->

## Props

<!-- BEGIN GENERATED: props of kinds.Italic (tools/docs/egui/gen_reference.py). Do not edit. -->
No props of its own.

From [`Run`](run.md):

| Prop | Type | Default | `bind:` | Description |
|---|---|---|---|---|
| `font-weight` | `number` | `0` | no | CSS font weight (100-900) of this run and the runs in it; 0 (the default) inherits the enclosing run's or label's weight. `b` has 700 from the theme's base sheet. |
| `text` | `string` | `""` | no | The run's own text, before its child runs (its text children fold into it). |
<!-- END GENERATED -->

## Events

<!-- BEGIN GENERATED: events of kinds.Italic (tools/docs/egui/gen_reference.py). Do not edit. -->
No events of its own.
<!-- END GENERATED -->

## Methods

<!-- BEGIN GENERATED: methods of kinds.Italic (tools/docs/egui/gen_reference.py). Do not edit. -->
None.
<!-- END GENERATED -->

## Behaviour you need to know

<!-- TODO(DOC7): what the tables don't say: timing, edge cases, limits. -->

## CSS

<!-- BEGIN GENERATED: css of kinds.Italic (tools/docs/egui/gen_reference.py). Do not edit. -->
**Selector:** `i`, plus `.class`, `#id` and attribute selectors on its props.

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

Drawn as a section of its label's paragraph: `color`, `background-color`, `font-size`, `font-weight`, `font-style`, `font-family` (monospace when it names one) and `text-decoration-line` apply; box properties do not.

Full matrix: [css/properties](../../css/properties.md). States: below.
<!-- END GENERATED -->

## States

<!-- BEGIN GENERATED: states of kinds.Italic (tools/docs/egui/gen_reference.py). Do not edit. -->
No built-in state is set on it.

Never set on it (rules with them don't match): `:hover`, `:active`, `:focus`, `:focus-within`, `:disabled`, `:checked`, `:selected`, `:open`, `:expanded`, `:invalid`, `:read-only`, `:dragging`, `:exiting`, `:busy`, `:indeterminate`, `:placeholder-shown`, `:default`, `:drop-target`, `:current`, `:backdrop`, `:required`, `:user-invalid`, `:has-children`.
<!-- END GENERATED -->

## Accessibility

<!-- BEGIN GENERATED: a11y of kinds.Italic (tools/docs/egui/gen_reference.py). Do not edit. -->
| | |
|---|---|
| Role | none of its own; its content is announced in place |
| `anchor`, `invalid` | not announced on this element (`anchor` still keys its ids) |

Part of its label's text (the label's AccessKit value holds all its runs).
<!-- END GENERATED -->

## Testing

<!-- TODO(DOC7): how to find and drive it in a kittest harness. -->

## See also

<!-- TODO(DOC7) -->
