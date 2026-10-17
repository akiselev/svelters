# P1 - JS lane

Owns `packages/`, `fixtures/` and root JS config.

## Work
- Vite 8 + `@sveltejs/vite-plugin-svelte` library build of a fixture app to a single ESM bundle:
  - `compilerOptions: { fragments: "tree" }`, browser export condition;
  - no `{@html}`;
  - source maps on;
  - output loadable by a plain ES-module evaluator. No Node built-ins in the bundle.
- `@svelters/dom`:
  - `Node`/`Element`/`Text`/`Comment`/`DocumentFragment` with `firstChild`/`nextSibling` getters on `Node.prototype`;
  - `document`, `window`, `navigator.userAgent`, a `HTMLMediaElement` stub;
  - `before`/`append`/`appendChild`/`insertBefore`/`remove`/`removeChild`/`cloneNode`/`textContent`;
  - attributes and property setters;
  - `addEventListener` and `composedPath()` up to `document`.
  - Start from the scratchpad shim described in `docs/notes/svelte5-custom-renderer.md`.
- Mapping to host ops (`plans/architecture/backend-abi.md#js-side-host-ops`):
  - lowercase tag → kind, e.g. `<button>`, `<box>`, `<window>`;
  - attributes/props → `setProp`;
  - `on*` → `listen`;
  - insertion under a native-backed parent → `create` (lazily) + `insert`;
  - text nodes inside a label-like kind → the `label` prop;
  - comment anchors stay JS-only and never reach the host.
- `@svelters/testhost`: a JS `SveltersHost` that records an op log and can inject events (`__svelters_dispatch`).
- Fixtures:
  - `counter`: window > box > label + button, click increments;
  - `keyed-list`: entry + add button + keyed `{#each}` of rows with remove buttons, plus reorder;
  - `if-toggle`.
  - Each has a scripted interaction and an expected op-log snapshot.

## Acceptance
- `pnpm test` runs all fixtures in Node through the TestHost. The op-log snapshots are stable, and the scripted interactions produce the expected ops.
- The bundle has no references to Node built-ins and runs with only the shim's globals defined.

## Completion record

Versions: svelte 5.57.2, vite 8.3.4, @sveltejs/vite-plugin-svelte 7.3.1, pnpm 10.33.0, Node 22.22.2. Contract for P2 and mapping table: `docs/notes/p1-js-lane.md`.

Commands run from the repo root:

```text
$ pnpm install
$ pnpm test            # = pnpm -r --workspace-concurrency=4 test; each fixture runs vite build, then svelters-test
fixtures/counter test: PASS counter (19 ops)
fixtures/keyed-list test: PASS keyed-list (87 ops)
fixtures/if-toggle test: PASS if-toggle (35 ops)
fixtures/timers test: PASS timers (9 ops)
```

- Bundles: `fixtures/<name>/dist/app.js`, 137-163 kB unminified each (counter 140010 B), source maps on, `fragments: "tree"`, `browser` condition, no `{@html}`.
- `svelters-test` rejects bundles containing `node:`, `require(`, `process.`, `Buffer`, top-level `import`/`export` or dynamic `import()`: all four bundles pass. The runner evaluates each bundle with `vm.SourceTextModule` in a context that has only `console`, `queueMicrotask`, virtual-time `setTimeout`/`setInterval`/`clear*` and `__svelters_host`.
- Op-log snapshots are `fixtures/<name>/expected.oplog`; scripts are `fixtures/<name>/script.json`. Known unsupported: see "Not done" in the notes.
