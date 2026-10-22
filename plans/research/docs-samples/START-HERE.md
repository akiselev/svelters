---
title: svelters-egui, start here
description: The one page an agent reads before writing a Svelte panel, a Rust kind or an embedding change in cadatomic.
status: available
audience: cadatomic agents, extension authors
verified_at: 424ebcf4
api:
  - svelters_egui::Embed
  - svelters_egui::embed::{EmbedConfig, SlotOptions, MountSpec, Props, EmbedEvent}
  - svelters_egui::egui
---

<!-- SAMPLE (plans/research/DOCS-PLAN.md §8). Final path: docs/egui/START-HERE.md. Links point at the final
     layout of DOCS-PLAN.md §4; most targets do not exist yet. Facts checked against master 424ebcf4. -->

# svelters-egui: start here

Read this page before you write a Svelte panel, a Rust kind, a behaviour or embedding code. It is
short on purpose: each rule links to the page that explains it.

**Read the docs at your pin.** cadatomic builds against the sibling checkout `../svelters`, at
the sha in `pins.toml`. Read `../svelters/docs/egui/` in that checkout, not GitHub's master. A feature
documented on master may not be in your pin. Every page has a `status:` line and a `verified_at:` sha
at the top.

## The model in six lines

1. **Rust owns the shell** (docking, tabs, toolbars, menus, windows). **Svelte renders panel contents.**
   Each panel is a *root* mounted into a *slot*. You draw the slot wherever you like with
   `embed.show_slot(ui, slot)`.
2. **There is one retained scene, and svelters-egui owns it.** JS edits it through host ops, taffy lays
   it out, and each egui pass walks and paints it. You extend it with kinds, behaviours, state
   providers and scene passes. You never build a second tree.
3. **JS runs only inside `Embed::logic`.** `show_slot` never runs JS. Events raised while a slot is
   drawn are queued and delivered at the next `logic`.
4. **Elements are kinds.** There are 45 built-in tags, plus your `#[svelters::widget]` kinds and
   `#[svelters::behavior]` behaviours (`use:`). The compiler checks every tag, prop, event and
   `bind:` against the spec, so a typo is a compile error, not a silent no-op.
5. **Styling is CSS,** run by svelters' Rust engine on the scene, with VS Code-derived tokens
   (`--sv-*`). One CSS `px` is one egui point.
6. **Rust and JS exchange typed values** (records, enums, lists, `Json<T>`, stores, events). Values
   never cross as JSON text.

## The frame contract

```rust
use svelters_egui::{Embed, egui};
use svelters_egui::embed::{EmbedConfig, EmbedEvent, MountSpec, SlotOptions};

// Once, at startup.
let embed = Embed::new(ctx, EmbedConfig { prepass_plugin: false, ..EmbedConfig::default() })?;
let entry = embed.load("ui/dist/embed.js")?;
let slot = embed.create_slot("panel.task", SlotOptions::fill().label("Task"));
embed.mount(MountSpec::new(entry, "TaskPanel", slot, "panel.task"))?;

// Every frame, in this order.
embed.logic(ctx);                         // 1. all JS runs here, once per frame
// ... your shell draws its panes ...
embed.show_slot(ui, slot);                // 2. from any pane or window; never runs JS
for ev in embed.take_events() {           // 3. mounts, faults, titles, emits, errors
    match ev { EmbedEvent::Faulted { root, error } => { /* log; the slot shows the fault UI */ } _ => {} }
}
```

`prepass_plugin: false` is right when your shell owns the frame order (cadatomic does). The full
contract, with deferred viewports and `show_viewport_passes`, is in
[rust/frame-order](rust/frame-order.md).

## Rules that prevent the mistakes we have seen

| Do | Don't | Why / page |
|---|---|---|
| Use `svelters_egui::egui` (re-exported) | add an `egui` dependency at another version | egui is pinned `=0.37.0`. Two copies mean two `Context` types. [rust/versions](rust/versions.md) |
| Pass records, enums, `Vec`, `Option`, `Json<T>` through exports | pass JSON text and call `JSON.parse` in Svelte | Values cross natively; errors carry a path (`at shape.r: …`). [rust/data-across-the-bridge](rust/data-across-the-bridge.md) |
| Give the panel app state through a `#[svelters::store]`, an `#[svelters::object]` passed in mount props, or a `#[svelters::service]` | keep a `thread_local!` bridge that free functions reach into | [rust/data-across-the-bridge](rust/data-across-the-bridge.md#app-state) |
| Call `your_crate::link()` from `main` for every crate that registers kinds, behaviours or exports | rely on the linker to keep `inventory` registrations | Registrations in an rlib nothing references are dropped silently. [rust/setup](rust/setup.md#linking) |
| Write native event names: `onclicked`, `onchanged={(text) => …}` | write DOM names: `onclick`, `oninput`, `e.target.value` | Handlers get the signal's arguments. `onclick` is `unknown_event`. [svelte/events](svelte/events.md) |
| Style with `<style>`, classes and `:state(cad.invalid)` | paint outlines yourself in `Widget::ui` to show a state | [css/states](css/states.md), [recipes/invalid-field](recipes/invalid-field.md) |
| Lay out with `display: grid` or `<grid>` | align columns with fixed-width spacer boxes | [svelte/layout](svelte/layout.md) |
| Give stable `anchor`s to nodes that tests or automation find | find nodes in tests by the `Debug` text of a role | Anchors become AccessKit author ids. [rust/testing](rust/testing.md) |
| Wait with `EmbedHarness::run_until_settled(n)` / `Embed::settled()` | call `harness.run()` a fixed number of times | [rust/testing](rust/testing.md) |
| Route host shortcuts through `Embed::shortcut_override` and edit commands through `Embed::text_command` | read `ctx.egui_wants_keyboard_input()` | The focused field claims its keys; IME composition takes every key. [rust/focus-and-keys](rust/focus-and-keys.md) |
| Name component props anything except `slot` (and the other reserved names) | use `slot` as a prop name | `slot` is Svelte's legacy slot attribute. [svelte/components](svelte/components.md#reserved-names) |

## Before you build something, check that it doesn't exist

| You need | It exists as | Status |
|---|---|---|
| A native Rust widget used from Svelte | `#[svelters::widget]` + `Widget` | available |
| Per-frame Rust logic attached to any element (`use:`) | `#[svelters::behavior]` + `Behavior` | available |
| Custom states for CSS (`:state(cad.pending)`) | `state_bits!`, `WidgetCx::set_state`, `StateProvider` | available |
| A pass over the scene each frame (overlays, picking aids) | `ScenePass`, `Embed::register_pass`, `show_viewport_passes` | available |
| Reading or changing the scene from Rust | `SceneHandle`, `Query`, anchors, the change journal | available |
| Thousands of rows | `<virtual-list>`, `<table>`, `VirtualChildren` / `RowPool` | available |
| Keyboard navigation, type-ahead and selection in lists and tables | `virtual-list`, `table` | available (WG5a) |
| Menus, context menus, popups, modals, inner windows | `menu-bar`, `menu-button`, `context-menu`, `popup`, `modal`, `inner-window` | available |
| Focus scopes, focus restore, default and cancel buttons | `focus-scope`, `autofocus`, `default` and `cancel` on `button` | available |
| Clipboard, file and message dialogs, drag and drop | `svelters:clipboard`, `svelters:app/egui`, `drag-type` / `drop-types` | available |
| Settings, persisted UI state, standard paths, logging | `svelters:settings`, `svelters:paths`, `svelters:log` | available |
| Theme tokens shared by Svelte, stock egui and your Rust UI | `svelters_egui::theme::Theme`, `EguiBackend::set_theme` | available |
| A Svelte panel test with expectations in the file | `svelters-testkit` cases (`@test` blocks) | available |
| Methods on your kind callable from Svelte (`editor.revealLine(3)`) | kind methods | in progress (RA3) |
| Capture and bubble of pointer and key events, event filters | EV1 | in progress |
| `bind:` with commit-on-Enter, draft values | `\|commit` (CX3) | in progress |
| Semantic icon ids (`mcad.extrude`), SVG icons | DS5, IC1, AS1 | in progress |
| UI zoom (Ctrl+= / Ctrl+-) | ZM1 | planned |

The full list, keyed to cadatomic's SV ids with the sha that delivered each, is
[status](status.md). If something you need is missing, don't build a workaround in silence. File it
in cadatomic's `docs/plan/04-kernel-requests.md` and tell main-cadatomic. svelters-agent answers.

## Where to look

| Task | Page |
|---|---|
| Put a Svelte panel in a dock pane | [recipes/panel-in-dock-pane](recipes/panel-in-dock-pane.md) |
| A task panel (form) on a Rust model | [recipes/task-panel](recipes/task-panel.md) |
| What a tag takes (props, events, `bind:`, CSS, states, a11y) | [reference/widgets/](reference/widgets/index.md), one page per tag |
| What CSS works on egui | [css/properties](css/properties.md) (generated matrix) |
| A compiler error code | [diagnostics/](diagnostics/index.md), or `svelters-compile --explain <code>` |
| A runtime warning or a faulted root | [troubleshooting](troubleshooting.md) |
| Write your own kind or behaviour | [rust/custom-kinds](rust/custom-kinds.md), [rust/behaviours](rust/behaviours.md) |
| Test a panel | [rust/testing](rust/testing.md) |
| Rust API details | rustdoc: `tools/dev/cargo-slot cargo doc -p svelters-egui -p svelters --no-deps` |

Quick lookups without opening files (after DOC1 adds the egui pack): `svelters docs egui text-edit` and
`svelters docs egui text-edit.changed`, which read the CLI's offline docs pack. `grep -rl "tag: text-edit" docs/egui/`
also works.

## Status words

Every page and every table row uses one of these:

- **available**: on master at `verified_at` or earlier, with tests. Check that your pin includes it.
- **in progress (LANE)**: being built. Don't use it, and don't build your own version. Wait, or ask.
- **planned**: decided, not started.
- **deferred**: not now. The page says what to do instead.
