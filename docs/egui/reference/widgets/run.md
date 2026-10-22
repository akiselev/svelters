---
title: run
kind: Run
category: display
description: "Common properties of the inline runs (`span`, `b`, `i`, `code`, `a`): text inside a `label`, laid out with the label's other runs as one paragraph, never by taffy."
status: stub
verified_at: ~
generated: partial   # regions between BEGIN/END GENERATED markers come from the spec
sources:
  - metadata/semantics/egui.json#kinds.Run
---

# `Run` (base)

<!-- BEGIN GENERATED: summary of kinds.Run (tools/docs/egui/gen_reference.py). Do not edit. -->
Common properties of the inline runs (`span`, `b`, `i`, `code`, `a`): text inside a `label`, laid out with the label's other runs as one paragraph, never by taffy.

**Children:** [`span`](span.md), [`b`](b.md), [`i`](i.md), [`code`](code.md), [`a`](a.md) · **Text content:** sets `text` · **Abstract:** documents the props its family shares; not written as a tag
<!-- END GENERATED -->

<!-- TODO(DOC7): when to use it, and what to use instead. -->

## Example

<!-- TODO(DOC7): a checked example (a testkit case under docs/egui/cases/). -->

## Props

<!-- BEGIN GENERATED: props of kinds.Run (tools/docs/egui/gen_reference.py). Do not edit. -->
| Prop | Type | Default | `bind:` | Description |
|---|---|---|---|---|
| `accessible-name` | `string` | `""` | no | AccessKit name of an `a` run's link (defaults to its text); other runs are read as part of their label's text. |
| `font-weight` | `number` | `0` | no | CSS font weight (100-900) of this run and the runs in it; 0 (the default) inherits the enclosing run's or label's weight. `b` has 700 from the theme's base sheet. |
| `text` | `string` | `""` | no | The run's own text, before its child runs (its text children fold into it). |
<!-- END GENERATED -->

## Events

<!-- BEGIN GENERATED: events of kinds.Run (tools/docs/egui/gen_reference.py). Do not edit. -->
No events of its own.
<!-- END GENERATED -->

## Methods

<!-- BEGIN GENERATED: methods of kinds.Run (tools/docs/egui/gen_reference.py). Do not edit. -->
None.
<!-- END GENERATED -->

## Behaviour you need to know

<!-- TODO(DOC7): what the tables don't say: timing, edge cases, limits. -->

## CSS

<!-- BEGIN GENERATED: css of kinds.Run (tools/docs/egui/gen_reference.py). Do not edit. -->
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

<!-- BEGIN GENERATED: states of kinds.Run (tools/docs/egui/gen_reference.py). Do not edit. -->
No built-in state is set on it.
<!-- END GENERATED -->

## Accessibility

<!-- BEGIN GENERATED: a11y of kinds.Run (tools/docs/egui/gen_reference.py). Do not edit. -->
| | |
|---|---|
| Role | none of its own; its content is announced in place |

Part of its label's text (the label's AccessKit value holds all its runs).
<!-- END GENERATED -->

## Testing

<!-- TODO(DOC7): how to find and drive it in a kittest harness. -->

## See also

<!-- TODO(DOC7) -->
