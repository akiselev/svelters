# SVR00 - archaeology, preservation, and baseline

## Goal

Preserve everything learned by the 2021 experiments before replacing the architecture, and establish measurable baselines for build time, binary size, behavior, and supported examples.

## Scope

Audit `svelters`, `svelte-qt`, `svelters-zig`, and `cadatomic-svelte-nodegui`. Tag the current `svelters` head as a historical v1 snapshot. Do not mechanically move the entire old tree into the future workspace if a tag plus selected fixtures/docs preserves it more cleanly.

Create `docs/legacy/ARCHAEOLOGY.md` covering at minimum:

- fake Document/Node design;
- QuickJS/GC ownership experiments;
- GTK GObject proxy/registry/marshalling work;
- Qt metaobject and ordinary-method limitations;
- Zig experiment conclusions;
- list-model/list-view implementation;
- HMR behavior;
- generated-binding compile/binary explosion;
- old custom compiler/TS strip/build stack;
- event-loop approaches and known failure modes.

Extract representative old apps into `fixtures/legacy/` with expected behavior notes.

## Baselines

On one documented Linux environment, capture what can still be built without heroic patching. If the old project no longer builds, capture dependency/toolchain failure points rather than spending this phase modernizing them.

Record old source/generated LOC, generated Rust size, last known release-build timing from history, current repository size, and native API codegen shape.

## Deliverables

- immutable legacy tag;
- archaeology document with links to relevant commits/files;
- fixture inventory;
- baseline report;
- explicit keep/rewrite/delete table for major old subsystems.

## Acceptance

An agent beginning SVR01 can understand why each old subsystem is being replaced without reading git history. Every claim about an old success/failure points to a fixture, file, or commit.
