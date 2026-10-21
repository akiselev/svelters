// Core internals the native runtime's block machinery uses (../index.js, ../lists.js): the effect
// tree, scheduling, contexts and error boundaries. Not part of the compiler-facing contract
// (./index.js). Phase 1 of plans/research/RUNTIME.md: backed by Svelte through the adapter.
export {
  root,
  block,
  branch,
  destroy,
  destroyed,
  active,
  reorder,
  boundary_handler,
  boundary_scope,
  current_boundary,
  root_boundary,
  queue_task,
  subscriber,
  get_all_contexts,
  set_context,
  flush,
  tick,
  clsx,
} from './svelte.js';
