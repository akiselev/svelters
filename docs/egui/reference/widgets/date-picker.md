---
title: date-picker
tag: date-picker
kind: DatePicker
category: pickers
description: "A button showing a date; clicking it opens a calendar popup (Save/Cancel)."
status: stub
verified_at: ~
generated: partial   # regions between BEGIN/END GENERATED markers come from the spec
sources:
  - metadata/semantics/egui.json#kinds.DatePicker
---

# `<date-picker>`

<!-- BEGIN GENERATED: summary of kinds.DatePicker (tools/docs/egui/gen_reference.py). Do not edit. -->
A drop button showing a date; clicking it opens the calendar (the `calendar` kind's) in a popup, with Save/Cancel unless `commit-mode="immediate"`.

**Base:** [`Widget`](widget.md) · **Children:** none · **Text content:** no
<!-- END GENERATED -->

<!-- TODO(DOC7): when to use it, and what to use instead. -->

## Example

<!-- TODO(DOC7): a checked example (a testkit case under docs/egui/cases/). -->

## Props

<!-- BEGIN GENERATED: props of kinds.DatePicker (tools/docs/egui/gen_reference.py). Do not edit. -->
| Prop | Type | Default | `bind:` | Description |
|---|---|---|---|---|
| `calendar-week` | `boolean` | `true` | no | Shows ISO week numbers in the calendar. |
| `commit-mode` | `'confirm' \| 'immediate'` | `"confirm"` | no | `confirm`: a picked day is pending until Save (Enter saves, Cancel or Escape drops it); `immediate`: a pick commits and closes the popup (Qt). |
| `day-parts` | `string[]` | `null` | no | Extra part names per date, `"YYYY-MM-DD name…"` each: `["2026-10-22 milestone"]` makes that day match `::part(day milestone)`. |
| `first-day` | `'auto' \| 'monday' \| 'tuesday' \| 'wednesday' \| 'thursday' \| 'friday' \| 'saturday' \| 'sunday'` | `"auto"` | no | The weekday the grid starts on; `auto` is the locale's (Monday until IN1). |
| `format` | `string` | `"%Y-%m-%d"` | no | How the button shows the date. |
| `max` | `string` | `""` | no | The last day that can be picked (ISO 8601); days after it are `:disabled`. `""`: no limit. |
| `min` | `string` | `""` | no | The first day that can be picked (ISO 8601); days before it are `:disabled`. `""`: no limit. |
| `navigation` | `boolean` | `true` | no | Shows the header with the previous/next month buttons and the month and year. |
| `read-only` | `boolean` | `false` | no | Shows and roves the calendar but never picks (Qt's `NoSelection`). |
| `value` | `string` | `""` | **yes**, on `changed` | The date, ISO 8601 (`2026-10-22`); `""` for none (the button shows today). Picking a day (and, with `commit-mode="confirm"`, pressing Save) changes it. |
| `weekday-format` | `'narrow' \| 'short' \| 'long' \| 'none'` | `"short"` | no | The weekday header: `narrow` (M), `short` (Mo), `long` (Monday) or `none` (no header row). |

Plus the 26 props every element has, from [`Widget`](widget.md#props): `accessible-name`, `align-self`, `anchor`, `autofocus`, `bottom`, `drag-data`, `drag-type`, `drop-types`, `enabled`, `flex-basis`, `flex-grow`, `flex-shrink`, `height`, `invalid`, `left`, `margin`, `max-height`, `max-width`, `min-height`, `min-width`, `position`, `right`, `tooltip`, `top`, `visible`, `width`.
<!-- END GENERATED -->

## Events

<!-- BEGIN GENERATED: events of kinds.DatePicker (tools/docs/egui/gen_reference.py). Do not edit. -->
| Attribute | Handler | When |
|---|---|---|
| `onchanged` | `(value: string) => void` | The user saved a date; carries it (ISO 8601). |
| `oncommit` | `(value: string) => void` | A pick was applied (§2.4): Save, or the pick itself with `commit-mode="immediate"`. |
| `onpage-changed` | `(year: number, month: number) => void` | The user moved the calendar to another month. |

`on:changed` works too, and a bare `on:<event>` on a component forwards the component's `on<event>` prop.

Plus the events of [`Widget`](widget.md#events): `onanimationend`, `onclick`, `oncontextmenu`, `ondblclick`, `ondrag-begin`, `ondrag-end`, `ondrag-enter`, `ondrag-leave`, `ondrop`, `onfocusin`, `onfocusout`, `onkeydown`, `onkeyup`, `onpointerdown`, `onpointerup`, `onwheel`.
<!-- END GENERATED -->

## Methods

<!-- BEGIN GENERATED: methods of kinds.DatePicker (tools/docs/egui/gen_reference.py). Do not edit. -->
None.
<!-- END GENERATED -->

## Behaviour you need to know

<!-- TODO(DOC7): what the tables don't say: timing, edge cases, limits. -->

## CSS

<!-- BEGIN GENERATED: css of kinds.DatePicker (tools/docs/egui/gen_reference.py). Do not edit. -->
**Selector:** `date-picker`, plus `.class`, `#id` and attribute selectors on its props.

**Painted as:** a box under the kind's own drawing. The CSS engine paints shadows, background and border at its rect; the kind's `ui` draws the rest.

| Property | Applies | Theme default |
|---|---|---|
| `background-color` | yes | — |
| `border-color` | yes | — |
| `border-width` | yes | — |
| `border-radius` | yes | — |
| `color` | yes (its text) | — |
| `box-shadow` | yes (outer and inset) | — |
| `opacity` | yes | — |

**Layout properties** (as attributes or in CSS; an explicit attribute wins): `align-self`, `bottom`, `flex-basis`, `flex-grow`, `flex-shrink`, `height`, `left`, `max-height`, `max-width`, `min-height`, `min-width`, `position`, `right`, `top`, `visible`, `width`.
Also in CSS only: `display`, `grid-column`, `grid-row`, `margin-bottom`, `margin-left`, `margin-right`, `margin-top`.

The button: R4's field look with R14's chevron (`::part(chevron)`); the popup is `::part(popup)` (OW3's surface) holding the `calendar` kind's parts (`header`, `prev`, `next`, `month`, `year`, `weekday`, `week-number`, `grid`, `day`) and `::part(save)`, `::part(cancel)`.

Full matrix: [css/properties](../../css/properties.md). States: below.
<!-- END GENERATED -->

## States

<!-- BEGIN GENERATED: states of kinds.DatePicker (tools/docs/egui/gen_reference.py). Do not edit. -->
| Pseudo-class | Set when | Source |
|---|---|---|
| `:hover` | the pointer is over it | response |
| `:active` | the primary button is down on it | response |
| `:focus` | it has keyboard focus | response |
| `:focus-within` | it, or an element inside it, has keyboard focus | scene |
| `:disabled` (`:enabled` is the opposite) | `enabled` is false | prop |
| `:exiting` | it plays an out transition before it is removed | scene |
| `:open` | its calendar popup is shown | kind |
| `:read-only` (`:read-write` is the opposite) | `read-only` is set | kind |

Never set on it (rules with them don't match): `:checked`, `:selected`, `:expanded`, `:invalid`, `:dragging`, `:busy`, `:indeterminate`, `:placeholder-shown`, `:default`, `:drop-target`, `:current`, `:backdrop`, `:required`, `:user-invalid`, `:has-children`.

The kind's own states (`state_bits!`, `WidgetCx::set_state`) match as `:state(<namespace>.<name>)`.

States are tracked once the scene has a style input (a `<style>` block, `class`, `style=` or a theme).
<!-- END GENERATED -->

## Accessibility

<!-- BEGIN GENERATED: a11y of kinds.DatePicker (tools/docs/egui/gen_reference.py). Do not edit. -->
| | |
|---|---|
| Role | `Button` |
| Name | `accessible-name`, else the shown date |
| Actions | `Click`, `Focus` |
| `invalid` | the `invalid` prop sets AccessKit `invalid` |
| Author id | `anchor` (unique in its root) |

A `Button` named by the shown date (or its accessible name), `expanded` while open, `has_popup` dialog. The popup holds the calendar's `Grid` (cells named by ISO date) and, with `commit-mode="confirm"`, Save and Cancel buttons; Escape closes it and focus returns to the button.
<!-- END GENERATED -->

## Testing

<!-- TODO(DOC7): how to find and drive it in a kittest harness. -->

## See also

<!-- TODO(DOC7) -->
