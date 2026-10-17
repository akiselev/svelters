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
