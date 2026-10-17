# Svelters v2 status

The global progress ledger. States: `QUEUED`, `IN PROGRESS`, `BLOCKED`, `DONE`. Record the master commit and a one-line evidence summary when a milestone completes. Detailed evidence goes in the milestone file under `## Completion record`.

| Milestone | State | Commit | Notes |
| --- | --- | --- | --- |
| P0 setup | DONE | e64b97c | tag `v1-legacy`, move 2021 tree to `legacy/`, new workspace skeleton |
| P1 JS lane | DONE | 3f94c1a | 4 fixtures (19/87/35/9 ops) pass in Node TestHost |
| P1b renderer fork | DONE | 75e6ff4 | fork vendored @ f3de3f2; renderer + shim produce identical op logs; conformance node==qjs |
| P2 host lane | DONE | 5ebc7aa | 21/21 tests; 4 P1 fixtures byte-identical in QuickJS; engine-agnostic core + JsEngine trait |
| P3 GTK lane | DONE | 8f5dfe1 | 443 Gtk kinds ingested; 298 kinds/1304 props/349 signals in tables; 22 tests + P1 oplog replay pass |
| P4 Qt lane | DONE | dd6c28c | 192 classes ingested; 8 elements annotated; 41 generated tests pass; leaf-crate rebuild verified |
| F1 fork bindings | IN PROGRESS | | phase A: bind:this + bind hook in fork; phase B after P5 |
| P5 integration | IN PROGRESS | | |
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
| QuickJS JIT (quickjs-jit, Cranelift) | quickjs-jit-cranelift.md | done: no JIT for now; V8 if perf demands |
| Windows GUI backend (Win32 / WinUI 3) | windows-gui-backend.md | done: WinUI 3 via windows-rs; Win32 fallback |
| Apple GUI backends (AppKit / UIKit / SwiftUI) + JavaScriptCore | apple-gui-backend.md | done: AppKit+UIKit via objc2; JSC optional engine |
| JS ↔ native serdes | js-native-serdes.md | in progress |
| Ownership and lifetimes | ownership-and-lifetimes.md | done: explicit tree-driven lifetime, 4 lifetime classes, ABI v2 proposal |
| Svelte feature survey (lists, snippets, bindings, modifiers) | svelte-feature-survey.md | done |
| Styling: GTK CSS and Qt QSS | styling-gtk-css-qss.md | in progress |
