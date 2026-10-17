# Widget metadata, annotations and codegen

Status: direction set. The M3 implementation starts after P5. Research is in `docs/notes/widget-metadata-ingestion.md` and `docs/notes/signals-and-slots-semantics.md`.

## Sources

| Toolkit | Machine-readable sources | Gives |
| --- | --- | --- |
| GTK4 | `.gir` XML (stdlib Python parser) | types, nullability, transfer, array lengths, callbacks, defaults (as C literals) |
| GTK4 | PyGObject live pass | typed defaults, ranges, interface props, signal flags; cross-checks GIR against the installed library |
| Qt 6 | qdoc `.index` XML | the full public API: signatures, default args, virtuals, property accessors |
| Qt 6 | QMetaObject dump (PySide6) | properties, signals, slots, invokables, enums |
| Qt 6 | PySide typesystem XML | ownership rules (parent ownership, old child released by `setCentralWidget` etc.) |
| Qt 6 | uic behaviour on `.ui` samples | canonical child placement for QMainWindow, QTabWidget, layouts |

Ingestion scripts start from `docs/notes/scripts/` and move to `tools/metadata/` in M3.

## What no upstream source encodes

These are hand-annotated per element kind:

- Child policy and named slots: cardinality, insert/remove ops, per-child attributes, whether a slot replaces a default sub-widget, and whether replacing destroys the old one.
- Ownership transfer where the typesystem is silent or wrong.
- Signal behaviour: sync-required/return value, run-first vs after, echo suppression for two-way binding, overload collapsing (Qt `clicked()`/`clicked(bool)`).
- Which widget-typed properties are references (`default-widget`) rather than slots.

## Annotation loop

Agents draft the annotations, and generated tests verify them.

1. An agent drafts annotations for a batch of kinds from docs and source, with a provenance field on every fact.
2. The generator emits a verification test per annotated fact and runs it headless (Xvfb):
   - **Slots:** insert into the slot, read it back through the native getter, remove it, check the default is restored or the slot is empty.
   - **Ownership:** create, parent, destroy the parent, check the child's liveness through a weak ref or `QPointer`. Replace a slot child and check whether the old one survives.
   - **Signals:** emit or trigger natively, check delivery, the return-value effect (e.g. a `close-request` veto keeps the window open) and echo suppression.
3. A failing test sends the annotation back to the agent with the failure output. Merged annotations always have passing tests.
4. The GIR/PyGObject cross-check runs again on a toolkit upgrade. Changed facts re-run their tests.

## Outputs

- Element-kind tables (`metadata/<toolkit>/*.json`, checked in and versioned).
- `.d.ts` typings for native elements, wrapper components and `bind:this` targets.
- Qt: C++ dispatch/subclass/connector code via `svelters-qt-build` (see `backend-abi.md`).
- GTK: dev uses dynamic GObject property/signal access plus GIRepository/libffi method invocation. Release uses generated thin `-sys` calls for the tree-shaken roots (M6).
