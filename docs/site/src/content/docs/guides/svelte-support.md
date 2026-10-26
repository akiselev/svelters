---
title: Svelte support
description: Which Svelte 5 features the svelters compiler accepts on native widgets, which it rejects, and where it departs from Svelte.
---

Svelters compiles components with its own compiler, `svelters-compile` (crate `crates/svelters-compiler`). It is a Rust port of Svelte 5.57.2's parser, acorn and scope analysis. Script semantics follow Svelte's runes mode and emit the same reactive calls; the template is lowered to native operations instead of DOM code. This page lists what it accepts.

## Status words

| Status | Meaning |
| --- | --- |
| **works** | Compiles and runs on the native runtime. |
| **partial** | Compiles with the restriction named in the notes. |
| **error** | A compile error. The code is given where it is not `unsupported_feature`. |
| **n/a** | Meaningless for native UI. |

Behaviour not listed as a departure here or in `docs/notes/native-compiler/deviations.md` is meant to match Svelte. Two checks keep it honest: the oracle compares the parser and scope ports with upstream, and the conformance ratchet (`experiments/native-compiler/conformance`) runs Svelte's own `runtime-runes` samples through the compiler and runtime on a DOM test backend. 591 of the 774 samples with a DOM variant pass; most of the rest use features rejected below (transitions, legacy syntax, SVG) or call DOM APIs on what is a native handle.

:::note[Native widgets, not DOM]
Elements are native kinds (`window`, `box`, `label`, `button`, `entry`, ...), not HTML. The compiler checks every tag, prop, event, child and binding against the toolkit's semantic spec (`metadata/semantics/<toolkit>.json`), so a mistake is a compile error with a position. See [the element reference](/api/) for the tags each toolkit provides.
:::

## Script and runes

| Feature | Status | Notes |
| --- | --- | --- |
| `$state`, `$state.raw`, `$state.snapshot`, `$state.eager` | works | Also in class fields, constructors (`this.x = $state(v)`), nested functions and destructured declarations. |
| `$derived`, `$derived.by` | works | Synchronous only. `await` inside `$derived` is an error. |
| `$effect`, `$effect.pre`, `$effect.root`, `$effect.tracking`, `$effect.pending` | works | |
| `$props`, `$bindable`, `$props.id()` | works | Identifier, destructuring, defaults and rest. |
| `$inspect`, `$inspect(...).with`, `$inspect.trace` | works (dev) | Kept with `svelteConfig(root, { dev: true })`, stripped otherwise. |
| `$host()` | error | `host_invalid_placement`: no custom elements. |
| Stores (`$store` reads, writes, `bind:` to `$store.x`) | works | |
| `<script module>`, instance `export function` / `export const` | works | Re-exports from the instance script are an error. |
| `.svelte.js` modules | works | Compiled by the same binary (`--module`). |
| `<script lang="ts">` | works | TypeScript is parsed and stripped, as Svelte does. |
| Legacy mode (`export let`, `$:`) | error | `legacy_export_invalid`; runes mode is forced. |

The compiler writes its output as edits over your source rather than reprinting it, so generated code keeps your formatting. Dev mode keeps `$inspect` and labels signals; Svelte's other dev instrumentation (ownership checks, mutation warnings) is not emitted.

## Template

| Feature | Status | Notes |
| --- | --- | --- |
| Static attributes | works | Typed at compile time from the spec: `spacing="8"` is the integer 8, `<spinner spinning />` is `true`. A value that does not fit is `bad_attribute_value`. Values the backend also takes as text (fonts, colours, icons, shortcuts) pass as strings. |
| `{expr}` attributes | works | One render effect per reactive prop. Dynamic values are not type-checked by the compiler; the element typings in `@svelters/native-runtime/types` cover them in an editor or `svelte-check`. |
| Text and `{expr}` | partial | Only in elements with a text-content prop (`label` and `button` on GTK, where it sets `label`). Elsewhere it is `text_not_allowed`. Inside such an element, `{#if}` with text-only branches folds into the text; other blocks are `text_content_children`. |
| `{#if}` / `{:else if}` / `{:else}` | works | The new branch is built first, then the old one removed and destroyed. |
| `{#each}` keyed, unkeyed, index, `{:else}` | partial | The item must be a plain identifier: destructuring is an error. Keyed moves are minimal (`remove` plus `insert` outside a longest increasing subsequence). See [Lists and snippets](/guides/lists-and-snippets/). |
| `{#key}` | works | Recreates the subtree. |
| `{#await}` | partial | `{:then v}` and `{:catch e}` take plain identifiers. `pending` is shown from a microtask unless the promise already settled. |
| `await` in template expressions | not supported | There is no async mode. Do not use it: the compiler does not reject it yet. |
| `{@const}` | partial | Plain identifiers only. |
| `{@html}` | error | n/a. A rich-text label is a property (`GtkLabel use-markup`, `QLabel` rich text). |
| `{@debug}` | error | |
| `{#snippet}`, `{@render}`, `children` | partial | Snippet parameters must be plain identifiers (no destructuring or defaults). |
| `{@attach}` | works | Runs in an effect after mount, with the runtime node. |
| `use:` actions | error | Use `{@attach}`. |
| Spread `{...obj}` on native elements | works | An object literal is checked at compile time. Any other spread is checked at run time against the toolkit's spec: each key becomes a property (strings are converted to the property's type), an event handler, a class or style, or a child attribute (`parent:`). A key the element does not have prints a warning in dev builds and is skipped. A key that goes away goes back to the default. Later attributes win, as in Svelte. |
| Spread on components | works | |
| `parent:<name>={v}` | works | Svelters syntax: a child attribute (grid cell, stretch, tab title) checked against the actual parent's slot. |
| `<sv-slot name="x">` | works | Svelters syntax: routes its content to the parent's named slot. |

## Components and special elements

| Feature | Status | Notes |
| --- | --- | --- |
| Components, props, callback props | works | |
| `bind:prop` and `bind:this` on components | works | `bind:this` binds the component's exports. |
| Dynamic components, `<svelte:component this>` | works | Recreated when the component changes. |
| `<svelte:boundary>` | partial | `onerror` and `failed` work; `pending` is ignored (no async mode). |
| Context (`setContext`, `getContext`, `getAllContexts`) | works | `ListView` rows get the list's context. |
| `<svelte:element this={tag}>` | works | `tag` is a native tag (`label`, `button`). Its attributes are checked at run time, as for a spread. A new tag recreates the element; a nullish tag renders nothing. Only `bind:this` among bindings. |
| `<svelte:self>`, `<svelte:fragment>` | error | |
| `<svelte:window>`, `<svelte:document>`, `<svelte:body>`, `<svelte:head>` | error | DOM-only. See [Replacing svelte:window](/guides/events/#replacing-sveltewindow). |
| `<slot>` (Svelte 4) | error | Use snippets. |
| `<svelte:options>` | works | At the top level. |

## Toplevels and attached kinds

| Feature | Status | Notes |
| --- | --- | --- |
| Windows and dialogs anywhere in the tree | works | A toplevel inside an element is transient for the enclosing toplevel and shown after it. |
| `show="modal\|modeless\|none"` on a toplevel | works | Static only (`show_invalid`). Dialogs default to modal, windows to modeless. With `none` the app opens it: `bind:this` plus `call('present')`. |
| GTK event controllers (`<event-controller-key>`, `<gesture-click>`, ...), `<context-menu>`, the Qt `<menu>` inside a non-menu widget | works | Attached to the enclosing widget whatever its slots, also inside a text element: `<button>Text <context-menu>...</context-menu></button>`. |
| Menus and actions (`menu-bar`, `tool-bar`, `menu`, `action`, `menu-separator`) | works | GTK vocabulary; Qt uses `menu title=` and `action text=`. Fixtures `menus`, `menus-qt`. |
| `<canvas>` with `{@attach draw(...)}` from `@svelters/canvas` | works | GTK and Qt. |

## Events

| Feature | Status | Notes |
| --- | --- | --- |
| `onclicked={f}`, `onclose-request={f}` | works | `on` plus the native signal name. There are no DOM-style aliases: `onclick` is `unknown_event`. See [Events](/guides/events/). |
| `on<signal>capture={f}` | works | Where the spec allows the capture phase; else `listen_phase_unsupported`. |
| `{@attach listen(sig, fn, { after, capture })}` | works | `listen` from `@svelters/native-runtime`. |
| Sync signals (`close-request`, `key-pressed`) | works | Handlers get a `NativeEvent` first: `e.preventDefault()`, `e.result = v`. |
| Cancelable value signals (skelter's `changed` on checkbox, switch, slider) | works | Handlers get the arguments as on any signal, then the `NativeEvent` last: `(v, e) => e.preventDefault()` cancels. |
| Legacy `on:name={handler}` | works | A listener like `onname={handler}`; on a component, the prop `onname={handler}`. Modifiers are errors. |
| Bare `on:name` (forwarding) | works | Forwards the component's own `onname` callback prop to one handler, the parent's; no component events ([events guide](/guides/events/)). |

## Bindings

| Feature | Status | Notes |
| --- | --- | --- |
| `bind:this` on elements | works | The runtime node: `call`, `getProp`, `StaleHandleError` after destroy. |
| `bind:<prop>` on elements | works | Where the spec has a binding plan (`bind:text` on an entry, `bind:checked` on an action, `bind:value` on a Qt slider). Else `bind_no_plan`, `unknown_property` or `read_only_property`. See [Bindings](/guides/bindings/). |
| Function bindings `bind:v={get, set}` | works | On elements and components. |
| `bind:group`, `bind:clientWidth` and other DOM bindings | error | Not native properties. |

## Lifecycle

| Feature | Status | Notes |
| --- | --- | --- |
| `onMount`, `onDestroy`, `tick`, `untrack`, `flushSync` | works | Svelte's own runtime. At `onMount` the widget exists and is inserted; sizes read there are 0 until the toolkit lays it out. |
| Hot reload | works | `svelters-build --watch` with the host's reload (M2b); state resets. |

## Styling

| Feature | Status | Notes |
| --- | --- | --- |
| `class`, `class:x`, `class={{...}}` | works | GTK CSS classes, Qt `class` property. See [Styling](/guides/styling/). |
| `id` | works | Widget name (GTK), object name (Qt). |
| `style`, `style:x={v}` | partial | `style:x` shorthand and `style:x\|important` are errors. |
| `<style>` | works | Scoped by Svelte's CSS compile, translated to GTK CSS and QSS; setter properties are matched at compile time. |

## Motion

| Feature | Status | Notes |
| --- | --- | --- |
| `transition:`, `in:`, `out:`, `animate:` | error | Native replacements (`GtkRevealer`, `GtkStack`, `QPropertyAnimation`) are not wrapped yet. |
| `svelte/motion` (`Tween`, `Spring`) | compiles | Schedules through `requestAnimationFrame`, which the host drives from the toolkit's frame clock. Not covered by a fixture. |

## Not applicable

SSR and hydration do not exist: the compiler emits client code only.

Sources: `docs/notes/native-compiler/language-subset.md`, `docs/notes/native-compiler/deviations.md`, `docs/notes/native-compiler/runtime-contract.md`, `docs/notes/native-compiler/semantic-spec.md`, `experiments/native-compiler/conformance/REPORT.md`, `crates/svelters-compiler/src/lower/mod.rs`, `fixtures/`, `STATUS.md`.
