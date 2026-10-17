# X3: child attributes and listen phases (working notes)

Lane X3, started 2026-10-17 on master 3070367. Plan: `plans/milestones/X3-childattr-listen.md`. ABI text: `plans/architecture/backend-abi.md`, "ABI v2.2 additive".

## JS syntax: why prefixed attributes

`<label parent:column={1} parent:row={2}>` was chosen over an `sv-item` wrapper and `{@attach childAttrs(...)}`.

- The Svelte compiler accepts any `prefix:name` attribute whose prefix is not a directive; the renderer receives it as `setAttribute(el, 'parent:column', v)`, so it is reactive through the ordinary attribute path with no compiler support. Verified with `fixtures/child-attrs`.
- The attribute is set while the node is still being built, before the child is inserted. The renderer sends it right after the native `insert` (same op sequence), which Qt needs because a grid cell is fixed at `addWidget` time.
- Moving a child (keyed `{#each}` reorder, a new parent) is remove + insert; the renderer replays the stored attributes after the insert. A removed child keeps its values, the backend clears its own state on `remove`.
- `sv-item` adds a JS-only node to the placement walk (`flat`/`placement`), a second place that decides the child's slot, and a component per cell; its only advantage is a type-checked wrapper API, which a Svelte component can still provide on top (`<Cell row col>` rendering `<label parent:row={row}>`).
- `{@attach}` runs after the element is mounted, so every first placement would be insert + move, and it needs its own reactivity and cleanup.
- Cost of the choice: `parent:` is a reserved attribute prefix. TypeScript needs the names declared per element (`'parent:column'?: number`); the generators emit them from the `attrs` facts.

## Renderer and host shape

- Renderer: `setAttribute`/`removeAttribute` with the `parent:` prefix go to `setChildAttr` (stored in `el.cattrs`); `applyChildAttrs` runs after `host.insert` in `nativeInsert` and in `realize`. `host.setChildAttr(parent, child, name, value)` takes names (the host resolves with `SVL_ATTR` scoped by the parent kind); `null` resets.
- Host: `op_set_child_attr` checks that `child` is inserted under `parent` (host tree), resolves the name against the parent's kind and converts the value with the untyped slot (numbers, bools, strings). `op_get_child_attr` reads back.
- Listen phases: `renderer.addEventListener(target, type, handler, options)` takes `options.capture` (Svelte passes it for `on<event>capture`) and `options.after`. Bookkeeping is per (native signal, flags). `host.listen(h, sig, on)` for flags 0 and `listen(h, sig, on, flags)` otherwise. `dispatch(h, signal, args, deferred, flags)` and `dispatchSync(h, signal, args, flags)` run only the listeners registered with exactly those flags.
- Flags travel in bits 28..30 of the signal id of `post_event`/`call_sync`. Reason: `post_event` and `call_sync` keep their signatures and old backends are untouched; the cost is that signal ids must stay below 2^28, which holds (id spaces are per-kind tables of a few thousand).
- Op log: `listen #h sig on|off [after|capture|after+capture]`, `attr #parent #child name json`, and `event`/`sync` lines carry the flags word as a trailing token. Scripts: `{"event": [h, sig, args, "capture"]}`. The Node TestHost and the log backend print identical lines (checked by `fixtures/child-attrs`, `fixtures/listen-phases`).

## Tests so far

- `packages/renderer/test/renderer.test.mjs`: child attrs (order, change, move, reset, remove, detached), phases (flags per listen, dispatch by flags, `listen()` attachment).
- `crates/svelters-host/tests/child_attrs.rs` over the log backend; fixtures `child-attrs`, `listen-phases` (testhost, log backend).
- `packages/testhost/bin/svelters-test.mjs`: the "node: specifier" bundle check now looks for quoted specifiers only; `{@attach}` pulls in a JSDoc comment containing `node:`.
