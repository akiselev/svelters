# Custom renderer PR trial (sveltejs/svelte#18042) vs DOM shim

Run on 2026-10-16. Background: `svelte5-custom-renderer.md`.

Hands-on work is in the scratchpad `/tmp/claude-1000/-projects-svelters/10c0a3c6-7a5c-4d9b-9688-b5af0249a13a/scratchpad/renderer-trial/`. Paths below are relative to it.

| File | Contents |
|---|---|
| `renderer.mjs` | Test renderer: a widget tree plus an op log |
| `shim/shim.mjs`, `shim/shim-adapter.mjs` | The earlier DOM shim (with two fixes, see §4.3), behind the same helper surface |
| `apps/*.svelte`, `apps/all.mjs`, `harness.mjs` | Test apps and scenarios |
| `build.mjs` | esbuild + svelte compiler, one bundle per backend |
| `out/pr.txt`, `out/shim.txt` | Full op streams, step by step |
| `out/compare.md` | Per-step comparison table |
| `merge-replay.sh`, `merge-replay.txt` | Conflict history |

## 1. Build under test

- **PR #18042** head `svelte-custom-renderer` @ **`ece83ddb7b063cbdeb1925a540bca8482eabc436`** (2026-10-13, "Merge remote-tracking branch 'origin/main'"). Installed from the preview build: `npm i https://pkg.svelte.dev/svelte/c/ece83ddb7b063cbdeb1925a540bca8482eabc436`. It reports version `5.57.2`.
- Merge base with `main`: `d9acd2d` (2026-10-13). The branch is **8 commits behind** `main` @ `40bb084` (2026-10-15).
- Shim baseline: stock `svelte@5.57.2` from npm, compiled with `fragments: 'tree'`.
- Building from source: see §7.4.

## 2. Stacked follow-up PRs

All three are open. Each targets the PR below it, not `main`. All were last refreshed on 2026-10-13, and every CI check passes.

| PR | Branch @ head | Size vs #18042 | What it adds |
|---|---|---|---|
| [#18405](https://github.com/sveltejs/svelte/pull/18405) "fix: properly handle interleaving renderers" | `fix-interleaving` @ `2c40417` | +1056/-200, 55 files | Mixing renderers in one tree (DOM inside custom, custom inside DOM, or two custom renderers). `custom_renderer` becomes a global option: every component pushes its renderer (DOM pushes `null`), and `push_renderer` records `parent_renderer`. Adds an optional `foreign` object on the renderer with `insertForeign`, `removeForeign`, `insertIntoForeign` and `removeFromForeign`. Without it, mixing renderers throws a descriptive error. Effects whose nodes span renderers are removed by **"segments"**, which use more memory but only apply when `foreign` is defined. It also **removes the single-node template optimisation**, so every template gets a fragment. SSR of custom-renderer components becomes a no-op. |
| [#18461](https://github.com/sveltejs/svelte/pull/18461) "chore: add `custom_renderer` flag" | `add-custom-renderer-flag` @ `fc2d5de` | +83/-33 | A tree-shakeable internal flag (`svelte/internal/flags/custom-renderer`). Compiled modules turn it on only when a renderer is configured, so plain DOM apps pay no runtime cost. The author says it must land before #18405. |
| [#18511](https://github.com/sveltejs/svelte/pull/18511) "chore: force `custom-renderer` custom condition" | `custom-condition` @ `f3de3f2` | +162/-29 | Dual modules (`svelte`, `svelte/store`, `svelte/reactivity`) resolve to the client build under the `browser` **or** the new `custom-renderer` export condition. Under any other condition, the server fallback throws `custom_renderer_unavailable_on_server`. Node users must run `node --conditions=custom-renderer`. This replaces closed #18505. |

**Relevance to svelters:**
- We need **#18461**, for zero cost and correctness.
- We need **#18511**. We bundle anyway, but the condition name is part of the API we would rely on.
- We **do not need #18405** functionally, because we never interleave with the DOM. It still changes internals we depend on: every template becomes a fragment, and the renderer object gets a new optional `foreign` field.

The top of the stack (`custom-condition`) is what [gpuix-svelte](https://github.com/khromov/gpuix-svelte) vendors. Against the merge base it is 278 files, +5472/-593.

## 3. Test renderer

`renderer.mjs` implements all 19 methods over plain `WNode {id, kind, tag, props, children, parent, listeners}` objects. Every mutating call is logged as one op line, and read calls are counted. It is about 150 lines, roughly the same size as the shim, but needs no globals or prototypes. Ops:

- `create #id <tag>|text|comment|fragment`
- `insert` / `move #id into #parent before #anchor` (`move` means the node already had a parent)
- `set #id.key = value [type]`, `unset #id.key`
- `text #id = "…"`
- `remove #id`
- `listen` / `unlisten #id type`

Host helpers:
- `emit(node, type, ...args)` simulates a native signal and returns the handlers' return values.
- `dump`, `count` and `find` inspect the tree.

The shim side is wrapped by `shim-adapter.mjs`, which has the same helper surface. It logs `create` (via `document.createX`), `clone`, `insert`, `remove`, `attr`, `text` and `listen`. Both backends run the same `apps/all.mjs` through `build.mjs <pr|shim>`.

A PR-only variant is used where a feature is forbidden: `X.pr.svelte` replaces `bind:this` with `{@attach}`.

## 4. Test apps: behaviour and op streams

Apps covered (all in `apps/`):

| App | What it covers |
|---|---|
| Counter | `$state`, `onclick`, `disabled` |
| If | `if` / `else if` / `else` |
| Each | Keyed: reverse, insert, remove, rename |
| Snippet | Two `@render` calls |
| Key | `{#key}` |
| Await | pending, then, catch, re-pending |
| BindThis | `bind:this` |
| Props | Child component props, callback prop, children snippet |
| Events | Multi-arg handlers, return values, `preventDefault` |
| Values | Prop value types, through spread |
| Values2 | Prop value types, without spread |
| Directives | `class:` / `style:` |
| Attach | `use:` / `{@attach}` |
| Form | Two-way binding through a wrapper |
| ShimApp1/2 | The original shim-trial `App.svelte` / `App2.svelte` |

**Behaviour.** Behaviour is the same on both backends for every app, in prod and dev builds. The resulting trees match node for node at every step: same elements, same text nodes, same anchors (`out/compare.md`). Unmount leaves the target empty on both. The only exceptions are the PR features covered in §5.

### 4.1 Node counts (anchors)

Anchors are still created with the PR. Counts are identical to the shim:

- **Mount anchor:** 1 empty text node `""` appended to the target per `mount`, removed on unmount.
- **`{#if}`, `{#key}`, `{#await}`, `{@render}`, component:** 1 empty comment each, placed after the block content. Snippet: 2 `@render` calls give 2 comments. A `Child` component with a children snippet gives 2 comments.
- **Keyed `{#each}` as the only child:** uses an empty **text** anchor in the parent, not a comment.
- **Keyed insert:** creates an offscreen fragment holding an empty text anchor. That anchor never reaches the tree and is garbage after the insert.
- **ShimApp1:** 6 elements, 8 text nodes, 2 comments on both backends. ShimApp2: 4 / 5 / 3.

So the native backend still has to treat comments and empty text nodes as invisible anchors. The renderer API explicitly allows comments to be non-rendered. There is no per-block saving.

### 4.2 Op streams: where they differ

| | PR renderer | Shim |
|---|---|---|
| Template instantiation | Builds every instance from scratch with `createFragment`, `createElement`, `createTextNode` and `insert`. **No cloning**: `clone_node` throws under a renderer. Each element with children is built through its own temporary fragment (Counter mount: 3 fragments for 2 elements). | Builds the template once, then `cloneNode(true)` for each instance. The deep clone hides creates, so a native backend would have to defer widget creation until insert. |
| Ops per instance | More create/insert ops. For example, `{#if}` toggle: PR 4 create + 4 insert; shim 1 clone + 2 insert. Each insert at 1: PR 6/6; shim 1 clone + 2. | Fewer but coarser. |
| Static attributes | `set` before the node is inserted, during template build | Copied by clone |
| Moves | Explicit `move` (insert of a parented node). Keyed reverse is 2 moves on both backends. | Logged as `remove` + `insert` |
| Listeners | One `listen` per element and event, **nothing on the root**. Attached before the element is inserted. | Delegated events add 2 root listeners per event type (target + `document`). Delegated handlers live in an expando, not in `addEventListener`. |
| Reads | `getFirstChild` / `getNextSibling` / `getParent` / `getNodeValue` / `nodeType` (counts in the outputs). No `nodeName` or `namespaceURI`. | Getter-based reads through `Node.prototype` |

Fragments are pure JS bookkeeping in both cases. The PR's extra create/insert ops on fragments never need to reach a native toolkit if the host keeps fragments in JS. The op stream for real widgets is clean:

- create widget
- set props
- insert into parent before anchor
- later `set`, `text`, `move` and `remove`

### 4.3 Shim bugs found while porting (fixed in the scratchpad copy)

The DOM-shaped shim has to copy DOM edge cases exactly. Its first real keyed-each test hit two bugs:

1. **`before()` computed the index before detaching a node from the same parent.** Keyed `{#each}` reverse then produced the wrong order (c, b, anchor, a).
2. **`before()` on a parentless node threw.** In the DOM it is a no-op, and keyed `{#each}` relies on that: it renders new items against an offscreen `create_text()` anchor.

The renderer API states these semantics in its contract (`insert(parent, node, anchor)`), so this class of bug is much less likely there.

## 5. Native-widget concerns

| Concern | PR (#18042 @ ece83dd) | Shim (5.57.2) |
|---|---|---|
| **Element `bind:`** | **Compile error** `incompatible_with_custom_renderer` for **every** element `bind:`, **including `bind:this`**. Component `bind:prop` and component `bind:this` are allowed. | `bind:value` on a non-HTML tag is also a compile error (`bind_invalid_target`). Svelte only allows it on input/textarea/select. `bind:this` works. |
| **Two-way binding** | **Works with a wrapper component** (`apps/Entry.svelte` + `Form.svelte`): `let { value = $bindable('') } = $props()` → `<entry text={value} onchanged={(t) => (value = t)} />`; the parent uses `<Entry bind:value={name} />`. Trace: the native signal `emit(entry,'changed','Bob')` gives the op `set entry.text="Bob"` and `set label.text="Hello Bob"`, and the parent's `name` becomes `Bob`. A parent write gives the reverse. The native widget gets its value back once (echo). The host should skip a `set` that equals the current widget value, to avoid cursor jumps. | Same wrapper pattern works, but the handler receives a DOM-style event object (`e.args[0]` in our shim), not the signal args. |
| **Element refs** | Use `{@attach (node) => { el = node; return () => el = undefined }}` instead of `bind:this`. Tested in `BindThis.pr.svelte` and `ShimApp2.pr.svelte`. | `bind:this` |
| **Event handler args** | **Raw passthrough.** In renderer mode `create_event` calls `handler.apply(this, args)` with whatever the host passes. `emit(w,'close-request',{widget:1},7)` arrived as 2 args. There is no event object unless the host makes one. | One DOM-like event object. It must have `type`, `target`, `composedPath()` and a writable `cancelBubble`, and Svelte redefines `currentTarget`. |
| **Return values** | **Propagated.** The renderer's `addEventListener` handler returns the user handler's return value (`true`, `'handled'`, `42` observed). This maps to GTK signal return values. | Lost. Delegation discards them. |
| `preventDefault` | No special handling. The host passes an object with `preventDefault` if it wants that; the handler's call reached our object. | Only works if the shim implements it. Delegation reads `cancelBubble`. |
| **Delegation** | **None.** Every `on*` attribute becomes `renderer.addEventListener(el, type, handler, {})` on the element. Event names may contain dashes (`onclose-request` → `close-request`). Options are always `{}` unless capture/passive. There is no bubbling unless the host implements it. Legacy `on:click` is a compile error. | `click`, `input`, … are delegated to the root and to `document`. Others get per-element listeners. `pointer*`/`touch*`/`wheel` are attached in a microtask. |
| **Non-string prop values** | **Passed intact** to `setAttribute`: number, boolean, object, array, function (`Values2`: `fn = fn cb [function]`, `model = object {"rows":1}`). `null`/`undefined` become `removeAttribute`. `value`/`checked` go through `setAttribute('value', v)` and `setAttribute('checked','')`/`removeAttribute('checked')`, so `checked` is **not** a boolean. **Bug:** inside an element with a **spread** (`{...rest}`), function-valued non-`on*` props are **silently dropped** (`set_attributes` → `typeof value !== 'function'` guard). | Strings only through `setAttribute` (`String(v)`), unless the shim defines setters that `get_setters` finds on the prototype chain. |
| `class:` / `style:` | **Work**, but flattened to strings: `set class = "active"`, `set style = "color: red"`. `class={{...}}` → `clsx` string. There is no structured style map. A native backend would parse CSS text or not use `style:`. | Work through `classList.toggle` / `style.setProperty` (structured), if the shim implements them |
| `use:` / `{@attach}` | **Work.** The action gets the `WNode` and its `update`/`destroy` run. The attachment reruns on dependency change, with cleanup. | Work |
| HMR | **Works.** `compile({dev, hmr})` emits `$.hmr(...)` plus `import.meta.hot.accept`. With `import.meta.hot` rewritten to a host registry (`build.mjs`), a hot swap replaced the subtree and the new version kept handling events. State resets, as in Svelte 5 generally. Op cost: 1 `remove` + a full rebuild. | Works the same way |
| `mount` / `unmount` | `mount(App, { target: anyWNode, renderer, props })` works on a node from our tree. `anchor` is typed. `unmount` leaves 0 nodes and calls `remove` on top-level nodes only. Children are dropped implicitly, so the host must free subtrees itself. | Same, plus the target needs to be "connected" for delegation |
| Other forbidden features | transitions, `animate:`, `svelte:window`/`head`/`body`/`document`, `{@html}`, `createRawSnippet`, `on:` directive, hydration, `css:'injected'`. `<svelte:element>` compiles. | All compile. They only fail at runtime if the shim lacks the DOM APIs. |
| Globals needed | **None.** The renderer-only bundle runs in bare quickjs-ng with no `window`, `document` or `navigator` (§6). | `window`, `document`, `Node`, `Element`, `Text`, `HTMLMediaElement`, `navigator.userAgent`, and prototype accessors |

**PR bugs found:**
1. **Compiler crash:** an inline `{#snippet failed()}` or `{#snippet pending()}` inside `<svelte:boundary>` with a custom renderer and `experimental.async` off throws `Cannot read properties of undefined (reading 'body')` (`SvelteBoundary.js:95`, `snippet_fn.body`). This happens because the snippet is wrapped in `renderer_snippet(...)`. Passing the snippet as an attribute (`<svelte:boundary {failed}>`) works.
2. **Spread drops function props** (above).
3. **Unreachable `$$pop_renderer()`:** every component that ends with `return $.pop($$exports)` places `$$pop_renderer()` after the `return`, in 13 of our compiled components. It is harmless when only one renderer is used. It may matter for interleaving, but #18405 reworks this anyway.

## 6. QuickJS

I used quickjs-ng **v0.17.0** (`qjs-linux-x86_64` from the GitHub release, sha256 `0bfc0251…ceace5`, in `qjs-dl/`).

- **Both bundles run unmodified in qjs**, prod and dev, PR and shim (bundled with esbuild: `format: esm`, conditions `browser` + `production`/`development`). Output diffs are **byte-identical to Node**, apart from the read counters.
- `qjs-prelude.mjs` only maps `setTimeout`/`clearTimeout` from `qjs:os`; `queueMicrotask` is built in. The prelude is only needed for the `{#await}` test's timers.
- **PR bundle in bare qjs (no prelude):** 12 apps that don't use `{#await}` ran with no host globals at all (`qjs-bare.mjs`).
- HMR bundle also runs in qjs, with identical output.
- Bundle sizes (unminified, all apps plus runtime):

  | Build | PR | Shim |
  |---|---|---|
  | prod | 240 KB | 271 KB |
  | dev | 257 KB | 286 KB |

## 7. Fork maintainability

### 7.1 Diff size

Measured against the merge base `d9acd2d`:

| Branch | Files | Lines | `packages/svelte/src` only |
|---|---|---|---|
| #18042 (`svelte-custom-renderer`) | 248 | +4357/-517 | **76 files, +1954/-489** |
| Whole stack (`custom-condition`) | 278 | +5472/-593 | 83 files, +2463/-538 |

About 60% of #18042 is tests: 47 renderer samples in `tests/custom-renderers/samples`, plus snapshot updates.

**Largest source changes in #18042:**
- `dom/operations.js` +578: every DOM call routed through `current_renderer`
- `render.js`
- `blocks/branches.js`
- `elements/attributes.js`
- `dom/template.js`
- `blocks/each.js`
- `custom-renderer/types.d.ts`
- `index.d.ts`
- `blocks/boundary.js`
- `elements/events.js`
- `blocks/snippet.js`
- compiler: `validate-options`, `RegularElement` (analyze and transform), `transform-client`

It is a **cross-cutting** change to the client runtime: almost every block and element helper is touched.

**History:**
- 177 commits on the branch: 112 non-merge, nearly all by paoloricciuti, plus 3 by Rich Harris.
- 68 merges of `main` into the branch. The history is merge-based, so a literal `git rebase` would replay 112 commits through 68 merge points and is not practical. Use merges (or squash) instead.

### 7.2 Conflicts with `main`

`merge-replay.sh` replays each historical "merge main into the branch" with `git merge-tree` and counts conflicted files:

- **66 merges (Jan to Oct 2026): 17 had conflicts, 26 conflicted files in total**, never more than 3 in one merge.
- Since July: 20 merges, **8 with conflicts**, roughly one every 1–2 weeks.
- Lag between merges was usually 1–14 `main` commits (max 52).

Conflicts land repeatedly in the same hot files:

| File | Merges with a conflict |
|---|---|
| `dom/elements/attributes.js` | 4× |
| `dom/blocks/boundary.js` | 3× |
| `svelte-element.js` | 2× |
| `events.js` | 2× |
| `operations.js` | 2× |
| `each.js`, `render.js`, `css.js`, `css-props.js`, `async.js`, `await.js`, `branches.js`, `svelte-head.js`, `client/index.js`, `transform-template/index.js` | 1× each |
| generated error files (`errors.js`, `client-errors.md`, and the docs copy) | 1× (one merge) |

These are also the files where `main` is busiest: async and boundary work.

**Today:** merging `main` @ `40bb084` (8 commits ahead) into each of the four branches is **conflict-free** (`git merge-tree` exit 0 for #18042, #18405, #18461 and #18511).

### 7.3 CI and review status

**CI:** all checks are green on #18042 head `ece83dd` and on #18511 head `f3de3f2`:
- Tests on Node 20 for ubuntu, macOS and windows, and on Node 22 and 24
- TestNoAsync, Lint, TSGo, Benchmarks, CodeQL

**API stability, from maintainer comments:**
- **Rich-Harris, 2026-05-28:** "Implementation-wise this basically seems... ready?". His API "niggle" led to a breaking change in June: `createRenderer(...).render` was removed in favour of `mount(App, { renderer })`. paoloricciuti pinged NathanWalker and kevmodrome about it.
- **Rich-Harris, 2026-04/05 reviews:** the renderer generics were reshaped from four positional type arguments into a `RendererNodes` map ("too many… not future-proof"). He suggested `globalThis` instead of `WeakMap`s, plus naming nits.
- **On transitions**, Rich wrote "strong agree: transitions in their current form are absolutely not something we want to bring with us". The plan is to give attachments transition-like hooks later.
- **paoloricciuti, 2026-08-13:** the team is focused on SvelteKit 3, and "as soon as that's out Rich is gonna review it".
- The stacked PRs **change the renderer contract**:
  - #18405 adds `foreign`.
  - #18511 adds the required export condition.
  - #18461 adds the flag.

**Expectation:** the 19 core methods look settled. `types.d.ts` on #18042 last changed on 2026-05-28 (#18244, type-argument reshape); #18405 adds +72 lines to it (the `foreign` API). Expect additive changes (`foreign`, conditions) and possibly renames during Rich's final review. There is no merge date. It is blocked behind SvelteKit 3.

### 7.4 Building from source

Verified on this machine, at `ece83dd` in `svelte/` (the scratchpad clone):

```sh
pnpm install --frozen-lockfile        # 34 s
cd packages/svelte && pnpm build      # rollup + generate (types, messages, browser-support scan)
pnpm pack                             # → svelte-5.57.2.tgz
```

`pnpm build` also runs a slow browser-support fixture scan (`generate:browser-support`, several minutes). For a vendored tarball, `rollup -c && node scripts/process-messages && node scripts/generate-types.js` is enough.

**Result:**
- Install and build exit 0. The full `pnpm build` took about 15 minutes, most of it the browser-support scan.
- The tarball is `svelte-5.57.2.tgz` (755 KB).
- Our apps against the source-built tarball (`src/`, `out/src.txt`): output is **byte-identical** to the pkg.svelte.dev preview build.

### 7.5 Top of the stack (`custom-condition` @ `f3de3f2`)

Installed from `https://pkg.svelte.dev/svelte/c/f3de3f236bd586c764fd48bab04f41c54c5ff80d` (`stack/`, `out/stack.txt`):

- **All apps pass.** The op streams are identical to #18042 apart from node ids, even with the single-node template optimisation removed by #18405.
- The `browser` condition still works.
- The run in qjs is identical to Node.
- **Unreachable `$$pop_renderer()`: fixed** on the stack (the compiled output no longer has it after `return`).
- **Boundary inline-snippet compiler crash: still present.**
- **Spread drops function props: still present.**

## 8. Pros/cons and recommendation

| | DOM shim on stock Svelte 5.57.2 | PR #18042 renderer (fork) |
|---|---|---|
| **Availability** | npm release; follows Svelte patch releases for free | Unreleased; we vendor a tarball and merge `main` ourselves |
| **Host contract** | Implicit: whatever subset of the DOM the runtime happens to touch. It must mimic DOM edge cases exactly; we found 2 ordering and no-op bugs in one keyed-each test (§4.3). Any Svelte patch can start touching a new DOM API. | Explicit 19-method interface with types. Svelte maintainers test against it (47 samples). |
| **Globals** | `window`, `document`, `navigator`, `Node`/`Element`/`Text` prototypes with accessor getters, `HTMLMediaElement` | None. Runs in bare quickjs-ng. |
| **Prop values** | Stringified by `setAttribute`. Typed props need prototype-setter tricks (`get_setters`). | Passed intact: numbers, objects, functions. Exception: spread drops functions (bug). |
| **Events** | DOM delegation (root and document listeners, `composedPath`, `cancelBubble`). Single event object; return values lost. | Direct per-element listeners. Raw native signal args, return values propagated. No delegation. |
| **Templates** | Built once and deep-cloned, so widget creation must be deferred until insert | Built fresh per instance with explicit create/insert ops; no clone path to support |
| **Two-way binding** | Element `bind:value` only for input/textarea/select tags; otherwise a wrapper | Element `bind:` forbidden; wrapper component with `$bindable` works (§5) |
| **Element refs** | `bind:this` | `{@attach}` only (`bind:this` forbidden) |
| **`class:` / `style:`** | Structured (`classList`, `style.setProperty`) | Strings (`class="a b"`, `style="color: red"`) |
| **Feature surface** | Everything compiles; DOM-only features fail at runtime | Transitions, animations, `svelte:window`/`head`, `{@html}` and legacy `on:` are compile errors (clear, early) |
| **Anchors and node count** | Same | Same (no savings) |
| **HMR, dev mode, QuickJS** | Work | Work |
| **Maintenance** | Shim upkeep when Svelte internals change; silent breakage risk | Merge `main` into the fork: conflicts in ~1 of 3 merges, 1–3 files, concentrated in `attributes.js` and `boundary.js`; currently clean |
| **Upstream trajectory** | Svelte is moving toward the renderer API; DOM internals are not a contract | Likely to land (Rich: "basically ready"), after SvelteKit 3. Additive API changes expected (`foreign`, export condition). |
| **Known bugs** | Our own shim bugs | Boundary inline-snippet crash (non-async), spread drops functions, unreachable `$$pop_renderer` (fixed on the stack) |

**Recommendation: use the PR.**

- The things that matter most for native widgets are clearly better with the renderer API:
  - typed prop values;
  - raw signal args and return values;
  - no DOM globals;
  - an explicit, upstream-tested contract.
- The losses are:
  - `bind:this`, which `{@attach}` covers;
  - element `bind:value`, which isn't usable on custom tags in either approach;
  - structured `style:`, which we probably don't want for native widgets anyway.
- Maintenance cost is modest and measurable: about one small conflict every 1–2 weeks, in a known set of files.
- **Keep the shim harness** (`shim/` + `apps/all.mjs`) as a fallback. If the PR stalls for a long time or changes direction, the same apps run on stock Svelte.

**Before relying on it, file upstream:**
1. The boundary inline-snippet crash.
2. Spread dropping function props. Until fixed, avoid spreads that carry callbacks, or pass callbacks as `on*` props.

### Fork-tracking procedure

**Pin:** the **top of the stack, `custom-condition` (#18511)**, currently `f3de3f236bd586c764fd48bab04f41c54c5ff80d`.
- It contains #18042, #18405, #18461 and #18511, which is what upstream intends to merge, and the same base gpuix-svelte vendors.
- It already fixes the `$$pop_renderer` issue.
- Record the pinned SHA in the repo (for example `vendor/svelte/PIN`), together with the tarball's sha256.

**Our fork:**
1. Fork `sveltejs/svelte` and create a branch `svelters/renderer` from `origin/custom-condition`.
2. Carry any local fixes (for example the spread-function fix) as separate commits on top, each linked to an upstream issue or PR.

**Build and publish locally (tarball in the repo):**

```sh
git -C svelte-fork checkout svelters/renderer
pnpm -C svelte-fork install --frozen-lockfile
(cd svelte-fork/packages/svelte && npx rollup -c && node scripts/process-messages && node scripts/generate-types.js && pnpm pack --pack-destination ../../../vendor/svelte)
# package.json in svelters:
#   "svelte": "file:vendor/svelte/svelte-5.57.2-<shortsha>.tgz"
#   (rename the tarball to include the SHA so lockfiles change when it changes)
```

- **Bundling:** use export conditions `custom-renderer` (or `browser`) plus `production`/`development`.
- **Tarball over `pnpm link`:** the tarball is reproducible for CI and for other agents. `pnpm link` is only for a developer iterating on the fork.
- **Fallback:** to skip the local build, a pkg.svelte.dev URL for the exact commit works: `https://pkg.svelte.dev/svelte/c/<sha>`. Download it into `vendor/` rather than depending on the URL.

**Weekly agent job** (bounded: one merge, one test run, one report; stop and ask on anything non-trivial):

1. **Fetch.** `git fetch upstream main svelte-custom-renderer fix-interleaving add-custom-renderer-flag custom-condition`, then record the upstream SHAs.
2. **Check whether upstream already merged.**
   - If upstream `custom-condition` moved, it has been refreshed against `main` (this happens about weekly). Fast-forward or merge it into `svelters/renderer` first.
   - If #18042 merged into `main`, switch the pin to an npm release and retire the fork.
3. **Merge `upstream/main` into `svelters/renderer`.** Use a merge, not a rebase: the branch history is merge-based.
   - Before merging, `git merge-tree --write-tree --name-only HEAD upstream/main` gives the conflict count.
   - If there are conflicts, resolve them only when they are mechanical (imports, renamed helpers, generated `errors.js`/`*.md`, which should be regenerated with `node scripts/process-messages`).
   - Otherwise stop and report the files. Expected hot spots: `attributes.js`, `boundary.js`, `svelte-element.js`, `events.js`, `operations.js`.
4. **Build:** `pnpm install`, then the build in packages/svelte, then `pnpm test custom-renderers`, the upstream renderer suite. It must be green; at `ece83dd` it ran 4 files and 87 tests in 15 s.
5. **Run our apps** with the new tarball:
   - `node build.mjs <dir> apps/all.mjs out/x.mjs`, then run under Node **and** qjs.
   - Diff the op streams against the committed golden output with node ids stripped (`sed 's/#[0-9]*//g'`).
   - Any diff is reported, not auto-accepted.
6. **Publish:** pack the tarball, update `vendor/svelte/PIN` and the tarball, and open a PR in svelters. The PR lists:
   - the upstream SHAs;
   - the conflict count and files;
   - the test results;
   - any op-stream diffs.
7. **Check the upstream threads** of #18042, #18405, #18461 and #18511 for new comments or force-pushes that change the API. Summarise them in the PR.

Prerequisite for step 5: move `renderer.mjs`, `harness.mjs`, `apps/` and the golden op streams from the scratchpad into the repo (for example `tests/renderer-conformance/`). That is a code change outside `docs/notes/`, so it was not done in this trial.
