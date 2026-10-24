---
title: menu-entry
kind: MenuEntry
category: base
description: "Common properties of the menu entries (`menu`, `action`, `menu-separator`, `context-menu`): drawn inside a menu, never laid out by taffy."
status: stub
verified_at: ~
generated: partial   # regions between BEGIN/END GENERATED markers come from the spec
sources:
  - metadata/semantics/egui.json#kinds.MenuEntry
---

# `MenuEntry` (base)

<!-- BEGIN GENERATED: summary of kinds.MenuEntry (tools/docs/egui/gen_reference.py). Do not edit. -->
Common properties of the menu entries (`menu`, `action`, `menu-separator`, `context-menu`): drawn inside a menu, never laid out by taffy.

**Abstract:** documents the props its family shares; not written as a tag
<!-- END GENERATED -->

<!-- TODO(DOC7): when to use it, and what to use instead. -->

## Example

<!-- TODO(DOC7): a checked example (a testkit case under docs/egui/cases/). -->

## Props

<!-- BEGIN GENERATED: props of kinds.MenuEntry (tools/docs/egui/gen_reference.py). Do not edit. -->
| Prop | Type | Default | `bind:` | Description |
|---|---|---|---|---|
| `accessible-name` | `string` | `""` | no | AccessKit name; the label by default. |
| `anchor` | `string` | `""` | no | A name unique in its embedded root. |
| `anchor-scope` | `string` | `""` | no | Set by the runtime on the top nodes of a keyed `{#each}` item (its keys, U+001F-separated): scopes the egui id of an `anchor` inside it. Not for apps to set. |
| `enabled` | `boolean` | `true` | no | Can be picked when true; drawn greyed out otherwise. |
| `tooltip` | `string` | `""` | no | Hover text; empty for none. |
| `visible` | `boolean` | `true` | no | Shown in its menu when true. |
<!-- END GENERATED -->

## Events

<!-- BEGIN GENERATED: events of kinds.MenuEntry (tools/docs/egui/gen_reference.py). Do not edit. -->
No events of its own.
<!-- END GENERATED -->

## Methods

<!-- BEGIN GENERATED: methods of kinds.MenuEntry (tools/docs/egui/gen_reference.py). Do not edit. -->
None.
<!-- END GENERATED -->

## Behaviour you need to know

<!-- TODO(DOC7): what the tables don't say: timing, edge cases, limits. -->

## CSS

<!-- BEGIN GENERATED: css of kinds.MenuEntry (tools/docs/egui/gen_reference.py). Do not edit. -->
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

Menu entries are drawn by the menu that shows them; CSS reaches the menu's own content only.

Full matrix: [css/properties](../../css/properties.md). States: below.
<!-- END GENERATED -->

## States

<!-- BEGIN GENERATED: states of kinds.MenuEntry (tools/docs/egui/gen_reference.py). Do not edit. -->
No built-in state is set on it.
<!-- END GENERATED -->

## Accessibility

<!-- BEGIN GENERATED: a11y of kinds.MenuEntry (tools/docs/egui/gen_reference.py). Do not edit. -->
| | |
|---|---|
| Role | none of its own; its content is announced in place |
| Name | `accessible-name` |
| `invalid` | the `invalid` prop sets AccessKit `invalid` |
| Author id | `anchor` (unique in its root) |
<!-- END GENERATED -->

## Testing

<!-- TODO(DOC7): how to find and drive it in a kittest harness. -->

## See also

<!-- TODO(DOC7) -->
