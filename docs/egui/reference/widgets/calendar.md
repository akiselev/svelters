---
title: calendar
tag: calendar
kind: Calendar
category: pickers
description: "An inline month calendar (Qt's `QCalendarWidget`): a header with previous/next, a weekday row, ISO week numbers and a 6\u00d77 day grid."
status: stub
verified_at: ~
generated: partial   # regions between BEGIN/END GENERATED markers come from the spec
sources:
  - metadata/semantics/egui.json#kinds.Calendar
---

# `<calendar>`

<!-- BEGIN GENERATED: summary of kinds.Calendar (tools/docs/egui/gen_reference.py). Do not edit. -->
An inline month calendar (Qt's `QCalendarWidget`): a header with previous/next, a weekday row, ISO week numbers and a 6×7 day grid. `date-picker` shows the same calendar in its popup.

**Base:** [`Widget`](widget.md) · **Children:** none · **Text content:** no
<!-- END GENERATED -->

<!-- TODO(DOC7): when to use it, and what to use instead. -->

## Example

<!-- TODO(DOC7): a checked example (a testkit case under docs/egui/cases/). -->

## Props

<!-- BEGIN GENERATED: props of kinds.Calendar (tools/docs/egui/gen_reference.py). Do not edit. -->
| Prop | Type | Default | `bind:` | Description |
|---|---|---|---|---|
| `calendar-week` | `boolean` | `true` | no | Shows ISO week numbers in front of each week (`::part(week-number)`). |
| `day-parts` | `string[]` | `null` | no | Extra part names per date, `"YYYY-MM-DD name…"` each: `["2026-10-22 milestone"]` makes that day match `::part(day milestone)`. |
| `first-day` | `'auto' \| 'monday' \| 'tuesday' \| 'wednesday' \| 'thursday' \| 'friday' \| 'saturday' \| 'sunday'` | `"auto"` | no | The weekday the grid starts on; `auto` is the locale's (Monday until IN1). |
| `max` | `string` | `""` | no | The last day that can be picked (ISO 8601); days after it are `:disabled`. `""`: no limit. |
| `min` | `string` | `""` | no | The first day that can be picked (ISO 8601); days before it are `:disabled`. `""`: no limit. |
| `navigation` | `boolean` | `true` | no | Shows the header with the previous/next month buttons and the month and year. |
| `read-only` | `boolean` | `false` | no | Shows and roves the calendar but never picks (Qt's `NoSelection`). |
| `value` | `string` | `""` | **yes**, on `changed` | The picked date, ISO 8601 (`2026-10-22`); `""` for none. |
| `weekday-format` | `'narrow' \| 'short' \| 'long' \| 'none'` | `"short"` | no | The weekday header: `narrow` (M), `short` (Mo), `long` (Monday) or `none` (no header row). |

Plus the 31 props every element has, from [`Widget`](widget.md#props): `accessible-name`, `align-self`, `anchor`, `anchor-scope`, `autofocus`, `bottom`, `dialog-role`, `drag-data`, `drag-type`, `drop-types`, `enabled`, `flex-basis`, `flex-grow`, `flex-shrink`, `height`, `invalid`, `left`, `margin`, `max-height`, `max-width`, `min-height`, `min-width`, `position`, `right`, `source-loc`, `status-section`, `status-tip`, `tooltip`, `top`, `visible`, `width`.
<!-- END GENERATED -->

## Events

<!-- BEGIN GENERATED: events of kinds.Calendar (tools/docs/egui/gen_reference.py). Do not edit. -->
| Attribute | Handler | When |
|---|---|---|
| `onchanged` | `(value: string) => void` | The user picked a day; carries it (ISO 8601). |
| `oncommit` | `(value: string) => void` | A pick is a commit (§2.4); carries the date. |
| `onpage-changed` | `(year: number, month: number) => void` | The user moved to another month (prev/next, PageUp/PageDown, arrows across a month). |

`on:changed` works too, and a bare `on:<event>` on a component forwards the component's `on<event>` prop.

Plus the events of [`Widget`](widget.md#events): `onanimationend`, `onclick`, `oncontextmenu`, `ondblclick`, `ondrag-begin`, `ondrag-end`, `ondrag-enter`, `ondrag-leave`, `ondrop`, `onfocusin`, `onfocusout`, `onkeydown`, `onkeyup`, `onpointerdown`, `onpointermove`, `onpointerup`, `onwheel`.
<!-- END GENERATED -->

## Methods

<!-- BEGIN GENERATED: methods of kinds.Calendar (tools/docs/egui/gen_reference.py). Do not edit. -->
None.
<!-- END GENERATED -->

## Behaviour you need to know

<!-- TODO(DOC7): what the tables don't say: timing, edge cases, limits. -->

## CSS

<!-- BEGIN GENERATED: css of kinds.Calendar (tools/docs/egui/gen_reference.py). Do not edit. -->
**Selector:** `calendar`, plus `.class`, `#id` and attribute selectors on its props.

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

**Layout properties** (as attributes or in CSS; an explicit attribute wins): `align-self`, `bottom`, `dialog-role`, `flex-basis`, `flex-grow`, `flex-shrink`, `height`, `left`, `max-height`, `max-width`, `min-height`, `min-width`, `position`, `right`, `status-section`, `top`, `visible`, `width`.
Also in CSS only: `display`, `grid-column`, `grid-row`, `margin-bottom`, `margin-left`, `margin-right`, `margin-top`.

Parts: `header`, `prev`, `next`, `month`, `year`, `weekday`, `week-number`, `grid`, `day`. Day states `:hover`, `:selected`, `:current` (the keyboard cursor, ringed while `:focus-visible`), `:disabled`, and the part names `today`, `weekend`, `other-month` plus `day-parts`' names, painted from the UA tables until per-day part styles reach egui.

Full matrix: [css/properties](../../css/properties.md). States: below.
<!-- END GENERATED -->

## States

<!-- BEGIN GENERATED: states of kinds.Calendar (tools/docs/egui/gen_reference.py). Do not edit. -->
| Pseudo-class | Set when | Source |
|---|---|---|
| `:hover` | the pointer is over it | response |
| `:active` | the primary button is down on it | response |
| `:focus` | it has keyboard focus | response |
| `:focus-within` | it, or an element inside it, has keyboard focus | scene |
| `:disabled` (`:enabled` is the opposite) | `enabled` is false | prop |
| `:exiting` | it plays an out transition before it is removed | scene |
| `:read-only` (`:read-write` is the opposite) | `read-only` is set | kind |

Never set on it (rules with them don't match): `:checked`, `:selected`, `:open`, `:expanded`, `:invalid`, `:dragging`, `:busy`, `:indeterminate`, `:placeholder-shown`, `:default`, `:drop-target`, `:current`, `:backdrop`, `:required`, `:user-invalid`, `:has-children`.

The kind's own states (`state_bits!`, `WidgetCx::set_state`) match as `:state(<namespace>.<name>)`.

States are tracked once the scene has a style input (a `<style>` block, `class`, `style=` or a theme).
<!-- END GENERATED -->

## Accessibility

<!-- BEGIN GENERATED: a11y of kinds.Calendar (tools/docs/egui/gen_reference.py). Do not edit. -->
| | |
|---|---|
| Role | `Grid` |
| Name | `accessible-name` |
| Value | the picked date |
| Actions | `Focus` |
| `invalid` | the `invalid` prop sets AccessKit `invalid` |
| Author id | `anchor` (unique in its root) |

A `Grid` of six `Row`s of `GridCell`s, each named by its ISO date (`selected`, `disabled`, Click); previous/next are `Button`s. The grid is one tab stop and the arrow keys rove its days.
<!-- END GENERATED -->

## Testing

<!-- TODO(DOC7): how to find and drive it in a kittest harness. -->

## See also

<!-- TODO(DOC7) -->
