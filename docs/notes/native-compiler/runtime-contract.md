# Native runtime contract (`@svelters/native-runtime`)

Generated components target this API. It sits on Svelte's own signal/effect runtime
(`svelte/internal/client`, plus `block`/`branch`/`destroy_effect`/`component_root` from
`svelte/src/internal/client/reactivity/effects.js` through a resolver alias, since the package's
exports hide them) and talks to the host directly (`globalThis.__svelters_host`, see
`plans/architecture/backend-abi.md` "JS-side host ops"). There is no JS mirror of a DOM tree and no
anchor nodes: the runtime keeps only the ordering information native insertion needs.

## Shape of generated code (counter fixture)

```js
import * as $ from 'svelte/internal/client';
import * as $n from '@svelters/native-runtime';

export default function App($$anchor, $$props) {
  $.push($$props, true);
  let count = $.state(0);
  const n1 = $n.element('window', 'present');      // toplevel kind: second arg = present method
  $n.set(n1, 'title', 'Counter');                  // static props: values already typed by the compiler
  const n2 = $n.element('box');
  $n.set(n2, 'orientation', 'vertical');
  $n.set(n2, 'spacing', 8);
  const n3 = $n.element('label');
  $.template_effect(() => $n.set(n3, 'label', `Count: ${$.get(count) ?? ''}`));
  $n.append($n.slot(n2, null), n3);
  const n4 = $n.element('button');
  $n.on(n4, 'clicked', () => $.update(count));
  $n.set(n4, 'label', 'Increment');
  $n.append($n.slot(n2, null), n4);
  $n.append($n.slot(n1, null), n2);
  $n.append($$anchor, n1);
  $.pop();
}
// entry: $n.mount(App, { props: {} })
```


Required op order (matches the JS pipeline's op logs): `create`, static `set`s / `listen`s /
first effect runs in source order, children inserted into a node before that node is inserted
into its parent, `present` after a toplevel's subtree is complete.

Update order: render effects run in effect-tree order during a flush, so with template effects
emitted in source order (above) an update can order its ops differently from the JS pipeline,
where Svelte groups a fragment's template effects after its blocks (e.g. a label update before the
`{#if}` below it instead of after). The resulting native state is the same; see
`experiments/native-compiler/handwritten/README.md`.

Text content folded into a prop (`label` of label/button) must be a string: the compiler emits a
template literal (`` `${$.get(x) ?? ''}` ``) unless the expression is statically a string.

Building: `@svelters/native-runtime/vite` exports `nativeConfig(root, { entry, outDir })` (one ES
module `dist/app.js`, es2022, unminified, source map, conditions `custom-renderer`/`browser`, and
the alias `svelte/internal/client/reactivity/effects` -> Svelte's effects file).

## API

| Function | Semantics |
| --- | --- |
| `element(kind, present?)` | `host.create(kind)`; registers the node with the current ownership scope. `present` marks a toplevel: when appended to the root container the runtime calls `host.call(h, present, [])`. |
| `set(node, prop, value)` | `host.setProp` unless `value` equals the last value written (`Object.is`). No-op on a dead or destroyed node. |
| `capture(node, prop)` / `RESTORE` | `capture` reads `host.getProp` once (before the first write) and remembers it; the read value also counts as the last value, so restoring an untouched prop is a no-op. `set(node, prop, RESTORE)` writes the captured value. Used when the spec has no default to restore. |
| `token(node, prop, token, on)` | Maintains a space-separated token list prop (GTK `cssClasses`): reads the initial native value once via `getProp` (or uses the last written value), adds/removes `token`, writes the joined list when it changes. |
| `on(node, signal, handler)` | Registers `handler` (called with the event args, `this` = the runtime node) and `host.listen(h, signal, true)` on the first handler for that signal. Removed when the scope that registered it is disposed; `listen off` only if the node outlives that scope and has no handler left for the signal, never when the node itself is destroyed (backends disconnect on destroy: `disconnect_on: handle_drop`). |
| `bind(node, prop, signal, arg, get, set)` | Two-way binding plan from the backend spec: a render effect writes `get()` through `set(node, prop, …)`; on `signal` the new value is `args[arg]` (or `host.getProp` when `arg < 0`); it is recorded as the node's last value for `prop` (so echoes and redundant writes are no-ops) and passed to `set(v)` when it differs from `get()`. |
| `track(node, prop, signal, arg)` | For a one-way prop that has a spec binding (`text={draft}` on an entry, without `bind:`): on `signal` records `args[arg]` (or `getProp` when `arg < 0`) as the node's last value for `prop`, so writing the same value back is not echoed. Emitted after the prop's effect and before the element's own handlers (so its `listen` comes first). The JS renderer hard-codes this for `entry`/`changed`. |
| `slot(node, name)` | The container for `node`'s slot `name` (`null` = default slot; the host gets `slot` undefined). Same object for the same (node, name). |
| `append(container, node)` | Appends at the end of the container (slot, region or root): `host.insert(parent, h, before, slot)` where `before` is the next inserted live native after it in the same parent+slot, looking through regions (null = end). For the root container: `present`. Insertion is immediate: branches are always built in place (see `each_block`), so no deferred attach is needed. Skipped when the node or its native parent is dead. |
| `if_block(container, test, consequent, alternate)` | A region in `container`. A Svelte `block` effect evaluates `!!test()`; on change it builds the other branch (a `branch` effect + scope, `fn(anchor)` with `anchor` the branch's region) after the current one, then disposes the current one (effect, then natives). Op order: create/insert new, remove/destroy old, as Svelte's own if block. `alternate` may be null. |
| `each_block(container, collection, key, render, fallback)` | `render(anchor, item, index)` with `item`/`index` sources (`$.get(item)` in the body); `key(value, index)` or null for index-based; `fallback(anchor)` or null. Keyed: duplicate keys throw; removed items are disposed first, kept items get `$.set(item)`/`$.set(index)`, items outside a longest increasing subsequence of old positions are moved back to front (`host.remove` + `host.insert(before)`: minimal moves), then new items are built front to back (creation order = handle order follows the list). Index-based: item sources updated in place, surplus items disposed from the end, new ones appended. |
| `region(container)` | A child region (used for component calls: `Child($n.region(c), props)`). |
| `mount(Component, { props })` | Installs `__svelters_dispatch` / `__svelters_gone`, creates the root container, runs `Component(root, props)` in Svelte's `component_root` (the component does its own `$.push`/`$.pop`), returns `unmount()` (destroys effects, then natives). |

## Ownership and teardown

Every native node belongs to the ownership scope active when it was created (a branch, an each
item, or the root); blocks remember the scope they were created in, so branches built on later
updates belong to it. Disposing a branch: Svelte's `destroy_effect` on its effect (user teardowns
run while the natives still exist), then (1) `host.remove` every inserted native of the branch's
region from its native parent, including the natives of nested regions (child scopes never
remove: their natives either went with this removal or with a removed ancestor); (2) dispose child
scopes; (3) `host.destroy` its own natives, children before parents (post-order over the native
tree, creation order among siblings); (4) drop the handlers registered in the scope. Dead nodes
(native death reported through `__svelters_gone`) are skipped by every op; they are never removed
or destroyed again.

## Events

`__svelters_dispatch(h, signal, args)` calls the node's handlers for `signal` in registration
order. Unknown handles (destroyed or dead) are ignored. Return values are not used (handlers
returning values for sync signals need `call_sync`, M4). State writes in handlers are flushed by
Svelte's microtask batch, i.e. at the host's next job drain, as in the JS pipeline.
