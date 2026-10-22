# R2: compiler `bind:` lowering onto observation, `|nonreactive`, diagnostics (plans/research/REACTIVITY.md §8-§10, decisions §13)

Lane: `crates/svelters-compiler` (semantics, lowering, codegen, `svelters-compile`), `packages/native-runtime`
(`src/bind-observe.js`, typings; until the R1 merge a stand-in `src/observe.js`), `tools/metadata/semantics/gen_observe_outputs.py`,
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
| `bind:p={x}`, `p` read/write | `exact` | `$n.bind_prop(node, "p", get, set[, signal, arg])` | the declared signal plan rides along as the runtime fallback, used only when the host refuses the observation (`observeProp(...).observed === false`) |
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

- `src/bind-observe.js`: `bind_prop`, `bind_readonly`, `bind_once`, re-exported from `src/index.js` next to R1's
  tracker exports. Each binding registers one `observeProp(h, prop, fn)` (R1's `src/observe.js`); its
  `unsubscribe.observed` decides the signal fallback, and an `$effect.pre` teardown unsubscribes before the natives
  are destroyed. `fn(undefined)` (proportional props) is read live.
- Until R1 merged, `src/observe.js` was a stand-in with the same API over R1a; it was replaced by R1's file wholesale
  in f10c1127.

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
| GTK | 23: 21 in fixtures and galleries, `packages/native-runtime/check`, the compiler's `tests/runes/stores` input | 6: `<action bind:checked>` (Svelters.Action has no fact) |
| Qt | 26 (unchanged by R3: these props were already `exact`) | 2: `group-box checked`, `time-edit time` (`partial`, with a warning) |
| AppKit, WinUI | none in the repo; `fx1_specs.rs` cases moved to `bind_prop` with their signal fallback | |

New fixtures: `bind-observe` / `bind-observe-qt` (op logs + real host) and `bind-readonly-qt` (real host only; the
op-log hosts' Qt schema has no read-only props). `menus-qt` gained empty `ontoggled` listeners, so the scripted
`toggled` events stay deliverable on the log backend and in the Qt replay.

Op logs that changed: `bind-value`, `bind-value-qt`, `buffer-bind`, `menus-qt` and the compiler's `form-binding`
(`observe #h p on` instead of `listen`; the TestHost and the log backend cannot change a native value from an `event`
step, so the state no longer follows those steps there; the real toolkits do, `examples/counter/tests/integration.rs`).
The GTK and Qt op-log replays execute `observe` lines against the real backend; the GTK replay (R1's arm) also
produces each `changed` step natively and checks it is announced exactly when observed. See the completion record.

## Completion record

State: done on branch `r2-lane`, pushed as `ci/r2`; not merged. Head **c5adc09f**. Base master 9231ec3f; merged
with master three times: 26e97040 (`5c23c0f7`, cargo-slot fix), 6c99a63a (R1 + R4, `f10c1127`; conflicts only in
`packages/native-runtime/src/{index,observe}.js`) and b700d71f (R3, `6c9d3e10`, no conflicts).

### Commits

| Commit | What |
|---|---|
| 3b75e585 | `PlatformSemantics` + `observability()`, `bind:` lowering (`bind_prop`/`bind_readonly`/`bind_once`), `\|nonreactive`, runtime helpers, observe typings |
| 9c62bd26 | diagnostics (§8.4), `--strict-native`, portability mode, `native_read_not_observable`, `RuntimeTarget`; golden tests |
| 004d03ea | `bind-observe` fixtures (GTK/Qt), integration tests, updated op logs and replays, docs, this file |
| c7e159c5, bb9aae09 | GTK read-only binding driven by a native selection |
| 7bb8dfa4 | Qt read-only binding in `bind-readonly-qt` (real host only); `menus-qt` keeps `toggled` deliverable |
| e2f347fb | op-log replays execute `observe` lines natively; event delivery asserted for every `event` line again |
| f10c1127 | merge R1 + R4: R1's `observe.js` wholesale (stand-in and `__svelters_prop_changed` gone); `bind-observe.js` on `observeProp(...).observed`; `bind-observe(-qt)` scripts use value-carrying `changed` steps; R1's GTK replay arms kept; `gen_observe_outputs.py` reads `observed()`/`observedLocal()` getters |
| 8d804ee4 | GTK replay: announcement asserted only for observed pairs (and none otherwise); Qt: a named radio action is checkable as soon as it has its `group` (fix below) |
| 6c9d3e10, d89f1b5e | merge R3; observe typings regenerated (newly exact Qt props gain `bind:`) |
| c5adc09f | `packages/types/qt/gui/QTextDocument.js` + its docs page regenerated (stale on master b700d71f; same content as the coordinator's uncommitted fix) |

### Gates (all on c5adc09f, run after the last edit)

| Gate | Result |
|---|---|
| `tools/dev/headless tools/dev/cargo-slot cargo nextest run --workspace --no-fail-fast` | 2320 run, **2320 passed**, 7 skipped, no retries (686 s) |
| `cargo build -p svelters-compiler`, then `pnpm -r build` | rc 0, rc 0 |
| `pnpm test` (incl. test:meta) | rc 0: types 19/19, style 17/17, native-runtime 85/85, svelte-check 93 files 0 errors 0 warnings, canvas 3/3, lists 2/2, compiled harness 129 PASS 0 FAIL, test:meta pytest 140 passed 1 skipped, all `--check` generators clean |
| `python3 tools/docs/build_ir.py`, then `pnpm docs:test` | rc 0: 11 Python tests OK (1 skipped), 17/17 node |
| conformance `UPSTREAM=<stock svelte@5.57.2> node run.mjs --check` | **500 / 774** (rejected 168, runtime errors 27, mismatches 76, timeouts 0, skipped 3); baseline 500: +0 -0 |
| `cargo nextest run -p svelters-compiler` (after the R3 merge) | 169/169; the `tests/r2` goldens needed no update after R1, R3 or R4 |

CI `ci/r2` @ c5adc09f, all five green: js 39556105484, linux-gtk 39556105439, linux-qt 39556105514, macos-appkit
39556105526, windows-winui 39556105507. Earlier: e2f347fb all green (39545588259/-279/-265/-307/-276); f10c1127
linux-gtk 39550929900 and linux-qt 39550929898 red (the two R1 interactions fixed in 8d804ee4); 8d804ee4 all green
(39553674732/-678/-673/-869/-737); d89f1b5e js 39555420292 red (stale `QTextDocument.js` inherited from master).

### Review of the op-log replay edits (backend crates' test files)

004d03ea changed `crates/svelters-gtk/tests/oplog_replay.rs` and `examples/counter-qt/tests/oplog_replay.rs` to
(a) treat `observe`/`changed` lines as no-ops and (b) assert event delivery only for listened (handle, signal) pairs.
Master's code + logs and the branch's code + logs give **identical per-log delivered-event counts on all 23
pre-existing fixtures**, so (b) never fired and removed no coverage, but it would have hidden a dropped listener.
e2f347fb reverted (b) (every `event` line must be delivered, as on master) and made `observe` lines call the
backend's `observe`. After R1: the GTK file is R1's version (its `observe` arm calls the backend, its `changed` arm
produces the change natively and checks the announcement) plus one R2 change: the announcement is required only when
the pair is observed and forbidden otherwise (`bind-observe` changes the unobserved `|nonreactive` entry too). The Qt
file keeps R2's `observe` arm (calls `BackendRef::observe`); `changed` is a no-op there. **Net: both replays are
stricter than on master; nothing that worked before is weakened.**

Fixture-level coverage moved by design: in `bind-value(-qt)`, `buffer-bind`, `menus-qt` and `form-binding` the op-log
hosts no longer move native→state from an `event` step (the binding observes instead of listening). Native→state on
the real toolkits is covered by `examples/counter/tests/integration.rs` (`bind_value_*`, `bind_observe_*`) and
`examples/counter-qt/tests/host_menus.rs`; `bind-observe(-qt)` shows it on the op-log hosts through value-carrying
`changed` steps.

### Found while merging R1: a stale observation baseline on Qt radio actions

`host_menus` (Qt, real host) failed after R1: Large toggled, Small stayed `true` in state. Qt keeps `setChecked(true)`
on a non-checkable QAction but reports `isChecked()` false; a radio action (`group=`) became checkable only at
insert (`menu_apply_group`), and `setCheckable` then revealed the checked state without emitting `toggled`. R1's
host read the baseline `last=false` at `observe`, so the group's later uncheck (`false`) compared equal and was
dropped. Fix (8d804ee4, `tools/metadata/qt/templates/menus.hpp.in`): a named action becomes checkable when the
name is written, so `checked` reads back as written. The same class of bug can hit any prop whose value a toolkit
reveals without its NOTIFY (see open items).

### Lowering and diagnostics as built

See "Lowering table (as built)" and "Diagnostics (§8.4)" above; they match the code on c5adc09f. Deviations from
§9.2: `observability()` returns `Option` (None = unknown); `bind_prop` carries the declared signal plan as a runtime
fallback when the host refuses the observation; a `partial` prop with a declared plan keeps the signal plan
(+ warning); `bind_readonly` reads at creation, once after the first layout pass (`setTimeout 0`), then observes.
After R4 the portability goldens are unchanged (no `tests/r2` case changes class on AppKit/WinUI).

### `bind:` uses whose lowering changed (recounted on c5adc09f with `svelters-compile`, new R2 fixtures excluded)

| Toolkit | moved to `bind_prop` | kept on the signal plan |
|---|---|---|
| GTK | 23 (21 fixtures/galleries + `native-runtime/check` + compiler `tests/runes/stores`) | 6 `<action bind:checked>` (`menus`, `gallery-gtk` 10-menus) |
| Qt | 26 (incl. `form-binding/qt`) | 2: `gallery-qt` 05-containers `group-box checked`, 10-display `time-edit time` (`partial`) |
| AppKit, WinUI | none in the repo | |

R3 did not change these (the 26 Qt uses were already on `exact` NOTIFY props); it widened what *can* be bound
(`observe.d.ts` now offers `bind:` on the newly exact Qt props, e.g. `int-value`, `double-value`, `text-value`).
The new fixtures add GTK 1 `bind_prop` + 1 `bind_readonly` + 2 `bind_once`, Qt 1 + 1 `bind_once`, and Qt 1
`bind_readonly`.

### R1 coordination (done at the merge)

The handoff's list was applied in f10c1127: R1's `observe.js` taken wholesale; the stand-in's
`__svelters_prop_changed` is gone (only R1's `__svelters_changed` remains); `bind-observe.js` no longer takes its
own `host.observe` reference (R1's `observeProp` holds it and reports `.observed`); the `bind-observe(-qt)` scripts
use value-carrying `changed` steps with a value ("Kim") different from JS's last write ("Zed"), because R1 drops the
echo of our own `setProp`; the unit tests drive changes through `TestHost.nativeChange` + `__svelters_changed`.

### Open items

- **Resolved in FU3** (no observe-time baseline; `last` starts unknown, REACTIVITY.md §5.2): silent reveals on
  other props (like the QAction `checkable`/`checked` case): a baseline read while a value is
  latent gives a stale `last`. R3/R1 owners: consider re-reading `last` after structural ops (insert) or listing
  such causes as `missed` in the facts.
- The Qt op-log replay still treats `changed` as a no-op (the GTK one produces it natively).
- The op-log Qt schema (`packages/testhost/src/schema.qt.json`) has no read-only props, so the Qt read-only
  binding is checked only on the real host (`bind-readonly-qt`).
- `native_read_not_observable` ignores shadowing; under `--strict-native` a false positive becomes an error.
- GTK `insert_text` does not move the cursor; the GTK read-only check uses `selectRegion(1, 2)`.
- After future fact changes: rerun `gen_semantics.py`, `gen_observe_outputs.py`, and `UPDATE=1` the `tests/r2`
  goldens if lowering changes (the intended effect).
