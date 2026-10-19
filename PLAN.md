# Svelters v2 plan

Status: adopted 2026-10-16. Supersedes `plans/archive/PLAN-2026-08.md`.

Svelters v2 runs Svelte 5 components in embedded QuickJS-NG and renders them with native toolkits: GTK4 first, QtWidgets second, AppKit and WinUI in progress, egui later. Application code is ordinary Svelte/TypeScript, compiled by our own Rust compiler. Native toolkit APIs remain reachable from JS.

The 2026-08 plan had a sound architecture but an infrastructure-first order: nothing reached the screen until its tenth phase, and it depended on eight sibling projects. This plan keeps the useful decisions and reorders the work around an on-screen vertical slice built by parallel lanes.

## Guiding rules

1. **Something on screen first.** Every milestone ends with a runnable app or a test that drives one.
2. **No sibling-project dependencies yet.** appfs, switchyard, artifactum, outboard, daemonkit, deterministic-rs, desktop-cli and egui-workbench integrations are deferred. Their design notes stay in `plans/archive/architecture/` for later.
3. **Codegen everywhere it pays.** Widget metadata is ingested from upstream structured sources and hand annotations. Agents produce the annotations, and generated tests verify them.
4. **No raw toolkit pointer crosses into JavaScript.** JS sees stable handles.
5. **No ordinary `.svelte`/`.ts` edit invokes Cargo or a C++ compiler in development.**
6. **Each toolkit's event loop is authoritative.** QuickJS jobs are pumped at host checkpoints and never busy-polled. JS timers and animation frames use the toolkit's native timers and frame clock. The host never runs its own timing thread; the P2 timer thread is a stopgap (`plans/architecture/backend-abi.md#planned-change-timers-belong-to-the-toolkit-before-m2`).

## Core layers

```text
L0  build        Vite + svelters-compile (Rust Svelte compiler, native lowering
                 from per-backend semantic specs) via @svelters/build
                 → one ESM bundle (dist/app.js) + source maps
                              │
L1  runtime      @svelters/native-runtime: host ops, blocks, styling, lists;
                 reactivity from its core (`/core`, Svelte 5's signals behind
                 one adapter file; an own core is RUNTIME phase 2+)
                              │  host ops: create / insert / remove / setProp /
                              │  listen / call / destroy; events come back queued
L2  host core    Rust (rquickjs/QuickJS-NG): module loading, job pump, timers,
                 console, source maps, handle table, event queue, hot reload
                              │  svelters backend C ABI (fixed, ~15 functions)
L3  backends     GTK4 (Rust/gtk4-rs)   QtWidgets (generated C++)   egui (Rust)
                              │
L4  toolkit      GTK4 / Qt 6 / egui
```

Widget metadata and codegen run alongside these layers: upstream sources plus annotations produce element-kind tables, the TypeScript typings, the Qt C++ dispatch code, and the GTK method bindings.

- L0/L1 decision (user, 2026-10-17, `plans/research/CLI.md` decisions 2 and 6; done in RT1, `plans/milestones/RT1-retire-fork-runtime-core.md`): the Rust compiler (`crates/svelters-compiler`, `svelters-compile`) plus `@svelters/native-runtime` is the only frontend, not behind a flag.
  - The 2026-10-16 L1 (stock Svelte compiling through a vendored fork of Svelte PR #18042 into `@svelters/renderer`, with the `@svelters/dom` shim as fallback and weekly upstream tracking) is retired; history in `docs/notes/custom-renderer-trial.md`, `docs/notes/fork-bindings.md` and `plans/milestones/P1-js-lane.md`.
  - Svelte is the stock npm `svelte@5.57.2`: its runtime backs the core (`packages/native-runtime/src/core/svelte.js`, the only importer of `svelte/internal/*`, checked by `tools/check-svelte-internals.mjs`) and its compiler only scopes component CSS. Owning the runtime: `plans/research/RUNTIME.md`.
  - Element facts the compiler needs come from the semantic specs (`metadata/semantics/<toolkit>.json`, `docs/notes/native-compiler/semantic-spec.md`); generated code targets `docs/notes/native-compiler/runtime-contract.md`.
- L2/L3 boundary: `plans/architecture/backend-abi.md`. The ABI is generic and never grows per native method. Per-method knowledge lives in generated backend code keyed by IDs.
- Backends are generated from widget metadata from the first prototype. The generators stay Python through development and move to Rust near release (M6), once their shape is final, so that `build.rs` scripts are self-contained. Ingestion scripts stay Python.
- Qt backend: C++ generated from a project-level **leaf crate's** `build.rs`, compiled with `cc` and linked in. Only that crate's config/manifest changes rebuild it. See `plans/architecture/backend-abi.md#qt-build-leaf-crate`.
- Metadata and annotations: `plans/architecture/widget-metadata.md`.

## Milestones

| ID | Goal | Depends on |
| --- | --- | --- |
| **P** | **Prototype: counter + keyed list on screen in GTK4 and Qt from one JS bundle** | — |
| P1 | JS lane: build pipeline, `@svelters/dom` shim, fixtures, op-log TestHost in Node | — |
| F1 | Fork work: element `bind:this` + renderer binding hook for `bind:<prop>` in our Svelte fork (phase A: compiler/runtime; phase B: renderer + binding metadata, after P5) | P1b |
| P1b | Switch L1 to the vendored renderer fork (`@svelters/renderer`), renderer conformance suite, weekly upstream tracking | P1 |
| P2 | Host lane: Rust workspace, ABI crate + C header, rquickjs host, log backend | ABI draft |
| P3 | GTK lane: Python GIR/PyGObject ingestion → metadata → generated GTK4 backend tables + verification tests | ABI draft, schema v0 |
| P4 | Qt lane: Python qdoc/metaobject/typesystem ingestion → metadata → generated C++ backend, built by a leaf-crate `build.rs` | ABI draft, schema v0 |
| P5 | Integration: P1 bundle runs in P2 host on P3 and P4 backends; Xvfb screenshot tests | P1b, P2–P4 |
| M2 | ABI v2 (`plans/architecture/backend-abi.md#abi-v2`): native timers via `schedule_wake` replace the host timer thread; lifetime classes, `handle_info`, `on_destroyed` reasons, cascading native death, host-decided quit. Dev loop: Vite watch → reload bundle in the running host without Cargo; source-mapped errors | P5 |
| M3 | Codegen at scale: annotate beyond the P kinds through the agent loop (A1), GTK dynamic GI method invocation, full `.d.ts`, widget gallery per toolkit | P5 |
| M4 | Native semantics: named child slots/replaceable sub-widgets, sync handlers and return values, two-way binding, ownership/lifetime | M3, signals/slots research |
| M5 | Virtualized lists/model-view (GListModel, QAbstractItemModel) | M4 |
| M6 | Release builds: port the generators (not the ingestion scripts) to Rust so `build.rs` needs no Python; binding linker (tree-shaken native roots → thin generated bindings) | M3, generators stable |
| M7 | egui backend | M4 |
| Later | Styling, accessibility/semantic graph, devtools, packaging, upstream-compat automation, ecosystem integrations | — |

The four P lanes run in parallel once `plans/architecture/backend-abi.md` is agreed. Each lane has a file under `plans/milestones/`. Later milestones get a file when they start, not before.

## Decisions carried over from the 2026-08 plan

These are still valid. The archive has the reasoning.

- Vite 8 bundles; components are compiled by `svelters-compile` through the `@svelters/native-runtime/vite` plugin (superseded 2026-10-17: stock Svelte + `@sveltejs/vite-plugin-svelte` and the PR #18042 fork, see L0/L1). No Svelte compiler inside QuickJS, no SWC/oxc.
- QuickJS is execution-only: no TS/Svelte compilation and no npm resolution at runtime.
- pnpm workspace, ESM only. Rust edition 2024, resolver 3, pinned stable toolchain.
- Source maps in dev/test, mapping QuickJS stacks to `.svelte`/`.ts`.
- Native binding linker for release (`plans/archive/architecture/native-binding-linker.md`), now M6.
- Native API IR concepts (`plans/archive/architecture/native-api-ir.md`) inform the M3 schema.

## Lessons from the 2021 versions

See `docs/notes/gtk-v1-archaeology.md`.

- Full static per-class Rust codegen made release builds take 16 minutes. Keep the per-method surface out of Rust: use dynamic invocation in dev, and generate thin bindings only for release roots.
- Most 2021 crashes were in our own lifetime handling: toggle refs, GC cycles, use-after-free on signal disconnect. Design one canonical wrapper per native object and disconnect-safe handlers up front.
- Namespaced child tags (`<child:titlebar>`) and a JS-backed `GListModel` with Svelte-rendered rows worked. Revisit them in M4/M5.

## Agent protocol

- Research and lane agents write notes incrementally to `docs/notes/<topic>.md`. Scripts go in `docs/notes/scripts/`. Large generated output stays out of git.
- Status lives in `STATUS.md`. Work lands on `master` after the milestone's acceptance checks pass.
- Lane agents edit only their lane's directories. ABI changes go through `plans/architecture/backend-abi.md` first.
