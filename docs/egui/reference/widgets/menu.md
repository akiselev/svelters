---
title: menu
tag: menu
kind: Menu
category: menus
description: "A menu: a menu-bar button, or a submenu row inside another menu."
status: stub
verified_at: ~
generated: partial   # regions between BEGIN/END GENERATED markers come from the spec
sources:
  - metadata/semantics/egui.json#kinds.Menu
---

# `<menu>`

<!-- BEGIN GENERATED: summary of kinds.Menu (tools/docs/egui/gen_reference.py). Do not edit. -->
A menu: a menu-bar button, or a submenu row inside another menu. Inside a `menu-button` its items are the button's menu (its label is unused).

**Base:** [`MenuEntry`](menu-entry.md) · **Children:** [`menu`](menu.md), [`action`](action.md), [`menu-separator`](menu-separator.md) · **Text content:** sets `label`
<!-- END GENERATED -->

<!-- TODO(DOC7): when to use it, and what to use instead. -->

## Example

<!-- TODO(DOC7): a checked example (a testkit case under docs/egui/cases/). -->

## Props

<!-- BEGIN GENERATED: props of kinds.Menu (tools/docs/egui/gen_reference.py). Do not edit. -->
| Prop | Type | Default | `bind:` | Description |
|---|---|---|---|---|
| `label` | `string` | `""` | no | The title: the menu-bar button, or the submenu row. |

Plus the 6 props every menu entry has, from [`MenuEntry`](menu-entry.md#props): `accessible-name`, `anchor`, `anchor-scope`, `enabled`, `tooltip`, `visible`.
<!-- END GENERATED -->

## Events

<!-- BEGIN GENERATED: events of kinds.Menu (tools/docs/egui/gen_reference.py). Do not edit. -->
No events of its own.
<!-- END GENERATED -->

## Methods

<!-- BEGIN GENERATED: methods of kinds.Menu (tools/docs/egui/gen_reference.py). Do not edit. -->
None.
<!-- END GENERATED -->

## Behaviour you need to know

<!-- TODO(DOC7): what the tables don't say: timing, edge cases, limits. -->

## CSS

<!-- BEGIN GENERATED: css of kinds.Menu (tools/docs/egui/gen_reference.py). Do not edit. -->
**Selector:** `menu`, plus `.class`, `#id` and attribute selectors on its props.

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

Full matrix: [css/properties](../../css/properties.md). States: below.
<!-- END GENERATED -->

## States

<!-- BEGIN GENERATED: states of kinds.Menu (tools/docs/egui/gen_reference.py). Do not edit. -->
No built-in state is set on it.

Never set on it (rules with them don't match): `:hover`, `:active`, `:focus`, `:focus-within`, `:disabled`, `:checked`, `:selected`, `:open`, `:expanded`, `:invalid`, `:read-only`, `:dragging`, `:exiting`, `:busy`, `:indeterminate`, `:placeholder-shown`, `:default`, `:drop-target`, `:current`, `:backdrop`, `:required`, `:user-invalid`, `:has-children`.
<!-- END GENERATED -->

## Accessibility

<!-- BEGIN GENERATED: a11y of kinds.Menu (tools/docs/egui/gen_reference.py). Do not edit. -->
| | |
|---|---|
| Role | `Button` |
| Name | `accessible-name`, else `label` |
| Actions | `Click`, `Focus` |
| `invalid` | the `invalid` prop sets AccessKit `invalid` |
| Author id | `anchor` (unique in its root) |
<!-- END GENERATED -->

## Testing

<!-- TODO(DOC7): how to find and drive it in a kittest harness. -->

## See also

<!-- TODO(DOC7) -->
