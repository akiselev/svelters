# Svelters v2 status

The global progress ledger. States: `QUEUED`, `IN PROGRESS`, `BLOCKED`, `DONE`. Record the master commit and a one-line evidence summary when a milestone completes. Detailed evidence goes in the milestone file under `## Completion record`.

| Milestone | State | Commit | Notes |
| --- | --- | --- | --- |
| P0 setup | DONE | e64b97c | tag `v1-legacy`, move 2021 tree to `legacy/`, new workspace skeleton |
| P1 JS lane | DONE | 3f94c1a | 4 fixtures (19/87/35/9 ops) pass in Node TestHost |
| P1b renderer fork | IN PROGRESS | | adopt PR #18042 as Svelte fork before P5 |
| P2 host lane | DONE | 5ebc7aa | 21/21 tests; 4 P1 fixtures byte-identical in QuickJS; engine-agnostic core + JsEngine trait |
| P3 GTK lane | IN PROGRESS | | |
| P4 Qt lane | IN PROGRESS | | |
| P5 integration | QUEUED | | |
| M2 dev loop | QUEUED | | |
| M3 metadata + codegen | QUEUED | | |
| M4 native semantics | QUEUED | | |
| M5 lists / model-view | QUEUED | | |
| M6 release binding linker | QUEUED | | |
| M7 egui | QUEUED | | |

## Research (docs/notes/)

| Topic | File | State |
| --- | --- | --- |
| Svelte 5 custom renderer + DOM surface | svelte5-custom-renderer.md | done |
| 2021 GTK version archaeology | gtk-v1-archaeology.md | done |
| GIR / Qt metadata ingestion | widget-metadata-ingestion.md | done |
| Signals, return values, child slots | signals-and-slots-semantics.md | done |
| Custom renderer PR hands-on trial | custom-renderer-trial.md | done |
| Node / Deno runtime integration | node-deno-integration.md | done |
| QuickJS JIT (quickjs-jit, Cranelift) | quickjs-jit-cranelift.md | in progress |
| Windows GUI backend (Win32 / WinUI 3) | windows-gui-backend.md | in progress |
| Apple GUI backends (AppKit / UIKit / SwiftUI) | apple-gui-backend.md | in progress |
