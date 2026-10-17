# P1 JS lane: working notes

Svelte is the vendored custom-renderer fork (`vendor/svelte/PIN`, `5.57.2` from `custom-condition` @ `f3de3f2`), Vite `8.3.4`, `@sveltejs/vite-plugin-svelte` `7.3.1` (all pinned exactly). Node 22 only for the test runner.

**Two build paths, one host contract.** `SVELTERS_BACKEND=renderer` (default) compiles components with `experimental.customRenderer` and runs on `@svelters/renderer`; `SVELTERS_BACKEND=shim` builds the stock-style DOM output (`fragments: "tree"`) on `@svelters/dom`. Both produce the same `dist/app.js` contract and byte-identical op logs for all four fixtures. Everything below that talks about "the shim" describes `@svelters/dom`; the renderer implements the same mapping table, see "P1b" at the end for the differences.

## The contract P2 implements

### Bundle
- `fixtures/<name>/dist/app.js`: one self-contained ES module (Svelte runtime + `@svelters/dom` shim + app). No `import`/`export`, no dynamic `import()`, no Node built-ins; `pnpm test` fails if the text contains any of those. Source map beside it (`app.js.map`). `dist/` is git-ignored: build with `pnpm install && pnpm -r build` (or `pnpm test`, which builds first).
- Load it as a module (it has no imports and no exports; script-mode evaluation would also work but module is the contract).

### Globals the host provides before evaluating the bundle
| Global | Required | Notes |
|---|---|---|
| `__svelters_host` | yes | the `SveltersHost` object (below) |
| `queueMicrotask` | yes | Svelte schedules its flush with it; the DOM shim schedules its sweep with it. Falling back to `Promise.resolve().then` exists in the shim only. |
| `setTimeout`, `clearTimeout` | yes | `setInterval`/`clearInterval` only if an app uses them |
| `console` | no | the fixtures never log |
| `Promise`, `Symbol`, `Map`, `Set`, `Proxy`, `Reflect`, `JSON`, `Object.*` | ES builtins | |

The bundle itself defines `__svelters_root` and `__svelters_dispatch` (both paths). The shim path additionally defines `window` (= `globalThis`), `document`, `Node`, `Element`, `Text`, `Comment`, `CharacterData`, `DocumentFragment`, `EventTarget`, `HTMLMediaElement`, `navigator`, `requestAnimationFrame` (only if missing; a 16 ms `setTimeout`). The renderer path defines no DOM globals at all.

### `SveltersHost` (JS object, per `backend-abi.md#js-side-host-ops`)
- A handle is an opaque JS value compared with `===`; the TestHost issues integers `1, 2, ...` and the log prints `#<n>`. P2 must issue the same integers in `create` order.
- `create(kind)`, `insert(parent, child, before, slot)`, `remove(parent, child)`, `destroy(h)`, `setProp(h, name, value)`, `getProp(h, name)`, `call(h, method, args)`, `listen(h, signal, on)`.
- `insert`: `before` is `null` for "end" (never `undefined`); `slot` is `undefined` for default (the log prints `slot=default`).
- `setProp` values are `string | number (integers only) | boolean | null`. `null` means "unset". The JS never passes `undefined`, lists or objects.
- Return values: `create` returns the handle; `getProp` returns the value or `null`; `call` returns `null` (nothing in P uses a result). The bundle never calls `getProp`.

### Events
`globalThis.__svelters_dispatch(h, signal, args)` is synchronous: it runs the JS handlers and returns. Effects and DOM mutations that follow happen in microtasks (Svelte batches them), so ops show up at the next checkpoint, not inside `dispatch`.

Return value (both paths; P2 may ignore it until sync signals exist): `{ results: unknown[], event: object | null }`.
- `results`: each handler's return value in registration order (handlers for the signal, e.g. `onclicked`, and for its DOM alias, e.g. `onclick`). Promises are passed through unresolved; the host decides what an async result means (see `signals-and-slots-semantics.md` 1.5: use flags set before the first `await`).
- `event`: `args[0]` when it is a non-null object, i.e. the host's own event object, so any flags the handlers set on it (`defaultPrevented`, `cancelBubble`, `returnValue`, out-fields) are visible to the host by reference, and also through this return value. `null` when `args[0]` is not an object. On the shim path with no object arg, `event` is the synthetic DOM-style event.
- Handler exceptions propagate out of `dispatch` (not swallowed); remaining handlers for that dispatch do not run.
- Handler `this` is the node; args are the raw native args spread (`handler(...args)`) on the renderer path. The shim path hands DOM-style handlers one event object (`type`, `target`, `args`, `detail`, `defaultPrevented`, `cancelBubble`, `preventDefault()`, `stopPropagation()`), so apps that must run on both paths take `arg?.detail ?? arg` for the first native arg, as `fixtures/keyed-list` does.

### Running a fixture and the op log (what P2's log backend must match)
1. Fresh runtime, install host + globals, evaluate `dist/app.js`. No implicit checkpoint afterwards.
2. For each step in `script.json`:
   - `{"event": [h, signal, args]}`: append `event #h signal <json args>` to the log, then call `__svelters_dispatch(h, signal, args)`.
   - `{"checkpoint": true}`: loop { drain all QuickJS jobs; if any timers are pending, fire the earliest one (by due time, then creation order; advance a virtual clock to its due time, no real sleeping needed); else stop }. Timers fired by a checkpoint may enqueue more jobs and timers; keep going until both are empty. A cap of 10 000 timer firings is a failure.
3. After the last step, drain jobs once more (no timers) before taking the log.
4. The log is `expected.oplog` exactly: one op per line, `\n` terminated, including the last line.

Format details beyond `backend-abi.md#op-log-format`:
- `set` values: `JSON.stringify` of strings, booleans, `null`, and integers (`8`, never `8.0`; serde_json prints an `i64` as `8`, so use `i64` not `f64` for `spacing`).
- `insert ... slot=<name>`: the slot string verbatim.
- `call #1 present []`: `args` array is `[]`.
- No log line for `getProp`, for `console`, or for `queueMicrotask`.
- The log backend must not log ops it triggers itself; only the lines JS-side host calls produce, plus `event` lines for injected events.

## Mapping table (DOM-shaped input -> host ops)

| Svelte / DOM operation | Host op |
|---|---|
| element's lowercase tag name | the `kind` passed to `create` |
| `setAttribute(k, v)` (static or dynamic) | `setProp(h, k, coerce(v))`; the value is stringified by Svelte, `spacing` is coerced to an integer, everything else stays a string |
| `removeAttribute(k)` | `setProp(h, k, null)` |
| attributes `class`, `style`, `id` | JS-only, never reach the host |
| `data-slot="name"` (also `slot`) | the `slot` argument of the `insert` that attaches the element |
| `onclick={...}` | `listen h clicked on` (one per element/type, `off` when the last handler is removed) |
| `onchange={...}` | `listen h changed on` |
| any other `on<name>` | `listen h <name> on` (name passed through unchanged) |
| dispatch of signal `clicked` / `changed` | DOM event `click` / `change`; other signals use their own name as the DOM type |
| `Text` child of a `label` or `button` | folded into one `label` prop: concatenation of the element's text children, set when non-empty at realize, and again whenever it changes |
| `Text` / `Comment` anywhere else | JS-only |
| element attached under a realized parent | `create` (if not yet created), `insert` |
| element removed from a realized parent | `remove`, then `destroy` at the end of the current job unless it was re-attached (a move is `remove` + `insert`) |
| `<window>` attached to the mount root | `create`, props, children, then `call #h present []` (there is no `insert` for it) |
| any other kind attached to the mount root | error: only `window` may be top-level |

`entry` events: `changed` carries `[text]`; the shim mirrors it into the JS-side `text` prop, so Svelte writing the same value back does not emit a redundant `set`. Handlers read `e.detail` (the first arg) or `e.args`.

Event handler `this`/`event`: the object passed has `type`, `target`, `args`, `detail`, `composedPath()`, `cancelBubble`, `stopPropagation()`, `preventDefault()`.

### Op ordering inside one realize (deterministic, P2 must not reorder)
`create`, `set` for each stored prop in first-assignment order, `listen` for each signal in registration order, `set label` (if non-empty), then for each element child: realize it recursively and `insert` it. A subtree is realized when it is attached to a realized parent (or the root), not when it is cloned or built. Creation order is pre-order, so handle numbers are assigned top-down; `insert` ops are emitted post-order.

`destroy` is emitted children first (post-order) for every realized handle in a removed subtree; only the top of the removed subtree gets an explicit `remove`. Backends should treat `destroy` as "drop the host's handle"; widgets owned by a native parent stay alive with it.

## Decisions and gotchas

1. **(Shim path only.) Delegated events are invisible to `addEventListener`.** In 5.57.2 `$.delegated()` stores the handler under a module-private `Symbol('events')` on the element (`el[sym][type] = fn`), and `mount` only listens on the target and `document`. The shim scans each element's own symbol-keyed properties when it is realized and turns every function-valued entry into a native `listen`. Consequence: a delegated handler assigned after the element is already attached will not produce a `listen` (not hit by any fixture). If a Svelte upgrade changes the storage shape, the `counter` fixture's `listen #4 clicked on` disappears and the test fails. P5 or the SVR22 gate should watch this.
2. **`slot=` is rejected by the Svelte compiler** on elements that are not children of a component (`Element with a slot='...' attribute must be a child of a component or a descendant of a custom element`). Slot routing therefore uses `data-slot="name"` (provisional; the `slot` attribute set through a spread would also work). Only static, set-before-insert values are honoured.
3. **`before()`/`after()` on a parentless node are no-ops**, as in the DOM; keyed `{#each}` relies on that.
4. Fragment insertion moves children; a node that already has a parent is detached first. For a realized node that logs `remove` followed by `insert`: Svelte's keyed reorder produces moves like this (see `keyed-list`), and backends must support re-inserting a removed-but-not-destroyed handle.
5. `Element.prototype.__click = undefined` style expando writes by `init_operations` are harmless data properties here.
6. `a11y_*` compiler warnings are filtered out in `@svelters/build` (`<label>` without a control, etc., are meaningless for native kinds).
7. Redundant `set` ops are only suppressed when the JS side already knows the value (same attribute written twice, or the `entry` text mirror). A backend that echoes a user edit as a programmatic `set text` must not re-emit `changed`.
8. `disabled`, `checked`, `value` property writes (Svelte uses direct property assignment for these) are plain JS expandos, not host props. Add accessors on `Element.prototype` when a kind needs them.
9. Only the production client compile has been exercised (no `dev: true`; dev needs `Element.prototype.__svelte_meta` and more).
10. TestHost is stricter than the ABI: it throws on unknown handles, double-parent inserts, `before` that is not a child, `remove` of a non-child, and `undefined` values. P2's log backend should reject the same cases so a JS bug surfaces in both.
11. Fixture handle numbers in `script.json` come from the creation order printed in `expected.oplog`; if a fixture's markup changes, regenerate with `UPDATE=1` and re-check the script's handles by hand.

## Not done / left for later
- `bind:*`, transitions, `svelte:*` special elements, `{@html}`, CSS, snippets-as-slots, `getProp`/`call` use from JS app code.
- Source-map translation of exceptions (M2).
- Properties for kinds beyond window/box/label/button/entry/scrolledwindow/headerbar are passthrough; the `headerbar` kind is only exercised by `if-toggle`.

## P1b: renderer path (default)

### What changed
- `vendor/svelte/svelte-5.57.2-f3de3f2.tgz` is resolved for every package via `overrides.svelte` in `pnpm-workspace.yaml`. Provenance and rebuild steps: `vendor/svelte/PIN`. Weekly refresh: `tests/renderer-conformance/WEEKLY.md`.
- `@svelters/renderer` (`packages/renderer`): `createRenderer({...})` over `globalThis.__svelters_host`. Default export is the renderer object (needed by `mount(App, { target, renderer })` and as the compile-time `experimental.customRenderer` module); `./install` publishes `__svelters_root` and `__svelters_dispatch`.
- `@svelters/build` selects the path. Renderer path: `compilerOptions.experimental.customRenderer` = absolute path of `@svelters/renderer`, resolve conditions `['custom-renderer', 'browser']`, entry `packages/build/entry-renderer.js`. The `custom-renderer` export condition (from #18511) is required for any bundler/Node resolution of `svelte`, `svelte/store`, `svelte/reactivity`, `svelte/renderer`; without `custom-renderer` or `browser` the server fallback throws `custom_renderer_unavailable_on_server`. Node unit tests run with `node --conditions=custom-renderer`.
- Fixtures no longer carry `src/main.js`; the entry comes from `@svelters/build` and imports the app through the `@app` alias (`src/App.svelte`).
- `fixtures/keyed-list`: `onchange={(e) => draft = e.detail}` became `onchanged={(arg) => draft = arg?.detail ?? arg}`; nothing else needed porting (no `bind:this` in the fixtures).

### Op logs: renderer vs shim
Identical for all four fixtures (`counter` 19, `keyed-list` 87, `if-toggle` 35, `timers` 9 ops), checked by building each fixture on both paths against the same `expected.oplog`. The reasons it holds, and the places it could diverge later:
- Lazy creation, label folding, `data-slot`, sweep-destroy, `present` and listener bookkeeping are the same algorithm on a JS-side tree in both implementations. Fragments, comments, text and anchors are JS-only in both.
- Svelte (fork) builds each template instance with explicit `createFragment`/`createElement`/`insert` instead of cloning, so more renderer calls happen than shim calls, but all of them touch JS-only nodes until the subtree is attached to a realized parent, so no extra host op appears.
- Static attributes arrive as `setAttribute` calls on the not-yet-attached element and become `set` ops at realize, in first-assignment order, as before.
- The shim relied on a scan of Svelte's private delegated-handler symbol to emit `listen`; the renderer gets an explicit `addEventListener` per element, so the fragile part is gone. `listen` lands in the same place (after the element's props, before its label).
- Possible future divergence: values are now passed with their JS type (booleans, numbers) instead of strings. Only `spacing` is typed in P and it is coerced on both paths.

### Renderer details beyond the shim
- `sv-slot` proxy elements are implemented: `<sv-slot name="titlebar"><headerbar/></sv-slot>` routes its native descendants to the nearest non-proxy ancestor with slot `titlebar` (innermost proxy name beats `data-slot`, which beats the default). The proxy never reaches the host; `before` is computed among siblings of the same slot. `data-slot` still works. `sv-item` (per-child attached props) is not implemented: it needs the `set_child_attr` ABI op from the slots note. The shim path does not support `sv-slot`; only the renderer unit test (`packages/renderer/test`) covers it today.
- Non-string prop values are passed with their type, but objects and functions are rejected with a `TypeError` (the ABI has no nested values in P). Spread of function props is silently dropped by Svelte itself; see `docs/notes/svelte-fork-upstream-bugs.md`.
- `bind:this` and other element `bind:` are compile errors under a renderer; use `{@attach}` for refs and a `$bindable` wrapper component (`tests/renderer-conformance/apps/Entry.svelte`) for two-way values.
- `class`/`style` arrive as strings and stay JS-only, as before.
- No bubbling or delegation: a native signal reaches only the element's own listeners.

### Tests added
- `packages/renderer/test/renderer.test.mjs` (`node --test`): lazy creation and ordering, dispatch return shape, `sv-slot` routing, move vs destroy.
- `tests/renderer-conformance`: the trial harness (test renderer, 15 scenario apps, qjs prelude), golden streams, Node + quickjs-ng runner. Its prod, dev, values2 and hmr streams, with node ids stripped, match the trial's `out/stack.txt` and `out/pr-dev.txt`.

### Consequences for P2 (no contract change)
Globals, handle numbering, log format, step semantics and `present`/`destroy` behavior are unchanged. P2 gets `dispatch`'s return value (`{ results, event }`) to use when sync signals arrive; nothing needs to consume it now. The bundle no longer needs any DOM globals on the default path, still `queueMicrotask` + `setTimeout`/`clearTimeout`.

### Open
- `sv-item` and `set_child_attr`.
- Dev-mode bundles of the fixtures (`__svelte_meta` etc.) are covered only by the conformance harness, not by the host-level fixtures.
- Upstream: boundary-snippet crash and spread-drops-functions are unreported; stack PRs are open and may rename renderer methods.
