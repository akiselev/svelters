# Native runtime contract (`@svelters/native-runtime`)

Generated components target two modules: the reactive core `$` (`@svelters/native-runtime/core`,
"Core API" below) and the host-op runtime `$n` (`@svelters/native-runtime`, "API" below). The
runtime talks to the host directly (`globalThis.__svelters_host`, see
`plans/architecture/backend-abi.md` "JS-side host ops"). There is no JS mirror of a DOM tree and no
anchor nodes: the runtime keeps only the ordering information native insertion needs. Since RT1
this is the only frontend (`plans/milestones/RT1-retire-fork-runtime-core.md`); the op logs the
fixtures check (`fixtures/*/expected.oplog`) are this runtime's.

## Shape of generated code (counter fixture)

```js
import * as $ from '@svelters/native-runtime/core';
import * as $n from '@svelters/native-runtime';

export default function App($$anchor, $$props) {
  $.push($$props, true);
  let count = $.state(0);
  const n1 = $n.toplevel('window', 'present');     // toplevel kind: second arg = present method
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


Required op order (as the retired JS pipeline's op logs): `create`, static `set`s / `listen`s /
first effect runs in source order, children inserted into a node before that node is inserted
into its parent, `present` after a toplevel's subtree is complete.

Update order: render effects run in effect-tree order during a flush, so with template effects
emitted in source order (above) an update can order its ops differently from the retired JS pipeline,
where Svelte groups a fragment's template effects after its blocks (e.g. a label update before the
`{#if}` below it instead of after). The resulting native state is the same; see
`experiments/native-compiler/handwritten/README.md`.

Text content folded into a prop (`label` of label/button) must be a string: the compiler emits a
template literal (`` `${$.get(x) ?? ''}` ``) unless the expression is statically a string.

Building: `@svelters/native-runtime/vite` exports `nativeConfig(root, { entry, outDir, plugins })`
(one ES module `dist/app.js`, es2022, unminified, source map, condition `browser`, the aliases of
the runtime's own modules and of the three Svelte modules the core adapter imports that Svelte's
exports map hides) and `svelteNative({ backend, spec, dev })`, the Vite plugin that runs
`svelters-compile`. `@svelters/build`'s `svelteConfig(root)` wraps both for an app directory
(toolkit from `package.json` `"svelters": { "toolkit" }`, entry `src/App.svelte`).

## Core API (`@svelters/native-runtime/core`)

The `$` namespace: exactly the names the compiler can emit, with Svelte's names and call shapes
(`packages/native-runtime/test/core-contract.test.mjs` reads them off the compiler's sources and
checks the module exports exactly those). Today (RUNTIME.md phase 1) each is Svelte's own,
re-exported from the core adapter `src/core/svelte.js`, the only module of the native runtime that
imports `svelte/internal/*`; `tools/check-svelte-internals.mjs` asserts what the adapter takes from
the installed Svelte (version pin, these names, the hidden modules, effect flag bits, the
`Boundary` members Svelte calls on `effect.b`, the effect-tree fields).

- graph: `state, derived, derived_safe_equal, get, set, update, update_pre, safe_get, untrack, eager, pending,
  effect_tracking, proxy, snapshot, to_array`
- effects: `effect_root, user_effect, user_pre_effect, template_effect`
- component: `push, pop, prop, fallback, rest_props, spread_props, exclude_from_object, props_id,
  bind_this, attach, action, noop`
- stores: `setup_stores, store_get, store_set, store_mutate, store_unsub, validate_store,
  update_store, update_pre_store, update_prop, update_pre_prop, mark_store_binding`
- dev: `tag, tag_proxy, inspect, trace, wrap_snippet`

Core internals for the block machinery (`src/core/internal.js`, not part of the compiler contract):
`root` (Svelte's `component_root`), `block`, `branch`, `destroy`, `destroyed(effect)`, `active()`,
`reorder(parent, effects)` (keyed-each effect order), `boundary_handler({ parent, transform_error,
error })` / `boundary_scope(handler)` / `current_boundary()` / `root_boundary(handler)` (error
boundaries as Svelte's runtime sees them on `effect.b`), `queue_task`, `get_all_contexts`,
`set_context`, `flush`, `tick`, `clsx`.

## API

| Function | Semantics |
| --- | --- |
| `element(kind)` | `host.create(kind)`; registers the node with the current ownership scope. (`element(kind, present)` is `toplevel(kind, present)`.) |
| `toplevel(kind, present, show?)` | A toplevel kind (X7, the compiler emits it for every one): created like `element`. Appended to the root container it is shown by its policy; appended anywhere else it is inserted into the nearest enclosing toplevel (`host.insert(enclosing, h, null)`: transient, backend-abi.md "Toplevels in the tree") once the appends around it are complete, and shown after that one was shown. Policy: the static `show` (`modal`, `modeless`, `none`; `show_invalid` at compile time otherwise), else `modal` for dialog kinds (`TOPLEVELS` in `src/bindings.generated.js`) and `modeless` for windows. `modeless` calls `present` (the spec's method; null: the first `TOPLEVELS` candidate the host resolves); `modal` calls the kind's modal method (`open` on Qt dialogs, else `present`), setting `modal` to true first when the kind has it and the app did not set it; `none` shows nothing (the app calls `present` through `bind:this`). Removal: `host.remove(enclosing, h)`, then destroy. |
| `attached(kind)` | A kind the spec marks `attaches_to` (GTK event controllers and `context-menu`, a Qt `menu` outside a menu container) placed in an element whose slots do not take it: created like `element`, inserted into its parent with no slot and no position (`host.insert(parent, h, null)`), never used as the `before` of a sibling, not moved by keyed updates. In a text-content element (`<button>Copy <context-menu>…`) it does not count as content. |
| `set(node, prop, value)` | `host.setProp` unless `value` equals the last value written (`Object.is`). No-op on a dead or destroyed node. |
| `assume(node, prop, value)` | Records the spec's native default as the last value without writing it (a fresh widget is at its default), so a style fallback to the default is not written redundantly. |
| `capture(node, prop)` / `RESTORE` | `capture` reads `host.getProp` once (before the first write) and remembers it; the read value also counts as the last value, so restoring an untouched prop is a no-op. `set(node, prop, RESTORE)` writes the captured value. Used when the spec has no default to restore. |
| `attr(node, name, value)` | `class`, `id` or `style` for the X6 styling (ST1): stored on the node and sent with `host.style` (CLASSES / NAME / INLINE / OPACITY) at the end of the job by the styling runtime (`src/style.js`, `@svelters/native-runtime/style`); inline setter declarations of a dynamic `style` are written with `set`. `class_name(base, set, toggles)` builds the class string, `style_text(base, pairs)` the inline style, `setter_value(backend, name, v)` parses a `style:<setter>` value. |
| `token(node, prop, token, on)` | Only for a backend whose `class` is a token-list prop (the DOM conformance backend); GTK `cssClasses` is never written whole (ST1). Maintains a token list prop (a list of strings under ABI v3): reads the initial native value once via `getProp` (or uses the last written value), adds/removes `token`, writes the joined list when it changes. |
| `on(node, signal, handler, flags = 0)` | Registers `handler` (called with the event args, `this` = the runtime node) for (signal, `flags`) and `host.listen(h, signal, true)` on the first handler for that pair (`host.listen(h, signal, true, flags)` when `flags` is not 0: `LISTEN_AFTER = 1`, `LISTEN_CAPTURE = 2`, ABI v2.2 `listen_ex`). Each (signal, flags) is its own native connection. Removed when the scope that registered it is disposed; `listen off` only if the node outlives that scope and has no handler left for the pair, never when the node itself is destroyed (backends disconnect on destroy: `disconnect_on: handle_drop`). |
| `listen(type, handler, options)` | The attachment helper components import from `@svelters/native-runtime` (`{@attach listen('key-pressed', fn, { after: true })}`): on attach `on(node, signal, handler, flags)`, the cleanup removes it. `type` is the native signal name (no aliases); `options` is `{ after, capture }` or a flags number. The compiler emits `$.attach(node, () => $n.listen(signal, handler, flags))` with the spec-resolved signal when type and options are static; this module also exports `NativeEvent`, `StaleHandleError`, `LISTEN_AFTER`, `LISTEN_CAPTURE`. |
| `child_attr(node, name, value)` | `parent:<name>={value}`: stored on the node; sent with `host.setChildAttr(parent, node, name, value)` right after every insert (first placement, keyed move, any re-insert) and on every change while inserted. `undefined`/`null` resets (sent as `null`) and drops it. Same rules as the retired renderer's `setChildAttr`. |
| `bind(node, prop, signal, arg, get, set)` | Two-way binding plan from the backend spec: a render effect writes `get()` through `set(node, prop, …)`; on `signal` the new value is `args[arg]` (or `host.getProp` when `arg < 0`); it is recorded as the node's last value for `prop` (so echoes and redundant writes are no-ops) and passed to `set(v)` when it differs from `get()`. |
| `bind_prop(node, prop, get, set, signal?, arg?)` | R2: two-way binding on observation, emitted for `bind:` on a prop that is `exact` on the platform with a delivery-verified fact (OV1; an `exact` fact without delivery evidence keeps the signal plan, or without one is emitted here with `bind_observation_unverified`), and on a `partial` one without a signal plan, with `bind_partially_observed`. A render effect writes `get()` through `set(node, prop, …)`; the binding observes (handle, prop) through `src/observe.js` (`observeProp`, R1's tracker; a stand-in over `host.observe` + `__svelters_prop_changed` until R1 merges) for the enclosing effect's lifetime, holding one host-side observation itself. Each delivered value (read with `getProp` when it comes as `undefined`, a proportional prop) becomes the node's last value and goes to `set(v)` when it differs from `get()`. `signal`/`arg` are the spec's signal plan, used only when `host.observe` answers false (the backend has no mechanism for the prop yet). |
| `bind_readonly(node, prop, set)` | R2: a read-only observable prop (no `bindings` annotation needed): `set` gets the value at creation, again after the first layout pass (the next host turn) if it changed, then every observed change. Never writes. |
| `bind_once(node, prop, get, set, readonly)` | R2: `bind:prop\|nonreactive` (REACTIVITY.md section 8.2). Two-way: state -> native as usual, native -> state once right after the first write (a bound `undefined` takes the native value instead of being written), then never: no observation, no signal, no mirror. Read-only: one snapshot after the first layout pass. |
| `track(node, prop, signal, arg)` | For a one-way prop that has a spec binding (`text={draft}` on an entry, without `bind:`): on `signal` records `args[arg]` (or `getProp` when `arg < 0`) as the node's last value for `prop`, so writing the same value back is not echoed. Passive: it runs (before the handlers) when the signal is delivered because a handler listens, but never makes the host listen itself, exactly as the JS renderer's hard-coded `entry`/`changed` mirror. |
| `slot(node, name)` | The container for `node`'s slot `name` (`null` = default slot; the host gets `slot` undefined). Same object for the same (node, name). |
| `append(container, node)` | Appends at the end of the container (slot, region or root): `host.insert(parent, h, before, slot)` where `before` is the next inserted live native after it in the same parent+slot, looking through regions (null = end). For the root container: `present`. Insertion is immediate: branches are always built in place (see `each_block`), so no deferred attach is needed. Skipped when the node or its native parent is dead. |
| `if_block(container, test, consequent, alternate)` | A region in `container`. A Svelte `block` effect evaluates `!!test()`; on change it builds the other branch (a `branch` effect + scope, `fn(anchor)` with `anchor` the branch's region) after the current one, then disposes the current one (effect, then natives). Op order: create/insert new, remove/destroy old, as Svelte's own if block. `alternate` may be null. |
| `each_block(container, collection, key, render, fallback)` | `render(anchor, item, index)` with `item`/`index` sources (`$.get(item)` in the body); `key(value, index)` or null for index-based; `fallback(anchor)` or null. Keyed: duplicate keys throw; removed items are disposed first, kept items get `$.set(item)`/`$.set(index)`, items outside a longest increasing subsequence of old positions are moved back to front (`host.remove` + `host.insert(before)`: minimal moves), then new items are built front to back (creation order = handle order follows the list). Index-based: item sources updated in place, surplus items disposed from the end, new ones appended. |
| `region(container)` | A child region (used for component calls: `Child($n.region(c), props)`, and direct snippet calls: `row($n.region(c), () => a)`). |
| `key_block(container, key, render)` | A region. A `block` effect evaluates `key()`; on the first run and whenever the key differs (SameValueZero: `NaN` equals `NaN`, objects by identity, as Svelte's `BranchManager` map), it builds a new branch `render(anchor)` after the current one, then disposes the current one (same op order as `if_block`). Other state read inside the branch updates in place. |
| `boundary(container, props)` | `<svelte:boundary>` (synchronous part of Svelte's `dom/blocks/boundary.js`), called like a component with props `onerror`, `failed`, `children`. A `block` effect renders `children` in a branch marked as Svelte's boundary effect (`BOUNDARY_EFFECT` + `effect.b`), so Svelte's `handle_error`/`invoke_error_boundary` deliver later errors of its effects; an error while creating it is caught directly. On an error the content is disposed and, in Svelte's task queue (`queue_micro_task`, drained by `flushSync`), `transformError` (mount option, inherited) is applied, `onerror(error, reset)` called and `failed(anchor, () => error, () => reset)` rendered. `reset()` renders the children again (once per error, `svelte_boundary_reset_noop` warning after; throws while `onerror` runs). Without `onerror`/`failed` the error goes to the enclosing boundary; an error creating `failed` too. `pending` is ignored (no async mode). `{@const}` tags inside it are declared inside `children` and each of its snippets (Svelte's SvelteBoundary visitor). |
| `snippet(container, getSnippet, ...args)` | `{@render f(...)}` whose callee reads reactive state (a snippet prop). A `block` effect tracks `getSnippet()`; a different function replaces the branch (`fn(anchor, ...args)`, new first, then the old one disposed), a nullish one renders nothing (in esm-env `DEV` it throws Svelte's `invalid_snippet`). `args` are thunks. |
| `validate_snippet_args(anchor, ...args)` | Dev (`--dev`): the first statement of every snippet, as Svelte's: throws `invalid_snippet_arguments` unless `anchor` is a runtime container (slot, region, root) and every arg a function. |
| `component(container, getComponent, props, bind)` | A dynamic component (`<svelte:component this>`, a component held in state) with `bind:this`: `snippet` semantics, and `bind(exports)` (`$.bind_this`) runs inside each instance's branch, so the binding is cleared with the old instance and set to the new one. |
| `await_block(container, input, pending, then, catch)` | Svelte 5's `{#await}` resolution order. A non-promise shows `then(anchor, value)` synchronously. A promise (any thenable) shows nothing synchronously; `pending(anchor)` is shown from Svelte's microtask queue (`queue_micro_task`) unless the promise settled first, so a promise that settles before that task runs never shows `pending`, and a `flushSync` that drains the queue first shows it at once (as in Svelte); on settlement `then`/`catch(anchor, error)` replaces it and the update is flushed (`$.flush()`, as Svelte's `flushSync`, unless one is running). Settling into the branch already shown only sets its value source (no rebuild). A newer input or teardown makes earlier promises stale (ignored). A rejection without `catch` shows no branch and is rethrown from the rejection handler (an unhandled rejection, as in Svelte). `value`/`error` are sources (`$.get(v)` in the branch). As in Svelte, the callbacks build their branch directly under the block effect (its context captured with Svelte's `capture()`), so a `pending` branch renders even when the same flush tears the block down. Any branch may be null. |
| `classes(value)` | The class set of a dynamic `class` value: `Set` of the names in Svelte's `clsx(value)` (strings, arrays, objects of booleans). |
| `mount(Component, { props })` | Installs `__svelters_dispatch` / `__svelters_gone`, creates the root container, runs `Component(root, props)` in Svelte's `component_root` (the component does its own `$.push`/`$.pop`), returns `unmount()` (destroys effects, then natives), with `unmount.exports` holding the component's exports (what Svelte's `mount` returns). Optional `target`: an existing host handle (or a runtime node, e.g. a list row outlet) to mount into; `transformError`: Svelte's mount option, applied to errors caught by `<svelte:boundary>`. |

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
`(...args)`: `event` is a `NativeEvent` (exported here). The result for the
toolkit: `event.result = v` wins; else `event.preventDefault()` / `stopPropagation()` give `true`
(veto the close, key handled); else `null` (toolkit default). Handler return values are ignored,
as Svelte ignores them; a Promise return draws a warning; `stopImmediatePropagation()`
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

**Borrowed handles.** `mount` installs `__svelters_node(h)` (`nodeFor`): the host
wraps handles in `call`/`getProp` results with it, so `node.call('getParent')` returns the
runtime node of an object the app created (identity holds), or one stable `borrowed` node per
other handle (held weakly, `host.destroy` once collected, stale after `gone`).

**Keyed each and effect order.** After a keyed update the item branch effects are relinked in list
order in Svelte's effect tree (as Svelte's each block moves effects), so later updates, and the ops
they produce (child attributes re-sent to a grid, whose Qt cell order follows the writes), come in
the JS pipeline's order.

## Child attributes

`parent:<name>={value}` on a native element (X3, `docs/notes/x3-childattr-listen.md`) is
resolved against the **actual parent element's slot** the child is placed in (spec
`children.slots.<slot>.attrs`): an unknown name is `unknown_child_attr` (with the closest names and
the slot that has it), a static value is coerced to the attribute's type
(`bool|i32|u32|i64|f64|string|enum`, `child_attr_bad_value`), a dynamic one becomes a render
effect `$n.child_attr(node, name, value)`. Elements whose parent is not known at compile time (a
component's root elements, snippet bodies) pass the value through unchecked;
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

## Virtualized lists (`@svelters/native-runtime/lists`)

A container of widgets and a virtualized native list are different abstractions, and the source
says which one it wants:

```svelte
<script>
  import { ListView } from '@svelters/lists';
  let rows = $state.raw([...]);
  let selected = $state(-1);
</script>

<!-- eager: one native subtree per item, children of the box -->
<box orientation="vertical">
  {#each rows as r (r.id)}<label>{r.text}</label>{/each}
</box>

<!-- virtualized: a native list view; the toolkit asks for the rows it shows -->
<ListView items={rows} key={(r) => r.id} bind:selected onactivate={(item, pos) => ...}>
  {#snippet row(item, index)}<label>{index}: {item.text}</label>{/snippet}
</ListView>
```

`{#each}` never virtualizes and `<ListView>` never builds rows eagerly. `<ListView>` is
`@svelters/native-runtime/lists` (`src/lists.js`); `@svelters/lists` re-exports it (and
`nativeConfig` resolves the specifier to it), with the props, exports and host protocol of the M5
component of the retired JS pipeline (`plans/architecture/backend-abi.md` "Model-view protocol
(M5)"). The native `virtuallist` element itself is a backend extra kind (not in the generated
specs) and is not authorable directly.

Props: `items` (array or iterable), `key(item, index)` (default: the index), `row` snippet
`(item, index)`, `layout` (`list` | `grid`), `selection` (`none` | `single`), `bind:selected`
(position, -1 for none), `rowHeight` (Qt's uniform row height, default 32), `onactivate(item, pos)`;
any other prop is set on the native `virtuallist` unchanged (`vexpand`, `minHeight`, ...).
`bind:this` gives the exports `{ scrollTo(index), controller() }`.

Compile-time checks (`lower/lists.rs`, for a `ListView` imported from `@svelters/lists` under any
local name): `list_row_missing` (no `row` snippet or prop), `list_row_params` (more than
`(item, index)`), `list_children` (content other than snippets: the list renders only `row`),
`list_row_toplevel` and the spec's `place_child` codes (each root element of an inline `row` must
be placeable in the row outlet, a `box`), `list_bind_invalid` (only `bind:selected`/`bind:this`),
`list_prop_invalid` (static `layout`/`selection` values). Fixture:
`experiments/native-compiler/fixtures/lists-diagnostics`.

### Semantics, and how they differ from `{#each}`

| | `{#each items as item (key)}` | `<ListView items key row>` |
| --- | --- | --- |
| Natives | one subtree per item, inserted into the parent's slot | one `virtuallist`; row content only for row slots the toolkit creates (about the visible rows plus a margin) |
| Item identity | the key; duplicate keys throw | the key; each key gets a native **id** (integer, from 1, per list instance) that the native model holds for as long as the key stays in the list; a key that leaves and comes back gets a new id. Keys must be unique (the diff assumes it) |
| Updates | keyed reconcile: dispose removed, move the LIS complement (`host.remove` + `insert`), build new | the same LIS diff over keys (`diffKeys`, shared with the JS pipeline) gives `splice(pos, removed, ids)` calls: removals last-first, then insertions; more than 16 splices collapse into one range replace. A replaced item object with the same key is no native op: only bound rows update |
| What is reused | an item's natives live exactly as long as its key | a **row slot** (GTK `GtkListItem`, Qt pooled viewport widget) is reused for different items as the view scrolls; its natives stay, `item`/`index` change |
| State in the row | per item: `$state` in the body belongs to that key | per **slot**: `$state` inside the row (or a component in it), `bind:this` targets and native widget state (focus, typed text, scroll) survive a rebind to another item and are *not* reset. Keep per-item state in the item (or a map by key), not in the row |
| `index` | the item's position, updated on moves | the bound position; updated without a rebind when splices shift a bound row |
| Lifetime | built when the item appears, destroyed when it leaves or the block goes | `setup(row)`: the slot gets an outlet (`box`, its own root, `setRow(row, outlet)`); first `bind(row, pos, id, gen)`: the `row` snippet is mounted into the outlet with reactive `item`/`index`; later binds swap them; `unbind(row, gen)`: the slot is idle (its natives keep the last content, hidden or off-screen); `teardown(row)`: content and outlet destroyed. Removing the `<ListView>` tears down every slot, then the view |
| Context | the enclosing component's | the `<ListView>`'s (captured at init, set on each row root) |
| Effects and errors | inside the block's effect tree | each row is a separate root (`mount` into the outlet): row effects are not children of the list's effects; destroying the list destroys them explicitly |

**Delivery.** The model (`n_items`/`get_item`/`rowCount`/`data`) is answered natively from the id
snapshot and never calls JS. Row lifecycle signals are sync signals delivered with `call_sync`
(`__svelters_dispatch_sync`, handlers receive `(event, ...args)`): the content is in place before
the toolkit paints the row (`bind` ends with `$.flush()`). When JS is busy (our own `splice`, a
toplevel's first layout during `present`) the host queues them and delivers them later as
`__svelters_dispatch(h, signal, args, deferred = true)`; the runtime accepts both argument shapes.
A deferred `bind` whose position is stale (more splices happened since) is resolved by its id; a
bind whose id is gone is ignored; `gen` increments on every bind of a slot, so a late `unbind` of
an older binding is ignored. `src/lists.js` installs a minimal `__svelters_dispatch_sync` when the
runtime has none (result: `event.result`, else `true` after `preventDefault`/`stopPropagation`,
else `null`).

**Selection and activation.** A native `select(pos)` writes `selected` without echoing it back; an
app write of `selected` sets the native `selected` prop. `activate(pos)` calls
`onactivate(item, pos)`. GTK's selection model follows splices itself (it may report a shifted
selection); Qt keeps the position.

**Tests.** `packages/native-runtime/test/lists.test.mjs` (protocol, minimal splices, slot reuse,
deferred delivery, teardown); compiled harness cases `virtual-list`, `lists-stress`,
`lists-recycle` (+ `script.deferred.json`, run live on both pipelines) and `lists-diagnostics`;
real toolkits through `native.mjs` with clicks-only twins of the scripts
(`experiments/native-compiler/compiled/scripts/*.native.json`).

**Timings** (historical, measured before RT1 with `bench-lists.mjs`, which compared the two
pipelines and was removed with the JS pipeline; debug `counter`, Xvfb, median of 3, ms from the
click to idle, 10 ms poll granularity; JS pipeline / compiled). `<ListView>` (lists-stress): GTK fill 1k 134 / 84,
fill 10k 199 / 183, append 49 / 51, prepend 85 / 66, reverse 10k 230 / 197, sortDesc 166 / 144,
odd filter 221 / 177, scroll to end 36 / 22; Qt fill 1k 14 / 13, fill 10k 71 / 75, reverse 83 / 86,
sortDesc 121 / 123, scroll 11 / 11. The eager `{#each}` baseline (bench/keyed-bench, 1000 rows of
box + label + button): GTK fill 362 / 478, reverse 237 / 57; Qt fill 452 / 573, reverse 871 / 701.
So a 10k `<ListView>` fills in about half the time of a 1k `{#each}`, and the two pipelines are
within noise for lists (the work is the key pass, the diff and the toolkit, which are the same
code); the compiled `{#each}` is faster on reverse (no Svelte `reconcile` scan) and slower on fill
and clear (per-item region/scope bookkeeping).
