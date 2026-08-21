# SVR05 - QuickJS-NG execution host, module runtime, and event loop

References: `plans/architecture/runtime-stack.md`.

## Goal

Replace the local `rquickjs` 0.1.x fork and embedded compiler task with a current, execution-only QuickJS-NG host that runs SVR04 output correctly and integrates with native UI loop semantics.

## Work

- Adopt current upstream `rquickjs`/QuickJS-NG.
- Inventory old fork deltas before deleting the submodule. Upstream or locally isolate only capabilities that are still demonstrably required.
- Use a synchronous QuickJS runtime/context pinned to the UI thread unless measurement proves the async wrapper necessary.
- Implement deterministic module IDs, build-manifest resolver/loader, embedded release module support, and development versioned-module loading.
- Implement native host modules for graph operations, event dispatch, binding dispatch, console/tracing, timers, and HMR control.
- Add a bounded pending-job pump for promises/microtasks. Running another JS job schedules a toolkit wakeup instead of a busy loop.
- Define host scheduler APIs for timeout/interval/animation-frame-style callbacks without relying on a browser.
- Define shutdown ordering: stop accepting native events, cancel host tasks, drain/cancel jobs, invalidate callbacks/handles, destroy context/runtime, then backend resources as appropriate.
- Configure stack/memory limits, interrupt/time budget, promise rejection tracking, uncaught exception hooks, and runtime memory metrics.
- Translate QuickJS stack frames through the source-map registry to original `.svelte`/`.ts` positions.

## Security rule

QuickJS is not a sandbox. Release bytecode is trusted local build output only. Never ingest arbitrary bytecode from plugins/network as data.

## Acceptance

- SVR04 fixture corpus runs inside QuickJS, not Node.
- Promises, queued native events, timers, and Svelte ticks execute without polling loops.
- A runaway JS fixture is interruptible and memory limits are tested.
- A deliberately thrown error in a nested `.svelte`/TS call reports the original source location.
- The active workspace no longer depends on `lib/rquickjs` or `lib/compiler`.
