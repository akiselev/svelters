# Runtime and build stack

This document defines the modernization boundary between build-time JavaScript/TypeScript work and the embedded runtime.

## What is being retired

The 2021 codebase has three overlapping compilation paths: a custom esbuild 0.13 Svelte plugin, `svelte-preprocess`/TypeScript 4-era scripts, and `lib/compiler`, which runs the Svelte compiler inside QuickJS and vendors an old SWC TypeScript strip transform. The old Qt host also manually polls `QApplication::processEvents()` and QuickJS pending jobs on a 10 ms Tokio interval.

All four patterns are retired.

## Build-time pipeline

```text
source .svelte/.ts/.js
        |
        v
Vite 8 + official Svelte plugin
        |
        +-- Svelte compiler: generate client, fragments tree, HMR in dev
        +-- vitePreprocess/Oxc: TS syntax lowering
        +-- Rolldown: resolve, bundle, DCE, native-binding externals
        +-- source maps
        |
        v
optimized ESM bundle/module graph
```

Use an ESM-only pnpm workspace. `package.json` pins the package manager through Corepack metadata. Vite's Node requirement is the minimum host requirement; prefer a current Node LTS in CI/dev containers.

Do not embed Node in the application. Node/Vite are build and development tools.

## TypeScript

Transpilation and type checking are separate jobs.

- Svelte accepts type-only TypeScript directly.
- For TS constructs that emit code, configure the official `vitePreprocess({ script: true })` path.
- Oxc/Vite performs isolated-file syntax transforms.
- `tsconfig.json` uses `verbatimModuleSyntax: true` and `isolatedModules: true` as recommended by Svelte.
- TypeScript 7 is the preferred `.ts` checker.
- `svelte-check` is authoritative for Svelte diagnostics. During the current TS7 transition, install the supported TS6 compatibility alias alongside TS7 and exercise the maintained `--tsgo` path. Remove TS6 once Svelte tooling no longer requires it.

Do not vendor TypeScript or SWC transforms into Rust. Native-call overload resolution may use a build-time TypeScript semantic service, but that is a tooling process/library concern and must not become part of the app runtime.

## Embedded JavaScript

Use upstream `rquickjs`/QuickJS-NG.

QuickJS responsibilities:

- evaluate already-compiled application/runtime ESM;
- run Svelte reactivity and event callbacks;
- own JS values, proxies, promises, and microtasks;
- expose narrow host modules such as `svelters:host`, `svelters:bindings`, and dev/HMR control.

QuickJS does not:

- parse/transpile TypeScript;
- compile Svelte;
- search `node_modules`;
- perform production tree shaking/minification;
- act as a security sandbox.

Release bytecode is optional and must be treated as trusted build output. Never accept arbitrary QuickJS bytecode from plugins or users.

## Module model

Build-time Vite/Rolldown owns Node/npm/package-exports resolution. The embedded runtime receives a deterministic module manifest:

```text
ModuleId -> source/bytecode/blob + imports + source-map key
```

Development uses versioned transformed modules delivered by the Vite environment. Release may use one bundle or an embedded module bundle, selected by measurement. No runtime `node-resolve` equivalent.

Pure platform-neutral npm libraries may be bundled normally. Browser-only packages fail with capability-oriented diagnostics instead of a growing fake-browser polyfill layer.

## Event-loop ownership

QuickJS is pinned to the UI thread. Each backend owns its native loop.

A host checkpoint performs:

1. deliver queued native events to JS;
2. run Svelte/effect work;
3. drain a bounded number of QuickJS pending jobs;
4. flush graph patches;
5. return control to the toolkit loop.

Additional jobs schedule another toolkit wakeup. Never busy-poll.

- GTK: wake/integrate through GLib main-context facilities.
- QtWidgets: QApplication owns `exec`; use posted events/QTimer/QSocketNotifier-style wakeups rather than replacing the dispatcher.
- egui: finish the `Ui` borrow, dispatch queued events, then call `request_repaint` as necessary.

Timers, animation frames, and host async completions share a scheduler abstraction. Tokio may power background I/O/services, but it does not own the UI loop.

## Safety and limits

Configure QuickJS memory and stack budgets, an interrupt/time-budget mechanism for runaway JS, rejection tracking, uncaught-exception reporting, and explicit shutdown ordering. Test GC/native-handle cycles and callback-after-destroy cases.

## Source maps and diagnostics

Development/test builds always emit source maps. Maintain a source-map registry keyed by runtime module ID. Translate QuickJS stacks and Svelte runtime diagnostics back through Rolldown/Oxc/Svelte maps to original `.svelte`/`.ts` positions.

Native binding errors add the canonical native symbol and toolkit version. The final diagnostic should identify both the Svelte source callsite and backend failure.

## Rust workspace modernization

- edition 2024 and workspace resolver 3;
- pinned stable `rust-toolchain.toml`;
- workspace dependency versions/features centralized;
- narrow Tokio features instead of `full` where possible;
- `cargo nextest`, clippy, rustfmt;
- `cargo-deny`/license/advisory policy;
- `cargo-bloat`/link-map baselines for representative release applications;
- ASan/UBSan for generated Qt C++ and FFI test jobs; Miri where practical.

Keep the minimum supported Rust version undefined until the architecture settles; pin the implementation toolchain first and establish an MSRV before the first public compatibility promise.
