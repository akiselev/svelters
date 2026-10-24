---
title: Start here
description: "The index of svelters' egui docs: which page to read for which task."
status: available
verified_at: c0faf05a
sources:
  - plans/research/DOCS-PLAN.md   # §3.2 item 1, §5
---

# Start here

svelters renders Svelte 5 components on egui. Rust owns the app shell. Svelte renders the contents of
panels, dialogs, windows and overlays. svelters draws every widget itself; egui supplies only the painter,
input, text shaping, layers and AccessKit. JS modules and `.svelte` files are loaded and compiled live by
svelters' own Rust loader, in release builds too.

Read the page for your task. `plans/research/*` and `plans/milestones/*` are design history, not API
docs. If a feature you need isn't covered here, check `STATUS.md` before building your own.

**Status words** (each page's front matter): `available` (on master at `verified_at`), `pending` (built on
a lane branch, not merged; the page says which), `stub` (a page skeleton, still to be written).

## Tasks

| You want to | Read |
|---|---|
| look up a tag's props, events, parts and states | [Widget reference](reference/widgets/index.md) |
| use the own menus, popup, select and tooltips | [menu-bar](reference/widgets/menu-bar.md), [context-menu](reference/widgets/context-menu.md), [popup](reference/widgets/popup.md), [combo-box](reference/widgets/combo-box.md); the common `tooltip` prop works on every tag |
| use the own calendar, colour editor and pickers | [calendar](reference/widgets/calendar.md), [color-editor](reference/widgets/color-editor.md), [color-picker](reference/widgets/color-picker.md), [date-picker](reference/widgets/date-picker.md) |
| build a control the built-in tags don't have, drawn by svelters | [Writing an own widget](rust/own-widgets.md) |
| restyle a widget: parts, states, icons in CSS, painters, transitions, list rows | [Styling own widgets](css/own-widgets.md) |
| show icons by key, register app or extension icons | [Icons](icons.md) |
| test an app or a component end to end, with seeds and virtual time | [Automation and end-to-end tests](rust/automation.md) |
| feed a `table` (or a tree) from Rust rows at 20k+ rows, sorted and filtered in Rust | [Rust row models](rust/row-models.md) |
| handle pointer events on a canvas, make a pane you paint hit-testable, anchor rows of a keyed list | [Events, canvases and panes](rust/events.md) |
| load an app or extension from source, ship a `.svb` bundle, control what extensions can import | [Loading code and release bundles](rust/loading-code.md) |
| understand what an edit does to the running app and which state survives | [Hot reload](rust/hot-reload.md) |

## All pages

### Rust

- [Writing an own widget](rust/own-widgets.md): `#[svelters::widget]` with behaviour units, paint
  primitives and parts; events, focus and the paint cache. Read it before writing any widget code.
- [Automation and end-to-end tests](rust/automation.md): `#[svelters_automation::test]`,
  `@svelters/test` with `svelters-e2e`, `mount`, locators, `svelters-run`, `Session`. Read it before
  writing UI tests.
- [Rust row models](rust/row-models.md): `RowModel`, `RowView` (sort, filter, tree rows),
  `set_table_model`, and `RowSource` for realised `virtual-list` rows. Read it before giving a table
  or list rows that live in Rust.
- [Events, canvases and panes](rust/events.md): DOM-style events with local points (`pointermove`),
  Rust filters, `PaneHit` panes the host paints, anchors in keyed lists. Read it before wiring a canvas.
- [Loading code and release bundles](rust/loading-code.md): `SourceProvider`, `SvbProvider`,
  `ModuleHooks` and extension isolation, `svelters-bundle`, the bytecode cache. Read it when you wire up
  loading or ship a release.
- [Hot reload](rust/hot-reload.md): component swaps, kept `$state`, style-only restyles, fallbacks. Read
  it when you set up a dev loop or an editor that saves into the running app.

### CSS

- [Styling own widgets](css/own-widgets.md): `::part()`, which parts take CSS today, list rows, state
  pseudo-classes and their bits, `content: icon()`, `paint()`, transitions, the DESIGN rules in CSS. Read
  it before writing a `<style>` for widgets.

### Icons

- [Icons](icons.md): keys, `ext.<id>.*`, registration, optical sizes, tint and colour slots.

### Reference (generated)

- [Widget reference](reference/widgets/index.md): one page per tag, generated from
  `metadata/semantics/egui.json` by `tools/docs/egui/gen_reference.py`. Don't edit the generated regions.
  Look a tag up here before you guess a prop name.
