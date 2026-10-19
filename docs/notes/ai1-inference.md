# AI1: annotation-inference pipeline (lane notes)

Plan: `plans/research/ANNOTATION-AUTOMATION.md` (section 11 has the decisions). Branch
`worktree-agent-ae50fa4f78edc518f`. Slices: 1 GTK, 2 Qt, 3 WinUI (only if the windows-winui CI can verify it).

## Slice 1 (GTK): design as built

- `tools/metadata/gtk/probe.py` (moved from `docs/notes/scripts/annot/gtk_prober.py`): maintainer-run, deterministic
  (checked by running it twice: identical output). Output `metadata/probe/gtk/<version>.json`. Machine paths and
  addresses are scrubbed. New against the prototype: enum samples are nicks (the first bug of the draft generator was
  `jsonable` testing `int` before the enum case), synthetic emission evidence for signals with fillable parameters,
  abstract kinds and interfaces probed through a concrete subject (`subjects:` in `metadata/rules/gtk.yaml`).
- `tools/metadata/gtk/infer.py`: pure function of ingested records (only `origin: gir|runtime`), the probe and
  `metadata/rules/gtk.yaml`. Writes `metadata/inferred/gtk/<Kind>.yaml` (override schema), `<Kind>.evidence.json` and
  `_review.json` (everything below `promote_at` = 0.9, by pattern, plus skipped counts).
- Merge precedence in `ingest.py`: ingested < inferred < manual. `annotated.origin` says which layer wrote a fact;
  `annotated.fp` is the fingerprint of the inferred YAML record. A manual override can `deny:` inferred facts by
  `<section>:<glob>`.
- Ledger: manifest entries carry `origin` and, for inferred facts, `record_fp`. `ledger.py check` is strict for manual
  facts, reports inferred ones. `refresh` parks failing inferred facts (and ones the generator cannot test) in
  `metadata/inferred/gtk/_parked.json`, regenerates, reruns what changed.
- Typings: props without a passing test are `/** @unverified */`. Coverage: facts by state table, `verified: delivery`.
- Generic `bind:<prop>`: inferred `bindings` + a derived `notify::<prop>` signal per writable scalar notify property
  of a concrete kind; one generated echo test per binding (native change delivered and readable, programmatic set
  echoes to the listener).
- V1: typed (gtk4-rs) slot calls only if setter, getter, prop and kind are not newer than the `v4_6` feature baseline;
  everything else is dynamic and the generated test skips below the member's version (so GTK 4.14 CI stays green).
  Ingest now copies the GIR `version` of methods into `since`.
- CI: `ledger.py check` fails when `metadata/probe/gtk/<ledger version>.json` is missing; `infer.py --check` in
  `pnpm test:meta` keeps the inferred layer current.

## Log

- 2026-10-17: prototype moved, deterministic probe, infer/ingest/gen/ledger/coverage code written; first inference
  run over 57 kinds gave 291 props, 472 methods, 45 signals, 228 generic bindings, 3 slots (concrete kinds only,
  before abstract subjects and the synthetic-signal probe).
- 2026-10-19: review of the 135 middle/low-band facts (`docs/notes/ai1-gtk-review.md`): `_reviewed.yaml` decisions,
  four rule changes; 38 accepted, 76 rejected, 21 overridden; queue empty.
