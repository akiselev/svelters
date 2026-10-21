// The Svelte adapter of the core (plans/research/RUNTIME.md phase 1): the ONLY module of the native
// runtime that imports Svelte's runtime internals (`svelte/internal/client` and the three modules its
// exports map hides, resolved by the aliases in ../../vite.js and test/register.mjs). Everything that
// depends on Svelte's private data lives here too: the effect flag bits, the duck-typed `Boundary`
// that Svelte's error handling and scheduler call on `effect.b`, and the relinking of the effect
// tree. tools/check-svelte-internals.mjs asserts all of it against the installed Svelte.
//
// ./index.js re-exports the compiler-facing contract from here; ./internal.js the internals the
// block machinery uses. Phase 2 replaces this file with an own core behind the same two modules.
import * as $ from 'svelte/internal/client';
import { block as svelteBlock, branch as svelteBranch, component_root, destroy_effect } from 'svelte/internal/client/reactivity/effects';
import { queue_micro_task } from 'svelte/internal/client/dom/task';
import { getAllContexts, setContext } from 'svelte/internal/client/context';
import { capture, unset_context } from 'svelte/internal/client/reactivity/async';
import * as batches from 'svelte/internal/client/reactivity/batch';

// ---- the compiler-facing contract (docs/notes/native-compiler/runtime-contract.md "Core API") ----

export {
  // graph
  state,
  derived,
  derived_safe_equal,
  get,
  set,
  update,
  update_pre,
  safe_get,
  untrack,
  eager,
  pending,
  effect_tracking,
  proxy,
  snapshot,
  to_array,
  // effects
  effect_root,
  user_effect,
  user_pre_effect,
  template_effect,
  // components and props
  push,
  pop,
  prop,
  fallback,
  rest_props,
  spread_props,
  exclude_from_object,
  props_id,
  bind_this,
  attach,
  noop,
  // stores
  setup_stores,
  store_get,
  store_set,
  store_mutate,
  store_unsub,
  validate_store,
  update_store,
  update_pre_store,
  update_prop,
  update_pre_prop,
  // dev
  tag,
  tag_proxy,
  inspect,
  trace,
} from 'svelte/internal/client';

// ---- internals of the block machinery (./internal.js) ----

/** Svelte's effect flags (svelte/src/internal/client/constants.js; checked by tools/check-svelte-internals.mjs). */
export const BOUNDARY_EFFECT = 1 << 7;
export const DESTROYED = 1 << 14;
export const DESTROYING = 1 << 25;

/** `fn` in a root effect (Svelte's `component_root`); returns the dispose function. */
export const root = component_root;
/** A block effect: re-runs `fn` when what it reads changes; its branches are its children. */
export const block = svelteBlock;
/** A branch effect: the content of a block (or of the root). */
export const branch = svelteBranch;
/** Destroys an effect and its children (teardowns run). */
export const destroy = destroy_effect;
/** The effect running now, or null. */
export const active = () => $.active_effect;
/** Whether `effect` is destroyed or being destroyed. */
export const destroyed = (effect) => effect !== null && (effect.f & (DESTROYED | DESTROYING)) !== 0;
/** Runs `fn` in Svelte's microtask queue (drained by flushSync / `flush`). */
export const queue_task = queue_micro_task;
/**
 * Svelte's `capture()`: the running effect, reaction, component context and batch. The returned
 * `restore(false)` makes them current again (from a promise callback, so a branch created there
 * belongs to the captured effect) without activating the batch; `release()` clears them after.
 */
export const capture_context = capture;
export const release_context = () => unset_context(false);
/** Whether a `flushSync` is running (Svelte's `is_flushing_sync`). */
export const flushing_sync = () => batches.is_flushing_sync;
/**
 * Svelte's `createSubscriber` (`svelte/reactivity`, 5.57.2), on the client internals: Node resolves
 * the public `svelte/reactivity` to its server build (a no-op) without the `browser` condition. When
 * the returned function is called in a tracking effect, `start(update)` runs for the first subscriber
 * and its cleanup after the last one is gone (one microtask later); `update()` re-runs the readers.
 */
export function subscriber(start) {
  let subscribers = 0;
  const version = $.mutable_source(0);
  let stop;
  return () => {
    if (!$.effect_tracking()) return;
    $.get(version);
    $.render_effect(() => {
      if (subscribers === 0) stop = $.untrack(() => start(() => $.update(version)));
      subscribers += 1;
      return () => {
        queue_micro_task(() => {
          subscribers -= 1;
          if (subscribers === 0) {
            stop?.();
            stop = undefined;
            $.update(version);
          }
        });
      };
    });
  };
}
export const get_all_contexts = getAllContexts;
export const set_context = setContext;
export const flush = $.flush;
export const tick = $.tick;
export const clsx = $.clsx;

/** The members Svelte's runtime reads or calls on `effect.b` (its `Boundary` class's public members). */
export const BOUNDARY_MEMBERS = [
  'parent',
  'is_pending',
  'transform_error',
  'defer_effect',
  'is_rendered',
  'has_pending_snippet',
  'update_pending_count',
  'get_effect_pending',
  'error',
];

/**
 * An error boundary as Svelte's runtime sees it on `effect.b` (`handle_error`, `batch.js`,
 * `async.js`, `$effect.pending()`): `error(e)` handles an error of an effect inside it (or throws to
 * pass it on); synchronous only, so it is never pending. `parent`: the enclosing boundary or null.
 */
export function boundary_handler({ parent = null, transform_error, error }) {
  return {
    parent,
    is_pending: false,
    transform_error: transform_error ?? parent?.transform_error ?? ((e) => e),
    is_rendered() {
      return !this.is_pending && (!this.parent || this.parent.is_rendered());
    },
    has_pending_snippet: () => false,
    update_pending_count(d, batch) {
      this.parent?.update_pending_count(d, batch);
    },
    defer_effect() {},
    get_effect_pending: () => 0,
    error,
  };
}

/** The boundary handling errors of the running effect's content, or null. */
export const current_boundary = () => $.active_effect?.b ?? null;

/** Makes the running effect (a branch) a boundary: Svelte delivers errors of its effects to `handler.error`. */
export function boundary_scope(handler) {
  const e = $.active_effect;
  e.b = handler;
  e.f |= BOUNDARY_EFFECT;
}

/** Puts the running effect's boundary handler in place without making it a boundary (the root's). */
export function root_boundary(handler) {
  $.active_effect.b = handler;
}

/**
 * Puts `effects` (the item branches of a keyed each block, `parent` its block effect) in list order
 * in Svelte's effect tree, as Svelte's own each block moves effects, so later updates run in list
 * order. Only when the block's children are exactly those branches.
 */
export function reorder(parent, effects) {
  if (!parent || !effects.length) return;
  let n = 0;
  for (let e = parent.first; e; e = e.next) n++;
  const set = new Set(effects);
  if (n !== effects.length || set.size !== n) return;
  for (let e = parent.first; e; e = e.next) if (!set.has(e)) return;
  let prev = null;
  for (const e of effects) {
    e.prev = prev;
    if (prev) prev.next = e;
    else parent.first = e;
    prev = e;
  }
  prev.next = null;
  parent.last = prev;
}
