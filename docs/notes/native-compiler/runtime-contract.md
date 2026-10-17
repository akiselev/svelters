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
| `assume(node, prop, value)` | Records the spec's native default as the last value without writing it (a fresh widget is at its default), so a style fallback to the default is not written redundantly. |
| `capture(node, prop)` / `RESTORE` | `capture` reads `host.getProp` once (before the first write) and remembers it; the read value also counts as the last value, so restoring an untouched prop is a no-op. `set(node, prop, RESTORE)` writes the captured value. Used when the spec has no default to restore. |
| `token(node, prop, token, on)` | Maintains a token list prop (GTK `cssClasses`, a list of strings under ABI v3): reads the initial native value once via `getProp` (or uses the last written value), adds/removes `token`, writes the joined list when it changes. |
| `on(node, signal, handler, flags = 0)` | Registers `handler` (called with the event args, `this` = the runtime node) for (signal, `flags`) and `host.listen(h, signal, true)` on the first handler for that pair (`host.listen(h, signal, true, flags)` when `flags` is not 0: `LISTEN_AFTER = 1`, `LISTEN_CAPTURE = 2`, ABI v2.2 `listen_ex`). Each (signal, flags) is its own native connection. Removed when the scope that registered it is disposed; `listen off` only if the node outlives that scope and has no handler left for the pair, never when the node itself is destroyed (backends disconnect on destroy: `disconnect_on: handle_drop`). |
| `listen(type, handler, options)` | @svelters/renderer's attachment helper (`{@attach listen('key-pressed', fn, { after: true })}`): on attach `on(node, signal, handler, flags)`, the cleanup removes it. `type` goes through the renderer's alias table (`click` -> `clicked`); `options` is `{ after, capture }` or a flags number. The compiler emits `$.attach(node, () => $n.listen(signal, handler, flags))` with the spec-resolved signal when type and options are static; a `@svelters/renderer` import is rewritten to this module (which also exports `NativeEvent`, `StaleHandleError`, `LISTEN_AFTER`, `LISTEN_CAPTURE`). |
| `child_attr(node, name, value)` | `parent:<name>={value}`: stored on the node; sent with `host.setChildAttr(parent, node, name, value)` right after every insert (first placement, keyed move, any re-insert) and on every change while inserted. `undefined`/`null` resets (sent as `null`) and drops it. Same rules as the renderer's `setChildAttr`. |
| `bind(node, prop, signal, arg, get, set)` | Two-way binding plan from the backend spec: a render effect writes `get()` through `set(node, prop, …)`; on `signal` the new value is `args[arg]` (or `host.getProp` when `arg < 0`); it is recorded as the node's last value for `prop` (so echoes and redundant writes are no-ops) and passed to `set(v)` when it differs from `get()`. |
| `track(node, prop, signal, arg)` | For a one-way prop that has a spec binding (`text={draft}` on an entry, without `bind:`): on `signal` records `args[arg]` (or `getProp` when `arg < 0`) as the node's last value for `prop`, so writing the same value back is not echoed. Passive: it runs (before the handlers) when the signal is delivered because a handler listens, but never makes the host listen itself, exactly as the JS renderer's hard-coded `entry`/`changed` mirror. |
| `slot(node, name)` | The container for `node`'s slot `name` (`null` = default slot; the host gets `slot` undefined). Same object for the same (node, name). |
| `append(container, node)` | Appends at the end of the container (slot, region or root): `host.insert(parent, h, before, slot)` where `before` is the next inserted live native after it in the same parent+slot, looking through regions (null = end). For the root container: `present`. Insertion is immediate: branches are always built in place (see `each_block`), so no deferred attach is needed. Skipped when the node or its native parent is dead. |
| `if_block(container, test, consequent, alternate)` | A region in `container`. A Svelte `block` effect evaluates `!!test()`; on change it builds the other branch (a `branch` effect + scope, `fn(anchor)` with `anchor` the branch's region) after the current one, then disposes the current one (effect, then natives). Op order: create/insert new, remove/destroy old, as Svelte's own if block. `alternate` may be null. |
| `each_block(container, collection, key, render, fallback)` | `render(anchor, item, index)` with `item`/`index` sources (`$.get(item)` in the body); `key(value, index)` or null for index-based; `fallback(anchor)` or null. Keyed: duplicate keys throw; removed items are disposed first, kept items get `$.set(item)`/`$.set(index)`, items outside a longest increasing subsequence of old positions are moved back to front (`host.remove` + `host.insert(before)`: minimal moves), then new items are built front to back (creation order = handle order follows the list). Index-based: item sources updated in place, surplus items disposed from the end, new ones appended. |
| `region(container)` | A child region (used for component calls: `Child($n.region(c), props)`, and direct snippet calls: `row($n.region(c), () => a)`). |
| `key_block(container, key, render)` | A region. A `block` effect evaluates `key()`; on the first run and whenever the key differs (SameValueZero: `NaN` equals `NaN`, objects by identity, as Svelte's `BranchManager` map), it builds a new branch `render(anchor)` after the current one, then disposes the current one (same op order as `if_block`). Other state read inside the branch updates in place. |
| `boundary(container, props)` | `<svelte:boundary>` (synchronous part of Svelte's `dom/blocks/boundary.js`), called like a component with props `onerror`, `failed`, `children`. A `block` effect renders `children` in a branch marked as Svelte's boundary effect (`BOUNDARY_EFFECT` + `effect.b`), so Svelte's `handle_error`/`invoke_error_boundary` deliver later errors of its effects; an error while creating it is caught directly. On an error the content is disposed and, in Svelte's task queue (`queue_micro_task`, drained by `flushSync`), `transformError` (mount option, inherited) is applied, `onerror(error, reset)` called and `failed(anchor, () => error, () => reset)` rendered. `reset()` renders the children again (once per error, `svelte_boundary_reset_noop` warning after; throws while `onerror` runs). Without `onerror`/`failed` the error goes to the enclosing boundary; an error creating `failed` too. `pending` is ignored (no async mode). `{@const}` tags inside it are declared inside `children` and each of its snippets (Svelte's SvelteBoundary visitor). |
| `snippet(container, getSnippet, ...args)` | `{@render f(...)}` whose callee reads reactive state (a snippet prop). A `block` effect tracks `getSnippet()`; a different function replaces the branch (`fn(anchor, ...args)`, new first, then the old one disposed), a nullish one renders nothing. `args` are thunks. |
| `await_block(container, input, pending, then, catch)` | Svelte 5's `{#await}` resolution order. A non-promise shows `then(anchor, value)` synchronously. A promise (any thenable) shows nothing synchronously; `pending(anchor)` is shown from a microtask unless the promise settled first (so an already-resolved promise never shows `pending`); on settlement `then`/`catch(anchor, error)` replaces it and the update is flushed (`$.flush()`, as Svelte's `flushSync`). Settling into the branch already shown only sets its value source (no rebuild). A newer input or teardown makes earlier promises stale (ignored). A rejection without `catch` shows no branch and is rethrown from the rejection handler (an unhandled rejection, as in Svelte). `value`/`error` are sources (`$.get(v)` in the branch). Branch switches happen inside a second block effect that reads a status source, so branches are always built within the effect tree, never from a promise callback. Any branch may be null. |
| `classes(value)` | The class set of a dynamic `class` value: `Set` of the names in Svelte's `clsx(value)` (strings, arrays, objects of booleans). |
| `mount(Component, { props })` | Installs `__svelters_dispatch` / `__svelters_gone`, creates the root container, runs `Component(root, props)` in Svelte's `component_root` (the component does its own `$.push`/`$.pop`), returns `unmount()` (destroys effects, then natives), with `unmount.exports` holding the component's exports (what Svelte's `mount` returns). Optional `target`: an existing host handle to mount into; `transformError`: Svelte's mount option, applied to errors caught by `<svelte:boundary>`. |

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

`__svelters_dispatch(h, signal, args, deferred, flags)` calls the node's handlers registered for
(`signal`, `flags`) in registration order, with the args. Unknown handles (destroyed or dead) are
ignored. State writes in handlers are flushed by Svelte's microtask batch, i.e. at the host's next
job drain, as in the JS pipeline.

**Sync signals** (X2, `docs/notes/x2-sync-frame.md`). A signal whose spec record has `sync: true`
(GTK: `handler_result != none` or `sync_required`, e.g. `close-request`, `mnemonic-activate`; Qt:
the synthetic `close-request` and `key-press`) is delivered through `call_sync` as
`__svelters_dispatch_sync(h, signal, args, flags)`. Its handlers get **`(event, ...args)`**, not
`(...args)`: `event` is a `NativeEvent` (the renderer's class, exported here). The result for the
toolkit: `event.result = v` wins; else `event.preventDefault()` / `stopPropagation()` give `true`
(veto the close, key handled); else `null` (toolkit default). Handler return values are ignored,
as Svelte ignores them; a Promise return draws the renderer's warning; `stopImmediatePropagation()`
skips the remaining handlers. When JS was busy the host delivers the event later as
`__svelters_dispatch(h, signal, args, true, flags)`: same `(event, ...args)` shape with
`event.deferred = true`, flags ignored. The compiler knows which signals are sync and warns
(`sync_handler_return`) about a handler that returns a boolean literal, and
(`sync_handler_arguments`) about one whose first parameter is named like the signal's first
argument.

**Phases** (X3). `on<event>capture={fn}` (Svelte's capture suffix) listens with `LISTEN_CAPTURE`;
`{@attach listen(sig, fn, { after: true })}` with `LISTEN_AFTER`. The spec lists the phases each
signal accepts (`phases`); anything else is `listen_phase_unsupported` at compile time (GTK:
`after` on any signal, `capture` on event controllers only; Qt: `after` on QObject signals,
`capture` on `key-press` only).

## Child attributes

`parent:<name>={value}` on a native element (X3, `docs/notes/x3-childattr-listen.md`) is
resolved against the **actual parent element's slot** the child is placed in (spec
`children.slots.<slot>.attrs`): an unknown name is `unknown_child_attr` (with the closest names and
the slot that has it), a static value is coerced to the attribute's type
(`bool|i32|u32|i64|f64|string|enum`, `child_attr_bad_value`), a dynamic one becomes a render
effect `$n.child_attr(node, name, value)`. Elements whose parent is not known at compile time (a
component's root elements, snippet bodies) pass the value through unchecked, as the renderer;
snippet roots are checked at each local `{@render}` site. Runtime semantics: `child_attr` above.

## Spreads with static keys and `{@attach}`

`{...{ key: value }}` and `{...obj}` where `obj` is a top-level `const` object literal are lowered
as the attributes they name (`on...` events with phase suffixes, `parent:` child attributes,
props; `slot`/`class`/`style` are rejected): the keys are checked against the spec, the values are
the property expressions (`obj[key]`, read when the event fires or the effect runs). Any other
spread on a native element stays `native_spread_unsupported`. `{@attach f}` is Svelte's
`$.attach(node, () => f)` with the runtime node (the attachment runs in an effect after mount).

## Template blocks (native semantics)

What the compiler emits for Svelte's template blocks and how they map to natives. Lowering
(`crates/svelters-compiler/src/lower`) produces typed IR items (`ir::Item::{Key, Await, Render}`,
`Fragment::{consts, snippets}`, `ComponentCall::{snippets, children, consts}`); codegen
(`codegen/component.rs`) emits the calls below. Fixtures: `experiments/native-compiler/fixtures/`
`snippets`, `snippet-slots-gtk`, `snippet-slots-qt`, `blocks`, `dynamic-class`, `blocks-diagnostics`.

**Snippets.** `{#snippet name(a, b)}` is a native content factory with Svelte's shape:
`const name = ($$anchor, a = $.noop, b = $.noop) => { ...body appended to $$anchor... }`.
Parameters are thunks (`a()` reads; assigning one is `constant_assignment`); only plain identifier
parameters are supported (no destructuring/defaults: `unsupported_feature`). Declarations are
hoisted to the start of the fragment that declares them (after its `{@const}`s); snippets among an
element's children get a JS block scope. A snippet body has no native parent of its own: its
natives are created in the ownership scope that is active where it is rendered (a branch, an each
item, a component), so they are torn down with the render site.

**`{@render f(args)}`.** Arguments are passed as thunks (`() => arg`). When the callee reads
reactive state (a snippet prop such as `children`) the call is `$n.snippet(container, () => f, ...)`
(`f?.()` becomes `() => f ?? $.noop`); otherwise it is a direct call into a fresh region:
`f($n.region(container), ...)` / `f?.(...)`. The region sits in the render site's
(parent, slot), so the snippet's natives keep their place among their siblings.
Placement is validated at the render site when the snippet is declared in the same component (by
name, declared once): each root element of the snippet body (looking through
`{#if}`/`{#each}`/`{#key}`/`{#await}`) must be placeable in the parent element's target slot
(backend spec `place_child`, including kind routing: a Qt `<statusbar>` routes to the window's
`statusbar` slot), and all of them must land in the same slot (`block_mixed_slots`). Errors keep
the spec's code (`slot_rejects_child`, `unknown_slot`, `children_not_allowed`, ...) with a message
naming the snippet and the element. Snippets whose content is unknown at compile time (props,
imports) are checked only for the existence of the target slot (`unknown_slot`, `no_default_slot`,
`children_not_allowed`); the host rejects a wrong child kind at insertion.

**Snippets as named-slot content: `<sv-slot name="x">`.** A slot proxy (the runtime primitive
recommended in `docs/notes/signals-and-slots-semantics.md` 2.4): a compile-time element, never a
native, allowed only as a child of a native element (`slot_proxy_placement`); `name` must be a
static slot of that element (`unknown_slot`). Its content (elements, blocks, `{@render}`,
components) goes to the parent's slot `x`, exactly as if each child had `slot="x"` (a child naming
another slot: `slot_conflict`). Typical use in a wrapper component:
`{#if titlebar}<sv-slot name="titlebar">{@render titlebar()}</sv-slot>{/if}`; when the snippet
prop goes away the branch's natives are removed from the slot and destroyed.

**Snippets passed to components.** As Svelte 5: `{#snippet x()}` directly inside a component tag
becomes the prop `x` (declared in a JS block before the call); every other child node becomes the
implicit `children` snippet (`children: ($$anchor, $$slotProps) => {...}`), plus
`$$slots: { x: true, default: true }`. `{@const}` directly inside the component tag is declared in
that block (visible to the snippets and `children`). Bindings are passed after other props (a
spread cannot override them). Explicit `children` snippet plus other content: `snippet_conflict`.

**`{#key expr}`** → `$n.key_block`: destroy + re-create the natives (new first, then old removed
and destroyed) when the key changes. `{@const}` is not allowed directly inside (as in Svelte).

**`{#await}`** → `$n.await_block` (semantics above); `{:then v}`/`{:catch e}` bindings must be plain
identifiers and are read as `$.get(v)`.

**`{@const name = expr}`** → `const name = $.derived(() => expr)` at the start of its fragment,
read as `$.get(name)`. Allowed where Svelte allows it (directly inside `{#snippet}`, `{#if}`,
`{:else}`, `{#each}`, `{:then}`, `{:catch}`, a component tag), else `const_tag_invalid_placement`.
Destructuring is `unsupported_feature`.

**Spread attributes.** On components: Svelte's `$.spread_props(obj, () => spread, obj, ...)` in
source order (a thunk when the spread reads reactive state). On native elements: **rejected**
(`native_spread_unsupported`). Rationale: the compiler's point is to resolve every prop, event and
value type against the backend spec at compile time; the runtime does not know the spec, and a
per-element allowed-prop table would have to carry every prop of the kind's chain (hundreds on GTK)
plus value coercion and restore-to-default for keys that disappear. Set the props explicitly or
spread onto a component.

**Dynamic `class`.** `class={expr}` or an interpolated `class="a {b}"` declares
`const c<id> = $.derived(() => $n.classes(expr))`; every style rule whose selector needs a class the
element does not have statically (and has no `class:` directive for) becomes a runtime condition
`$.get(c<id>).has("name")` in the element's (or a descendant's) style ops: the compiled rule table
is the set of conditional candidates, highest precedence first, falling back to the native default.
A `class:` directive decides its class (it overrides the attribute, as in Svelte). A declaration
that reaches an element only through its dynamic class and does not apply to its kind is skipped
silently (other declaration errors are still reported).

**Text content with blocks.** In a text-content element (`<label>`, `<button>`), `{#if}` blocks
(and `{:else if}` chains) whose branches hold only text and expressions fold into the one text
prop: `` `${(a) ? `x` : `y`} / ${n ?? ''}` ``. Branch edges are trimmed (Svelte's whitespace rules);
anything else is `text_content_children`.
