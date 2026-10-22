> **Superseded** by `plans/milestones/FU3-followups.md` (the completion record). Kept for history.

# FU3 handoff (follow-ups lane)

Branch `ci/fu3`, based on local master 8e92af95. **origin/master has moved since (E2, E3, FU2 merged) and has
NOT been merged into this branch yet.** The milestone record `plans/milestones/FU3-followups.md` is not
written yet; this note holds its material.

## Items

| # | Item | State | Commit |
|---|---|---|---|
| 1 | Qt wake coalescing | done | 6c482605 |
| 2 | Qt wrong-kind error names member + runtime class | done | 7b0474fb |
| 3 | Qt op-log replay produces `changed` natively | done | 7b0474fb |
| 4 | `native_read_not_observable` scope-aware | done | 81035503 |
| 5 | RT1 small items: `{await p}` diagnostic, `language-subset.md`, site `concepts/abi.md` | done | 7e03908f |
| 6 | NT0/NT1 GTK fact/invoker mismatches | done | a2d2159b |
| 7 | Stale observation baselines | done: native (a2e3c0e3, 0c62db18), docs (91f216a7), E3 store fields after the merge | see §7 |

### 1. Qt wake coalescing
`tools/metadata/qt/templates/backend.cpp.in`: `b_wake` does `if (g.queued.exchange(true)) return;`; the posted
lambda is `wake_checkpoint()` which clears `queued` before `run_checkpoint()`; the `schedule_wake` timer no
longer clears the ticket; `b_run` posts only if no checkpoint is pending. Test
`examples/counter-qt/tests/wake_coalesce.rs` (800 wakes from 8 threads -> 1 checkpoint; wake after -> 1; two
UI-thread wakes -> 1; a checkpoint that re-wakes twice -> 3). **Old code: fails** ("800 checkpoints, expected 1";
"two wakes from the UI thread: 2 checkpoints"). New: passes, `wake_priority` still passes. Docs:
`backend-abi.md` "Wake delivery", FU2 record §3 table.

### 2. Qt wrong-kind error
`fail_wrong_kind(op, what, id, e)` in the template (after `//@@TABLES@@`); `gen_cpp.py` emits it as the
fallback of `gen_set_prop`/`gen_get_prop`/`gen_call`/`gen_listen`. Message:
`set_prop: property push-button:flat does not belong to QLabel (kind label)`. Test
`examples/counter-qt/tests/wrong_kind.rs` (a `label` and a borrowed `QWidget` viewport; set/get/call/listen).
**Old generator: fails** on all 8 checks ("unknown property id for this kind").

### 3. Qt replay `changed`
New test hook `svl_qt_native_set` (template) / `counter_qt::hooks::native_set` (QObject::setProperty, bypasses
`set_prop`). `examples/counter-qt/tests/oplog_replay.rs`: host gets a `prop_changed` recorder; `changed` lines
are produced natively and must be announced exactly when observed (as GTK). `bind-observe-qt` now must replay
(2 native changes). Mutation check: with the recorder disabled the replay fails
"bind-observe-qt:35: changed #3 text "Kim": change not announced".

### 4. Shadowing
`crates/svelters-compiler/src/lower/native_reads.rs` takes the `ComponentScopeAnalysis`; wrappers are keyed by
the top-level `BindingId`, a receiver counts only if `arena.resolve(ident)` is that binding. Golden
`tests/r2/shadowing-gtk.{svelte,expected}` + test `shadowing_gtk`. **Old code: 9 diagnostics, 6 false
positives** (each item, each index, snippet param, `{@const}`, derived local, derived param); new: the 3 real
reads. All 176 compiler tests pass. CF1 conflict risk: `compile.rs` (call site), `native_reads.rs`.

### 5. RT1 items
`crates/svelters-compiler/src/lower/template_await.rs` (gated on `analysis.template.has_await`):
`unsupported_feature` for `await` in a template expression; async handlers and `{#await}` stay fine. Case
`tests/runes/template-await` (diagnostics @7:19 and @8:13). **Old code** emitted `await` inside
non-async `$.template_effect(() => ...)` (invalid JS). `docs/notes/native-compiler/language-subset.md`
(lang="ts" yes, .svelte.ts no, `{await}` row). `docs/site/src/content/docs/concepts/abi.md` rewritten to the
current header (v3 + 2.1–2.7; anchors `#schedule_wake`, `#destroy`, `#on_destroyed`, `#abi-v2-planned` kept).
CF1 conflict risk: `compile.rs`, `lower/mod.rs` (one `pub mod` line).

### 6. GTK facts vs invoker
Widened `types_members` from the N1 surface to every bound member: 77 disagreements on the old facts
(GType-less enums/flags 28, caller-allocated out values 6, by-value structs 3, interface/abstract kinds that
did not resolve 39, `GLib.Variant.parse_error_quark` 1) plus 32 `GLib.Variant` members hidden by the `since`
excuse. Fixes:
- `tools/metadata/gtk/types_facts.py`: block "enum without GType", "caller-allocated out value", "inout
  array", "struct by value" (out/inout cell, array element without C type) — exactly what gi.rs refuses.
  Diff of bound members before/after = exactly the 38 refused + 33 Variant members, nothing else.
- `metadata/overrides/gtk/types/gtk-types.yaml`: exclude `GLib.Variant` (fundamental, crosses as a JS value).
- `tools/metadata/gtk/gen_backend.py` `gen_types`: `ensure_registered` covers interfaces and abstract classes
  (real invoker fix: Gtk.Editable/Actionable/NativeDialog/ATContext/AccessibleText members now resolve).
- `gen_types_extra.py`: members check over every member; `since` excused only when the running library
  (glib/gtk/pango check) is older.
Facts regenerated with `--girdir` = /usr/share/gir-1.0 + cairo-1.0.gir from the Arch gobject-introspection
1.86.0-2 package (only `-runtime` is installed). Result: `types_members` + `types_verify` 92/92 pass,
`test_types.py` 13 passed 1 skipped, `gen_types.py --check` and `types_facts.py --check` current.

### 7. Stale baselines (partial)
Root cause: `Observed.last` is documented as "last value delivered to JS or read back after our own set", but
`op_observe` 0→1 filled it with a baseline read JS never saw. If JS's belief differs then (it wrote a value the
toolkit clamped/kept latent — the Qt QAction radio case of R2), a later change back to that value compared equal
and was dropped. Fix (WIP commit): no baseline read; `last: None` at 0→1 (`core/ops.rs`), so the first
announcement is always delivered; TestHost mirrors it (`last: undefined`). New test
`batch.rs::a_change_back_to_the_value_at_observe_time_is_delivered` — **old code: fails** (`left: []`);
new: passes. R1 tests updated (no baseline read in logs; first null announcement delivered; boxed test now checks
release of an unseen read-back). `cargo nextest -p svelters-host`: all pass except 4 `reload` tests that needed
`pnpm install` (done afterwards; not re-run).

Done since (second agent): `pnpm test` green after updating `packages/native-runtime/test/observe.test.mjs`
(first announcement after observe is delivered even when equal to the value written before) and regenerating
`metadata/semantics/types/gtk.json` (`gen_observe_outputs.py`; item 6's GLib.Variant exclusion, was stale);
svelters-host 66/66; svelters-gtk + counter-qt 1976/1976 (incl. op-log replay, observe, bind-observe);
docs: backend-abi.md Host contract, REACTIVITY.md §5.2 (+ "No observe-time baseline" paragraph), R1/R2 records
annotated. Merged origin/master (99053f38, no textual conflicts). E3 store fields had the same baseline read
(`core/reactive.rs` `app_observe`): removed too, for the same reason (a writer thread can change a field
between JS's read and the observe-time read); E3 test `update_marks_every_field_and_the_compare_drops_the_unchanged`
now expects one delivery of the 5 fields on the first touch and 0 on the second.

Originally left for 7: run `pnpm test` (testhost/native-runtime JS tests may assert the baseline:
`crates/svelters-host/tests/js/observe.js`, `packages/testhost` tests), fixture op-log parity
(`tools/oplog-compare.mjs`), GTK/Qt `observe`/`bind_observe_*` integration tests; update
`plans/architecture/backend-abi.md` "Host contract" refcount row ("At 0→1 ... read once as the baseline") and
`plans/research/REACTIVITY.md` (§5.2 + open item), R2 record open item.

## CI (ci/fu3)
- 7e03908f (items 1–5): all five green (js 39678454669, linux-gtk 39678454562, linux-qt 39678454863,
  macos-appkit 39678454815, windows-winui 39678454718).
- a2d2159b (+ item 6): linux-qt 39680200219, macos-appkit 39680200224, windows-winui 39680200300 success;
  js 39680200293 and linux-gtk 39680200271 were still running at handoff.
- WIP commit (item 7): not checked.

## Ledgers
Not refreshed. `python3 tools/metadata/ledger.py check` passed after items 1–6 (gtk 580 / qt 634 / appkit 205 /
winui 105 manual facts): the changes do not touch fingerprinted sources. Run `ledger.py refresh --toolkit qt`
(no cargo-slot wrapper) if a reviewer wants it anyway, then `check`.

## Local gates
Not run as a whole. Done per crate: counter-qt (all Qt tests except 5 needing `pnpm -r build` bundles),
svelters-compiler 176/176, svelters-gtk types_members/types_verify 92/92, svelters-host batch 15/15.

## Next steps
1. Finish item 7 (above), then merge origin/master (E2, E3, FU2 merged there; expect conflicts in
   `crates/svelters-host` and maybe `examples/exports`).
2. Gates: `cargo check --workspace --all-targets`; `flock /run/user/1000/svelters-nextest.lock setsid
   tools/dev/headless tools/dev/cargo-slot cargo nextest run --workspace --test-threads 4` with
   SVELTERS_SLOT_JOBS=4; `cargo build -p svelters-compiler`; `pnpm -r build`; `pnpm test`;
   `python3 tools/docs/build_ir.py`; `pnpm docs:test` (pnpm via corepack shim `/tmp/claude-1000/pnpmshim`).
3. Write `plans/milestones/FU3-followups.md` from this note; green CI on all five workflows.

## Known failures
- `svelters-host::reload` (4) fail without `pnpm install` in the worktree (precondition, not a regression).
- counter-qt `host_types`, `lists_e2e`, `lists_models` need `pnpm -r build` bundles.
