# SVR05 - QuickJS-NG execution host, application storage, and event loop

References: `plans/architecture/runtime-stack.md` and `plans/architecture/application-storage.md`.

## Goal

Replace the local `rquickjs` 0.1.x fork and embedded compiler task with a current, execution-only QuickJS-NG host that runs SVR04 output correctly, integrates with native UI loop semantics, and establishes validated appfs-backed application storage before HMR and higher-level platform services begin accumulating ad hoc filesystem behavior.

## QuickJS/runtime work

- Adopt current upstream `rquickjs`/QuickJS-NG.
- Inventory old fork deltas before deleting the submodule. Upstream or locally isolate only capabilities that are still demonstrably required.
- Use a synchronous QuickJS runtime/context pinned to the UI thread unless measurement proves the async wrapper necessary.
- Implement deterministic module IDs, build-manifest resolver/loader, embedded release module support, and development versioned-module loading.
- Implement native host modules for graph operations, event dispatch, binding dispatch, console/tracing, timers, HMR control, and storage capabilities.
- Add a bounded pending-job pump for promises/microtasks. Running another JS job schedules a toolkit wakeup instead of a busy loop.
- Define host scheduler APIs for timeout/interval/animation-frame-style callbacks without relying on a browser.
- Configure stack/memory limits, interrupt/time budget, promise rejection tracking, uncaught exception hooks, and runtime memory metrics.
- Translate QuickJS stack frames through the source-map registry to original `.svelte`/`.ts` positions.

## Appfs validation and integration

Appfs is the default Svelters application-storage implementation, not an SVR21 optional extra. Before pinning it here, run the validation gate in `plans/architecture/application-storage.md`: compile it, run its provider conformance suite, black-box probe, and crash-recovery stress tests, and land required generic fixes upstream in appfs rather than carrying a private fork in Svelters.

Create a small Svelters storage service/crate boundary (working name `svelters-storage`) backed by:

- `appfs-native` for normal desktop hosts;
- `appfs-memory` for TestHost/headless tests.

Expose a Promise-based `@svelters/storage` host module for semantic Config/Data/State/Cache/Runtime/Temp/Logs/Resources/External access. The native host owns the `AppFs`; JavaScript receives semantic/capability-scoped operations, not arbitrary ambient absolute-path filesystem access.

Reserve `.svelters/` inside framework-owned spaces. At minimum, make Runtime available for local session/control metadata, State for deliberate recovery/checkpoint data, Cache for regenerable host data, Logs for optional file logging, and Resources for packaged immutable assets/module/source-map resources where the build profile uses appfs-backed resources.

Durable I/O is asynchronous from JavaScript and must not block the UI thread. Storage completion events re-enter JavaScript only through the same queued host checkpoint/event mechanism as other asynchronous work.

Do not move Artifactum's reproducible build artifacts into appfs; do not put Switchyard live generations in appfs; do not add secrets/keychain semantics to the storage API.

## Shutdown ordering

Define explicit shutdown ordering:

1. stop accepting new native UI/events and new application storage requests;
2. cancel host tasks/timers and drain or cancel pending QuickJS jobs according to policy;
3. invalidate callbacks/handles that may re-enter JS;
4. complete/abort outstanding storage operations and flush framework/app state required by the shutdown contract;
5. destroy the QuickJS context/runtime;
6. release storage/provider resources and then backend resources as appropriate.

The exact native backend shutdown integration can be refined later, but no durable write may depend on a JS callback after the JS runtime has been destroyed.

## Security rule

QuickJS is not a sandbox. Release bytecode is trusted local build output only. Never ingest arbitrary bytecode from plugins/network as data. The storage module is capability-scoped and is not a substitute for a JavaScript sandbox.

## Acceptance

- SVR04 fixture corpus runs inside QuickJS, not Node.
- Promises, queued native events, timers, storage completions, and Svelte ticks execute without polling loops.
- A runaway JS fixture is interruptible and memory limits are tested.
- A deliberately thrown error in a nested `.svelte`/TS call reports the original source location.
- The active workspace no longer depends on `lib/rquickjs` or `lib/compiler`.
- appfs passes its required validation protocol at the pinned revision/version; fixes needed for Svelters are upstreamed to appfs rather than copied locally.
- a headless QuickJS fixture using `appfs-memory` reads packaged Resources, writes/reads Config and State, discards Cache without affecting durable state, and cannot obtain an undeclared ambient absolute path;
- a native smoke test persists Config/State across process restart and uses Runtime/Logs without hard-coded platform paths;
- storage operations do not block the UI thread and shutdown leaves no required durable framework state unflushed.
