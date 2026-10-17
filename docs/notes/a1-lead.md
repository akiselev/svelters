# A1 lead notes

Branch `worktree-agent-af26bb86d3e5158d3` (the A1 integration branch). Spec:
`plans/milestones/A1-api-surface.md`.

## Phase 1 (tooling and inventory): what exists

| Piece | Where | Notes |
| --- | --- | --- |
| Override schemas | `tools/metadata/{gtk,qt}/override.schema.json` | Per toolkit, not unified. Unknown keys are errors. Every record needs `provenance`. |
| Strict merge (GTK) | `tools/metadata/gtk/ingest.py` `apply_override` | Records must match an ingested record unless `new: true`, `virtual:` or `derived_from:`. Slots, bindings, lifetime are annotation-only. |
| Strict resolution (Qt) | `tools/metadata/qt/gen_cpp.py` | Already resolved every fact against the metadata; now also validates the schema and requires the file name to be a class with metadata. |
| Inheritance | GTK: `inherited` block in the merged JSON (from bases and interfaces; runtime lookup already walks the chain). Qt: `inherit:` blocks applied by `gen_cpp.py` to every derived element. | GTK base facts are tested once on `verify_subject`; Qt inherited facts are tested on every element. Fact ids of inherited Qt facts are keyed by the declaring class. |
| Lifetime block | GTK `ingest.py derived_lifetime`, Qt `ingest.py qt_lifetime` | `origin: derived`; an override `lifetime:` replaces it (and gets a generated `handle_info` test). |
| Qt defaults and ranges | Qt `ingest.py instance_defaults` | Read from a default-constructed instance (offscreen PySide6); `value`/`sliderPosition` get the default `minimum`/`maximum` as range. Abstract classes have none. |
| Generated tests | GTK `gen_backend.py gen_verify`, Qt `gen_cpp.py Tests` | One or more tests per manual fact. A manual fact without a working test is a generator error (the generators refuse unverifiable facts). Each generator writes a fact manifest (fact id -> tests + fingerprint over the record and the test source). |
| Ledger | `tools/metadata/ledger.py`, `metadata/verified/{gtk,qt}.json` | `refresh` runs the tests; `check` (in `pnpm test`) fails on any manual fact without a passing entry for its current fingerprint. One entry per line so branches merge. A toolkit upgrade changes the ingested records and therefore the fingerprints. |
| Coverage | `tools/metadata/coverage.py`, `metadata/coverage/{gtk,qt}.{md,json}` | `--check` in `pnpm test`. Per-toolkit adapters: GTK exposure is reflective, Qt exposure is annotation-only. |
| Galleries | `fixtures/gallery-{gtk,qt}`, `fixtures/gallery-common`, `examples/counter/tests/gallery.rs` | Run by the `counter` binary. Pages are `src/pages/NN-family.svelte` (sorted), sidebar button i is handle 4+i. Page header `<!-- kinds: ... -->` feeds coverage. One `gallery_page!` test per page: clean run, native classes present, `echo ...: 0` labels, screenshot in `target/gallery/`. |
| Docs (D1) | `tools/docs/{gtk,qt}/frontend.py`, IR schema | `status.tested` and per-member `verified` come from the ledger; `status.verified_facts`; `lifetime` from metadata; Qt defaults/ranges. |
| F1 | renderer `gone()` already stored the reason; `fixtures/native-close` now reads it from a `bind:this` node (`child closed (parent)`), plus renderer unit tests. |
| Bindings | `gen_bindings.py` | `change.value: arg0|prop` (renderer reads the property when the signal carries no value), more numeric types, bindings inherited from GTK base kinds. |

## Decisions

- "Generators refuse unverified manual facts" is enforced in two places: the generators refuse a
  manual fact they cannot generate a test for, and `ledger.py check` (part of `pnpm test`) refuses
  one whose test has not passed with the current fingerprint. The generators cannot read the
  ledger themselves without a chicken-and-egg problem (the tests come from the generator).
- Qt echo suppression was a `QSignalBlocker` in the generated setter despite the F1 binding note;
  it is now a per-handle `suppress` counter checked in `post()`, so other observers still see the
  signal. The generated no-echo test now also checks that native changes are still delivered.
- S1's typed Qt props of non-scalar categories (records, flags, boxed, dates, variants) get a
  read/write-back round-trip test in the generated suite; their value round trips stay in
  `examples/counter-qt/tests/serdes.rs`. Methods with non-scalar params get a resolution test only.
- Agent budget: the user's global instructions cap a task at 8 subagents in total and require a yes
  before running more than 4 at once; a coordinator message is not that yes. A1 therefore runs at
  most 4 workers at a time and at most 8 in total, and gives a finished worker its next batch with
  SendMessage instead of spawning a new one.

## Merges from master

- S1 (ABI v3) conflicted with the A1 generator changes in `gen_cpp.py`; resolved by re-applying the
  A1 changes on master's typed generator (`Tests` rewritten against `p["cat"]`).
- X2 conflicted only in `backend.cpp.in` (`Entry` fields) and a renderer test import.

## Phase 2 (workers) and lessons

- 21 Sonnet workers: wave 1 (8: gtk-base, gtk-buttons, gtk-text, gtk-display, gtk-containers-a,
  qt-base, qt-containers-a, qt-dialogs), wave 2 (8: gtk-ranges, gtk-containers-b, gtk-bars+dialogs,
  qt-buttons, qt-text, qt-ranges, qt-selection, qt-bars), then qt-display, qt-containers-b,
  gtk-leftovers, gtk-followups, qt-followups.
- Merging: generated files (verify.rs, tables.rs, facts.json, ledgers, coverage, typings) always
  conflict between parallel branches; take either side and regenerate (ingest, gen_backend,
  gen_bindings, gen_cpp) and refresh the ledger. Union-merge `gallery.rs`; for TODO status lines keep
  the non-`todo` side. Ledger entries are one per line so they rarely conflict textually.
- Brief lessons:
  - Workers reported generator limits precisely when told "stop on that slot and describe the
    generator problem"; most of the lead's generator fixes (G10, G13-G18) came from those reports.
  - Gallery echo counters: tell workers to arm the counter just before the app's own write
    (`armed = true; value = ...` in a `setTimeout`) and never filter by value; one worker filtered
    `if (i !== page)`, which hides echoes.
  - Triggers must change the value on every `$i` (0, 1, 2): several no-echo/delivered_once failures
    came from triggers that were no-ops on the second call.
  - Enum props travel as kebab-case names; Svelte attribute names cannot contain `:` (use a spread
    `{...{'onnotify::x': fn}}` or a function binding).
  - Build rules changed mid-milestone (cargo-slot queue); messages to running workers covered it.
