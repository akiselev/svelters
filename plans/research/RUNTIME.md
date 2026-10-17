# RUNTIME: should svelters own its Svelte runtime?

Status: research proposal, 2026-10-17. Working notes, measurements and probes:
`docs/notes/runtime-research.md`, `docs/notes/scripts/runtime-research/`.

## Recommendation in one paragraph

Yes, own it, but not in one jump and not all of it. Do the **hybrid (option d)**:
1. Put our own runtime contract between the compiler and Svelte right away. The compiler stops
   importing `svelte/internal/client` and imports a `core` module that we own. At first that module
   only re-exports Svelte.
2. Write our own **synchronous reactivity core and scheduler** behind that contract. These are the
   parts that churn upstream, that we already half re-implement, and that cost the most on QuickJS.
3. **Port** the stable periphery with its imports remapped: `$state` proxy, props, stores, context,
   `svelte/reactivity`, `svelte/motion`.
4. Switch from the Svelte-backed core to the own core when both pass the same conformance ratchet
   and differential tests.

As an immediate safety net, pin the native path to stock `svelte@5.57.2` and add a contract check
(phase 0). Async mode stays out of scope until there is a reason for it.

## Why (findings)

- **Coupling is deeper than imports.** `packages/native-runtime/src/index.js` imports three hidden
  modules (`reactivity/effects`, `dom/task`, `context`) through resolver aliases. It also reaches
  into Svelte's private data in three ways:
  - It hard-codes effect flag bits (`BOUNDARY_EFFECT`, `DESTROYED`, `DESTROYING`, index.js:746).
  - It plants a duck-typed copy of Svelte's `Boundary` class on `effect.b` (index.js:765,
    index.js:1095). This copy has 8 methods that Svelte's `batch.js` and `async.js` call.
  - It rewrites the effect tree's linked list to reorder keyed-each items (`relink`, index.js:1018).

  A new `Boundary` method or a renumbered flag would break us silently.
- **The fork is not load-bearing for the native runtime today.** The probe
  `stock-svelte-hook.mjs` runs the native-runtime suite against stock 5.57.2, and all 50 tests pass.
  The reactivity files we use differ from stock only by the custom-renderer hooks, which sit behind
  a flag.
  - **The build path does depend on the fork.** `vite.js` `nativeSheets` calls
    `svelte/compiler` with `experimental.customRenderer`. Stock rejects that option ("Unrecognised
    compiler option"). The plugin catches the error and only warns `<style> not translated`, so on
    stock every component would quietly lose its CSS.
- **Upstream churn is in exactly the parts we couple to.** Commits in the last 12 months:
  `batch.js` 95, `runtime.js` 59, `async.js` 48, `effects.js` 47, `boundary.js` 45,
  `deriveds.js` 42. The parts we would port are stable: `proxy.js` 9, `context.js` 7, `props.js` 5.
- **QuickJS cost is real but not dramatic, except in one case.** Compared with a ~150-line
  push-pull core that uses the same algorithm, Svelte's signals are:
  - 2-4x slower on qjs for most workloads;
  - ~64 µs vs ~6 µs for each write+flush;
  - **77x slower on "1000 idle template effects, a few writes per flush"**, because Svelte's
    flush walks every sibling effect in the tree instead of only the dirty ones.

  Node's JIT hides most of this.
- **Code size.** A counter app today is ~112 KB unminified (~96 KB of it Svelte), 54 KB of
  stripped bytecode. Even the narrowest signals-only slice of Svelte bundles ~100 modules: 43 KB
  minified, 57 KB bytecode. An own core plus ported periphery should come to roughly 25-35 KB
  minified for a typical app.
- **Inventory size.**
  - Generated code uses 38 `$.` names and 27 `$n.` names.
  - The runtime uses 14 `$.` names plus 7 hidden imports.
  - Behind them sit ~6.5k lines of Svelte: core graph ~2.6k, scheduler ~1.5k, async ~0.5k,
    boundary and errors ~0.8k, props and proxy ~0.9k, stores ~0.8k, context ~0.5k.
  - The public extras (`svelte/reactivity` and `svelte/motion`) add ~2.2k lines.
  - The block machinery (`each`/`if`/`key`/`await`/snippet/boundary) is already ours: 1436 lines.

## Options compared

| | (a) pin stock, keep internals | (b) vendor-copy internals | (c) own runtime from scratch | **(d) hybrid** |
| --- | --- | --- | --- | --- |
| Initial effort | 1-2 days | 1.5-2.5 weeks (cut DOM/hydration/custom-renderer out of `effects.js`, `boundary.js`, `batch.js`) | 6-10 weeks to parity, including a rewrite of proxy/props/stores/reactivity classes | **4-6 weeks**, incremental, nothing blocked meanwhile |
| Fidelity | highest for reactivity (it is Svelte), until a bump | equal at copy time, then drifts in a controlled way | lowest at first; proxy and effect-ordering details are easy to get subtly wrong | Svelte's own code for the semantics-dense parts (proxy, props, stores); own core proven by differential testing against Svelte's |
| Risk | silent breakage on any bump (flags, `Boundary` methods, tree layout); async-mode work keeps reshaping `batch.js` | merges of `batch.js`/`boundary.js` are hard (DOM and async are interleaved); we would carry 4k+ lines we do not understand | long tail of semantic mismatches; delays features | own core may diverge in ordering edge cases; mitigated by running both cores on the same suites |
| Maintenance | an audit on every Svelte bump; we stay stuck on the version we audited | a diff script plus periodic hard merges | ours entirely | own core (~1.5-2k lines we understand); ported periphery tracked by a diff script (rarely changes) |
| QuickJS perf / size | unchanged (2-4x, 77x worst case; ~45-60 KB bytecode floor) | slightly smaller after stripping; same algorithm | best | best where it matters (core and scheduler); periphery the same as Svelte |
| Blocks / native fit | keeps faking `Boundary`, relinking the tree | can own `Boundary` and reorder APIs | native-first design | native-first core API (`block`/`branch`/`reorder`/`boundary` as real exports) |

(a) is a stopgap and becomes phase 0 of (d). (b) costs as much as (d) to set up but leaves us
maintaining the hardest upstream code. (c) duplicates work for no gain on the stable,
semantics-dense parts.

## The contract

The compiler keeps two namespaces, and both become ours.

```js
import * as $ from '@svelters/native-runtime/core';   // was 'svelte/internal/client'
import * as $n from '@svelters/native-runtime';       // unchanged: host ops + blocks
```

**`$` (core): the 38 names the compiler emits today.** Their Svelte names and call shapes stay, so
the codegen change is one line (`codegen/component.rs:62`) and the conformance harness keeps
working. This list becomes a documented, frozen contract: a new section "Core API" in
`docs/notes/native-compiler/runtime-contract.md`, which replaces that file's "It sits on
Svelte's own signal/effect runtime" preamble.

- graph: `state, derived, get, set, update, update_pre, safe_get, untrack, eager, pending,
  effect_tracking, proxy, snapshot, to_array`
- effects: `effect_root, user_effect, user_pre_effect, template_effect`
- component: `push, pop, prop, fallback, rest_props, spread_props, exclude_from_object, props_id,
  bind_this, attach, component, noop`
- stores: `setup_stores, store_get, store_set, store_mutate, store_unsub, validate_store,
  update_store, update_pre_store, update_prop, update_pre_prop`
- dev: `tag, tag_proxy, inspect, trace`

The surface is free to shrink later, because we own both ends. Candidates:
- `template_effect(fn)` without Svelte's async arguments;
- `pending` and `eager` as constants while there is no async mode;
- merging one element's template effects into a single effect.

**Core internals the block machinery uses.** These are exported from `core/` and are not part of
the compiler-facing contract. They replace the hidden imports and the pokes:

| new export | replaces |
| --- | --- |
| `root(fn) -> dispose` | `component_root` |
| `block(fn)`, `branch(fn)`, `destroy(effect)` | Svelte's effects plus the alias hack |
| `active()` | `$.active_effect` |
| `reorder(parent, effects)` | `relink` |
| `boundary_scope(fn, onError) / transform_error` | the fake `effect.b` and the flag bits |
| `queue_task(fn)` | `queue_micro_task` |
| `get_all_contexts / set_context` | the hidden `context` import |
| `flush()`, `tick()` | as today |

**Public modules redirected by `vite.js` aliases**, so an app has exactly one reactive graph:

| import | resolves to |
| --- | --- |
| `svelte` | `shims/svelte.js`: `onMount, onDestroy, tick, flushSync, untrack, get/set/has/getAllContexts, createEventDispatcher`; `mount`/`hydrate` throw with a pointer to `$n.mount` |
| `svelte/reactivity` | ported `SvelteMap/Set/Date/URL/URLSearchParams, MediaQuery`, `createSubscriber` |
| `svelte/store` | ported |
| `svelte/motion` | ported, with `raf`/`loop` on the host's timer |
| `svelte/internal/*` | an error naming the import, with a hint |

**Module layout** (inside `packages/native-runtime`, or whatever N1 renames it to):

```
src/index.js            $n: host ops + blocks (today's index.js, split)
src/host/*.js           element/set/on/bind/track/attr/child_attr/slot/append/region/mount
src/blocks/*.js         if/each/key/await/snippet/component/boundary
src/core/index.js       $: the contract above; phase 1 = re-exports of svelte/internal/client
src/core/graph.js       own: Source/Derived/Effect nodes, get/set/untrack, dirty check
src/core/tree.js        own: effect/owner tree, root/branch/block, teardown, reorder
src/core/scheduler.js   own: dirty queue, flush/flushSync/tick, update-depth guard
src/core/errors.js      own: boundary chain, Svelte's error codes and messages
src/core/{proxy,props,store,context,clone,dev}.js   ported from Svelte (MIT header, upstream commit noted)
src/shims/{svelte,reactivity,store,motion}.js       public modules (ported)
tools/svelte-port-diff.mjs                         diff of the ported files against the pinned upstream
```

**Design of the own core.** It uses Svelte's algorithm, with these choices for QuickJS and native:
- CLEAN / MAYBE_DIRTY / DIRTY colouring and write versions, as in Svelte.
- Deps and subscribers held in alien-signals-style linked `Link` nodes: O(1) unlink, no array
  growth.
- Iterative dirty checking, so a deep chain does not overflow qjs's stack.
- A flush that visits only dirty effects, in tree order: an "has dirty descendant" bit walked
  from the root.
- No DOM, hydration, anchors, `nodes`, transitions, or custom-renderer state.
- Flushing on a microtask, which is the host's job drain, as today.

## Migration plan

| phase | work | exit criterion |
| --- | --- | --- |
| 0 (1-2 days) | Resolve native bundles and tests against stock `svelte@5.57.2` (an npm-alias dependency such as `svelte-stock`, because `overrides.svelte` is global). Move `nativeSheets` CSS scoping off `experimental.customRenderer`, either to our compiler or to a stock compile with `css: 'external'`. Add `tools/check-svelte-internals.mjs`, which asserts the symbols, flag values and `Boundary` methods we depend on. | native-runtime tests, fixtures and the conformance ratchet are green on stock; the check fails loudly on a bump |
| 1 (2-3 days) | `src/core/index.js` re-exports exactly the 38 names. The compiler emits `@svelters/native-runtime/core`. `index.js` uses only `core/` internals; the internals that still back onto Svelte live in one adapter file, the only place that imports the hidden modules. | all suites green; `grep svelte/internal` matches one file |
| 2 (2-3 weeks) | Own `graph`/`tree`/`scheduler`/`errors` behind `SVELTERS_CORE=own` (a vite alias swap). Port the signals tests. Build the differential fuzzer. | own core passes the ported signals tests and the differential fuzzer, and passes every sample in `baseline.json` |
| 3 (~1 week) | Port proxy/props/store/context/clone/dev and the public modules onto the own core. Add the redirect aliases. Run Svelte's `store`, `motion` and `reactivity/*.test.ts` against the ports. | those suites green; one graph per app (an assertion in dev) |
| 4 (~1 week) | Make own the default. Delete the adapter, the hidden-module aliases and the fake `Boundary`. Re-measure qjs performance and bundle size. Update `runtime-contract.md`. Drop the fork once the renderer path is retired. | no `svelte/internal` import anywhere in the native path |

**Test first:** phase 0's stock run of the conformance ratchet, because it proves the safety net.
Then, in phase 2, the effect-ordering tests: the signals tests that cover `$effect.pre`, render
and user effect order, and user effects deferred to `$.pop`. Ordering is the semantics most likely
to diverge, and op-log order depends on it.

## Conformance plan

1. **Svelte's component suite through our stack (already exists).**
   `experiments/native-compiler/conformance/` runs `tests/runtime-runes/samples` through our
   compiler and runtime on the jsdom DOM host; 498 of 774 pass. `baseline.json` is the gate for
   every phase.
   - DOM-specific samples are already excluded or rejected: transitions, actions, `{@html}`, svg,
     hydration and SSR variants, spreads on natives, Svelte 4 slots.
   - Async samples are excluded via `SVELTE_NO_ASYNC=true`.
   - Point `UPSTREAM` at `/projects/svelte-fork` or a stock clone. The default `/tmp/claude-0/up`
     is missing on this machine.
2. **Svelte's reactivity-only suites.** These are DOM-free.
   - `tests/signals/test.ts` (~55 tests) runs through an import shim that maps `runtime`,
     `effects`, `sources` and `deriveds` to our core. About 10 assertions inspect
     `reactions`/`deps`/`CONNECTED`/`DESTROYED`; they get a small `core/debug.js` introspection
     API with the same meaning.
   - `tests/store`, `tests/motion` and `src/reactivity/*.test.ts` run against the ports.
   - Also run the signals suite inside qjs (a tiny `describe`/`it` shim) to catch engine
     differences.
3. **Differential testing (new, the main proof for the own core).**
   - (a) Run every fixture app and every passing conformance sample twice, with the Svelte-backed
     core and with the own core, and compare host op logs with `tools/oplog-compare.mjs`.
   - (b) A seeded fuzzer: random DAGs of states, deriveds and effects, with `$effect.pre`,
     nested roots, conditional reads, writes inside effects and teardowns. It runs the same
     program on both cores and compares the logs of effect runs and teardowns, plus thrown errors.
     Phase 1 makes this possible, because both cores implement one contract.
4. **What is hard, roughly in order:**
   - effect ordering: pre, then render, then user; tree order; init-time user effects deferred to
     `pop`; effects created during a flush;
   - unowned or disconnected deriveds: reads outside effects, reconnecting, no leaks;
   - writes inside effects and deriveds: `state_unsafe_mutation`, the
     `effect_update_depth_exceeded` loop guard, re-flush;
   - error propagation into boundaries during init vs update, reset, `transformError`;
   - writable `$derived` overrides;
   - `$state` proxy edge cases (ported, so low risk);
   - **async mode**: async deriveds, `await` in components, `pending`, `settled`, `fork`. This
     is the hardest part and is still moving upstream; it stays out of scope (the compiler
     rejects it).
   - Push-pull and dirty flags themselves are the easy part.

## Open questions

1. **Async mode.** Will native apps need `await` in components or async `$derived`
   (Svelte 5.36+)? If they will, do we design our own (simpler: no hydration, no `fork`) or track
   Svelte's? This decides whether the own scheduler must model batches.
2. **Effect order vs the JS pipeline.** Should the own core keep Svelte's update order exactly
   (template effects grouped after blocks), or adopt source order? The contract already notes that
   the two orders give the same native state.
3. **Error messages.** Do we copy Svelte's error and warning codes and messages verbatim? This is
   needed for conformance assertions and familiar to users, but it is a generated file to sync.
4. **Dev tooling.** `$inspect.trace` and signal labels (`tag`): port Svelte's dev layer, or ship a
   minimal one?
5. **Fork retirement order.** Retire `packages/renderer` first, which removes the global
   `overrides.svelte`, or add the `svelte-stock` npm alias now?
6. **Third-party libraries** that import `svelte/internal/*` or depend on Svelte's runtime identity:
   hard error, or a compatibility shim?
7. **Package naming.** N1 is renaming things; confirm the final package and subpath names before
   phase 1 changes the codegen import.
8. **Contract shrink.** Once both ends are ours, should the compiler emit fewer, coarser calls, for
   example one effect per element or `prop` getters inlined? Measure on qjs after phase 4.
