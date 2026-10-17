# P1 JS lane: working notes

Svelte `5.57.2`, Vite `8.3.4`, `@sveltejs/vite-plugin-svelte` `7.3.1` (all pinned exactly). Node 22 only for the test runner.

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

The bundle itself defines `window` (= `globalThis`), `document`, `Node`, `Element`, `Text`, `Comment`, `CharacterData`, `DocumentFragment`, `EventTarget`, `HTMLMediaElement`, `navigator`, `requestAnimationFrame` (only if missing; a 16 ms `setTimeout`), `__svelters_root`, and `__svelters_dispatch`.

### `SveltersHost` (JS object, per `backend-abi.md#js-side-host-ops`)
- A handle is an opaque JS value compared with `===`; the TestHost issues integers `1, 2, ...` and the log prints `#<n>`. P2 must issue the same integers in `create` order.
- `create(kind)`, `insert(parent, child, before, slot)`, `remove(parent, child)`, `destroy(h)`, `setProp(h, name, value)`, `getProp(h, name)`, `call(h, method, args)`, `listen(h, signal, on)`.
- `insert`: `before` is `null` for "end" (never `undefined`); `slot` is `undefined` for default (the log prints `slot=default`).
- `setProp` values are `string | number (integers only) | boolean | null`. `null` means "unset". The JS never passes `undefined`, lists or objects.
- Return values: `create` returns the handle; `getProp` returns the value or `null`; `call` returns `null` (nothing in P uses a result). The bundle never calls `getProp`.

### Events
`globalThis.__svelters_dispatch(h, signal, args)` is synchronous: it runs the JS handlers and returns. Effects and DOM mutations that follow happen in microtasks (Svelte batches them), so ops show up at the next checkpoint, not inside `dispatch`.

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

1. **Delegated events are invisible to `addEventListener`.** In 5.57.2 `$.delegated()` stores the handler under a module-private `Symbol('events')` on the element (`el[sym][type] = fn`), and `mount` only listens on the target and `document`. The shim scans each element's own symbol-keyed properties when it is realized and turns every function-valued entry into a native `listen`. Consequence: a delegated handler assigned after the element is already attached will not produce a `listen` (not hit by any fixture). If a Svelte upgrade changes the storage shape, the `counter` fixture's `listen #4 clicked on` disappears and the test fails. P5 or the SVR22 gate should watch this.
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
