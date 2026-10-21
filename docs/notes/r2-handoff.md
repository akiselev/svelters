# R2 handoff (compiler `bind:` lowering onto observation)

Branch `r2-lane` (local) in worktree `/home/dev/projects/svelters/.claude/worktrees/r2`, based on master 9231ec3f.
Pushed earlier as `ci/r2` at bb9aae09 (not at the later commits). Not merged. The original worktree
`agent-aaff62820d5b66d97` was removed by the environment before any commit; everything here was redone in `r2`.
Design, lowering table and diagnostics as built: `plans/milestones/R2-compiler-observe.md` (no completion record yet).

## Done (with paths)

- **Traits** (`crates/svelters-compiler/src/semantics/`): `mod.rs` renames `BackendSemantics` to `PlatformSemantics`
  (`pub use PlatformSemantics as BackendSemantics` kept) and adds `id`, `observability(el, prop) -> Option<Observability>`,
  `bind_modifiers`, `native_type`, `type_property`; R2 diagnostic codes plus `codes::STRICT_NATIVE`; `Unresolved::report_warning`.
  `observe.rs`: `Observability`, `BindLowering`, `BindPlan`, `plan_bind()` (the whole policy). `overlay.rs`: `Overlay<P>`.
  `types.rs`: `TypeTable` (non-GUI classes). `backend.rs`: `SpecBackend` embeds `metadata/semantics/types/{gtk,qt}.json`;
  no modifiers on the DOM conformance spec. `src/target.rs`: thin `RuntimeTarget` (`QuickJsHost` default, `NodeVite`),
  used for the `$`/`$n` imports in `codegen/component.rs` and `transform/mod.rs`.
- **Lowering** (`lower/mod.rs`, `lower/ir.rs`, `codegen/component.rs`): ops `BindProp` (`$n.bind_prop(node, p, get, set[, signal, arg])`,
  exact; the declared signal plan is a runtime fallback), `BindReadonly` (`$n.bind_readonly`), `BindOnce` (`$n.bind_once(..., readonly)`),
  `Bind` (signal plan kept for partial/unknown). Modifier checks: `native_bind_modifiers` / `no_bind_modifiers` (bind:this,
  component bindings, `lower/lists.rs` ListView).
- **Diagnostics**: `bind_not_observable`, `bind_set_only`, `bind_partially_observed`, `nonreactive_redundant`, `bind_modifier_invalid`,
  `bind_no_plan` (unknown fact, no plan) in `plan_bind`; `native_read_not_observable` in `lower/native_reads.rs` (best effort, scope
  described in its header); `observability_differs` in `lower/portability.rs`. `compile.rs`: `CompileOptions { target, strict_native,
  portability }`, `Diagnostics::promote` (`diag.rs`) for `--strict-native`. CLI `src/bin/svelters-compile.rs`: `--backend gtk,qt,...`
  (portability, output for the first), `--strict-native`, `--target`.
- **Runtime** (`packages/native-runtime`): `src/bind-observe.js` (`bind_prop`, `bind_readonly`, `bind_once`), re-exported by
  `src/index.js` (one export line, nothing else changed there). `src/observe.js` is a **STAND-IN** with R1's API
  (`observeProp(h, prop, fn) -> unsubscribe`, `trackProp(h, prop)`) over R1a (`host.observe` + `globalThis.__svelters_prop_changed`
  + `getProp`). At the R1 merge: take R1's `observe.js` wholesale; `bind-observe.js` only uses `observeProp`, reads `undefined`
  values live, and additionally holds one `host.observe(h, p, true)` itself (its answer `false` triggers the signal fallback; the
  host refcounts, so this coexists with R1's tracker). Check R1 does not also define `__svelters_prop_changed` differently.
  Unit tests: `test/bind-observe.test.mjs` (6).
- **Typings**: `tools/metadata/semantics/gen_observe_outputs.py` (in `pnpm test:meta` with `--check`) writes
  `packages/native-runtime/types/observe.d.ts` (per platform/tag `bind:p` with `@observable` remarks and `bind:p|nonreactive` with
  `@remarks Snapshot: not reactive`) and `metadata/semantics/types/{gtk,qt}.json`. `types/index.d.ts` merges the GTK table.
  `check/App.svelte` uses `bind:text|nonreactive` and `bind:cursor-position`; svelte-check: 0 errors (`bind:text|bogus` is rejected).
- **Tests**: `crates/svelters-compiler/tests/r2_observe.rs` + `tests/r2/*.svelte|.expected` (golden lowering GTK/Qt/AppKit/WinUI,
  diagnostics incl. strict, portability; Overlay fact flip; typings == plan_bind on every tag of 4 platforms). `fx1_specs.rs` and
  `tests/runes/stores/output.js` updated.
- **Fixtures**: new `fixtures/bind-observe`, `bind-observe-qt` (op logs; `changed` steps after a JS `setProp`), `bind-readonly-qt`
  (real host only). Real-host tests `bind_observe_{gtk,qt}` in `examples/counter/tests/integration.rs`. Updated op logs: `bind-value(-qt)`,
  `buffer-bind`, `menus-qt` (+ empty `ontoggled` listeners), `experiments/native-compiler/fixtures/form-binding`, diagnostics fixture.
  Replays `crates/svelters-gtk/tests/oplog_replay.rs` and `examples/counter-qt/tests/oplog_replay.rs`: `observe`/`changed` lines are
  no-ops, delivery asserted only for listened signals (**edit in a backend crate's test file; flag at merge**).
  `experiments/native-compiler/compiled/cases.mjs` gained `bind-observe`. Docs: `runtime-contract.md`, `semantic-spec.md`, `fixtures/README.md`.

## Changed lowering of existing `bind:` uses

GTK: 21 moved to `bind_prop`, 6 stay on the signal plan (`<action bind:checked>`, Svelters.Action has no fact). Qt: 26 moved, 2 stay
(`group-box checked`, `time-edit time`: partial, warn). AppKit/WinUI: no uses in the repo (fx1 test cases moved, with fallback).

## Gates (exact results)

| Gate | Result | Commit |
|---|---|---|
| `cargo test -p svelters-compiler --no-fail-fast` | all pass (after runes snapshot + fx1 updates) | 9c62bd26 |
| `pnpm -r build` | rc 0 | 004d03ea-era tree |
| `pnpm test` (incl. test:meta) | rc 0: native-runtime 78 + svelte-check 0 errors, types 19, style 17, canvas 3, lists 2, compiled harness all PASS, test:meta 140 passed 1 skipped | bb9aae09 (before the 7bb8dfa4 fixture split) |
| `pnpm docs:test` | 11 OK (1 skipped), 17 node pass | 004d03ea-era |
| conformance `UPSTREAM=<scratchpad>/up node run.mjs --check` | 500/774, baseline 500, +0 -0 | 004d03ea-era |
| `tools/dev/headless tools/dev/cargo-slot cargo nextest run --workspace --no-fail-fast` | 2304 run, 2303 passed, **1 failed: `svelters-host::fixtures p1_fixtures_match_expected_oplog`** (log backend: `bind-observe-qt: unknown Prop 'size'`, `menus-qt` dropped toggled events) | bb9aae09; fixture dists were rebuilt during the run. Log: `/tmp/claude-1000/-home-dev-projects-svelters/43ac01ab-2dd7-46f5-8f87-b6ead7af185e/scratchpad/log/nextest.log` |
| `nextest -p counter -E 'test(bind_)'` | 6/6 pass (bind_observe, bind_value, bind_this × gtk/qt) | bb9aae09 (bind_observe_qt then still used `bind:size` in bind-observe-qt) |
| CI `ci/r2` @ c7e159c5 | js, linux-qt, macos-appkit, windows-winui success; linux-gtk **failure** (same fixtures.rs failure; gtk-4-14 success) | run 39543031246 |

Not yet run on 7bb8dfa4 (which should fix the fixtures.rs failure): anything. Nothing is in flight.

## Left (ordered)

1. `pnpm -r build` (after `cargo build -p svelters-compiler`), then `nextest -p svelters-host -E 'test(fixtures)'`, the GTK replay
   (`-p svelters-gtk -E 'test(replay)'`), the Qt replay (`-p counter-qt`), `-p counter -E 'test(bind_)'` (bind_observe_qt now uses
   `run_dir("qt", "bind-readonly-qt", ...)`; check `run_dir` finds its dist).
2. Full gates on the final commit: workspace nextest, `pnpm -r build`, `pnpm test`, `docs:test` (after `tools/docs/build_ir.py`),
   conformance (`UPSTREAM` = a stock svelte@5.57.2 tree; the scratchpad copy at `.../scratchpad/up` may be gone: re-download the tag tarball).
3. Push as `ci/r2`, all five workflows green.
4. Completion record in `plans/milestones/R2-compiler-observe.md` (commits, counts, CI ids, open items); final report.

## Decisions, deviations, traps

- `observability()` returns `Option` (None = unknown), not a non-optional struct as in §9.2. Unknown + declared plan → signal plan
  silently; unknown + no plan → `bind_no_plan` error.
- `bind_prop` carries the declared signal plan as a **runtime fallback** when `host.observe` answers false (Qt props without NOTIFY,
  e.g. `text-edit plain-text`, until R3; AppKit/WinUI until R4). Decision 13 #8 is honoured at compile time; the fallback is a
  transition aid, documented in runtime-contract.md.
- Partial with a declared plan keeps the signal plan (+ warning); partial without one lowers onto observation (+ warning).
- `bind_readonly` reads at creation, once more after the first layout pass (`setTimeout 0`, deterministic in the op-log hosts), then observes.
- Op-log hosts cannot change a native value from an `event` step, so the changed op logs no longer show native→state; R1's
  value-carrying `changed` steps (§5.2 scripted mode) can restore it. `bind-observe` drives native changes with `bind:this.setProp` +
  `changed` steps; **under R1's compare-with-last (our own set_prop read back) those announcements may be dropped**: switch the script to
  R1's value-carrying `changed` steps at the R1 merge.
- GTK `insert_text` (the driver's typing) does not move the cursor; the GTK read-only check uses `selectRegion(1, 2)` (GTK 4.24 → pos 1).
- The op-log Qt schema (`packages/testhost/src/schema.qt.json`, from gen_cpp.py) has no read-only props, hence `bind-readonly-qt`.
- Generated files go stale when R3/R4 change facts: rerun `gen_semantics.py` then `gen_observe_outputs.py` after merging them, and
  `UPDATE=1` the `tests/r2` goldens if lowering changes (that is the intended effect).
- `native_read_not_observable` ignores shadowing; under `--strict-native` a false positive becomes an error.
- Conflict risks: `packages/native-runtime/src/observe.js` (R1 owns), `src/index.js` export line, `packages/testhost` untouched,
  the two replay test files, `examples/counter/tests/integration.rs`, `pnpm-lock.yaml` (new fixtures), `package.json` test:meta line.
- Never run destructive git here; the conformance run rewrites `REPORT.md`/`results.json` with path noise: restore them with
  `git show HEAD:<file> > <file>` only if they were clean before.
