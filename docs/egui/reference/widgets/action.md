---
title: action
tag: action
kind: Action
category: menus
description: "A menu item: plain, a check item (`checkable`) or a radio item (`group`), with an optional `shortcut`."
status: stub
verified_at: ~
generated: partial   # regions between BEGIN/END GENERATED markers come from the spec
sources:
  - metadata/semantics/egui.json#kinds.Action
---

# `<action>`

<!-- BEGIN GENERATED: summary of kinds.Action (tools/docs/egui/gen_reference.py). Do not edit. -->
A menu item: plain, a check item (`checkable`) or a radio item (`group`), with an optional `shortcut`. AccessKit role Button, CheckBox or RadioButton.

**Base:** [`MenuEntry`](menu-entry.md) · **Children:** none · **Text content:** sets `label`
<!-- END GENERATED -->

<!-- TODO(DOC7): when to use it, and what to use instead. -->

## Example

<!-- TODO(DOC7): a checked example (a testkit case under docs/egui/cases/). -->

## Props

<!-- BEGIN GENERATED: props of kinds.Action (tools/docs/egui/gen_reference.py). Do not edit. -->
| Prop | Type | Default | `bind:` | Description |
|---|---|---|---|---|
| `checkable` | `boolean` | `false` | no | A check item: picking it toggles `checked`. |
| `checked` | `boolean` | `false` | **yes**, on `toggled` | Check or radio state; picking a check item toggles it, picking a radio item sets it. |
| `group` | `string` | `""` | no | A radio item: the actions of one parent with the same group form an exclusive set; picking one checks it and unchecks the others (each announced). |
| `label` | `string` | `""` | no | The caption. |
| `shortcut` | `string` | `""` | no | Keyboard accelerator such as `Ctrl+O`, `Ctrl+Shift+S` or `F5`: shown right-aligned and active while the menu's owner (menu-bar, menu-button, context-menu) is drawn. Modifiers: `Ctrl`, `Shift`, `Alt`, `Cmd` (macOS command), `Primary` (Cmd on macOS, Ctrl elsewhere); keys are egui `Key` names. An unknown key is an error. |

Plus the 6 props every menu entry has, from [`MenuEntry`](menu-entry.md#props): `accessible-name`, `anchor`, `anchor-scope`, `enabled`, `tooltip`, `visible`.
<!-- END GENERATED -->

## Events

<!-- BEGIN GENERATED: events of kinds.Action (tools/docs/egui/gen_reference.py). Do not edit. -->
| Attribute | Handler | When |
|---|---|---|
| `onactivate` | `(checked: boolean) => void` | The user picked it (click, Enter, or its shortcut); carries `checked` after the pick. |
| `onhover` | `() => void` | It became the open menu's current row (the pointer or the arrow keys; QAction `hovered`). Svelters' own menus only (OW3). |
| `ontoggled` | `(checked: boolean) => void` | The user changed `checked` (a check item, or a radio item turned on or off by a pick); carries the new state. |

`on:activate` works too, and a bare `on:<event>` on a component forwards the component's `on<event>` prop.
<!-- END GENERATED -->

## Methods

<!-- BEGIN GENERATED: methods of kinds.Action (tools/docs/egui/gen_reference.py). Do not edit. -->
None.
<!-- END GENERATED -->

## Behaviour you need to know

<!-- TODO(DOC7): what the tables don't say: timing, edge cases, limits. -->

## CSS

<!-- BEGIN GENERATED: css of kinds.Action (tools/docs/egui/gen_reference.py). Do not edit. -->
**Selector:** `action`, plus `.class`, `#id` and attribute selectors on its props.

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

<!-- BEGIN GENERATED: states of kinds.Action (tools/docs/egui/gen_reference.py). Do not edit. -->
No built-in state is set on it.

Never set on it (rules with them don't match): `:hover`, `:active`, `:focus`, `:focus-within`, `:disabled`, `:checked`, `:selected`, `:open`, `:expanded`, `:invalid`, `:read-only`, `:dragging`, `:exiting`, `:busy`, `:indeterminate`, `:placeholder-shown`, `:default`, `:drop-target`, `:current`, `:backdrop`, `:required`, `:user-invalid`, `:has-children`.
<!-- END GENERATED -->

## Accessibility

<!-- BEGIN GENERATED: a11y of kinds.Action (tools/docs/egui/gen_reference.py). Do not edit. -->
| | |
|---|---|
| Role | `Button`; `CheckBox` when `checkable`; `RadioButton` when `group` is set |
| Name | `accessible-name`, else `label` |
| Value | checked or not, for a check or radio item |
| Actions | `Click`, `Focus` |
| `invalid` | the `invalid` prop sets AccessKit `invalid` |
| Author id | `anchor` (unique in its root) |

Its `shortcut` is announced as the keyboard shortcut.
<!-- END GENERATED -->

## Testing

<!-- TODO(DOC7): how to find and drive it in a kittest harness. -->

## See also

<!-- TODO(DOC7) -->
