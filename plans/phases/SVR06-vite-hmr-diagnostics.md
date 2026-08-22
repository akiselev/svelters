# SVR06 - embedded Vite environment, HMR, and diagnostics

References: `plans/architecture/runtime-stack.md`, `plans/architecture/ecosystem-integrations.md`, and `plans/architecture/application-storage.md`.

## Goal

Make Svelters development feel like Svelte development: `.svelte`/`.ts` changes update a running native application without Cargo/C++ rebuilds, while runtime errors map back to original source and development state uses the storage substrate established in SVR05 rather than ad hoc platform paths.

## Work

Build `@svelters/vite` around Vite 8's Environment/plugin APIs rather than emulating a browser dev client.

Development flow:

```text
file change
 -> Vite/Svelte transform
 -> versioned ESM module + source map
 -> Svelters dev transport
 -> QuickJS evaluate
 -> Svelte HMR update
 -> graph patches
 -> backend repaint/mutations
```

Implement the simplest local authenticated dev transport that satisfies the requirements: Unix socket/named pipe preferred, loopback TCP fallback. It has protocol versioning, module update messages, error reporting, runtime logs, full reload, and health/ping. Do not expose an unauthenticated network listener by default.

Use the appfs-backed Runtime space for Svelters-owned ephemeral session/control metadata and Logs for optional persistent dev/runtime logs. Do not scatter socket paths, session files, source-map registries, or reconnect metadata across direct platform-directory calls. Regenerable transformed-module caches that belong to the running application rather than the reproducible build pipeline use Cache; reproducible build products remain Artifactum's responsibility.

Do not introduce a resident daemon into this phase. `daemonkit` is deliberately deferred to SVR21 and is adopted only if measurements later show that a persistent `svelters-devd` materially improves startup, cache reuse or multi-app development.

Preserve Svelte's upstream HMR wrapper and adapt only node-anchor operations required by the native runtime.

Integrate `svelte-check`/TypeScript diagnostics into the dev command without blocking every transform. Compiler errors, runtime errors, storage errors, and native binding errors share source-location formatting.

Add an HMR performance harness measuring file-event-to-first-native-patch latency and state preservation.

## Acceptance

- edit text, structure, event callback, and rune state code in a running TestHost app with no native rebuild;
- component-local HMR preserves state where upstream Svelte HMR semantics say it should;
- syntax/type/runtime errors point at original `.svelte`/`.ts` source;
- dev server restart and app reconnect recover cleanly;
- runtime/control metadata is observable under the configured appfs Runtime space and no Svelters-specific hard-coded platform path is introduced;
- clearing Runtime/Cache recovers cleanly without deleting durable Config/State;
- no daemon/supervisor dependency is needed for the baseline workflow;
- recorded median HMR latency becomes a regression benchmark.
