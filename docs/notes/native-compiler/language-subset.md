# NC1 compiler: supported rune / script subset

Script semantics follow Svelte 5.57.2 runes mode (client output, `svelte/internal/client` as
`$`). Departures: `deviations.md` ("Rune transform"). Tests: `crates/svelters-compiler/tests/runes/*`
(generated JS snapshots, `UPDATE=1 cargo test -p svelters-compiler --test runes`) and the
behavioural fixtures `experiments/native-compiler/fixtures/runes-*`.

| Feature | Status | Output |
| --- | --- | --- |
| `let x = $state(v)` / `$state.raw(v)` | yes | `$.state(...)` when reassigned, else the plain (proxied) value, as upstream |
| `$derived(e)` / `$derived.by(f)` | yes (sync) | `$.derived(() => e)` / `$.derived(f)`; async (`await`) rejected |
| `$state` / `$derived` in nested functions, loops | yes | same as top level |
| Destructured `$state` / `$state.raw` / `$derived` declarations | yes | `tmp`, `$$array = $.derived($.to_array(..))`, `$.fallback`, `$.exclude_from_object` |
| Destructuring assignment to state / props / stores (`[a, b] = [b, a]`) | yes | IIFE / sequence as upstream |
| Assignments `=`, compound, logical (`??=`...), `++`/`--` | yes | `$.set`, `$.update(_pre)`, proxy flag as upstream |
| Class fields `x = $state()`, `#x = $state()`, `$state.raw`, `$derived(.by)` | yes | private source + get/set accessors |
| Constructor `this.x = $state(v)` | yes | backing `#x` + accessors, `.v` reads in the constructor |
| `$props()` (identifier, destructuring, defaults, `$bindable`, rest) | yes | `$.prop(..)` only when needed, else `$$props.x`; `$.rest_props` with hoisted `rest_excludes` |
| `$props.id()` | yes | `$.props_id()` |
| `$effect`, `$effect.pre`, `$effect.root`, `$effect.tracking`, `$effect.pending` | yes | `$.user_effect`, ..., `$.eager(() => $.pending())` |
| `$state.snapshot`, `$state.eager` | yes | `$.snapshot`, `$.eager(() => v)` |
| `$inspect(..)`, `$inspect(..).with(fn)`, `$inspect.trace()` | yes | dev (`--dev`): `$.inspect`, `$.trace`; prod: stripped |
| `$host()` | no | `host_invalid_placement` (no custom elements) |
| `$store` reads / writes / updates / member mutation, `bind:` to `$store.x` | yes | `$.setup_stores`, `$.store_get`, `$.store_set`, `$.update_store`, `$.store_mutate`, `$.store_unsub`, `$$cleanup()` |
| `<script module>` (runes, exports, imports) | yes | hoisted above the component |
| Instance `export function` / `export const` / `export { a as b }` | yes | `$$exports` object, `return $.pop($$exports)` |
| `.svelte.js` modules (`compile_module`, CLI `--module`, Vite plugin) | yes | `client_module` shape |
| `<script lang="ts">` (and TypeScript in its template expressions) | yes | parsed by the acorn-typescript port; the types are stripped from the output (`svelte/ts.rs`); enums, decorators, accessor fields, constructor parameter properties and value namespaces are `typescript_invalid_feature`, as upstream (`deviations.md`) |
| `.svelte.ts` modules | no | `compile_module` parses JavaScript only |
| `{await p}` in a template expression (Svelte's experimental async) | no | `unsupported_feature` (FU3; it used to compile into an `await` inside a non-async effect); an `await` inside a template function (an `async` handler) and `{#await p}` blocks are fine |
| Legacy mode (`export let`, `$:`, stores as state) | no | `legacy_export_invalid`; runes mode is forced |

## Native template attributes (X2/X3 renderer contract)

| Feature | Status | Output |
| --- | --- | --- |
| `parent:<name>={v}` / `parent:<name>="v"` | yes, typed against the parent's slot | `$n.child_attr(node, name, v)` (render effect when reactive) |
| `on<event>capture={fn}` | yes, where the spec allows `capture` | `$n.on(node, signal, fn, 2)` |
| `{@attach listen(sig, fn, { after, capture })}` (`listen` from `@svelters/native-runtime`) | yes; static type/options resolved and phase-checked | `$.attach(node, () => $n.listen(signal, fn, flags))` |
| A toplevel kind (window, dialog) anywhere, `show="modal\|modeless\|none"` (RT1, X7) | yes; `show` must be static (`show_invalid`) | `$n.toplevel(kind, present, show?)`: shown at the root, or transient for the enclosing toplevel and shown after it |
| An `attaches_to` kind (GTK event controllers, `context-menu`; Qt `menu` outside a menu container) inside any widget, also next to text in `<button>` (RT1) | yes | `$n.attached(kind)`: inserted with no slot or position |
| `{@attach f}` | yes | `$.attach(node, () => f)` |
| Handlers of sync signals | yes | listeners get `(event, ...args)` with a `NativeEvent` (`e.preventDefault()`, `e.result = v`); `(...args)` for every other signal |
| `{...{ k: v }}` (object literal) on native elements | yes (static keys, checked at compile time) | one op per key |
| `{...expr}` on native elements (SP1) | yes, resolved at run time against the spec tables; explicit attributes stay checked at compile time | `$n.spread(node, $$kinds, () => ({...all attributes in source order}))`; unknown keys warn in DEV (runtime-contract.md "Spread attributes") |
| `<svelte:element this={tag}>` (SP1) | yes; `bind:this`, `use:`, `{@attach}`; other `bind:` is `dynamic_element_bind` | `$n.dynamic_element(container, () => tag, render, $$kinds)`: recreated when the tag changes, nothing for a nullish tag |
| Static text for string-form values (`keySequence="Ctrl+K"`, icons, fonts, colors) | yes (spec `string_form`) | passed as a string |

## Lists: `{#each}` vs `<ListView>`

Two different things, chosen explicitly in the source (semantics: `runtime-contract.md`
"Virtualized lists"):

| | Status | Output |
| --- | --- | --- |
| `{#each items as item, i (key)}{:else}` | yes | `$n.each_block`: eager, one native subtree per item in the parent's slot; state per key |
| `import { ListView } from '@svelters/lists'` + `<ListView items key row bind:selected onactivate layout selection rowHeight>` with `{#snippet row(item, index)}` | yes | a component call; the build resolves the import to `@svelters/native-runtime/lists` (native `virtuallist`, same protocol as the JS pipeline): rows only for visible slots, row slots recycled, state per slot |
| `<virtuallist>` written directly | no | `unknown_element` (a backend extra kind, driven only by `ListView`) |
| Sort / filter / sections in the native model | no | sort or filter the array in JS (`$derived`); the keyed diff sends minimal splices |

Checked at compile time: `list_row_missing`, `list_row_params`, `list_children`,
`list_row_toplevel`, `list_bind_invalid`, `list_prop_invalid`, plus the spec's placement codes for
the `row` snippet's root elements in the row outlet (`box`).

Validation (Svelte codes and messages): `state_invalid_placement`, `props_invalid_placement`,
`props_id_invalid_placement`, `props_duplicate`, `bindable_invalid_location`,
`effect_invalid_placement`, `inspect_trace_invalid_placement`, `host_invalid_placement`,
`rune_invalid_arguments(_length)`, `rune_invalid_spread`, `constant_assignment` (const, import,
`$props.id()`, template declarations), `each_item_invalid_assignment`,
`snippet_parameter_assignment`, `state_field_duplicate`, `state_invalid_export`,
`derived_invalid_export`, `legacy_export_invalid`, `module_illegal_default_export`,
`bind_invalid_value`; plus the scope port's own (`store_invalid_subscription(_module)`,
`global_reference_invalid`, `props_invalid_pattern`, ...).
