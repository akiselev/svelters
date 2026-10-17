# Runtime research: should svelters own its Svelte runtime? (working notes)

Incremental notes for `plans/research/RUNTIME.md`. Probes live in `docs/notes/scripts/runtime-research/`.
Date: 2026-10-17. Svelte version everywhere: 5.57.2 (stock in `node_modules/.pnpm/svelte@5.57.2`,
fork tarball `vendor/svelte/svelte-5.57.2-27b7dfb.tgz`, fork checkout `/projects/svelte-fork`).

## 1. What we import today

### Fork vs stock

`packages/native-runtime/package.json` says `"svelte": "5.57.2"` but `pnpm-workspace.yaml`
`overrides.svelte` points every `svelte` at the fork tarball. Diff of `src/internal/client` stock vs
fork: the reactivity files that we use differ only by the custom-renderer hooks (`effect.r/pr`,
`push_renderer` in `update_effect` / `destroy_effect`, all behind `custom_renderers_flag`);
`deriveds.js`, `context.js`, `dom/task.js`, `props.js` are identical. Constants we hard-code
(`BOUNDARY_EFFECT = 1<<7`, `DESTROYED = 1<<14`, `DESTROYING = 1<<25`) are the same in both.

**Probe result:** `docs/notes/scripts/runtime-research/stock-svelte-hook.mjs` redirects every
`svelte`/`svelte/...` import (and the three hidden modules) to stock 5.57.2. The native-runtime
unit suite (`packages/native-runtime/test`, 50 tests) passes 50/50 against stock. So today the
fork is not load-bearing for the native path; the risk is future drift, not present breakage.

### Imports in packages/native-runtime/src (1436 lines: index.js 1130, lists.js 306)

| import | from | used for |
| --- | --- | --- |
| `* as $` | `svelte/internal/client` (public-ish, but undocumented) | `state`/`set`/`get` (16+8+4), `untrack` 6, `template_effect` 5, `prop` 5, `active_effect` 5, `user_effect` 3, `push`/`pop`, `flush` 3, `rest_props`, `clsx`, `bind_this` |
| `block, branch, component_root, destroy_effect` | `svelte/internal/client/reactivity/effects` (HIDDEN by exports map; Vite alias + Node loader hook) | every block (`if/each/key/await/snippet/component/boundary`), mount |
| `queue_micro_task` | `svelte/internal/client/dom/task` (HIDDEN) | `<svelte:boundary>` failed-snippet scheduling |
| `getAllContexts, setContext` | `svelte/internal/client/context` (HIDDEN) | list rows (lists.js) |

Beyond imports, the runtime reaches into **Svelte's private data structures**:
- `effect.f` flag bits (BOUNDARY_EFFECT, DESTROYED, DESTROYING) hard-coded as numbers (index.js:746).
- `effect.b` is assigned a duck-typed object imitating Svelte's `Boundary` class
  (`parent, is_pending, transform_error, is_rendered, has_pending_snippet, update_pending_count,
  defer_effect, error`) at index.js:765 and the root one at index.js:1095. Svelte calls these from
  `handle_error`, `batch.js`, `async.js`; any new method Svelte adds to `Boundary` and calls breaks us.
- `relink()` (index.js:1018) rewrites the effect tree's doubly linked list (`first/next/prev/last`)
  to reorder keyed-each item effects.
- `view.v` read directly on a source (index.js boundary reset).

### What the compiler emits (`crates/svelters-compiler/src/{codegen,transform,lower}`)

Every component imports `* as $ from 'svelte/internal/client'` and `* as $n from '@svelters/native-runtime'`
(codegen/component.rs:62). `$.` names emitted (38):

reactivity: `state, derived, get, set, update, update_pre, safe_get, untrack, eager, pending,
effect_tracking, effect_root, user_effect, user_pre_effect, template_effect, proxy, snapshot, to_array`
component/props: `push, pop, prop, fallback, rest_props, spread_props, exclude_from_object, props_id,
bind_this, attach, component, noop`
stores: `setup_stores, store_get, store_set, store_mutate, store_unsub, validate_store,
update_store, update_pre_store, update_prop, update_pre_prop`
dev: `tag, tag_proxy, inspect, trace`

`$n.` names emitted (27): `element, set, assume, capture, RESTORE, attr, token, on, listen, child_attr,
bind, track, slot, append, region, if_block, each_block, key_block, await_block, snippet, component,
boundary, classes, class_name, style_text, setter_value, mount`.

`.svelte.js` modules go through `svelters-compile --module` (same `$.` surface).

Public `svelte` modules user code imports, all backed by the same internals: `svelte`
(`onMount, onDestroy, tick, flushSync, untrack, getContext/setContext/hasContext/getAllContexts,
createEventDispatcher, mount, settled, fork, getAbortSignal`), `svelte/reactivity`
(`SvelteMap/Set/Date/URL/URLSearchParams, MediaQuery, createSubscriber`), `svelte/store`
(`fromStore/toStore` use effects), `svelte/motion` (`Spring/Tween` use `state`, `render_effect`, `loop`/`raf`).
**Consequence for any own-runtime option:** these modules must be redirected too, otherwise a
`SvelteMap` read inside our effect would subscribe to Svelte's graph, not ours. Two graphs in one app
is a non-starter.

### Svelte source behind it (stock 5.57.2, `src/internal/client`, lines)

| area | files | lines |
| --- | --- | --- |
| core graph | runtime.js 868, reactivity/sources.js 411, deriveds.js 483, effects.js 796, status.js 25, equality.js 31, utils.js 18 | ~2630 |
| scheduling | reactivity/batch.js 1473, dom/task.js 42 | ~1515 |
| async (5.36+ `await`) | reactivity/async.js 424, dom/blocks/async.js 69 | ~490 |
| boundary / errors | dom/blocks/boundary.js 624 (we re-implement ~150), error-handling.js 134 | ~760 |
| props / proxy | reactivity/props.js 433, proxy.js 443 | ~880 |
| stores interop | reactivity/store.js 224, store/* ~550 | ~770 |
| context/lifecycle | context.js 236, index-client.js 255 | ~490 |
| dev | dev/inspect.js 74, tracing.js 164, errors.js/warnings.js ~830 (generated) | ~1070 |
| block machinery (DOM) | each.js 839, branches.js 299, if 72, key 40, await 155, snippet 181, svelte-component 61 | ~1650 (NOT used: we have our own in native-runtime) |
| public extras | reactivity/* ~1390 (map 296, set 213, date 122, url 207, usp 190, window 161, ...), motion ~710, shared/clone.js 139 | ~2240 |

What we actually depend on at runtime: core graph + scheduling + boundary/error hooks + props/proxy +
stores + context = **~6.5k lines**, of which batch.js (1473) is the most complex and the most
actively changing upstream (async mode, `fork`, `settled`). Block machinery is already ours (1436 lines).

### Upstream churn (fork checkout `/projects/svelte-fork`, commits touching the file)

| file | last 6 months | last 12 months |
| --- | ---: | ---: |
| reactivity/batch.js | 30 | 95 |
| runtime.js | 29 | 59 |
| reactivity/async.js | 23 | 48 |
| dom/blocks/boundary.js | 21 | 45 |
| reactivity/effects.js | 15 | 47 |
| reactivity/deriveds.js | 11 | 42 |
| reactivity/sources.js | 8 | 26 |
| proxy.js | 4 | 9 |
| context.js | 4 | 7 |
| reactivity/props.js | 3 | 5 |

The scheduler/async/boundary half is where upstream is moving (async mode, `fork`, `settled`);
proxy/props/context are stable. That split decides the hybrid below: own what churns and what we
already half-reimplement (scheduler, effect tree, boundary), port what is stable (proxy, props,
stores, context, `svelte/reactivity`, `svelte/motion`).

## 2. QuickJS measurements

Probe: `bench.mjs` + `build-bench.mjs` + adapters `svelte-entry.mjs` (stock 5.57.2 signals,
production conditions, esbuild bundle) and `mini-signals.mjs` (~150-line push-pull core using
Svelte's colouring algorithm, sync only). qjs = QuickJS-NG 0.16.2 built from
`~/.cargo/registry/.../rquickjs-sys-0.14.0/quickjs` (the engine rquickjs 0.14 embeds), Release.
Best of 5, ms. The `check` column matched between cores on every workload (same results).

| workload | Svelte qjs | mini qjs | ratio | Svelte node | mini node |
| --- | ---: | ---: | ---: | ---: | ---: |
| create 5k signal+derived+effect | 179 | 86 | 2.1x | 43 | 3.0 |
| fan-out 1 source x 1000 effects, 200 updates | 1689 | 605 | 2.8x | 172 | 24 |
| chain of 100 deriveds, 500 updates | 591 | 154 | 3.8x | 82 | 12 |
| 1000 counters (1 effect each), 2000 flushes x 5 writes | 3983 | 52 | **77x** | 178 | 3.8 |
| 200 deriveds with equal-value cut-off, 500 updates | 2024 | 553 | 3.7x | 77 | 23 |
| 500 effects with dynamic deps, 500 updates | 5478 | 3506 | 1.6x | 247 | 418 |
| 1 counter, 20000 write+flush | 1275 | 117 | 11x | | |

Reading:
- Per-flush floor: Svelte ~64 us per write+flush on qjs (batch object, tree walk, queue
  bookkeeping), mini ~6 us. Not user-visible for a click, but it is paid on every host job drain.
- The 77x case is algorithmic: Svelte's flush walks the effect tree from the root and visits every
  sibling render effect of a dirty one (1000 template effects under one parent = 1000 visits per
  flush). A queue of dirty effects is O(dirty). A big form or a long static widget list in one
  component is exactly this shape.
- Elsewhere the constant factor is 2-4x on qjs. Node's JIT hides most of it (Svelte is tuned for
  V8/JSC), the interpreter does not.
- Recursion depth: a 500-deep derived chain overflows qjs's default stack in **both** cores
  (recursive `update_derived` / `isDirty`). An own core can make the dirty check iterative.
- The mini core is a floor, not a candidate: no batches, ownership, teardown, error propagation
  or effect ordering. A real own core would land somewhere between, likely 2-3x faster than
  Svelte's on qjs for the common cases and O(dirty) per flush.

Code size (esbuild, production conditions; qjs bytecode via `qjsc -b -m -s -s`):

| bundle | source | bytecode stripped |
| --- | ---: | ---: |
| `svelte/internal/client` + `svelte`, everything (no tree-shaking), stock min | 114 KB | 157 KB |
| same, fork | 120 KB | 179 KB (unminified input) |
| only signals used by the bench (state/derived/get/set/effect_root/template_effect/flushSync), min | 43.5 KB | 57 KB |
| mini core, min | 1.7 KB | 2.3 KB |
| handwritten counter `dist/app.js` today (unminified, ~96 KB of it Svelte) | 112 KB | 54 KB |

Even the narrowest slice of Svelte's runtime drags in ~100 modules (errors, warnings, dev, async,
batch, hydration flags, custom-renderer state) because they are wired into `runtime.js`/`batch.js`.
An own core with ported periphery should be ~25-35 KB min for a typical app.

## 3. Conformance assets that already exist

- `experiments/native-compiler/conformance/` runs Svelte's `tests/runtime-runes/samples` (5.57.2,
  `dom` variant, assertions unchanged) through **our compiler + native-runtime** on a jsdom host
  (`dom-host.mjs`, spec `metadata/semantics/dom.json`). REPORT.md: 498/774 pass, 171 rejected at
  compile time (transitions, actions, spreads on natives, svg ...), 27 runtime errors, 75
  mismatches. `baseline.json` is a ratchet. Async samples are excluded (`SVELTE_NO_ASYNC=true`).
  Needs the upstream checkout at `$UPSTREAM/svelte` (default `/tmp/claude-0/up`, missing on this
  machine; `/projects/svelte-fork` has the same samples).
- `tools/oplog-compare.mjs`: compares host op logs; usable for differential testing of two runtimes
  on the same compiled output.
- Svelte's own non-component suites that exercise exactly the reactivity layer:
  `tests/signals/test.ts` (1798 lines, ~55 tests; imports internals directly; ~10 assertions inspect
  `reactions`/`deps`/`CONNECTED`/`DESTROYED`), `tests/store/test.ts` (710), `tests/motion/test.ts` (79),
  `src/reactivity/*.test.ts` (~1770: map, set, date, url, url-search-params).
- `packages/native-runtime/test` (50 tests) runs on stock and fork alike (probe above).

## 4. Prior art (short)

- **Svelte itself**: `svelte/internal/client` IS Svelte's compiler<->runtime contract, but it is
  versioned only as "same package version": compiler and runtime ship together, so internal names
  change freely between minors. The custom-renderer stack (#18042 ...) is the upstream-sanctioned
  non-DOM route, but it keeps the DOM-shaped block machinery (anchors/fragments) we already left.
- **alien-signals** (also the basis of Vue 3.6 `@vue/reactivity`): push-pull, doubly linked `Link`
  nodes between deps and subs (no arrays, no Set), iterative propagation, effect scopes. Fastest
  known JS core; the linked-list layout is interpreter-friendly (no array growth, O(1) unlink).
- **Preact signals**: versioned nodes, lazy computeds, `batch`, linked-list sources; small (~1.5 KB)
  and simple; effects are flat (no owner tree), so cleanup ordering has to be added.
- **TC39 Signals polyfill**: State/Computed + `Watcher`, no effects or scheduling policy (left to
  frameworks); derived from Angular's implementation; useful as a spec reference for
  push-pull/equality semantics, not as a runtime (slow, no ownership).
- **Solid**: owner tree (roots, computations, `onCleanup`, context on owners) + signal graph; the
  owner tree is the same idea as Svelte's effect tree and our `Scope`. Solid 2.0 is moving to
  push-pull with first-class async, the same direction as Svelte async mode.
- **Vue Vapor**: compiler emits direct DOM ops + `renderEffect`, built on the public, separately
  versioned `@vue/reactivity` package, with block machinery in `runtime-vapor`. This is precisely
  the split proposed here: our block machinery + a reactivity package with a contract we own.
- **Svelte Native / NativeScript-Svelte**: never touched internals; they let Svelte's DOM runtime
  run against a fake DOM (svelte-native's own DOM, later `dominative`/`undom`), mapping DOM nodes to
  NativeScript views. Robust to Svelte updates, but pays for a JS DOM mirror and anchor nodes:
  exactly what svelters' renderer path did and what NC1 removes.

(Prior-art notes are from memory and were not re-checked against upstream repos in this session.)

## 5. Other findings

- `vite.js` `nativeSheets` compiles with `experimental: { customRenderer: 'svelters-native' }`, an
  option only the fork has. Stock 5.57.2 throws "Unrecognised compiler option
  experimental.customRenderer". The plugin catches it and only warns `<style> not translated`, so
  switching to stock would silently drop component CSS. This must move before the fork goes.
- The qjs default stack overflows on a 500-deep derived chain (both cores, both recursive).

## 6. Outcome

Recommendation (hybrid: own contract now, own sync core + scheduler, ported periphery,
differential testing against Svelte's core), contract sketch, phasing and open questions:
`plans/research/RUNTIME.md`.

Probes (`docs/notes/scripts/runtime-research/`):
- `stock-svelte-register.mjs` + `stock-svelte-hook.mjs`: run native-runtime tests on stock svelte
  (`cd packages/native-runtime && node --import ../../docs/notes/scripts/runtime-research/stock-svelte-register.mjs --test test/*.test.mjs`).
- `build-bench.mjs <outdir>` bundles `svelte-entry.mjs` / `mini-signals.mjs`; `bench.mjs <adapter>`
  runs the workloads (qjs built from rquickjs-sys's bundled quickjs-ng with CMake target `qjs_exe`).
