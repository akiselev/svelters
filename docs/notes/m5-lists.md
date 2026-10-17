# M5 lists: working notes (lane X5)

Plan: `plans/milestones/M5-lists.md`. Started on master 3070367.

## Log

- Branch already contained master 3070367 (fast-forward merge of the status commit).
- Workspace had no `node_modules`: `pnpm install --offline` works; adding `packages/lists` and `fixtures/virtual-list` changed `pnpm-lock.yaml` (`--no-frozen-lockfile`).
- JS side first (backend independent): `packages/lists` (`diff.js`, `controller.svelte.js`, `RowHost.svelte`, `ListView.svelte`), renderer `outlet()`/`releaseOutlet()`/`WNode.setProp`, TestHost prints `svlHandle` args as the bare handle number (same as the log backend).
- `fixtures/virtual-list` runs the whole protocol on the Node TestHost with scripted `sync` setup/bind/unbind/teardown steps.

## Findings

- Handlers of async signals get `(...args)`, handlers of sync signals (and deferred deliveries of them) get `(event, ...args)`. So `setup/bind/unbind/teardown` take the event first; `activate/select` do not.
- The fixture check rejected bundles whose Svelte runtime JSDoc contains `node:` or `import(`; the Svelte functions `mount`/`flushSync` pull those comments in. The `node:` check now looks for a quoted specifier. A `import(` in my own JSDoc was removed; keep JSDoc free of `import(` in packages that get bundled.
