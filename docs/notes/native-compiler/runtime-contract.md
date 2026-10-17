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

## API

| Function | Semantics |
| --- | --- |
| `element(kind, present?)` | `host.create(kind)`; registers the node with the current ownership scope. `present` marks a toplevel: when appended to the root container the runtime calls `host.call(h, present, [])`. |
| `set(node, prop, value)` | `host.setProp` unless `value` equals the last value written (`Object.is`). No-op on a dead node. |
| `capture(node, prop)` / `RESTORE` | `capture` reads `host.getProp` once (before the first write) and remembers it; `set(node, prop, RESTORE)` writes the captured value. Used when the spec has no default to restore. |
| `token(node, prop, token, on)` | Maintains a space-separated token list prop (GTK `cssClasses`): reads the initial native value once via `getProp`, adds/removes `token`, writes the joined list when it changes. |
| `on(node, signal, handler)` | Registers `handler` (called with the event args, `this` = node) and `host.listen(h, signal, true)` on the first handler for that signal. Removed when the owning scope is disposed; no explicit `listen off` when the node itself is destroyed (backends disconnect on destroy: `disconnect_on: handle_drop`). |
| `bind(node, prop, signal, arg, get, set)` | Two-way binding plan from the backend spec: a render effect writes `get()` through `set(node, prop, …)`; on `signal` the new value is `args[arg]` (or `host.getProp` when `arg < 0`); it is recorded as the node's last value for `prop` (so echoes and redundant writes are no-ops) and passed to `set(v)` when it differs from `get()`. |
| `slot(node, name)` | The container for `node`'s slot `name` (`null` = default slot; the host gets `slot` undefined). |
| `append(container, node)` | Appends at the end of the container: `host.insert(parent, h, before, slot)` where `before` is the next realized native sibling in the same parent+slot (null when none). For the root container: `present`. Inside an unattached branch region it only records; insertion happens when the region is attached. |
| `if_block(container, test, consequent, alternate)` | A region in `container`. A Svelte `block` effect evaluates `!!test()`; on change it destroys the current branch (effect + natives) and builds the other in a `branch` effect, inserting its natives at the region's position. `alternate` may be null. |
| `each_block(container, collection, key, render, fallback)` | Keyed (`key` non-null) or index-based list. Items are branches with `item` and `index` sources (`$.get(item)` in the body). Keyed updates move natives with `host.remove` + `host.insert(before)` using a longest-increasing-subsequence plan (minimal moves); removed items are disposed. |
| `region(container)` | A child region (used for component calls: `Child($n.region(c), props)`). |
| `mount(Component, { props })` | Installs `__svelters_dispatch` / `__svelters_gone`, creates the root container, runs the component in Svelte's `component_root`, returns `unmount()` (destroys effects, then natives). |

## Ownership and teardown

Every native node belongs to the ownership scope active when it was created (a branch, an each
item, or the root). Disposing a scope: (1) unless an enclosing scope is being disposed, remove its
top-level natives from their native parent (`host.remove`); (2) dispose child scopes; (3)
`host.destroy` its own natives, children before parents. Handlers registered in the scope are
dropped. Dead nodes (native death reported through `__svelters_gone`) are skipped by every op;
they are never removed or destroyed again.

## Events

`__svelters_dispatch(h, signal, args)` calls the node's handlers for `signal` in registration
order. Unknown handles (destroyed or dead) are ignored. Return values are not used (handlers
returning values for sync signals need `call_sync`, M4).
