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

## Slice 4 (AppKit): design as built

Mac handoff: `docs/notes/ai1-appkit-mac-handoff.md`.

- Results exist only on macOS, so everything that needs AppKit runs in the `macos-appkit` CI job (or on a Mac) and
  is committed from its `appkit-probe` artifact; everything else (infer, generators, ledger, coverage) runs on Linux.
- `tools/metadata/appkit/probe.py` + `crates/svelters-appkit/examples/appkit_probe.rs`: per class, on the element
  the class's facts are tested on, through the same backend harness as the generated tests. Per writable prop of a
  probe-able type: default, KVO observer on the KVC key, set the first rules sample that differs from the default,
  read back; outcome, KVO fired, listened signals emitted by the programmatic set (echo). Per class: target-action
  and notification signals deliver; the notification constants of the rules exist (dlsym) with that value. One
  JSON line per result; a crash marks the prop in flight and the class resumes after it (`--after`). Output
  `metadata/probe/appkit/<SDK version>.json` (runtime macOS version recorded inside).
- `tools/metadata/appkit/infer.py` + `metadata/rules/appkit.yaml` -> `metadata/inferred/appkit/` (YAML in the manual
  override schema, evidence, `_review.json`; `_parked.json` written by the ledger). Rules: exact round trips become
  props with samples (0.95 with KVO, 0.9 without); `bind:<value prop>` for NSControl value props through the nearest
  binding source (NSTextField: `control-text-did-change`; NSControl: `action`), never bare KVO, marked
  `observability: partial, missed: [programmatic]`, attached to the topmost element class; notification signals from
  the rules' per-class list when the probe found the constant (delivery only). No methods or slots (ingest only reads
  override-named methods).
- `tools/metadata/appkit/appkit_layers.py`: ingested < inferred < manual for every consumer (gen_backend, gen_semantics
  through its Model, gen_bindings, probe). A manual record replaces the inferred record of the same name; manual
  `deny: ["<section>:<glob>"]`; parked inferred records (by record fingerprint) are left out.
- `gen_backend.py`: manual facts keep one function per fact; inferred props and signals and all bindings are
  table-driven (`PROP_ROWS`, `SIGNAL_ROWS`, `BIND_ROWS`, one runner each, one named case per row via
  `svelters_apple::testing::run_table_cases`). Writes `crates/svelters-appkit/src/generated/facts.json`
  (fact -> tests, fingerprint over record + test source, origin, `record_fp`, `strength: delivery`) and lists
  inferred facts it cannot test under `untestable` (the ledger parks them).
- `svelters_apple::testing`: `SVELTERS_TEST_RESULTS=<file>` appends one JSON line per case with the panic message.
- `ledger.py import --toolkit appkit <results>` (JSON lines, JSON or JUnit XML; `--dry-run` in CI): parks failing
  inferred facts, regenerates, writes `metadata/verified/appkit.json`. `check`/`all` include a toolkit with a facts
  manifest once its ledger exists; `check` requires the appkit probe for the ledger version. `refresh --toolkit
  appkit` runs the tests on a Mac and imports them. `coverage.py` writes `metadata/coverage/appkit.{md,json}`.

## Log

- 2026-10-17: prototype moved, deterministic probe, infer/ingest/gen/ledger/coverage code written; first inference
  run over 57 kinds gave 291 props, 472 methods, 45 signals, 228 generic bindings, 3 slots (concrete kinds only,
  before abstract subjects and the synthetic-signal probe).
- 2026-10-19: review of the 135 middle/low-band facts (`docs/notes/ai1-gtk-review.md`): `_reviewed.yaml` decisions,
  four rule changes; 38 accepted, 76 rejected, 21 overridden; queue empty.

## Slice 2 (Qt): design as built

See `tools/metadata/qt/README.md`. Probe `metadata/probe/qt/6.11.0.json` (PySide6 6.11.0, 73 classes, 4 crashing
ops recorded: QColumnView/QDockWidget property writes SEGV), `infer.py` + `metadata/rules/qt.yaml` (accept-by-pattern),
`metadata/inferred/qt/` (43 classes), `qt_layers.py` (merge), table-driven tests in `verify.cpp.in`, ledger parking for Qt.
Net new verified facts after the manual layer wins: props 31, methods 24, signals 47 (37 `verified: delivery`),
bindings 13; 0 parked. Not done: abstract-base subjects (inherit-level inference), `@unverified` in qt.d.ts,
pure getters and actions (review rows only), enum-argument delivery-only signals.
