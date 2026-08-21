# Svelters v2 resurrection plan

Status: adopted execution plan

Svelters v2 is a Svelte 5 native application runtime. Normal application code remains Svelte/TypeScript, executes in embedded QuickJS-NG, and renders through pluggable native backends. The first supported backends are GTK4, QtWidgets, and egui. The design must retain the full native-toolkit escape hatch without recreating the old all-bindings-generated-into-Rust binary and compile-time explosion.

This plan replaces the 2021 architecture rather than incrementally porting it. The old implementation remains historical evidence and a behavioral fixture corpus.

## Core architecture

```text
.svelte / .ts
     |
     v
stock Svelte 5 + Vite 8 / Rolldown / Oxc
     |
     +---- native-call lowering ----> live binding manifest after JS DCE
     |
     v
@svelters/runtime
Svelte reactivity + logical native Node substrate
     |
     v
Svelters UI graph + batched patches + event queue
     |
     v
Switchyard renderer service / native fast path
     |
     +----------------+----------------+
     v                v                v
   GTK4           QtWidgets           egui
GObject/GI       metaobject +       immediate-mode
+ thin FFI       thin C++ thunks      traversal
```

The compiler is not forked initially. Svelte 5 already supports `generate: "client"`, `fragments: "tree"`, and HMR. We reuse upstream reactivity and maintain a small renderer-runtime overlay for the native logical node model. A compiler patch is permitted only after a measured incompatibility demonstrates that runtime adaptation is worse than a small compiler hook.

## Modern toolchain decisions

The old repository contains Svelte 3.44, TypeScript 4.3, esbuild 0.13, a hand-written esbuild Svelte plugin, an embedded Svelte compiler running inside QuickJS, a vendored/forked SWC TypeScript stripping pass, `rquickjs` 0.1.2, Node-12-era build scripts, and Rust 2018-era crates. None of these are retained as the v2 build pipeline.

The v2 baseline is:

- Svelte 5 pinned exactly in the lockfile, initially current 5.x; candidate updates pass the SVR22 compatibility gate before landing directly on `master`.
- `@sveltejs/vite-plugin-svelte` and Vite 8 with Rolldown/Oxc. No custom esbuild Svelte plugin.
- ESM-only JS packages. No hand-written ESM-to-CJS rewriting.
- Oxc/Vite handles syntax-only JS/TS transforms. Svelte's `vitePreprocess({ script: true })` handles TS constructs that emit JS when needed.
- Type checking is separate from transpilation. Use TypeScript 7 for `.ts` checking and `svelte-check` for `.svelte` diagnostics. While Svelte tooling still requires it, keep a TypeScript 6 npm alias alongside TypeScript 7 and run the supported `svelte-check --tsgo` path in CI.
- Current upstream `rquickjs`/QuickJS-NG, not the local `lib/rquickjs` fork, unless SVR05 finds a concrete missing capability that requires an upstreamable patch.
- QuickJS is execution-only. It does not compile Svelte or TypeScript and does not perform npm resolution.
- Build-time module resolution, dependency optimization, tree shaking, minification, CSS preprocessing, and source maps belong to Vite/Rolldown/Oxc.
- Runtime module loading consumes a build manifest or embedded bundle. Release bytecode is trusted build output only.
- Toolkit event loops remain authoritative. QuickJS promise jobs are pumped at explicit host checkpoints; do not poll `QApplication::processEvents()` every N milliseconds and do not replace Qt's dispatcher with libuv.
- Rust workspace moves to edition 2024, workspace resolver 3, a pinned stable toolchain, `cargo nextest`, clippy, rustfmt, sanitizers where applicable, and explicit build/size benchmarks.
- pnpm workspace with a pinned `packageManager` field replaces the old Yarn v1 layout.
- Source maps are mandatory in development and test builds, including mapping embedded QuickJS exceptions back to `.svelte`/`.ts` sources.

See `plans/architecture/runtime-stack.md`.

## Architectural invariants

1. The logical UI graph is canonical. Native widget instances are backend realizations and may be rebuilt.
2. No raw toolkit pointer crosses into JavaScript. JS receives stable handles/proxies.
3. No ordinary `.svelte` or `.ts` edit invokes Cargo or a C++ compiler in development.
4. Adding a statically resolvable native API call does not rebuild the fat development bridge.
5. Release builds generate only native bindings that survive JS tree shaking plus explicit manifest roots and their converter dependencies.
6. GTK, QtWidgets, and egui consume the same graph/API/binding abstractions; backend-specific semantics are represented explicitly rather than hidden in generic DOM shims.
7. Switchyard owns renderer/plugin generation publication and lifetime. Its generic byte RPC is a compatibility boundary, not the high-frequency UI mutation path.
8. All expensive generated API databases and binding packs are content-addressed and reproducible; Artifactum is the preferred cache/provenance layer.
9. Shiboken/PySide is a build-time Qt knowledge source, never a Svelters runtime dependency.
10. Dynamic native calls are explicit and auditable; broad wildcard roots are warned on in release builds.
11. Cross-project integration happens at explicit semantic interfaces. Svelters does not absorb another project's runtime merely because both are desktop-adjacent.
12. `master` is the execution branch. Phase and routine compatibility work lands directly after its acceptance gate; GitHub PRs are not part of the execution protocol.

## Q1: native binding linker

The detailed design is in `plans/architecture/native-binding-linker.md`. Native member references are lowered to stable virtual binding imports before bundling. Rolldown performs ordinary JS DCE first. Surviving external binding imports become the release native root set. The packager then computes converter/type dependencies, assigns dense per-build IDs, and generates only those native calls. Dynamic calls use explicit `svelters.toml` roots and can be traced during development.

## Q2: canonical API IR and Shiboken

The canonical API IR is defined in `plans/architecture/native-api-ir.md`. Qt-specific extraction and licensing boundaries are in `plans/architecture/qt-shiboken.md`. We reuse Shiboken ApiExtractor/PySide semantic knowledge at build time, export a neutral Svelters API database, and generate our own C++ thunk ABI. We do not generate Python bindings and do not link Shiboken into the shipped runtime.

## Phase index

Each phase has exactly one implementation-plan file under `plans/phases/`. Agents should load this file, `plans/STATUS.md`, the current phase, and only the architecture documents explicitly referenced by that phase.

| Phase | Goal | Depends on |
| --- | --- | --- |
| SVR00 | Archaeology, preservation, measurable baseline | none |
| SVR01 | Rust/JS workspace and dependency modernization | SVR00 |
| SVR02 | Vite/Oxc/TypeScript/Svelte build pipeline | SVR01 |
| SVR03 | Logical UI graph and TestHost | SVR01 |
| SVR04 | Svelte 5 native client-runtime overlay | SVR02, SVR03 |
| SVR05 | QuickJS-NG execution host, event loop, source maps | SVR02, SVR04 |
| SVR06 | Embedded Vite environment, HMR, diagnostics | SVR05 |
| SVR07 | Canonical native API IR and generated typings | SVR03 |
| SVR08 | Native binding linker and release manifest | SVR02, SVR07 |
| SVR09 | GTK4 API extraction and dynamic invocation | SVR07 |
| SVR10 | GTK4 renderer and thin release bindings | SVR05, SVR08, SVR09 |
| SVR11 | Qt/Shiboken API extraction | SVR07 |
| SVR12 | QtWidgets renderer and selective C++ thunk generator | SVR05, SVR08, SVR11 |
| SVR13 | Portable widget semantics and structural policies | SVR10, SVR12 |
| SVR14 | egui renderer and immediate-mode adaptation | SVR05, SVR13 |
| SVR15 | Switchyard renderer/plugin generations and shared hot-reload substrate | SVR10, SVR12, SVR14 |
| SVR16 | Model/view subsystem and virtualization | SVR13, SVR15 |
| SVR17 | Styling and native style escape hatches | SVR13, SVR15 |
| SVR18 | Broad native API escape hatch, callbacks, async | SVR08, SVR15 |
| SVR19 | Custom widget/plugin SDK and egui-workbench dogfood | SVR15, SVR18 |
| SVR20 | Devtools, graph/native inspector, accessibility/agent E2E | SVR06, SVR15 |
| SVR21 | Platform services, Artifactum/Outboard tooling, optional dev daemon, packaging | SVR15, SVR18 |
| SVR22 | Upstream Svelte/toolkit compatibility automation | SVR20, SVR21 |
| SVR23 | Production hardening, deterministic stress, release readiness | all prior |

SVR02 and SVR03 can run in parallel after SVR01. SVR09 and SVR11 can run in parallel after SVR07. Qt and GTK renderer work may run in parallel once SVR08 exists. egui deliberately lands after both retained backends have established the portable structural contract so it can challenge, not accidentally define, that contract.

## Agent execution protocol

- There are no implementation PRs. Work from the latest `master`; a local temporary branch/worktree is fine for isolation, but accepted work is fast-forwarded or committed directly to `master` after the phase gate.
- Default to one active phase at a time. Parallel phases are allowed only where the dependency graph explicitly says so and the agents coordinate disjoint files/APIs.
- Before coding, mark the phase `IN PROGRESS` in `plans/STATUS.md` if the status update can be committed without disrupting another active phase.
- Read only the current phase plus referenced architecture files. If another phase contains needed information, add an explicit cross-reference rather than loading the entire plan tree.
- Preserve the public behavior of completed phases. A later phase may evolve an API only with migration in the same master commit or contiguous validated commit series.
- Run every acceptance command before landing the completion commit. Unit tests alone are insufficient when the phase requires end-to-end or observational validation.
- Append a concise `## Completion record` to the current phase file with exact commands, fixture counts, benchmark deltas, generated artifact hashes and known unsupported cases. Put large evidence in Artifactum/CI and link its identity from the record.
- Mark the phase `DONE` only in the commit that records its accepted implementation, and put that master commit SHA plus a short evidence summary in `plans/STATUS.md`.
- If research invalidates an architectural invariant, update the relevant architecture document, current phase, and PLAN as part of the validated master change. Do not silently route around the design.
- A failed candidate never mutates `master`; keep the last green master and preserve diagnostics/artifacts outside the branch until the problem is resolved.
- Keep generated output out of git unless a phase explicitly identifies it as a checked-in compatibility fixture. Prefer Artifactum/CAS for large derived data.
- Do not revive the old `lib/rquickjs`, `lib/svelte`, old SWC strip fork, old esbuild plugin, or Qt libuv dispatcher as foundations. They are archaeology inputs only.

## Cross-project responsibilities

See `plans/architecture/ecosystem-integrations.md` for the detailed boundaries.

- `switchyard`: shared generation-safe native/static/process/Wasm plugin lifecycle. Generic staged publication, generation identity, uninstall, watch/build diagnostics and the optional native extension mechanism belong there rather than in Svelters.
- `egui-workbench`: design sibling and dogfood consumer, not a core dependency. Align stable widget identity, last-known-good publication, deferred events and Switchyard substrate; later mount Svelters inside a Workbench pane.
- `appfs`: optional application-platform storage service after its own compile/black-box validation. Use it for semantic Config/Data/State/Cache/Logs/Resources/External storage rather than inventing another filesystem facade.
- `desktop-cli`: GPL external black-box accessibility/agent test harness only; never link it into the permissive Svelters core.
- `deterministic-rs`: optional late-stage replay/fault testing for timers, event ordering and reload races; not a production dependency or early critical path.
- `daemonkit`: optional lifecycle owner for a resident development helper if measurement justifies one; not the renderer or HMR substrate.
- `artifactum`: reproducible storage for extracted API databases, generated typings, fat dev bridges, thin binding packs and compatibility evidence.
- `outboard`: independently installed build/extractor/packager tooling plugins. It is not the renderer hot path.

## Research snapshot, 2026-08-21

- Svelte 5.56.x is current; `fragments: "tree"` is a first-party compile option since 5.33.
- Vite 8 is Rolldown-powered and uses Oxc across the toolchain; Vite 8.1 is current in this planning window.
- `@sveltejs/vite-plugin-svelte` 7.x is the official integration.
- TypeScript 7 is the stable native Go implementation; `svelte-check` currently exposes a TS7/tsgo path that still requires a TS6 compatibility installation.
- `rquickjs` 0.12.2 targets QuickJS-NG and is current as of July 2026.

Primary references:

- https://svelte.dev/docs/svelte/typescript
- https://github.com/sveltejs/svelte
- https://vite.dev/blog/announcing-vite8
- https://vite.dev/guide/
- https://devblogs.microsoft.com/typescript/announcing-typescript-7-0/
- https://github.com/sveltejs/language-tools/tree/master/packages/svelte-check
- https://docs.rs/rquickjs/latest/rquickjs/
- https://github.com/quickjs-ng/quickjs
- https://doc.qt.io/qtforpython-6/shiboken6/
- https://docs.gtk.org/girepository/
- https://docs.rs/egui/latest/egui/
