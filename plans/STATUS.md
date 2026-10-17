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
| F1 fork bindings | DONE | 32fe00a | element bind:this (call/getProp/StaleHandleError) + renderer bind hook in fork 27b7dfb70 (pushed to akiselev/svelte master + svelters/renderer); entry.text two-way on GTK+Qt with native echo check |
| D1 docs generator | DONE | 77addd6 | per-toolkit docs IR frontends → Starlight site (881 pages, link-checked); docs ingestion pass; GFDL-safe Qt `brief` policy; `pnpm docs:build` / `docs:test` |
| S1 serdes main types | DONE | 8071bb2 | ABI v3 value model (tags, descriptors, records, boxed, ERROR, CALLBACK, async, variants, dates) on GTK+Qt; 114/114 nextest; remainder → S2 |
| S2 ABI v3 remainder | DONE | e5a208d | out/in-out params (host+GTK+Qt), Qt callbacks + async (QMenu::addAction functor, QDialog::open Promise), Qt UTF-16 results (1 KiB wide getProp 32.6→1.8 µs), typed GVariant, GValue props, VALUE-class handle_info; 167/167 nextest |
| X3 child attrs + listen phases | DONE | 7310c77 | `parent:<attr>` child attributes (GTK grid/fixed/notebook/stack/overlay; Qt grid/box/tabwidget/splitter), listen_ex after/capture, ABI v2.2 additive; 453/453 nextest; open: QMainWindow toolbar/dock areas, Qt tabIcon |
| X5 lists (M5) | DONE | 013f593 | portable `virtuallist` (GTK GListModel+ListView/GridView, Qt QAbstractListModel+viewport-pooled rows), JS-owned ids + LIS keyed diff → minimal splices, `@svelters/lists` ListView; 10k/100k rows e2e on both; 422/422 nextest |
| M2a ABI v2 | DONE | 417093a | schedule_wake native timers (no host thread), lifetime classes/handle_info/dead-handle ring, cascading on_destroyed reasons, onRequestClose + host-decided quit; 74/74 nextest |
| M2b dev loop | DONE | 79a423f | in-process hot reload on GTK+Qt (edit → new UI ~1.7–2 s, mostly Vite); errors mapped to .svelte:L:C; svelte-check in pnpm test; 66/66 nextest |
| A1 API surface + galleries | IN PROGRESS |  4893be5 (wave 1) | starts when F1 merges; plans/milestones/A1-api-surface.md (1 lead spawning Sonnet workers, ≤8 concurrent) |
| X1 object returns + GTK GI calls | DONE | 5ea82a6 | borrowed handles with stable identity + weak refs (GTK qdata, Qt by_obj map); GTK dynamic calls via GIRepository (scalars, strings, enums/flags, objects, GStrv, boxed); ~110 Qt object getters; 156/156 nextest; out params wait on S2 |
| X2 sync handlers + frame clock | DONE | ba5bb65 | call_sync with event-object results (e.result / preventDefault), close-request per-event veto, key handled on GTK+Qt, rAF on GdkFrameClock / QWindow::requestUpdate; ABI v2.1 additive; 127/127 nextest; listen phases deferred (listen_ex proposal) |
| P5 integration | DONE | 1b22e76 | counter on GTK+Qt; 57/57 nextest incl. 12 integration; first frame GTK ~1.2 s / Qt ~0.2 s; binaries 3.4 / 2.6 MB; JS dominates the 1000-row split (host/ABI < 20 ms) |
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
| JS ↔ native serdes | js-native-serdes.md | done: extend svl_value in place (ABI v3, M3) |
| Ownership and lifetimes | ownership-and-lifetimes.md | done: explicit tree-driven lifetime, 4 lifetime classes, ABI v2 proposal |
| Svelte feature survey (lists, snippets, bindings, modifiers) | svelte-feature-survey.md | done |
| Styling: GTK CSS and Qt QSS | styling-gtk-css-qss.md | done: 4-layer model, portable vocabulary + native escape hatch |
