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
| `.svelte.ts`, `<script lang="ts">` | no | TypeScript parser not ported yet |
| Legacy mode (`export let`, `$:`, stores as state) | no | `legacy_export_invalid`; runes mode is forced |

Validation (Svelte codes and messages): `state_invalid_placement`, `props_invalid_placement`,
`props_id_invalid_placement`, `props_duplicate`, `bindable_invalid_location`,
`effect_invalid_placement`, `inspect_trace_invalid_placement`, `host_invalid_placement`,
`rune_invalid_arguments(_length)`, `rune_invalid_spread`, `constant_assignment` (const, import,
`$props.id()`, template declarations), `each_item_invalid_assignment`,
`snippet_parameter_assignment`, `state_field_duplicate`, `state_invalid_export`,
`derived_invalid_export`, `legacy_export_invalid`, `module_illegal_default_export`,
`bind_invalid_value`; plus the scope port's own (`store_invalid_subscription(_module)`,
`global_reference_invalid`, `props_invalid_pattern`, ...).
