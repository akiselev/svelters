# Svelte 5 against a non-DOM widget tree: research notes

Researched 2026-10-16. Sources:

- `sveltejs/svelte` `main` @ `40bb084` (2026-10-15). `packages/svelte` is `5.57.2`, which is npm `latest`.
- Branch `svelte-custom-renderer` @ `ece83dd` (2026-10-13).
- A smoke test: stock `svelte@5.57.2` output compiled with `fragments: 'tree'`, run in plain Node with no jsdom against a ~90-line hand-written node tree. See section 4.

## 1. Official custom renderer API: unmerged, unreleased

| Item | State |
|---|---|
| [#15470 "Custom renderers support"](https://github.com/sveltejs/svelte/issues/15470) | Open tracking issue (benmccann, 2025-03-07). Cites Svelte Native and Lynx. |
| [#18042 "feat: custom renderers API"](https://github.com/sveltejs/svelte/pull/18042) | **Open, not merged.** paoloricciuti, base `main` ← head `svelte-custom-renderer`, +4357/-517. Last updated 2026-10-13 (a merge of `main`). |
| [#18244](https://github.com/sveltejs/svelte/pull/18244), [#18359](https://github.com/sveltejs/svelte/pull/18359), [#18317](https://github.com/sveltejs/svelte/pull/18317) | Merged **into the feature branch**, not into `main`: a single type argument, `customRenderer` as a global option, and a unified `mount` API (Rich-Harris). |
| [#18405](https://github.com/sveltejs/svelte/pull/18405) → [#18461](https://github.com/sveltejs/svelte/pull/18461) → [#18511](https://github.com/sveltejs/svelte/pull/18511) | Open stacked PRs: interleaving renderers, a `custom_renderer` flag, and a forced `custom-renderer` export condition. |
| [#18505](https://github.com/sveltejs/svelte/pull/18505) | Closed: an attempt to expose mount/unmount from `svelte/renderer` for non-browser hosts. |

- **Release status:** nothing is released. npm dist-tags are `latest: 5.57.2` and `next: 5.0.0-next.272` (stale). Preview builds are available through pkg.svelte.dev: `pnpm add https://pkg.svelte.dev/svelte/c/ece83ddb7b063cbdeb1925a540bca8482eabc436`. See also https://pkg.svelte.dev/repos/svelte/pr/18042.
- **Review status:** on 2026-05-28 Rich-Harris wrote "Implementation-wise this basically seems... ready?". On 2026-08-13 paoloricciuti said the team's focus is SvelteKit 3, and Rich will review the PR after that ships.
- **Unfinished items in the PR body:** documentation and more tests.

### API shape on the branch

The compile option is `experimental.customRenderer`. It can be `true`, a module path, or `({filename}) => path | null`. A component can override it with `<svelte:options customRenderer="...">`. The module's default export must come from `createRenderer`.

The interface is in `src/internal/client/custom-renderer/types.d.ts`. `createRenderer` is an identity function that exists only for typing.

```ts
import { createRenderer } from 'svelte/renderer';
export default createRenderer({
  createFragment(), createElement(name), createTextNode(data), createComment(data),
  nodeType(node) /* 'fragment'|'element'|'text'|'comment' */, getNodeValue(node),
  getAttribute(el, n), setAttribute(el, k, v /* any */), removeAttribute(el, n), hasAttribute(el, n),
  setText(node, text), getFirstChild(el), getLastChild(el), getNextSibling(node), getParent(node),
  insert(parent, node, anchor|null) /* DOM semantics: moves; fragment inserts its children */,
  remove(node), addEventListener(t, type, h, opts), removeEventListener(t, type, h, opts),
});
// mount: import { mount } from 'svelte'; mount(App, { target, props, renderer });
```

Example compiled output with a renderer, taken from the PR body:

```js
import $renderer from 'your-custom-renderer';
var root = $.from_tree([['p', null, 'hello']]);
export default function Main($$anchor) {
  var $$pop_renderer = $.push_renderer($renderer);
  var p = root(); $.append($$anchor, p); $$pop_renderer();
}
```

**Features forbidden with a renderer:**
- `bind:` on elements
- transitions and animations
- `svelte:window`, `svelte:body`, `svelte:document` and `svelte:head`
- `css: 'injected'`
- `createRawSnippet`
- hydration

Delegated events are also disabled. Every event goes through `renderer.addEventListener`.

## 2. `fragments: 'tree'` (stable since 5.33)

From `src/compiler/types/index.d.ts`: `html` (the default) fills a `<template>` through `innerHTML` and clones it. `tree` "creates the fragment one element at a time and _then_ clones it".

The only change is how templates are built: `$.from_html('<p>…</p>')` becomes `$.from_tree(structure, flags)`. Everything else in the output is identical. The structure works like this:

- A string is a text node.
- `[tag, attrs|null, ...children]` is an element.
- An array hole or `undefined` is an empty comment, which serves as a block anchor.
- `['// data']` is a preserved comment.

Snapshot test: `tests/snapshot/samples/functional-templating/_expected/client/index.svelte.js`:

```js
var root = $.from_tree([
  ['h1', null, 'hello'], ' ',
  ['div', { class: 'potato' }, ['p', null, 'child element'], ' ', ['p', null, 'another child element']]
], 1);
```

**Runtime (`src/internal/client/dom/template.js`):**
- `fragment_from_tree` calls `create_fragment`, `create_text`, `create_comment`, `create_element(name, ns, is)`, `set_attribute` (which uses `setAttribute`, or `setAttributeNS` for `xlink:`) and `.append()`.
- It reads `element.nodeName` to detect `TEMPLATE` (whose children go into `.content`) and `foreignObject`.
- Each instantiation clones the cached tree with `node.cloneNode(true)`. With the `TEMPLATE_USE_IMPORT_NODE` flag, or when `is_firefox` is true, it uses `document.importNode(node, true)` instead.
- `get_first_child` and `.lastChild` then set the effect's node range.

**Other helpers in compiled output** (from my compile of an if/each/key/await/snippet/events sample):
- Tree walking: `child`, `only_child`, `first_child`, `sibling`, `next`, `reset`, `append`
- Blocks and reactivity: `if`, `each`, `key`, `await`, `template_effect`
- Text and attributes: `set_text`, `set_attribute`, `set_value`, `set_class`, `set_style`
- Events: `delegated`, `delegate`, `event`
- `bind_this`
- Dev mode only: `add_svelte_meta`, `add_locations`, `tag`, `validate_snippet_args`, `wrap_snippet`

Known boolean props compile to plain property writes, for example `button.disabled = …`.

## 3. DOM surface of the client runtime (on `main`)

### Module load

Nothing needs globals at module load. Every access is guarded:

- `constants.js`: `globalThis.document?.contentType`
- `reconciler.js`: `globalThis?.window?.trustedTypes`
- `disclose-version.js`: `typeof window !== 'undefined'`, then writes `window.__svelte.v`

### `init_operations()` at mount (`dom/operations.js`)

```js
$window = window; $document = document;
is_firefox = /Firefox/.test(navigator.userAgent);
first_child_getter = get_descriptor(Node.prototype, 'firstChild').get;   // must be an accessor
next_sibling_getter = get_descriptor(Node.prototype, 'nextSibling').get; // must be an accessor
// writes expando defaults on Element.prototype / Text.prototype (if extensible); DEV: Element.prototype.__svelte_meta
```

### Core features (blocks, text, attributes, events, mount and unmount)

**Creating nodes and templates**
- `document.createElement(tag[, {is}])`
- `createElementNS` (for SVG and MathML)
- `createTextNode`, `createComment`, `createDocumentFragment`
- `cloneNode(true)`, and `importNode` (Firefox only, or with the flag)

**Navigating the tree**
- The `firstChild` and `nextSibling` getters, always read through the cached `Node.prototype` getters
- `.lastChild` (template range, branches, `{@html}`)
- `.parentNode` (each, mount cleanup)
- `nodeType`, compared against 1, 3, 8 and 11
- `nodeName` (uppercase, compared with `'TEMPLATE'`, `'INPUT'`, `'SELECT'` and similar)
- `namespaceURI`

**Changing the tree**
- `anchor.before(node|fragment)`. This is how all inserts happen: `append()`, branches, each, boundary.
- `fragment.append`, `appendChild` (mount anchor), `node.remove()` (`remove_effect_dom`), `removeChild`
- `textContent = ''` (fast clear in `each`)

**Text**
- `text.nodeValue` (read and write) in `set_text`
- `Comment.data`

**Attributes**
- `getAttribute`, `setAttribute`, `removeAttribute`, `hasAttribute`
- `get_setters()`: when the value is not a string, it walks the prototype chain from the element **up to `Element.prototype`** and assigns the property directly if it finds a setter
- `className` or `classList.toggle`
- `style.cssText`, `style.setProperty`, `style.removeProperty`
- `.value` / `__value` (`set_value`)

**Events**
- Delegated events (click, input, …): `$.delegated` stores `element[event_symbol][name] = fn`. `mount` then calls `addEventListener(name, handle_event_propagation)` on **both `target` and `document`**. On dispatch, the handler reads `event.composedPath()` (falling back to `event.target`), `event.type` and `event.cancelBubble`, and redefines `event.currentTarget`. It also checks `current_target.disabled`, and reads `this.ownerDocument`.
- Non-delegated events: `$.event` compares against `document.body`, `window` and `document`, and evaluates `dom instanceof HTMLMediaElement`. Listeners for `pointer*`, `touch*` and `wheel` are attached in a microtask.

**Scheduling**
- `queueMicrotask`, `setTimeout`
- `requestAnimationFrame` (only in `runtime.js` when the tab is unfocused, and in `loop.js`)

**Type checks**
- `instanceof Comment` in `first_child`
- `instanceof EventTarget` in `deep_read`, used by `$inspect` and legacy code

**Misc**
- `props_id` reads and writes `window.__svelte.uid`
- `hydratable.js` reads `window.__svelte?.h`

### Only for optional features

| Feature | DOM APIs |
|---|---|
| `bind:value`, `bind:checked`, `bind:group`, select bindings | `document.activeElement`, `MutationObserver`, `querySelector` |
| `bind:clientWidth` / `bind:contentRect` | `ResizeObserver` |
| `bind:this` | Nothing beyond the core surface |
| Transitions and animations | `getComputedStyle`, `element.animate`, `dispatchEvent(new CustomEvent)` |
| `svelte:window` / `svelte:document` / `svelte:head` | `window`, `document.head` |
| `{@html}` and `createRawSnippet` | A `<template>` element with `innerHTML` (`reconciler.js`) |
| `css: 'injected'` | `getRootNode`, `ownerDocument`, `<style>` |
| Custom elements | `customElements.get` |
| Hydration | Comment markers (`[`, `]`), `merge_text_nodes`, `.after()`. Only runs when `hydrating`; `mount()` never sets it. |

## 4. Doing it ourselves on stock Svelte 5.57.2: confirmed feasible

**Verified.** This ran in Node with no jsdom, using compiled output and a node tree with no DOM.

- The prod build ran mount, `{#if}`, keyed `{#each}`, `set_attribute`, `button.disabled`, `set_value`, delegated `onclick` through a hand-rolled `dispatchEvent` and `composedPath`, and `unmount`.
- The dev build (with `__svelte_meta` and locations) also ran snippets, `{#key}`, `{#await}`, `bind:this`, `style:`, `class:`, `$effect`, and a non-delegated custom event (`onactivate`).
- Script: `/tmp/claude-1000/-projects-svelters/10c0a3c6-7a5c-4d9b-9688-b5af0249a13a/scratchpad/compile/pkg/shim.mjs`.

**Globals actually required.** I deleted each one in turn:
- `Node` and `Text` are required at init.
- `HTMLMediaElement` is required by any non-delegated `$.event` (a dummy class is enough).
- `Element` is required by `init_operations` and `get_setters`.
- `window` and `document` are required.
- `navigator.userAgent` is required. In Node it had to be set with `Object.defineProperty`.
- These were not needed by the sample, but are cheap to provide: `Comment`, `EventTarget`, `DocumentFragment`, `requestAnimationFrame`.
- QuickJS also needs `queueMicrotask` and `setTimeout`.

**Methods each node class needs:**
- `Node`:
  - getters for `firstChild`, `lastChild`, `nextSibling` and `parentNode`
  - `nodeType`, `nodeName`, `nodeValue` (get and set), `textContent` (setting `''` clears the children)
  - `before`, `after`, `append`, `appendChild`, `removeChild`, `remove`, `cloneNode(deep)`
  - `addEventListener`, `removeEventListener`, `ownerDocument`, `isConnected`
- `Element`:
  - `get/set/has/removeAttribute`, `setAttributeNS`, `namespaceURI`
  - `className`, `classList.toggle`, `style.{cssText,setProperty,removeProperty}`
  - Elements also receive plain property writes: `value`, `__value`, `disabled`, `checked`, and any setter `get_setters` finds.
- `document`: `createElement`, `createElementNS`, `createTextNode`, `createComment`, `createDocumentFragment`, `importNode`, `body`, `addEventListener`
- Events: a plain object with `type`, `target`, `composedPath()` (target → ancestors → root → document), a writable `cancelBubble`, and a configurable `currentTarget`

**Gotchas:**
1. **`firstChild` and `nextSibling` must be accessor properties on `Node.prototype`.** The runtime caches the `.get` and calls it with `.call(node)`. Own data properties or getters defined on a subclass would be bypassed.
2. **Every element must inherit from `Element.prototype`.** Otherwise `get_setters` walks past `null` and throws. Setters defined on widget subclasses become property-assignment targets for non-string attribute values, which is a useful hook for native props.
3. `Element.prototype` and `Text.prototype` must stay extensible. The runtime adds expando caches to them (and to the event object).
4. **Inserting a fragment must move its children.** Inserting a node that already has a parent must detach it first, as `before` and `append` do in the DOM.
5. `cloneNode(true)` must deep-copy attributes and children. Svelte clones every template instance, so a native widget should probably be created lazily on insert rather than when the template is cloned.
6. **Delegated events are attached to `target` *and* `document`.** The walk uses `composedPath()`, not `parentNode`. The alternative is to compile with a custom event model, which stock 5.57 cannot do.
7. `nodeName` must be uppercase for the HTML special cases (`INPUT`, `SELECT`, `OPTION`, `TEMPLATE`, `PROGRESS`, `LINK`). Custom tags with `-` are treated as custom elements.
8. **Use the `browser` export condition when bundling.** Without it, `svelte` resolves to the server build. Dev builds also need the `development` condition.
9. Unmount removes the mount anchor through `parentNode.removeChild`.

**Comparison with the branch API.** The branch needs a 19-method renderer and no globals. Stock Svelte needs the DOM-shaped shim above, but it runs on today's npm release, and HTML-only features (`bind:`, transitions) keep working as long as the shim supports them.

## 5. Existing projects

- **[khromov/gpuix-svelte](https://github.com/khromov/gpuix-svelte):** renders to GPUI, the UI framework behind Zed. Built on the unreleased API (README links [#18511](https://github.com/sveltejs/svelte/pull/18511)) and ships a vendored `svelte-5.57.2-694f791.tgz`. 221 stars, active today.
- **[svelterm](https://svelterm.dev)** (tomyan): terminal output with a CSS engine. Built on the branch; write-up at [svelterm.dev/docs/how-it-works](https://svelterm.dev/docs/how-it-works). The PR author also prototyped an opentui terminal renderer.
- **Svelte Native / NativeScript:** [halfnelson/svelte-native#359](https://github.com/halfnelson/svelte-native/issues/359) ("RFC: support for svelte 5") is closed and points to #15470. There is no Svelte 5 release; the last version is `svelte-native@1.0.29`, from 2024-11. NathanWalker has been trying out the branch API (mentioned in the #18042 thread).
- **[nodegui/svelte-nodegui](https://github.com/nodegui/svelte-nodegui):** Svelte 3 only, marked "no longer being maintained" (2025-03). It relied on a DOM shim over Qt widgets.
- **Lynx:** [lynx-family/lynx#144](https://github.com/lynx-family/lynx/issues/144) "[Feature]: svelte guide" is open. No Svelte 5 binding was found.
- **Mainmatter:** [blog post (2025-05)](https://mainmatter.com/blog/2025/05/22/native-apps-with-svelte/); initiative site https://svelte-custom-renderers.com (sponsored by Syntax and SuppCo).
