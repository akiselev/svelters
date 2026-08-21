# SVR06 - embedded Vite environment, HMR, and diagnostics

References: `plans/architecture/runtime-stack.md`.

## Goal

Make Svelters development feel like Svelte development: `.svelte`/`.ts` changes update a running native application without Cargo/C++ rebuilds, while runtime errors map back to original source.

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

Implement a local authenticated dev transport (Unix socket/named pipe preferred; loopback TCP fallback) with protocol versioning, module update messages, error reporting, runtime logs, full reload, and health/ping. Do not expose an unauthenticated network listener by default.

Preserve Svelte's upstream HMR wrapper and adapt only node-anchor operations required by the native runtime.

Integrate `svelte-check`/TypeScript diagnostics into the dev command without blocking every transform. Compiler errors, runtime errors, and native binding errors share source-location formatting.

Add an HMR performance harness measuring file-event-to-first-native-patch latency and state preservation.

## Acceptance

- edit text, structure, event callback, and rune state code in a running TestHost app with no native rebuild;
- component-local HMR preserves state where upstream Svelte HMR semantics say it should;
- syntax/type/runtime errors point at original `.svelte`/`.ts` source;
- dev server restart and app reconnect recover cleanly;
- recorded median HMR latency becomes a regression benchmark.
