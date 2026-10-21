# R2: compiler `bind:` lowering onto observation, `|nonreactive`, diagnostics (plans/research/REACTIVITY.md §8-§10, decisions §13)

Lane: `crates/svelters-compiler` (semantics, lowering, codegen, `svelters-compile`), `packages/native-runtime`
(`src/bind-observe.js`, the stand-in `src/observe.js`, typings), `tools/metadata/semantics/gen_observe_outputs.py`,
the R2 fixtures and tests. Branch `ci/r2`. Out of scope: host batching/compare/read-back and the real tracker (R1),
Qt observation beyond NOTIFY (R3), AppKit/WinUI `observe` (R4). The observe facts are read, never edited.

## What the compiler does now

`semantics::plan_bind(platform, element, name, nonreactive)` is the one policy (decision 13 #8: everything `exact`
moves onto observation at once). It asks the platform for the property, `observability()` (the R0 fact; no fact =
unknown) and the declared signal plan (`bindings` annotations), so a changed fact only changes the output
(`tests/r2_observe.rs` `a_changed_fact_changes_the_lowering` flips one through an `Overlay`).

### Lowering table (as built)

| Source | Fact on the platform | Emitted | Notes |
|---|---|---|---|
| `bind:p={x}`, `p` read/write | `exact` | `$n.bind_prop(node, "p", get, set[, signal, arg])` | the declared signal plan rides along as the runtime fallback, used only when `host.observe` answers false (Qt props without NOTIFY until R3, AppKit/WinUI until R4) |
| `bind:p={x}`, `p` read-only | `exact` | `$n.bind_readonly(node, "p", set)` | no `bindings` annotation needed (rule B1 extended to read-only props) |
| `bind:p={x}` | `partial` with a signal plan | `$n.bind(node, "p", signal, arg, get, set)` (unchanged) + `bind_partially_observed` | signal plans stay where the prop is not `exact` |
| `bind:p={x}` | `partial`, no signal plan | `$n.bind_prop` / `$n.bind_readonly` + `bind_partially_observed` | |
| `bind:p={x}` | no fact, signal plan | `$n.bind(...)` (unchanged), silent | a missing fact is unknown: never lowered onto observation (R0) |
| `bind:p={x}` | no fact, no plan | error `bind_no_plan` (unknown wording) | |
| `bind:p={x}` | `set_only`, writable | error `bind_set_only` (decision 13 #6) | |
| `bind:p={x}` | `none` / `constant` | error `bind_not_observable` | |
| `bind:p\|nonreactive={x}` | any readable | `$n.bind_once(node, "p", get, set, readonly)` | `nonreactive_redundant` on `set_only`/`constant` |
| `bind:p\|other`, `\|nonreactive` twice | | error `bind_modifier_invalid` | also any modifier on `bind:this`, component bindings, `<ListView>` bindings, and on the DOM conformance spec (no modifiers there) |
| `p={x}` on a prop with a binding | | `$n.set` effect + `$n.track` (unchanged) | |
| `{obj.p}` on a wrapper | | unchanged code; `native_read_not_observable` when `p` is `none`/`partial` | |

Runtime semantics are in `docs/notes/native-compiler/runtime-contract.md` ("bind_prop", "bind_readonly", "bind_once").

### Diagnostics (§8.4)

| Code | Severity (`--strict-native`) | When |
|---|---|---|
| `bind_not_observable` | error | `bind:` on a `none` or `constant` prop |
| `bind_set_only` | error | two-way `bind:` on a `set_only` prop |
| `bind_partially_observed` | warning (error) | `bind:` on a `partial` prop; names the missed causes and the platform |
| `native_read_not_observable` | warning (error) | best effort: a template expression (outside handlers/functions), `$derived`, `$derived.by`, `$effect` or `$effect.pre` body reads `obj.p` where `obj` is a top-level `const obj = new C(...)` with `C` from `svelters:<tk>/<ns>` and `p` is `none`/`partial`; `untrack(...)`, method calls and writes are not reads; unknown receivers or props are silent; shadowing, `bind:this` targets and callback parameters are not tracked |
| `bind_modifier_invalid` | error | unknown or repeated modifier, `\|nonreactive` where it has no meaning |
| `nonreactive_redundant` | warning (error) | `\|nonreactive` on a `set_only`/`constant` prop |
| `observability_differs` | warning (error) | portability mode `svelters-compile --backend gtk,qt,appkit,winui`: a `bind:` that is `exact` on one platform and not on another (platforms without the tag or prop are left out); output is for the first platform |
| `bind_no_plan` | error | kept for "no fact and no declared binding" |

### Traits (§9, decision 13 #11)

- `PlatformSemantics` (renamed from `BackendSemantics`; `pub use PlatformSemantics as BackendSemantics` keeps old
  code compiling) gains `id()`, `observability(el, prop) -> Option<Observability>`, `bind_modifiers()`,
  `native_type(module, export)`, `type_property(ty, getter)`. `SpecBackend` answers them from the spec and, for GTK
  and Qt, the generated `metadata/semantics/types/<tk>.json`.
- `Overlay<P: PlatformSemantics>` overrides observability per (tag, prop) in Rust and delegates the rest.
- `target::RuntimeTarget` (thin): `id`, `runtime_imports` (the `$` and `$n` modules), `specifier` (identity for both
  built-in targets), `dev` (HMR shape, source-map delivery). `QuickJsHost` (default) and `NodeVite`;
  `svelters-compile --target`. `CompileOptions` gained `target`, `strict_native` and `portability`.

### Runtime (`packages/native-runtime`)

- `src/bind-observe.js`: `bind_prop`, `bind_readonly`, `bind_once`, re-exported from `src/index.js` (the only edit there).
  A binding holds one host-side observation itself (the host refcounts per (h, prop)); that call's answer decides the
  signal fallback, and an `$effect.pre`-style teardown releases it before the natives are destroyed.
- `src/observe.js` is a **stand-in** for R1's file with exactly the API R2 codes against: `observeProp(h, prop, fn) ->
  unsubscribe` (`fn(value)` per delivered change, `undefined` for proportional props) and `trackProp(h, prop)`,
  implemented over R1a (`host.observe`, one `globalThis.__svelters_prop_changed(h, prop)` per notification, `getProp`
  read-back). R1's file replaces it at merge; `bind-observe.js` reads `undefined` values live, so it works with both.

### Typings

`tools/metadata/semantics/gen_observe_outputs.py` (`--check` in `pnpm test:meta`) writes
`packages/native-runtime/types/observe.d.ts`: per platform and tag, `bind:<p>` for every prop the compiler accepts,
with `@observable exact on gtk (notify)` / `@observable partial on appkit: misses user edits` remarks, and
`bind:<p>|nonreactive` for every readable prop with `@remarks Snapshot: not reactive`. `types/index.d.ts` merges the GTK
table into the intrinsic elements (as it does for the other GTK tables). svelte2tsx keeps the modifier in the attribute
name, so `bind:text|nonreactive` type-checks and `bind:text|bogus` does not (`check/App.svelte`). A Rust test checks
that the typings and `plan_bind` agree on every tag of the four platforms.

### Changed lowering of existing `bind:` uses

| Toolkit | moved onto observation (`bind_prop`) | kept on the signal plan |
|---|---|---|
| GTK | 21 (fixtures, galleries, `examples`, `packages/native-runtime/check`) | 6: `<action bind:checked>` (Svelters.Action has no fact) |
| Qt | 26 | 2: `group-box checked`, `time-edit time` (`partial`, with a warning) |
| AppKit, WinUI | none in the repo; `fx1_specs.rs` cases moved to `bind_prop` with their signal fallback | |

Op logs that changed: `bind-value`, `bind-value-qt`, `buffer-bind`, `menus-qt` and the compiler's `form-binding`
(`observe #h p on` instead of `listen`; the TestHost and the log backend cannot change a native value from an `event`
step, so the state no longer follows those steps there; the real toolkits do, `examples/counter/tests/integration.rs`).
The GTK and Qt op-log replays (`crates/svelters-gtk/tests/oplog_replay.rs`, `examples/counter-qt/tests/oplog_replay.rs`)
treat `observe`/`changed` lines as covered elsewhere and assert event delivery only for listened signals.
