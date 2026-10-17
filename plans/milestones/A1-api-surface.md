# A1: API surface annotations and widget galleries

Status: QUEUED. Starts when F1 (fork bindings) merges, because F1 adds binding metadata to the same override files.

Goal: as much of the GTK4 and QtWidgets API surface as possible works from Svelte, with every annotated fact backed by a passing generated test. One widget gallery app per toolkit is both the showcase and the end-to-end check.

## Shape of the work

| Phase | Who | Output |
| --- | --- | --- |
| 1. Tooling + inventory | 1 lead agent (Opus) | override format unified, strict merge, verified ledger, coverage report, `metadata/A1-TODO.md` |
| 2. Annotation batches | Sonnet workers, dispatched by the coordinator from the TODO list | per-family override YAML + passing generated tests + a gallery page |
| 3. Integration | coordinator (inline) | merged batches, galleries complete, coverage report, completion record |

Agent budget: 1 lead plus at most 6 Sonnet workers, about 7 in all. At most 4 workers run at once. A worker that finishes a batch is given the next batch, rather than a new worker being spawned. Verification is the generated tests plus the coordinator's review inline; there are no reviewer agents.

## Phase 1: tooling and inventory (lead)

1. **Unify the override format.** GTK overrides patch the ingested kind (`properties:`, `children:`), while Qt overrides use an `elements: - tag: … construct: … props:` list. Make both patch the kind. Exposure to JS goes in its own `element:` section: the tag, the constructor, and which props, methods and signals are exposed, under what JS names. Migrate the existing 18 files and keep generated output byte-identical, or explain every diff.
2. **Strict merge + override schema.**
   - A JSON Schema for override files.
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
8. **Worker brief.** Write `metadata/A1-WORKER.md`: a self-contained prompt template that a Sonnet worker follows for one batch. It gives the file locations, the annotation format with examples, how to write and run the generated tests, the gallery page conventions, the gates, and the rule that it edits only its batch's files plus its gallery page.

Phase 1 ends with all gates green and the TODO list committed.

## Phase 2: Sonnet workers

The coordinator (the main session) dispatches batches from `metadata/A1-TODO.md`:
- one worktree per worker;
- Sonnet model;
- the prompt is `A1-WORKER.md` plus the batch.

A worker:
1. drafts the annotations, with provenance on every fact;
2. runs the generated tests under Xvfb, and fixes or drops any fact whose test fails;
3. adds its gallery page with a screenshot test;
4. updates the batch status and the ledger;
5. commits.

Facts it can't verify are left unannotated and listed in the batch's status line. They are never merged unverified.

The coordinator merges each batch to master after its gates pass, then hands that worker the next batch whose dependencies are done.

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
