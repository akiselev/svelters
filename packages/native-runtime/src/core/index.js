// @svelters/native-runtime/core: the reactive core compiled components import as `$`
// (`import * as $ from '@svelters/native-runtime/core'`). Exactly the names the svelters compiler
// emits, with Svelte's names and call shapes (docs/notes/native-compiler/runtime-contract.md
// "Core API"; test/core-contract.test.mjs keeps this list equal to what the compiler can emit).
// Phase 1 of plans/research/RUNTIME.md: they are Svelte's own, re-exported through the adapter.
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
} from './svelte.js';
