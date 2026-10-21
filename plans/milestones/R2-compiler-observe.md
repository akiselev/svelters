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

New fixtures: `bind-observe` / `bind-observe-qt` (op logs + real host) and `bind-readonly-qt` (real host only; the
op-log hosts' Qt schema has no read-only props). `menus-qt` gained empty `ontoggled` listeners, so the scripted
`toggled` events stay deliverable on the log backend and in the Qt replay.

Op logs that changed: `bind-value`, `bind-value-qt`, `buffer-bind`, `menus-qt` and the compiler's `form-binding`
(`observe #h p on` instead of `listen`; the TestHost and the log backend cannot change a native value from an `event`
step, so the state no longer follows those steps there; the real toolkits do, `examples/counter/tests/integration.rs`).
The GTK and Qt op-log replays (`crates/svelters-gtk/tests/oplog_replay.rs`, `examples/counter-qt/tests/oplog_replay.rs`)
treat `observe`/`changed` lines as covered elsewhere and assert event delivery only for listened signals.

## Completion record

State: done on branch `r2-lane`, pushed as `ci/r2`; not merged. Base: master 9231ec3f, merged with master
26e97040 (`5c23c0f7`, only the `cargo-slot` sccache fix was new; no conflicts, nothing regenerated).

### Commits

| Commit | What |
|---|---|
| 3b75e585 | `PlatformSemantics` + `observability()`, `bind:` lowering (`bind_prop`/`bind_readonly`/`bind_once`), `\|nonreactive`, runtime helpers, observe typings |
| 9c62bd26 | diagnostics (§8.4), `--strict-native`, portability mode, `native_read_not_observable`, `RuntimeTarget`; golden tests |
| 004d03ea | `bind-observe` fixtures (GTK/Qt), integration tests, updated op logs and replays, docs, this file |
| c7e159c5, bb9aae09 | GTK read-only binding driven by a native selection; either end of the selected region accepted as the cursor position |
| 7bb8dfa4 | Qt read-only binding moved into `bind-readonly-qt` (real host only: the op-log Qt schema has no read-only props); `menus-qt` gets empty `ontoggled` listeners so its scripted `toggled` events stay deliverable |
| 76b11106 | WIP handoff note (removed again with this record) |
| 5c23c0f7 | merge master 26e97040 |
| e2f347fb | op-log replays: `observe` lines are executed against the real backend; event delivery asserted for every `event` line again (see below) |

### Gates (all on e2f347fb, run after the last code edit)

| Gate | Result |
|---|---|
| `tools/dev/headless tools/dev/cargo-slot cargo nextest run --workspace --no-fail-fast` | 2304 run, **2304 passed**, 7 skipped (684 s) |
| `cargo build -p svelters-compiler`, then `pnpm -r build` | rc 0, rc 0 |
| `pnpm test` (incl. test:meta) | rc 0: types 19/19, style 17/17, native-runtime 78/78, svelte-check 93 files 0 errors 0 warnings, canvas 3/3, lists 2/2, compiled harness 129 PASS 0 FAIL, test:meta pytest 140 passed 1 skipped, all `--check` generators clean |
| `python3 tools/docs/build_ir.py`, then `pnpm docs:test` | rc 0: 11 Python tests OK (1 skipped), 17/17 node tests |
| conformance `UPSTREAM=<stock svelte@5.57.2> node run.mjs --check` | **500 / 774** (rejected 168, runtime errors 27, mismatches 76, timeouts 0, skipped 3); baseline 500: +0 -0 |
| targeted (before the replay edit, on the merge) | `-p svelters-host -p svelters-gtk -p counter-qt -p counter` fixtures/replay/`bind_*`/counter-qt: 73/73 |

CI `ci/r2` @ e2f347fb, all five green: js 39545588259, linux-gtk 39545588279 (jobs gtk and gtk-4-14 both success),
linux-qt 39545588265, macos-appkit 39545588307, windows-winui 39545588276. (bb9aae09/c7e159c5 had linux-gtk red on
`svelters-host::fixtures`, fixed by 7bb8dfa4; run 39543031246.)

### Review of the op-log replay edits (backend crates' test files)

004d03ea changed `crates/svelters-gtk/tests/oplog_replay.rs` and `examples/counter-qt/tests/oplog_replay.rs` to
(a) treat `observe`/`changed` lines as no-ops and (b) assert event delivery only for (handle, signal) pairs JS
listened to. Checked against master: replaying master's code + logs and the branch's code + logs gives **identical
per-log delivered-event counts on all 23 pre-existing fixtures** (bind-this 4, bind-value 3, buffer-bind 1,
menus-qt 4, keyed-list 9, ...), and every `event` line in the two new logs targets a listened `clicked`. So (b) never
fired: it removed no coverage, but it would have hidden a future dropped listener. e2f347fb therefore **reverts (b)**
(every `event` line must be delivered again, as on master) and **strengthens (a)**: `observe #h p on|off` now calls the
backend's `observe` (GTK `Harness::observe`, Qt `BackendRef::observe`) and fails on an error, so the replays check that
each observation the compiler emits is accepted natively. `changed` stays a no-op (a scripted announcement into the
op-log host, not a backend call). Net: the replay files are stricter than on master; nothing that worked before is
weakened.

What *did* lose coverage is at the fixture level, by design of the lowering: in `bind-value`, `bind-value-qt`,
`buffer-bind`, `menus-qt` and `form-binding` the op-log hosts can no longer move native→state from an `event` step
(e.g. `menus-qt` state now reads `|false|true|false` instead of `|true|false|true`; `bind-value` no longer shows
`set #6 ... "Hello Zed"`). Native→state on real toolkits is covered by `examples/counter/tests/integration.rs`
(`bind_value_*`, `bind_observe_{gtk,qt}`); the op-log side comes back with R1's value-carrying `changed` steps.

### Lowering and diagnostics as built

See "Lowering table (as built)" and "Diagnostics (§8.4)" above; they match the code on e2f347fb. Deviations from §9.2:
`observability()` returns `Option` (None = unknown); `bind_prop` carries the declared signal plan as a runtime fallback
when `host.observe` answers false (Qt props without NOTIFY until R3, AppKit/WinUI until R4); a `partial` prop with a
declared plan keeps the signal plan (+ warning); `bind_readonly` reads at creation, once after the first layout pass
(`setTimeout 0`), then observes.

### `bind:` uses whose lowering changed (inventory from 004d03ea)

| Toolkit | moved to `bind_prop`/`bind_readonly` | kept on the signal plan |
|---|---|---|
| GTK | 21 | 6 (`<action bind:checked>`: Svelters.Action has no fact) |
| Qt | 26 | 2 (`group-box checked`, `time-edit time`: `partial`, warn) |
| AppKit, WinUI | none in the repo (fx1 cases moved, with fallback) | |

### What must change when R1 merges

1. `packages/native-runtime/src/observe.js`: take R1's file wholesale (this one is a marked stand-in). Check R1 exports
   `observeProp(h, prop, fn) -> unsubscribe` and `trackProp(h, prop)` with the same shapes; `bind-observe.js` imports only
   `observeProp` and reads `undefined` values live, so it works with value-carrying and proportional delivery.
2. `bind-observe.js` additionally holds one `host.observe(h, p, true)` itself and uses its `false` answer for the
   signal fallback. Keep it only if R1's host still refcounts per (h, prop); otherwise route the "can observe?" answer
   through R1's API.
3. The stand-in installs `globalThis.__svelters_prop_changed(h, prop)`; R1 delivers through `__svelters_changed`
   (batched). Make sure only R1's dispatcher remains and nothing else defines `__svelters_prop_changed` differently.
4. `fixtures/bind-observe(-qt)` drive native changes with `bind:this.setProp` + `changed` steps. Under R1's
   compare-with-last, the announcement of JS's own `setProp` reads back equal and may be dropped: switch these scripts
   (and restore native→state in `bind-value(-qt)`, `buffer-bind`, `menus-qt`, `form-binding`) to R1's value-carrying
   `changed` steps (§5.2 scripted mode), then regenerate the expected op logs.
5. Conflict spots: `src/index.js` (one export line), `examples/counter/tests/integration.rs`, `pnpm-lock.yaml`,
   the `test:meta` line in `package.json`, the two replay files.

### Open items

- R3/R4 change facts: after merging them rerun `gen_semantics.py`, then `gen_observe_outputs.py`, and `UPDATE=1` the
  `tests/r2` goldens (lowering changes are the intended effect); the runtime signal fallback can then go away per platform.
  R4 (`r4-appkit-winui`) was not in master during this work.
- `native_read_not_observable` ignores shadowing; under `--strict-native` a false positive becomes an error.
- The op-log Qt schema (`packages/testhost/src/schema.qt.json`) has no read-only props, so the Qt read-only binding is
  checked only on the real host (`bind-readonly-qt`).
- GTK `insert_text` does not move the cursor; the GTK read-only check uses `selectRegion(1, 2)`.
- The GTK/Qt counts above come from the 004d03ea inventory and were not recounted after the merge (master added no `bind:` uses).
