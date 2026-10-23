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
import {
  block as svelteBlock,
  branch as svelteBranch,
  component_root,
  destroy_effect,
  pause_effect,
  resume_effect,
} from 'svelte/internal/client/reactivity/effects';
import { loop as svelteLoop } from 'svelte/internal/client/loop';
import { set_should_intro } from 'svelte/internal/client/render';
import { queue_micro_task } from 'svelte/internal/client/dom/task';
import { getAllContexts, setContext } from 'svelte/internal/client/context';
import { capture, unset_context } from 'svelte/internal/client/reactivity/async';
import * as batches from 'svelte/internal/client/reactivity/batch';
import { set_is_destroying_effect, set_untracked_writes } from 'svelte/internal/client/runtime';
import { enable_tracing_mode_flag } from 'svelte/internal/flags/index';
import { init_array_prototype_warnings } from 'svelte/internal/client/dev/equality';
import { STATE_SYMBOL } from 'svelte/internal/client/constants';

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
  attachment,
  action,
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
  mark_store_binding,
  // dev
  tag,
  tag_proxy,
  inspect,
  trace,
  wrap_snippet,
} from 'svelte/internal/client';

/** HMR (dev): Svelte's `HMR` / `FILENAME` symbols, so upstream tests and tools that read `C[HMR]` run unchanged. */
export const HMR = $.HMR;
export const FILENAME = $.FILENAME;

// ---- internals of the block machinery (./internal.js) ----

/** RT1: `v` is a `$state` proxy (deep state: an array or a plain object). */
export const is_state_proxy = (v) => STATE_SYMBOL in v;
export { STATE_SYMBOL };

/** Svelte's effect flags (svelte/src/internal/client/constants.js; checked by tools/check-svelte-internals.mjs). */
export const BOUNDARY_EFFECT = 1 << 7;
export const DESTROYED = 1 << 14;
export const DESTROYING = 1 << 25;
/** TR1: a paused (outroing) subtree; Svelte's scheduler skips it. */
export const INERT = 1 << 13;
/** TR1: the block effect has run once (local intros only play in blocks that already ran). */
export const REACTION_RAN = 1 << 15;
/** TR1: a block that is looked through for local transitions (else-if, snippet, dynamic component, boundary). */
export const EFFECT_TRANSPARENT = 1 << 16;
/** FX6: the effect kinds and statuses `recover` reads and repairs. */
export const BRANCH_EFFECT = 1 << 5;
export const ROOT_EFFECT = 1 << 6;
export const CLEAN = 1 << 10;
export const DIRTY = 1 << 11;
export const MAYBE_DIRTY = 1 << 12;

/** FX6: the root effects of the live `root`s (what `recover` repairs). */
const liveRoots = new Set();

/** `fn` in a root effect (Svelte's `component_root`); returns the dispose function. */
export function root(fn) {
  let effect = null;
  let dispose;
  try {
    dispose = component_root(() => {
      effect = $.active_effect;
      liveRoots.add(effect);
      return fn();
    });
  } catch (e) {
    // a first run that threw: Svelte destroyed the effect already
    liveRoots.delete(effect);
    throw e;
  }
  return (options) => {
    liveRoots.delete(effect);
    return dispose(options);
  };
}
/** A block effect: re-runs `fn` when what it reads changes; its branches are its children. */
export const block = svelteBlock;
/** A branch effect: the content of a block (or of the root). */
export const branch = svelteBranch;
/** Destroys an effect and its children (teardowns run). */
export const destroy = destroy_effect;
/**
 * TR1: Svelte's `pause_effect(effect, done, destroy)`: marks the subtree inert, plays the out
 * transitions that belong to it (local ones up to the next non-transparent block, global ones
 * anywhere), then destroys it (when `destroy`) and calls `done`. Synchronous without transitions.
 */
export const pause = pause_effect;
/** TR1: Svelte's `resume_effect`: un-pauses a paused subtree and plays its intros (reversal). */
export const resume = resume_effect;
/** TR1: whether `effect` is paused or inside a paused subtree (outroing). */
export const inert = (effect) => effect !== null && (effect.f & INERT) !== 0;
/** TR1: Svelte's `should_intro` (render.js): intros of the root component's first render play when true. */
export const set_intro = set_should_intro;
/** TR1: Svelte's `raf` (timing.js: `tick` = requestAnimationFrame, `now` = performance.now in the browser build). */
export const raf = $.raf;
/** TR1: Svelte's `loop(fn)`: `fn(now)` on every frame of `raf` until it returns false; `{ abort }`. */
export const loop = svelteLoop;
/** TR1: Svelte's `$.transition` / `$.animation` (dom/elements/transitions.js) on a runtime node. */
export const transition = $.transition;
export const animation = $.animation;
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
/** Svelte's `svelte/internal/flags/tracing`: sources record what `$.trace` reports (dev). */
export const enable_tracing = enable_tracing_mode_flag;
/** Whether a `flushSync` is running (Svelte's `is_flushing_sync`). */
export const flushing_sync = () => batches.is_flushing_sync;

/**
 * FX6 (plans/milestones/FX6-embed-destroy-mutation.md): puts Svelte's runtime back at rest after
 * the host aborted an entry into JS. QuickJS stops a script over its turn budget (or on the
 * interrupt flag) with an uncatchable error that skips every `finally`, so Svelte's module state
 * stays as it was mid-update: the running reaction, effect and component context, the batch being
 * flushed, `is_processing`, and the branches its traversal had not reached yet without `CLEAN`.
 * Left so, the next write outside any effect fails with `state_unsafe_mutation` (the reaction is
 * still "active") and no later batch reaches the effects below those branches.
 *
 * Runs no component code: it clears the context (Svelte's own `unset_context`), drops the aborted
 * batch (`clear`, Svelte's test reset) and flushes an empty one, whose `finally` resets the
 * scheduler's flags. Then it repairs the live roots' effect trees as a finished traversal leaves
 * them, every branch `CLEAN`, and schedules the effects still dirty, so the work the aborted flush
 * had not done runs in a later microtask (under a new budget). Inert (outroing) subtrees are left
 * as they are. Not reset: `is_flushing_sync` (no writer outside `flushSync`), see the milestone.
 * Since FX6b the embed's host calls flush in the host's job drain, not with `flushSync`, so only a
 * component's own `flushSync` can still leave it set.
 */
export function recover() {
  unset_context(true);
  set_is_destroying_effect(false);
  set_untracked_writes(null);
  batches.clear();
  batches.Batch.ensure().flush();
  const dirty = [];
  for (const r of liveRoots) {
    if ((r.f & (DESTROYED | DESTROYING)) !== 0) {
      liveRoots.delete(r);
      continue;
    }
    r.f = (r.f & ~(DIRTY | MAYBE_DIRTY)) | CLEAN;
    // depth first, as Svelte's #traverse walks
    let e = r.first;
    while (e !== null) {
      const f = e.f;
      const skip = (f & (INERT | DESTROYED | DESTROYING)) !== 0;
      if (!skip) {
        if ((f & (BRANCH_EFFECT | ROOT_EFFECT)) !== 0) e.f = (f & ~(DIRTY | MAYBE_DIRTY)) | CLEAN;
        else if ((f & (DIRTY | MAYBE_DIRTY)) !== 0) dirty.push(e);
        if (e.first !== null) {
          e = e.first;
          continue;
        }
      }
      while (e !== null && e !== r) {
        if (e.next !== null) {
          e = e.next;
          break;
        }
        e = e.parent;
      }
      if (e === r) break;
    }
  }
  if (dirty.length === 0) return;
  batches.Batch.ensure();
  for (const e of dirty) batches.schedule_effect(e);
  // Batch.ensure queued the flush, unless the abort left `is_flushing_sync` set
  if (batches.is_flushing_sync) queueMicrotask(() => batches.flushSync());
}
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
/**
 * Links an each item's source to `collection()[index]` for `$inspect.trace` (Svelte's each block
 * sets `v.trace` in DEV): a traced read of the item reports the collection's sources instead.
 */
export function trace_item(source, read) {
  source.trace = read;
}
/** A source whose writes always notify for objects (Svelte's `mutable_source`: `safe_equals`). */
export const mutable_state = (v) => $.mutable_source(v);
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

let devInitialized = false;
/**
 * What Svelte's DOM `init_operations` installs in DEV besides DOM getters: the `Array.prototype`
 * patches that warn `state_proxy_equality_mismatch` (`proxy.includes(raw)`). Once per realm.
 */
export function init_dev() {
  if (devInitialized) return;
  devInitialized = true;
  init_array_prototype_warnings();
}
