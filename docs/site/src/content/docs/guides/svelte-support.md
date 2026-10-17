---
title: Svelte support
description: Which Svelte 5 features work on native widgets today, which need more work, and which are compile errors.
---

Svelters compiles components with the vendored Svelte fork (`5.57.2`, branch `custom-condition` at `f3de3f2`, the #18042 custom renderer stack) and `experimental.customRenderer`, then runs them on `@svelters/renderer`. This page lists every Svelte feature the project surveyed.

## Status words

| Status | Meaning |
| --- | --- |
| **works** | Works today on `packages/renderer`. |
| **needs X** | Compiles or partly works; the named work is missing. |
| **n/a** | Meaningless for native UI. |
| **forbidden** | A compile error under the renderer. |
| **planned** | Not implemented. Design exists in the project notes. |

The survey is dated 2026-10-17. Every claim comes from compiling with the fork and running the result under Node against the production renderer.

:::note[Native widgets, not DOM]
Elements are native kinds (`window`, `box`, `label`, `button`, `entry`, ...), not HTML. See [the element reference](/api/) for the tags each toolkit provides.
:::

## Reactivity

| Feature | Status | Notes |
| --- | --- | --- |
| `$state`, `$state.raw` | works | Drives `set_prop`. Objects and arrays become deep proxies. |
| `$derived`, `$derived.by` | works | |
| `$effect`, `$effect.pre`, `$effect.root`, `$effect.tracking` | works | `.pre` runs before the block's host ops, `$effect` after. |
| `$props` | works | Basis of every wrapper component. |
| `$bindable` | works | Component two-way binding. See [Bindings](/guides/bindings/). |
| `$inspect`, `$inspect.trace` | works (dev) | Needs `console` from the host in dev. |
| `svelte/store` (`writable`, `$store`) | works | Needs the `custom-renderer` export condition. |
| Deep `$state` passed as a native prop | needs coercion | The value arrives as a `Proxy`. Deep mutations never reach `setAttribute`; use `$state.snapshot(x)` or reassign a `$state.raw` value. The renderer currently throws for object values (`objects are not valid host prop values`); planned: the renderer snapshots object props. |
| Static boolean attribute (`<spinner spinning />`) | needs metadata | Arrives as `""`, while `{expr}` booleans arrive as `true`/`false`. Coercion needs typed props from M3 metadata. |

## Template

| Feature | Status | Notes |
| --- | --- | --- |
| Text `{expr}` | works for `label`, `button` | Folded into the `label` property. Text anywhere else is dropped. Needs metadata for which kinds take text (`GtkTextView` buffer, `QPlainTextEdit::plainText`) and a warning for stray text. |
| Attributes and `{...spread}` | works | Spread drops non-`on*` function props (fork bug b). |
| `{#if}` / `{:else if}` / `{:else}` | works | One comment anchor per block, JS-only. Branches are removed and destroyed, new ones created and inserted. |
| `{#each}` keyed, unkeyed, index, destructuring, `{:else}` | works | Keyed moves are `remove` plus `insert`. See [Lists and snippets](/guides/lists-and-snippets/). |
| `{#key}` | works | Destroys and recreates the subtree. |
| `{#await}` | works | |
| `{@html}` | forbidden | n/a. A native rich-text label is a property (`GtkLabel use-markup`, `QLabel` rich text). |
| `{@const}` | works | |
| `{@debug}` | works (dev) | |
| `{@render}`, `{#snippet}`, `children` | works | Snippets are the replacement for slots. |
| `{@attach}` and `use:` | works | Runs after insert. The main way to reach the native node and to attach listeners with flags. |
| `createRawSnippet` | forbidden at runtime | Compiles, throws `invalid_snippet_in_custom_renderer`. |

## Events

| Feature | Status | Notes |
| --- | --- | --- |
| `onclicked={f}`, `onclose-request={f}`, `ontext-changed={f}` | works | `on` plus the native signal name in kebab case. There are no DOM-style aliases (`onclick`, `onchange`). See [Events](/guides/events/). |
| `on:event` directive | forbidden | `incompatible_with_custom_renderer`, in legacy and runes components. |
| `on<name>capture` | works | The capture flag reaches the renderer. Native capture-phase listening needs the `listen` flags ABI change (planned). |
| Other modifiers (`once`, `passive`, `self`, `stopPropagation`, ...) | needs `@svelters/events` | **planned**: `prevent`, `stop`, `once` helpers; `listen` with flags. |
| Component callback props (`onSave={...}`) | works | Plain props. |
| `createEventDispatcher` | works | Deprecated; do not use. |
| Handler return values | works for inline arrows and identifiers only | Props, calls and member expressions lose the return. Never rely on returns. |
| Sync handlers with results | **planned** (M4) | `call_sync` returns an error until then. |
| `svelte:window`, `svelte:document`, `svelte:body` | forbidden | Replaced by native elements and an app module (**planned**). |

## Bindings

| Feature | Status | Notes |
| --- | --- | --- |
| `bind:value`, `bind:checked`, `bind:group`, `bind:clientWidth` on elements | forbidden | Compile error under the renderer. Use component `$bindable` wrappers. |
| `bind:this` on elements | forbidden today | **In progress (F1):** coming through the Svelte fork. `{@attach}` is the interim way to get the node. |
| `bind:<prop>` on elements | forbidden today | **In progress (F1):** binding hook in the fork, renderer and binding metadata in phase B after P5. |
| `bind:prop` and `bind:this` on components | works | |
| Function bindings `bind:v={get, set}` | works | |
| Dimension bindings | forbidden | **planned**: a `resized` pseudo-signal and a `Measure` wrapper. |

## Snippets, lists and composition

| Feature | Status | Notes |
| --- | --- | --- |
| Snippets with parameters ("slot props") | works | Receive getters, so updates are fine-grained. |
| `List` with a row snippet | works | Right for tens to a few hundred rows. |
| Virtualized `ListView` | **planned** (M5) | Prototype only: `mount()` a `RowHost` per native row slot. |
| `ColumnView` and `Column` | **planned** (M5) | Prototype only. |
| Grids (`GtkGrid`, `QGridLayout`) | **planned** (M4) | Needs `sv-item` and a `set_child_attr` op. |
| Named native slots (`sv-slot`) | works | Routing works. Replace-default and restore semantics and generated wrapper components are **planned**. |
| Snippet declared directly inside a native element | footgun | Compiles, renders nothing. |
| Context (`setContext`, `getContext`, `getAllContexts`) | works | A separately `mount`ed root starts with no context unless it is passed. |
| `svelte:boundary` with `{failed}` attribute | works | |
| `svelte:boundary` with inline `{#snippet failed()}` | needs fix | Compiler crash when `experimental.async` is off (fork bug a). |
| Dynamic components, `<svelte:component>` | works | |
| `svelte:element` | works | A dynamic native kind. Widget state is lost on a kind change. |
| `svelte:self`, `svelte:fragment`, `svelte:options` | works | Compile where allowed. |
| Legacy `<slot>` | works | Compiles in legacy mode; do not use in new code. |

## Lifecycle

| Feature | Status | Notes |
| --- | --- | --- |
| `onMount`, `onDestroy`, `tick`, `untrack` | works | At `onMount` the widget exists and is inserted. Sizes read there are 0: allocation happens at the next frame. |
| `flushSync` | works | Needed in tests and factory handlers; a dev error inside sync signal handlers. |
| HMR | works in the trial | `compile({hmr})` plus a host registry; state resets. The M2 dev loop is **planned**. |

Order seen on the production renderer: script, `$effect.pre`, host ops, `mount()` returns, then `{@attach}`, `$effect`, `onMount`. On hide: `remove`, attach cleanup, `onMount` cleanup, `onDestroy`, then `destroy` in a microtask sweep.

## Styling

| Feature | Status | Notes |
| --- | --- | --- |
| `class`, `class:x`, `class={{...}}` | works | The class string is diffed and forwarded: GTK css classes, Qt `class` property. See [Styling](/guides/styling/). |
| `id` | works | Widget name (GTK) and object name (Qt), for `#id` selectors. |
| `style`, `style:x` | works | A structured channel: parsed, checked against the portable vocabulary and sent as declarations. `opacity` goes through the widget API. |
| `<style>` with `css: 'injected'` | forbidden | |
| `<style>` (external CSS) | works | Scoped by Svelte, translated at build time to GTK CSS and QSS with diagnostics, carried in the bundle as data. `@native gtk { }` and `@native qt { }` pass through. |
| `--sv-*` tokens, dark scheme | works | `setTokens` and `setColorScheme` from `@svelters/renderer/style`. |

## Motion

| Feature | Status | Notes |
| --- | --- | --- |
| `transition:`, `in:`, `out:`, `animate:` | forbidden | Native replacements: `GtkRevealer`/`GtkStack` wrappers, `QPropertyAnimation` (**planned**). |
| `svelte/motion` (`Tween`, `Spring`) | compiles | Schedules through `requestAnimationFrame`, which the host must provide from the frame clock (**planned**). |

## Compile options and runtime

| Feature | Status | Notes |
| --- | --- | --- |
| `experimental.customRenderer` | works | Required. |
| `experimental.async` (`await` in script or template, `pending` snippet) | works | Without it, `await` in the template is a compile error. Enabling it also avoids fork bug a. The decision to enable it is open. |
| SSR, hydration | n/a | The server build throws `custom_renderer_unavailable_on_server`. |
| `svelte:head` | forbidden, n/a | |

## Known fork bugs

Both reproduce on the top of the #18042 stack. They are written up but not filed upstream (`docs/notes/svelte-fork-upstream-bugs.md`); `tests/renderer-conformance/WEEKLY.md` re-verifies them weekly.

| Bug | Symptom | Workaround |
| --- | --- | --- |
| (a) Compiler crash | `TypeError: Cannot read properties of undefined (reading 'body')` compiling `<svelte:boundary>` with an inline `{#snippet failed()}` or `{#snippet pending()}` when `experimental.customRenderer` is set and `experimental.async` is off. | Pass the snippet as an attribute (`<svelte:boundary {failed}>`) or enable `experimental.async`. |
| (b) Spread drops functions | `{...spread}` on an element silently drops function-valued props whose names do not start with `on`. Explicit attributes pass intact. | Pass callbacks as `on*` props or as explicit attributes. |

## Unresolved items from the survey

- The renderer's O(n) sibling lookups were measured at 86 s for a reverse of 10,000 rows before the P5 fix. After the fix the bare renderer reverses 1000 rows in 46 ms; Svelte's own keyed `reconcile` is still quadratic. See [Lists and snippets](/guides/lists-and-snippets/).
- `.svelte` files are not yet typechecked against the generated element declarations (`svelte-check` is not installed in the workspace).

Sources: `docs/notes/svelte-feature-survey.md`, `docs/notes/svelte-fork-upstream-bugs.md`, `docs/notes/p5-integration.md`, `docs/notes/styling-gtk-css-qss.md`, `plans/STATUS.md`.
