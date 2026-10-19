# AI1 handoff (stopped early: weekly quota)

Branch `worktree-agent-ae50fa4f78edc518f`. Design and built behaviour: `tools/metadata/gtk/README.md`, `docs/notes/ai1-inference.md`.

## Done: slice 1 (GTK), code complete, NOT fully gated

- `tools/metadata/gtk/probe.py` (moved from the prototype; deterministic: three runs gave identical output, also under load),
  `infer.py`, `metadata/rules/gtk.yaml`, `metadata/probe/gtk/4.22.4.json`, `metadata/inferred/gtk/*` (YAML, evidence, `_review.json`, `_parked.json`).
- `ingest.py`: layers ingested < inferred < manual, nested fields of a manual record replace inferred ones whole, `deny:`,
  parked facts left out, method `since` copied from the GIR (so tests skip below 4.14). Schema: `deny`, `delivery_only`, provenance origin `rule`.
- `gen_backend.py`: facts carry `origin`/`record_fp`/`strength`; inferred facts the generator cannot test go to `untestable` (parked, not an error);
  enum triggers; generic `bind:<prop>` echo test; typings mark unverified props `/** @unverified */`.
- `ledger.py`: strict for manual facts, parks failing inferred facts (`_parked.json`), regenerates and reruns what changed, nextest `TRY n` parsing,
  `check` fails when `metadata/probe/gtk/<ledger version>.json` is missing or for another version. `coverage.py`: facts-by-state table, `verified: delivery`.
- `pnpm test:meta` now also runs `infer.py --check`. Docs IR accepts origin `inferred`. New pytest: `tools/metadata/gtk/test_infer.py`, `tools/metadata/test_ledger.py`.
- Three draft-generator bugs fixed: enum samples are nicks (probe `jsonable` order), object-typed setters/getters and gunichar are not inferred,
  plus the bool-trigger string bug in the binding echo test and unstable getter snapshots (`stable` probe flag).

## Counts (GTK, concrete kinds + Widget/Editable/Scrollable/Orientable/Actionable via subjects)

Facts in the ledger: manual 489 (before and after, unchanged), inferred verified 937, parked 1 (`ApplicationWindow.get_id`, a stale unstable-getter snapshot; infer no longer emits it).
Inferred by section: properties 109 (net new; 329 inferred, the rest were already manual), methods 565 (was 61 manual), bindings 220 (was 16), signals 40,
slots 3. `verified: delivery` (synthetic emission): 29. Unreviewed, not written: 45 middle band, 90 low band (`_review.json`, by pattern; no agents spawned).

## Gates actually run

On this tree: `pytest tools/metadata` (55 pass), `infer.py --check`, `gen_bindings.py --check`, `ledger.py check`, `coverage.py --check`, `gen_semantics.py --check`,
and one full `ledger.py refresh --toolkit gtk` (all 1451 GTK generated tests: 0 failing). NOT run: `pnpm -r build`, workspace nextest, `pnpm test`, `pnpm docs:test`,
the GTK 4.14 CI job, Qt/other-crate builds. So not merge-ready until those pass.

## Next steps

1. `cargo build -p svelters-compiler --bin svelters-compile` via cargo-slot, `pnpm -r build`, workspace nextest under xvfb, `pnpm test`, `pnpm docs:test`; fix fallout
   (likely: docs IR/frontend counts, renderer bundle size: `bindings.generated.js` grew 11KB -> ~60KB, semantics specs).
2. Merge master (NT0 edits `gi.rs`/`convert.rs`/ABI; `metadata/gtk/*.json` regenerate conflicts: just rerun ingest/gen), push a `ci/**` branch and check the `gtk-4-14` job.
3. Slice 2 (Qt): port `docs/notes/scripts/annot/qt_prober.py` with isolated-mode crash handling, `metadata/inferred/qt`, merge in `gen_cpp.Model`, table-driven C++ tests, accept-by-pattern rules. Slice 3 (WinUI): not started.
4. Decide on the review bands (135 facts) and abstract-kind generic bindings (skipped to avoid table bloat).

## Traps

- A full refresh takes ~45 min on the loaded box (build + ~1450 test processes); do not edit tools/metadata or metadata/ while it runs (it regenerates).
- Agent shell guard refuses some heredocs/pipelines mentioning odd content; write scripts to files.
- Do not `pgrep -f` your own wait loop command line.
- `infer.py` reads merged `metadata/gtk/*.json` but only `origin: gir|runtime` records, so rerunning is idempotent; keep it that way.
- Parked entries are keyed by record fingerprint; delete `_parked.json` entries to force a retry.
