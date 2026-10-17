# A1: API surface annotations and widget galleries

Status: QUEUED. Starts when F1 (fork bindings) merges, because F1 adds binding metadata to the same override files.

Goal: as much of the GTK4 and QtWidgets API surface as possible works from Svelte, with every annotated fact backed by a passing generated test. One widget gallery app per toolkit is both the showcase and the end-to-end check.

## Shape of the work

| Phase | Who | Output |
| --- | --- | --- |
| 1. Tooling + inventory | 1 lead agent (Opus) | per-toolkit override schemas, strict merge, verified ledger, coverage report, `metadata/A1-TODO.md` |
| 2. Annotation batches | Sonnet workers, spawned and dispatched by the lead from the TODO list | per-family override YAML + passing generated tests + a gallery page |
| 3. Integration | lead merges batches into the A1 branch; coordinator merges A1 to master in increments | galleries complete, coverage report, completion record |

Agent budget (user-approved 2026-10-17): 1 lead, which spawns its own Sonnet workers, **at most 8 running at a time**. Batches are sized so a worker finishes one in a session. Verification is the generated tests plus the lead's own review; there are no reviewer agents. With 8 workers the builds contend hard, so each worker builds with `CARGO_BUILD_JOBS=2` and nextest `--test-threads 2`, and runs only its batch's tests until its final full-gate run.

## Phase 1: tooling and inventory (lead)

1. **Override formats stay per toolkit.** Each toolkit's override format follows that toolkit's semantics, and the formats are not unified: GTK patches the ingested kind, Qt uses `elements:` entries, and WinUI/Apple will get their own. The only shared conventions are:
   - every fact has a stable fact id (for the ledger);
   - `origin`/`provenance` on every fact;
   - strict matching (step 2).

   Tooling that spans toolkits (the ledger, the coverage report) reads each toolkit through a small per-toolkit adapter.
2. **Strict merge + override schema.**
   - A JSON Schema per toolkit for its override files.
   - `svmeta.merge` errors when an override record doesn't match an ingested record, unless it is explicitly marked `virtual: …` or `new: true`.
   - Unknown keys are errors.
3. **Inheritance.** Annotations on base classes and interfaces (`GtkWidget`, `GtkEditable`, `GtkOrientable`, `QWidget`, `QAbstractButton`, `QAbstractSlider`, …) apply to subclasses unless overridden. The merged output records where each inherited fact came from.
4. **Verified ledger.** `metadata/verified/{gtk,qt}.json` records, per fact: the fact id, the generated test id, the result, and the toolkit version. The generators refuse unverified manual facts. A `--refresh` rerun updates the ledger, and a toolkit upgrade re-runs the facts whose source changed.
5. **Coverage report.** `tools/metadata/coverage.py` writes `metadata/coverage/{gtk,qt}.md`. For each kind it shows: exposed yes/no, props/signals/methods exposed out of the total, slots annotated, ownership annotated, signal results annotated, bindings annotated, verified facts out of the total, and whether it has a gallery page.
6. **Gallery skeletons.** Add `examples/gallery-gtk` and `examples/gallery-qt`, or a single `gallery --backend` binary like `counter`. Each has a sidebar of family pages and one screenshot test per page. Bundles come from `fixtures/gallery-*` or `apps/gallery`, and the lead chooses which.
7. **`metadata/A1-TODO.md`.** The full inventory, grouped into batches that a Sonnet worker can finish in one session (roughly 5–15 kinds, one family, one toolkit). Each batch lists:
   - its kinds, and the base classes that must be annotated first (dependencies between batches);
   - the facts to annotate: slots, ownership, signal results/sync, two-way binding, method exposure, enums;
   - its gallery page;
   - the exact commands for its tests;
   - a status field (`todo | in-progress | done | blocked: <why>`).

   Base classes and interfaces come first. Order families by value:
   1. buttons and toggles
   2. text entry
   3. ranges (slider, spin, progress)
   4. containers and layouts (box, grid, stack, paned, notebook/tabs, scrolled, frame, expander)
   5. selection (dropdown/combo, check/radio groups)
   6. lists and views (non-virtual first; models wait for M5)
   7. menus, toolbars, status/header bars
   8. dialogs and popovers (non-blocking only)
   9. display (image, label rich text, separator, spinner, calendar, level bar)
   10. everything left over

   Mark kinds that aren't worth exposing, and say why.
8. **Worker briefs.** Write `metadata/A1-WORKER-gtk.md` and `metadata/A1-WORKER-qt.md`: self-contained prompt templates that a Sonnet worker follows for one batch. It gives the file locations, the annotation format with examples, how to write and run the generated tests, the gallery page conventions, the gates, and the rule that it edits only its batch's files plus its gallery page.

Phase 1 ends with all gates green and the TODO list committed.

## Phase 2: Sonnet workers

The lead spawns Sonnet workers (`model: sonnet`) and dispatches batches from `metadata/A1-TODO.md`, keeping at most 8 running:
- one worktree per worker;
- Sonnet model;
- the prompt is the toolkit's `A1-WORKER-<toolkit>.md` plus the batch.

A worker:
1. drafts the annotations, with provenance on every fact;
2. runs the generated tests under Xvfb, and fixes or drops any fact whose test fails;
3. adds its gallery page with a screenshot test;
4. updates the batch status and the ledger;
5. commits.

Facts it can't verify are left unannotated and listed in the batch's status line. They are never merged unverified.

The lead reviews each batch's diff, merges it into the A1 branch after its gates pass, marks it done, and spawns a worker for the next batch whose dependencies are done. The lead sends the coordinator a short progress message after each wave; the coordinator merges the A1 branch to master in increments.

## Rules

- Builds use at most 4 cores: `CARGO_BUILD_JOBS=4`, nextest `--test-threads 4`, `CARGO_BUILD_BUILD_DIR=target/build`.
- Gates: `pnpm test` and `CARGO_BUILD_JOBS=4 xvfb-run -a cargo nextest run --workspace --test-threads 4`.
- Qt C++ is changed only through the generator and its templates.
- Blocking modal APIs are never exposed (`exec()`, `gtk_dialog_run`-style calls).
- Notes go in `docs/notes/a1-*.md`.

## Acceptance

- Coverage reports show the high-value families exposed and verified on both toolkits.
- Both galleries run and every page has a screenshot test.
- All gates are green.
- A completion record here includes the coverage numbers.
