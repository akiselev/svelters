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

## Design notes and findings (later)

- Snapshot vs call_sync: the native model holds ids; `splice` is the only path that changes it. `call_sync` is used only for row lifecycle (`setup/bind/unbind/teardown`). Our own `splice` makes GTK unbind removed rows inside the host op, so those arrive deferred (host answers busy); `gen` makes a late unbind harmless.
- The first GTK layout happens inside `present()` while JS runs, so every row event is deferred and the view sees empty rows. A placeholder child of `rowHeight` per slot keeps the first pool bounded.
- Svelte: an effect must not write state it reads. The controller reads items and keys tracked, then does everything else in `untrack`; row cells are written without `flushSync` inside effects, with `flushSync` in the bind handler (outside effects).
- `$state` deep proxies make a 10k-item key pass slow; the fixture uses `$state.raw`.
- QuickJS compiled at dev opt-level 0 is 5-10x slower and 100k fills hit the 10 s turn budget; root Cargo.toml now optimises `rquickjs-sys` in dev.
- Qt: `slots` is a Qt macro (use another member name in hand-written C++); static string attributes arrive as text (`minHeight="300"`) because Qt has no prop descriptors yet, so the hand-written props accept numeric/boolean text.
- Qt strategies: `setIndexWidget` (Qt deletes the old widget, no recycling; every call goes through persistent editors) was not built; a painting delegate cannot host Svelte rows; the viewport pool binds only rows entering the window. Selection works for clicks on non-interactive row content because unhandled mouse presses propagate to the viewport.
- Sort/filter: JS-side, see the plan. A native comparator needs n log n `call_sync` round trips and cannot run while JS is busy.
- Tooling: the agent shell rejects long inline python; scripts go through files.
