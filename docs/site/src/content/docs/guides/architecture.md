---
title: Architecture
description: The layers from Vite build to native toolkit, the backend C ABI, and the milestone lanes with their current state.
---

Svelters v2 runs stock Svelte 5 components in embedded QuickJS-NG and renders them with native toolkits: GTK4 first, QtWidgets second, egui later. Application code is ordinary Svelte and TypeScript. The plan was adopted on 2026-10-16.

## Layers

```text
L0  build        Vite + stock Svelte 5 (generate: client, fragments: "tree")
                 -> ESM bundle (browser export condition) + source maps
                              |
L1  renderer     @svelters/dom: DOM-style shim (Node/Element/Text/Comment,
    adapter      document, window) -- OR -- a custom renderer via the Svelte
                 PR #18042 fork. Both sit behind the same host-op interface.
                              |  host ops: create / insert / remove / setProp /
                              |  listen / call / destroy; events come back queued
L2  host core    Rust (rquickjs/QuickJS-NG): module loading, job pump, timers,
                 console, source maps, handle table, event queue, hot reload
                              |  svelters backend C ABI (fixed, ~15 functions)
L3  backends     GTK4 (Rust/gtk4-rs)   QtWidgets (generated C++)   egui (Rust)
                              |
L4  toolkit      GTK4 / Qt 6 / egui
```

Widget metadata and codegen run alongside the layers. Upstream sources plus annotations produce element-kind tables, TypeScript typings, the Qt C++ dispatch code and the GTK method bindings.

| Layer | Where | Responsibility |
| --- | --- | --- |
| L0 | `packages/build` (`@svelters/build`) | Vite config for apps; pins Svelte, Vite and the Svelte plugin exactly |
| L1 | `packages/renderer` (`@svelters/renderer`), `packages/dom` (`@svelters/dom`) | Turn Svelte's tree operations into host ops |
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

## L1: the renderer fork

Svelte PR #18042 (custom renderer API) is the project's Svelte fork until it merges upstream.

- It is vendored as a tarball pinned in `vendor/svelte/PIN` (`5.57.2`, branch `custom-condition` at `f3de3f2`, the top of the #18042 stack).
- An agent tracks upstream weekly (`tests/renderer-conformance/WEEKLY.md`).
- The compile option is `experimental.customRenderer`. Components compile to calls on a renderer object (`createElement`, `insert`, `setAttribute`, `addEventListener`, ...). `css: 'injected'`, `on:` directives, `{@html}`, transitions and `svelte:window`/`document`/`body` are compile errors under it.
- Handler return values reach the host directly, because the PR does not delegate events.
- `@svelters/renderer` implements the renderer. Native widgets are created lazily, when a node first lands under a realized native parent. Fragments, comments and anchors stay JS-only. Child lists are linked sibling lists, so keyed reorders are linear.
- `@svelters/dom` is the fallback: a DOM-style shim over stock Svelte output (`fragments: "tree"`). Select it with `SVELTERS_BACKEND=shim`; the default is `SVELTERS_BACKEND=renderer`. Both paths produce byte-identical op logs for the fixtures.

The two stacked PRs the project depends on are #18461 (a tree-shakeable `custom_renderer` flag) and #18511 (the `custom-renderer` export condition). PR #18405 (interleaving renderers) is not needed functionally. Two bugs found in the fork are recorded in [Svelte support](/guides/svelte-support/#known-fork-bugs).

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
| Generated parts | kind, prop, signal and slot tables, slot dispatch, verification tests, `.d.ts` into `crates/svelters-gtk/src/generated/` and `packages/renderer/types/` | ids and `resolve` tables, `create`, `set_prop`, `get_prop`, `call`, slot `insert`/`remove`, `listen` connectors, the verification file, `qt.d.ts` |
| Hand-written | handle table, value conversion, loop glue | handle table (`QPointer` plus a `destroyed` connection), error plumbing, vtable glue, `run`/`wake`/`quit`, test runtime |
| Build | ordinary crate; generated Rust is checked in | a project-owned **leaf crate** whose `build.rs` calls `svelters-qt-build`, compiles with `cc` and `pkg-config Qt6Widgets` |
| Dev invocation | tables now; dynamic GObject introspection for methods is M3 | `QMetaProperty` fallback for properties without direct accessors |

The leaf-crate rule: `build.rs` declares `rerun-if-changed` only on its own configuration, the metadata version and (in release) a bindings manifest. Nothing in an app's Svelte, TypeScript or Rust code rebuilds it. P5 verified this with `cargo build -vv`: touching `examples/counter/src/*` leaves `counter-qt` fresh.

## Metadata and codegen

Upstream machine-readable sources (GIR, PyGObject, qdoc, QMetaObject, PySide typesystem) are ingested into `metadata/<toolkit>/*.json` in schema `svelters-element-kind/v0`. Hand and agent annotations in `metadata/overrides/` add what no upstream source encodes: child slots, ownership, signal behaviour. Generated tests verify every annotated fact headless. See [Metadata and annotations](/guides/annotation-workflow/).

## Determinism

Generators and ingesters take only metadata JSON, overrides, templates and build config as input, and write byte-identical output on rerun. No clocks, environment or absolute paths appear in output. Each generated Qt file starts with an `inputs-digest` line. The fixture op logs (`expected.oplog`) are byte-exact: the Node `TestHost`, the Rust log backend and QuickJS must agree.

## Lanes and milestones

Four P lanes ran in parallel once the ABI was agreed, then integrated in P5. State as of `STATUS.md`:

| Milestone | State | Evidence |
| --- | --- | --- |
| P1 JS lane | DONE | 4 fixtures (19/87/35/9 ops) pass in the Node TestHost |
| P1b renderer fork | DONE | renderer and shim produce identical op logs; conformance node equals QuickJS |
| P2 host lane | DONE | 21 of 21 tests; fixtures byte-identical in QuickJS |
| P3 GTK lane | DONE | 443 Gtk kinds ingested; 298 kinds, 1304 props, 349 signals in tables; 22 tests plus P1 oplog replay |
| P4 Qt lane | DONE | 192 classes ingested; 8 elements annotated; 41 generated tests; leaf-crate rebuild verified |
| P5 integration | DONE | counter on GTK and Qt; 57 of 57 nextest including 12 integration tests; first frame GTK about 1.2 s, Qt about 0.2 s |
| F1 fork bindings | IN PROGRESS | phase A: `bind:this` and a binding hook in the fork; phase B after P5 |
| D1 docs generator | IN PROGRESS | generated API reference and this site |
| S1 serdes main types | IN PROGRESS | ABI v3 value model and converters; merges after P5 |
| M2 dev loop | QUEUED | ABI v2, native timers, Vite watch reload without Cargo, source-mapped errors |
| M3 metadata and codegen | QUEUED | Python generators ported to Rust, annotations at scale, GTK dynamic invocation, full `.d.ts` |
| M4 native semantics | QUEUED | named child slots, sync handlers and return values, two-way binding, ownership |
| M5 lists and model-view | QUEUED | virtualized lists (`GListModel`, `QAbstractItemModel`) |
| M6 release binding linker | QUEUED | tree-shaken native roots to thin generated bindings |
| M7 egui | QUEUED | egui backend |

Later: accessibility, devtools, packaging, upstream-compat automation, ecosystem integrations.

## P5 measurements

Release build, Xvfb, shared 4-core machine, GTK 4.22.4 software GL, Qt 6.11.

| What | GTK4 | Qt6 |
| --- | --- | --- |
| Cold start to first frame | about 1.16 s (0.69 s with the cairo renderer) | about 0.20 s |
| Release binary, one toolkit | 3.40 MB | 2.56 MB |
| Peak RSS, counter | 204 MB | 55 MB |

The bundle for the counter is 147 KB unminified (36 KB gzip). On a 1000-row list, QuickJS time dominates: the host/ABI layer costs 16 to 18 ms for 11,000 operations. See [Lists and snippets](/guides/lists-and-snippets/).

Sources: `PLAN.md`, `STATUS.md`, `plans/architecture/backend-abi.md`, `docs/notes/custom-renderer-trial.md`, `docs/notes/svelte5-custom-renderer.md`, `docs/notes/p1-js-lane.md`, `docs/notes/p2-host-lane.md`, `docs/notes/p3-gtk-lane.md`, `docs/notes/p4-qt-lane.md`, `docs/notes/p5-integration.md`, `docs/notes/quickjs-jit-cranelift.md`, `docs/notes/node-deno-integration.md`.
