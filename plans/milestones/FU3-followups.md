# FU3: follow-ups (Qt wake, Qt errors and replay, shadowing, RT1 items, GTK facts, stale baselines)

Seven small follow-ups from FU2, R2, RT1 and NT0/NT1, plus a flaky E3 test (item 8). Each has a test that fails on the old code (or, for docs
and generated facts, a check that was stale), and each is listed below with its evidence.

| # | Item | From | Commit(s) |
|---|---|---|---|
| 1 | Qt `wake` coalesces | FU2 open item | 6c482605 |
| 2 | Qt wrong-kind errors name the member and the runtime class | FU1 survey | 7b0474fb |
| 3 | Qt op-log replay produces `changed` natively | R2 open item | 7b0474fb |
| 4 | `native_read_not_observable` is scope-aware | R2 open item | 81035503 |
| 5 | RT1 small items: `{await p}` diagnostic, `language-subset.md`, site `concepts/abi.md` | RT1 | 7e03908f |
| 6 | GTK type facts agree with the invoker for every bound member | NT0/NT1 | a2d2159b, 0c62db18, 9f01e40f |
| 7 | No observe-time baseline (stale observation baselines) | R2 open item | a2e3c0e3, 0c62db18, 91f216a7, 7a21ba45 |
| 8 | `an_event_flood_warns_once` is deterministic | E3 (flaky on master) | 17749493 |

## 1. Qt wake coalescing

`tools/metadata/qt/templates/backend.cpp.in`: `b_wake` does `if (g.queued.exchange(true)) return;`. The posted
lambda is `wake_checkpoint()`, which clears `queued` before `run_checkpoint()`, so a wake requested during the
checkpoint gets a checkpoint of its own. The `schedule_wake` timer no longer clears the ticket, and `b_run`
posts only if no checkpoint is pending.

Test `examples/counter-qt/tests/wake_coalesce.rs`: 800 wakes from 8 threads give 1 checkpoint, and one wake after
that gives 1 more. Two wakes from the UI thread give 1. A checkpoint that re-wakes twice gives 3 in all. **Old
code fails** ("800 checkpoints, expected 1"; "two wakes from the UI thread: 2 checkpoints"). `wake_priority`
still passes. Docs: `backend-abi.md` "Wake delivery" and the FU2 record §3 table (Qt row: coalesced).

## 2. Qt wrong-kind errors

The template now has `fail_wrong_kind(op, what, id, e)` (after `//@@TABLES@@`), and `gen_cpp.py` emits it as the
fallback of `gen_set_prop`, `gen_get_prop`, `gen_call` and `gen_listen`. Example message:
`set_prop: property push-button:flat does not belong to QLabel (kind label)`.

Test `examples/counter-qt/tests/wrong_kind.rs` covers set, get, call and listen on a `label` and on a borrowed
`QWidget` viewport. **The old generator fails** all 8 checks ("unknown property id for this kind").

## 3. Qt op-log replay of `changed`

A new test hook, `svl_qt_native_set` (template) and `counter_qt::hooks::native_set`, calls `QObject::setProperty`,
bypassing `set_prop`. In `examples/counter-qt/tests/oplog_replay.rs` the host gets a `prop_changed` recorder.
`changed` lines are now produced natively and must be announced exactly when observed, as on GTK. `bind-observe-qt`
now must replay its 2 native changes. Mutation check: with the recorder disabled, the replay fails with
`bind-observe-qt:35: changed #3 text "Kim": change not announced`.

## 4. `native_read_not_observable` shadowing

`crates/svelters-compiler/src/lower/native_reads.rs` now takes the `ComponentScopeAnalysis`. Wrappers are keyed by
the top-level `BindingId`, and a receiver counts only if `arena.resolve(ident)` is that binding. The golden is
`tests/r2/shadowing-gtk.{svelte,expected}`, with test `shadowing_gtk`. **The old code gave 9 diagnostics, 6 of
them false positives**: an `{#each}` item, an index, a snippet parameter, a `{@const}`, a derived local and a
derived parameter. The new code gives the 3 real reads.

## 5. RT1 items

- `crates/svelters-compiler/src/lower/template_await.rs` (gated on `analysis.template.has_await`) reports
  `unsupported_feature` for `await` in a template expression. Async handlers and `{#await}` are still fine.
  Case `tests/runes/template-await` (diagnostics @7:19 and @8:13). **The old code emitted `await` inside a
  non-async `$.template_effect(() => ...)`**, which is invalid JS.
- `docs/notes/native-compiler/language-subset.md`: `lang="ts"` yes, `.svelte.ts` no, plus a `{await}` row.
- `docs/site/src/content/docs/concepts/abi.md` is rewritten to the current header (v3 + 2.1–2.7). The anchors
  `#schedule_wake`, `#destroy`, `#on_destroyed` and `#abi-v2-planned` are kept.

## 6. GTK facts vs the invoker

`types_members` was widened from the N1 surface to every bound member. On the old facts, 77 members disagreed:

| Cause | Members |
|---|---|
| enums/flags with no GType | 28 |
| caller-allocated out values | 6 |
| structs by value | 3 |
| interface or abstract kinds that did not resolve | 39 |
| `GLib.Variant.parse_error_quark` | 1 |

On top of those, the `since` excuse hid 32 `GLib.Variant` members. The fixes:

- `tools/metadata/gtk/types_facts.py` blocks exactly what `gi.rs` refuses: "enum without GType",
  "caller-allocated out value", "inout array" and "struct by value" (an out/inout cell, or an array element with
  no C type). The bound members before and after differ by exactly the 38 refused members and 33 Variant
  members, nothing else.
- `metadata/overrides/gtk/types/gtk-types.yaml` excludes `GLib.Variant`. It is fundamental and crosses the ABI as
  a JS value. The observe outputs (`metadata/semantics/types/gtk.json`) were regenerated to match (0c62db18).
- `tools/metadata/gtk/gen_backend.py` `gen_types`: `ensure_registered` covers interfaces and abstract classes.
  This is a real invoker fix: Gtk.Editable, Actionable, NativeDialog, ATContext and AccessibleText members now
  resolve.
- `gen_types_extra.py` checks every member. `since` excuses a member only when the running library (the
  glib/gtk/pango check) is older than it.
- Older libraries (9f01e40f; CI's GTK 4.14 job with GLib 2.80 failed 3 members of the widened check):
  - a row's `since` is the later of the member's and its class's (`Gtk.EnumListItem`, class since 4.24, has
    untagged members);
  - a member newer than the running library is skipped before it is resolved (`Gtk.AlternativeTrigger.newv`,
    since 4.24, resolved through the tables with no typelib entry);
  - `SHAPE_SINCE` lists members whose shape changed with no `since`. `GLib.Bytes.new_from_bytes` is an instance
    method in GLib 2.80 and a constructor in the GLib 2.90.1 the facts come from. On older libraries it must
    still resolve; its flags and arguments are checked from 2.90 on. The release where it changed is not pinned.
  - Locally (GLib 2.90.1, GTK 4.24.1) every row is still checked.

The facts were regenerated with `--girdir` set to /usr/share/gir-1.0 plus `cairo-1.0.gir`. Only
`gobject-introspection-runtime` is installed, so that file was extracted from the Arch `gobject-introspection`
1.86.0-2 package into a scratch directory outside the repo. Results:

- `types_members` and `types_verify`: 92/92 pass;
- `test_types.py`: 13 passed, 1 skipped;
- `gen_types.py --check` and `types_facts.py --check`: current.

## 7. No observe-time baseline

**Root cause.** `Observed.last` is documented as "the last value delivered to JS, or read back after our own
set". But `op_observe` 0→1 filled it with a baseline read that JS never saw. That read can differ from what JS
believes. In the Qt QAction radio case found while merging R1 (R2 record), JS wrote a value that the toolkit
clamped, kept latent or ignored until an insert. A later change back to the baseline value then compared equal
and was dropped, and JS stayed stale.

**What changed (the semantics):**

- **Native props** (`crates/svelters-host/src/core/ops.rs`): at 0→1, `host.observe` reads nothing. `last` starts
  unknown (`None`), and nothing compares equal to it. So **the first announcement after an observation starts
  is always delivered**, even when it changes nothing (a GTK re-notify of an identical set). From then on,
  `last` is only ever a value JS was told: either delivered, or read back after our own `set_prop` (and
  delivered if it was normalized).
- **TestHost** (`packages/testhost/src/index.js`): the same rule, with `last: undefined`, since values are never
  `undefined`.
- **E3 store fields** (`crates/svelters-host/src/core/reactive.rs` `app_observe`, after merging master): the
  same rule. The same staleness exists there: a writer thread can change a field between JS's read and the
  observe-time read, and a later write back to the read value was dropped.
- **Unchanged:**
  - one `__svelters_changed` batch per checkpoint, before the queued events, with no drain in between;
  - the echo of our own write is dropped (the read-back after `set_prop` still becomes `last`);
  - a normalized value is delivered once;
  - proportional props are never read;
  - `BOXED` values always count as changed;
  - `bind:` lowering (compiler) and OV1's delivery verification. The ledger rows assert backend announcements
    plus a changed read, below the host compare.
- **Cost:** at most one extra delivery per observation. Svelte drops it for an equal primitive; an object value
  re-runs its readers once. A toolkit that changes a value with no notification at all still leaves JS stale
  until the next announcement. That is a `missed` fact, not a compare problem.

**Evidence:**

- `crates/svelters-host/tests/batch.rs::a_change_back_to_the_value_at_observe_time_is_delivered`. JS writes
  150, the toolkit keeps 100, observe starts, then the toolkit changes the value silently and back to 100,
  announcing only the second change. **Old code fails** (`left: []`, the change is dropped); the new code delivers
  `changed 1,"value",100`, then drops an identical one.
- Updated R1 tests (`batch.rs`):
  - no `read` in the observe log;
  - the first `null` announcement is delivered, the second is dropped;
  - the boxed test now checks the release of an unseen read-back after `setProp`.
- `packages/native-runtime/test/observe.test.mjs`: the first announcement equal to a value written before
  observing is delivered, and an identical one after it is dropped.
- `crates/svelters/tests/reactive.rs::update_marks_every_field_and_the_compare_drops_the_unchanged`: the first
  `touchAll` after observe delivers the 5 fields once (one batch), and the second is dropped by the compare
  (`store_unchanged` +5). The other 17 reactive tests pass unchanged:
  - `ten_thousand_worker_writes_are_one_batch`;
  - `writable_field_clamps_once_and_drops_the_echo`;
  - `accepted_writes_echo_nothing_and_rejected_writes_throw`.
- Before the first merge: svelters-host 66/66, and svelters-gtk + counter-qt 1976/1976, including the GTK and Qt
  op-log replays (`oplog_replay`), `observe` and `bind-observe*`. On the merged tree, the full workspace nextest
  passes (see the completion record), as do `pnpm test`'s op-log parity tests
  (`packages/native-runtime/test/oplog-compare.test.mjs` and the fixture replays).

**Docs:**

- `plans/architecture/backend-abi.md` "Host contract": the refcount, compare and op-log/TestHost rows.
- `plans/research/REACTIVITY.md` §5.2: the refcount and compare rows, plus a "No observe-time baseline"
  paragraph.
- The R1, R2 and E3 records are annotated where they describe the baseline. The R2 open item is marked resolved.

## 8. Deterministic event-flood test

`crates/svelters/tests/reactive.rs::an_event_flood_warns_once` failed 2 of 7 runs on master under load (~50),
with `left: 0 right: 1` warnings. The flood was two worker `burst(1500)` calls, and the test's `until` loop
checkpoints every millisecond. Under load, a worker's emits were spread over several checkpoints, so none
collected more than `EVENT_FLOOD` (1000) listened events for the object, and the warning never fired.

The fix makes the test deterministic instead of loosening the assertion. A new test export,
`Sensor::burst_now(n)`, emits `n` readings synchronously on the JS thread, all in one call, so they all reach the
hub before the next checkpoint. That checkpoint collects 1500 listened events and warns. The second flood is
emitted from the reading handler while the first is being delivered, so a later checkpoint collects it. The test
now also asserts the event count (3000) and at least 2 collecting checkpoints, then exactly 1 warning.

Evidence: 20/20 passes in a loop under the nextest flock, with 48 busy-loop processes as load (load average
~54). The pre-fix flake rate (2 of 7 at load ~50) is the coordinator's observation on master and was not re-run
here.

## Completion record

Branch `ci/fu3`. Merged `origin/master` twice:

- 99053f38 (E2, E3, FU2): no textual conflicts. The only semantic overlap was E3's store baseline (§7).
- a1a13f0b (WP-A, ST2, iOS simulator): no textual conflicts, no overlap.

The code head is **a1a13f0b**; this record is a docs-only commit on top. CF1 and WP-W were not on master yet.

CI on a1a13f0b, all success:

| Workflow | Run |
| --- | --- |
| js | 39729128808 |
| linux-gtk | 39729128849 (GTK 4.14 and Arch; `types_members` passes on both) |
| linux-qt | 39729128767 (the first attempt failed in apt setup, a 403 from packages.microsoft.com, before any build; it passed on rerun) |
| macos-appkit | 39729128669 |
| windows-winui | 39729128711 |

Before the second merge, all five were also green on 9f01e40f (the item-6 fix for older libraries). The linux-gtk
run on 7a21ba45 had failed `types_members` on GTK 4.14, which is what 9f01e40f fixes.

Local gates on a1a13f0b, all rc 0, with `SVELTERS_SLOT_JOBS=4`:

| Gate | Result |
| --- | --- |
| `tools/dev/cargo-slot cargo check --workspace --all-targets` | rc 0 |
| `flock ... tools/dev/headless tools/dev/cargo-slot cargo nextest run --workspace --test-threads 4` | 2471/2471 passed, 10 skipped |
| `pnpm -r build` (after `pnpm install`, `cargo build -p svelters-compiler`, `python3 tools/docs/build_ir.py`) | rc 0 |
| `pnpm test` | rc 0, including `tools/metadata` pytest (155 passed, 1 skipped) and `ledger.py check` (gtk 580, qt 634, appkit 398, winui 105 manual facts) |
| `pnpm docs:test` | rc 0 (11 tests, 1 skipped) |

Ledgers were not refreshed: `ledger.py check` passes, and FU3 touches no fingerprinted source.

## Open items

- `GLib.Bytes.new_from_bytes`: the GLib release where it became a constructor is not pinned (2.80 has the
  method, 2.90.1 the constructor). `SHAPE_SINCE` uses the verified 2.90.
- A toolkit that changes a value with no notification (silent reveals such as QAction `setCheckable`) still
  leaves JS stale until the next announcement. The facts should list such causes as `missed`.
- The Qt `wake` coalescing is CI-verified on linux-qt only; AppKit and WinUI are unchanged (they already
  coalesce, FU2 §3).
- `item 6` facts depend on `cairo-1.0.gir`, which the dev machine lacks system-wide. A facts regeneration needs
  that GIR (from the `gobject-introspection` package) in `--girdir`.

## Conflict risks

- CF1 (compiler): `crates/svelters-compiler/src/compile.rs` (the `native_reads` call site and the
  `template_await` pass), `lower/native_reads.rs`, and `lower/mod.rs` (one `pub mod` line).
- WP-A / WP-W (metadata/common): none in common. `tools/metadata/gtk/*`, `metadata/overrides/gtk/types/` and the
  regenerated `packages/types/gtk/*` and `metadata/gtk/types/*` conflict only with GTK type work.
- svelters-host: `core/ops.rs` (`op_observe`), `core/reactive.rs` (`app_observe`), and `tests/batch.rs`.
- Qt: `tools/metadata/qt/templates/backend.cpp.in` (`b_wake`, `fail_wrong_kind`, the `svl_qt_native_set` hook)
  and `gen_cpp.py`.
