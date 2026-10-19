---
title: Architecture
description: The layers from Vite build to native toolkit, the backend C ABI, and the milestone lanes with their current state.
---

Svelters v2 runs Svelte 5 components in embedded QuickJS-NG and renders them with native toolkits: GTK4 first, QtWidgets second, AppKit and WinUI in progress, egui later. Application code is ordinary Svelte and TypeScript, compiled by the project's own Rust compiler. The plan was adopted on 2026-10-16. On 2026-10-17 the compiler was chosen as the only frontend; milestone RT1 retired the earlier one.

## Layers

```text
L0  build        Vite + svelters-compile (Rust Svelte compiler, native lowering
                 from per-backend semantic specs) via @svelters/build
                 -> one ESM bundle (dist/app.js) + source maps
                              |
L1  runtime      @svelters/native-runtime: host ops, blocks, styling, lists;
                 reactivity from its core (/core: Svelte 5's signals behind
                 one adapter file)
                              |  host ops: create / insert / remove / setProp /
                              |  listen / call / destroy; events come back queued
L2  host core    Rust (rquickjs/QuickJS-NG): module loading, job pump, timers,
                 console, source maps, handle table, event queue, hot reload
                              |  svelters backend C ABI (fixed, ~15 functions)
L3  backends     GTK4 (Rust/gtk4-rs)   QtWidgets (generated C++)
                 AppKit, WinUI (Rust slices)   egui (later)
                              |
L4  toolkit      GTK4 / Qt 6 / AppKit / WinUI 3 / egui
```

Widget metadata and codegen run alongside the layers. Upstream sources plus annotations produce element-kind tables, the compiler's semantic specs, TypeScript typings, the Qt C++ dispatch code and the GTK method bindings.

| Layer | Where | Responsibility |
| --- | --- | --- |
| L0 | `packages/build` (`@svelters/build`) | Vite config and `svelters-build` CLI for apps; pins Svelte and Vite exactly |
| L0 | `crates/svelters-compiler` (`svelters-compile`), `packages/native-runtime/vite.js` | Compile `.svelte` and `.svelte.js` to native-runtime calls, checked against `metadata/semantics/<toolkit>.json` |
| L1 | `packages/native-runtime` (`@svelters/native-runtime`) | Host ops, template blocks, bindings, events, styling runtime, virtualized lists, element typings |
| L1 | `packages/style`, `packages/lists`, `packages/canvas`, `packages/types` | CSS translation and tokens, `ListView` re-export and `arrange`, recording canvas, generated non-UI types |
| Tests | `packages/testhost` (`@svelters/testhost`) | Node host that records the op log, fixture runner |
| L2 | `crates/svelters-host` | QuickJS host, handle table, name-to-ID caches, event queue, timers, checkpoint |
| L2/L3 boundary | `crates/svelters-abi` | `svelters_backend.h` and its Rust mirror |
| L3 | `crates/svelters-gtk`, `crates/svelters-qt-build` plus a leaf crate, `crates/svelters-backend-log` | Toolkit backends and the op-recording test backend |
| Metadata | `metadata/`, `tools/metadata/` | Ingested element kinds, annotations, generators |

## Design rules

1. Something on screen first: every milestone ends with a runnable app or a test that drives one.
2. No sibling-project dependencies yet.
3. Codegen everywhere it pays. Agents write annotations, generated tests verify them.
4. No raw toolkit pointer crosses into JavaScript. JS sees stable handles.
5. An ordinary `.svelte` or `.ts` edit never invokes Cargo or a C++ compiler in development.
6. Each toolkit's event loop is authoritative. QuickJS jobs are pumped at host checkpoints and never busy-polled. JS timers and animation frames belong to the toolkit's native timers and frame clock; the host never runs its own timing thread. The timer thread in the P2 host is a stopgap that ABI v2 removes (see [the ABI](/concepts/abi/#schedule_wake)).

## L1: the compiler and native runtime

Components are compiled by `svelters-compile` (crate `crates/svelters-compiler`), a Rust port of Svelte 5.57.2's parser, acorn and scope analysis, with its own native lowering in place of Svelte's DOM output.

- The compiler knows no widget by name. It reads a per-backend semantic spec, `metadata/semantics/<toolkit>.json`, generated from the same metadata as the backends. Tags, props and their value types, signals, sync signals, listen phases, child slots, child attributes, bindings, toplevel kinds and the style table come from there.
- Mistakes are compile errors with positions: an unknown element, prop or signal, a child the parent's slot rejects, a bad static value, a binding the spec has no plan for.
- Static attribute values are typed at compile time (`spacing="8"` becomes the integer 8). The runtime never coerces.
- Generated code imports `$` from `@svelters/native-runtime/core` (the reactive core; Svelte's signals behind one adapter file, `src/core/svelte.js`) and `$n` from `@svelters/native-runtime` (host ops, blocks, events, bindings, styling). There is no DOM mirror and there are no anchor nodes; the runtime keeps only the ordering native insertion needs.
- Svelte itself is stock `svelte@5.57.2` from npm. Its runtime backs the core and its compiler only scopes component CSS.
- `@svelters/build` wires it together: `svelteConfig(root)` reads the toolkit from the app's `package.json` and compiles `src/App.svelte` into `dist/app.js`.

What the compiler accepts is on [Svelte support](/guides/svelte-support/). The earlier L1, stock Svelte compiled through a vendored fork of Svelte PR #18042 into a custom renderer with a DOM-shim fallback, is retired; its history is in `docs/notes/custom-renderer-trial.md` and `docs/notes/fork-bindings.md`.

## L2: the host core

`crates/svelters-host` embeds QuickJS-NG through `rquickjs` 0.14.0 (features `std` and `loader`).

- **Engine boundary.** `core.rs` is engine-agnostic behind a `JsEngine` trait. `quickjs.rs` is the QuickJS implementation, with a loader that accepts only `./` and `../` specifiers under the entry file's directory.
- **Handle table.** Handles are JS numbers (u64 below 2^53). The host keeps handle to kind ID, which scopes prop, signal, method and slot name resolution. Names resolve to IDs once and are cached.
- **Checkpoint.** Deliver queued events to JS, run Svelte effects and QuickJS jobs (bounded), apply the resulting operations through the vtable, and call `wake()` if work remains. A checkpoint requested while JS runs is deferred and becomes a later `wake`.
- **Limits.** `memory_limit` (default 256 MiB), `max_stack_size` (1 MiB), a `turn_budget` of 10 s per entry into JS.
- **Errors.** Uncaught exceptions and unhandled rejections go to `Host::take_errors()` with message and JS stack. Source-mapped errors arrive with M2.
- **Scripted mode.** `HostConfig::scripted` implements the fixture contract: virtual clock, deterministic steps. The log backend and the Node `TestHost` must produce the same op log for the same fixture.

JS engines other than QuickJS are covered on [Platforms](/guides/platforms/#js-engines).

## L3: the backend ABI

A fixed C ABI of about 15 functions separates the Rust host from every backend, whatever the backend's language. It never grows per widget, property or method: names resolve to `u32` IDs once, and per-ID behaviour lives in backend code. Summary:

| Direction | Functions |
| --- | --- |
| Host to backend (vtable) | `resolve`, `create`, `insert`, `remove`, `destroy`, `set_prop`, `get_prop`, `call`, `listen`, `run`, `wake`, `quit`, `last_error` |
| Backend to host (callbacks) | `post_event`, `checkpoint`, `call_sync`, `on_destroyed` |

Values cross as `svl_value`, a tagged union of null, bool, i64, f64, UTF-8 string, handle and list. Strings and lists are borrowed for the duration of a call. A stale or foreign handle is an error, never a crash. The full reference is [the ABI page](/concepts/abi/).

## L3: generated backends

Backends are generated from widget metadata. The generators are Python during P and move to Rust in M3, so that `build.rs` scripts become self-contained.

| | GTK4 | QtWidgets |
| --- | --- | --- |
| Language | Rust over `gtk4-rs` (0.11) | C++17, one translation unit generated per build |
| Generated parts | kind, prop, signal and slot tables, slot dispatch, verification tests, Rust into `crates/svelters-gtk/src/generated/`, `.d.ts` into `packages/native-runtime/types/` | ids and `resolve` tables, `create`, `set_prop`, `get_prop`, `call`, slot `insert`/`remove`, `listen` connectors, the verification file, `qt.d.ts` |
| Hand-written | handle table, value conversion, loop glue | handle table (`QPointer` plus a `destroyed` connection), error plumbing, vtable glue, `run`/`wake`/`quit`, test runtime |
| Build | ordinary crate; generated Rust is checked in | a project-owned **leaf crate** whose `build.rs` calls `svelters-qt-build`, compiles with `cc` and `pkg-config Qt6Widgets` |
| Dev invocation | tables now; dynamic GObject introspection for methods is M3 | `QMetaProperty` fallback for properties without direct accessors |

The leaf-crate rule: `build.rs` declares `rerun-if-changed` only on its own configuration, the metadata version and (in release) a bindings manifest. Nothing in an app's Svelte, TypeScript or Rust code rebuilds it. P5 verified this with `cargo build -vv`: touching `examples/counter/src/*` leaves `counter-qt` fresh.

## Metadata and codegen

Upstream machine-readable sources (GIR, PyGObject, qdoc, QMetaObject, PySide typesystem) are ingested into `metadata/<toolkit>/*.json` in schema `svelters-element-kind/v0`. Hand and agent annotations in `metadata/overrides/` add what no upstream source encodes: child slots, ownership, signal behaviour. Generated tests verify every annotated fact headless. See [Metadata and annotations](/guides/annotation-workflow/).

## Determinism

Generators and ingesters take only metadata JSON, overrides, templates and build config as input, and write byte-identical output on rerun. No clocks, environment or absolute paths appear in output. Each generated Qt file starts with an `inputs-digest` line. The fixture op logs (`expected.oplog`) are byte-exact: the Node `TestHost`, the Rust log backend and QuickJS must agree.

## Lanes and milestones

Four P lanes ran in parallel once the ABI was agreed, then integrated in P5 (counter on GTK and Qt from one bundle). Later milestones added two-way bindings, sync handlers, listen phases and child attributes, virtualized lists, menus, toplevels, styling, the canvas, the ABI v3 value model, the AppKit and WinUI slices and the native compiler. `STATUS.md` is the ledger: state, merge commit and evidence for each milestone.

Queued: M3 (generators ported to Rust, annotations at scale), M6 (release binding linker) and M7 (egui), among others.

## P5 measurements

Release build, Xvfb, shared 4-core machine, GTK 4.22.4 software GL, Qt 6.11.

| What | GTK4 | Qt6 |
| --- | --- | --- |
| Cold start to first frame | about 1.16 s (0.69 s with the cairo renderer) | about 0.20 s |
| Release binary, one toolkit | 3.40 MB | 2.56 MB |
| Peak RSS, counter | 204 MB | 55 MB |

At P5, with the since-retired renderer pipeline, the counter bundle was 147 KB unminified (36 KB gzip). On a 1000-row list, QuickJS time dominates: the host/ABI layer costs 16 to 18 ms for 11,000 operations. See [Lists and snippets](/guides/lists-and-snippets/).

Sources: `PLAN.md`, `STATUS.md`, `plans/architecture/backend-abi.md`, `docs/notes/native-compiler/runtime-contract.md`, `docs/notes/native-compiler/semantic-spec.md`, `packages/build/index.js`, `packages/native-runtime/vite.js`, `docs/notes/custom-renderer-trial.md` (history), `docs/notes/p1-js-lane.md`, `docs/notes/p2-host-lane.md`, `docs/notes/p3-gtk-lane.md`, `docs/notes/p4-qt-lane.md`, `docs/notes/p5-integration.md`, `docs/notes/quickjs-jit-cranelift.md`, `docs/notes/node-deno-integration.md`.
