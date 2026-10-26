// `bind:` on native elements lowered onto observation (milestone R2; plans/research/REACTIVITY.md
// sections 8.2-8.3, decisions 13 #4 and #8), next to the signal-based `bind` / `track` in ./index.js.
// The compiler picks the helper from the platform's observe facts:
//   bind_prop      two-way on an `exact` prop (or a `partial` one without a signal plan)
//   bind_readonly  a read-only observable prop
//   bind_once      `bind:prop|nonreactive`: native -> state once, then write-only (a snapshot)
// Observation goes through ./observe.js (`observeProp`).
import * as $ from './core/index.js';
import { observeProp } from './observe.js';
import { on, set } from './index.js';

const alive = (n) => !n.dead && !n.destroyed;

/**
 * Observes `prop` of `node` for the lifetime of the enclosing effect, calling `take(value)` per
 * delivered change. Returns false when the host refused the observation (`unsubscribe.observed`
 * false: a backend without the mechanism for this prop), true otherwise. The registration holds one
 * host-side reference (refcounted per (handle, prop)); the effect teardown releases it before the
 * natives are destroyed.
 */
function observeFor(node, prop, take) {
  if (!alive(node)) return true;
  const { host: hs, h } = node;
  const unsub = $.untrack(() =>
    observeProp(h, prop, (v) => {
      if (alive(node)) take(v === undefined ? hs.getProp(h, prop) : v);
    }),
  );
  if (unsub.observed === false) {
    unsub();
    return false;
  }
  $.user_pre_effect(() => unsub);
  return true;
}

/** Runs `fn` once after the first layout pass (the next host turn), unless the node is gone. */
function afterLayout(node, fn) {
  globalThis.setTimeout(() => {
    if (alive(node)) fn();
  }, 0);
}

/**
 * Two-way binding on observation: a render effect writes `get()` through `set`; each observed
 * change is recorded as the node's last value for `prop` (so echoes and redundant writes are
 * no-ops) and passed to `setv` when it differs from `get()`. `signal`/`arg` are the spec's signal
 * plan, used only when the host cannot observe the prop (as `bind`).
 */
export function bind_prop(node, prop, get, setv, signal, arg = -1) {
  $.template_effect(() => set(node, prop, get()));
  const take = (v) => {
    (node.last ??= new Map()).set(prop, v);
    if (!Object.is(v, $.untrack(get))) setv(v);
  };
  if (observeFor(node, prop, take) || signal == null) return;
  on(node, signal, (...args) => take(arg >= 0 ? args[arg] : node.host.getProp(node.h, prop)));
}

/**
 * CX3 native link: `bind:prop={store.field}` where `store` is a Rust store (`svelters:app/*`) and
 * the spec declares the `link` bind source. The host ties (store, field) to (node, prop) in Rust
 * (`host.link`): the node starts with the store's value, and changes on either side cross in the
 * host at the checkpoint, so typing, dragging and Rust writes run no JS. When the host refuses
 * (not a writable store field, a prop it cannot observe, an older host), `fallback` installs the
 * ordinary JS binding the compiler planned.
 */
export function bind_link(node, prop, obj, field, fallback) {
  const hs = node.host;
  const h = $.untrack(obj)?.svlHandle;
  if (alive(node) && h != null && typeof hs.link === 'function') {
    const id = hs.link(h, field, node.h, prop);
    if (id) {
      $.user_pre_effect(() => () => hs.unlink(id));
      return;
    }
  }
  fallback();
}

/**
 * Read-only observable binding: `setv` gets the value at creation, again after the first layout
 * pass (sizes are not known before it, fork-bindings section 6.4) if it changed, and then every
 * observed change.
 */
export function bind_readonly(node, prop, setv) {
  if (!alive(node)) return;
  const take = (v) => {
    if (node.last?.has(prop) && Object.is(node.last.get(prop), v)) return;
    (node.last ??= new Map()).set(prop, v);
    setv(v);
  };
  const read = () => take(node.host.getProp(node.h, prop));
  read();
  afterLayout(node, read);
  observeFor(node, prop, take);
}

/**
 * `bind:prop|nonreactive` (section 8.2). Two-way: state -> native as usual; native -> state once,
 * right after the first write (a bound `undefined` takes the native value instead of writing it),
 * then never: no observation, no signal, no mirror. Read-only: one snapshot after the first layout
 * pass.
 */
export function bind_once(node, prop, get, setv, readonly = false) {
  if (readonly) {
    afterLayout(node, () => {
      const v = node.host.getProp(node.h, prop);
      (node.last ??= new Map()).set(prop, v);
      setv(v);
    });
    return;
  }
  let first = true;
  $.template_effect(() => {
    const v = get();
    if (first) {
      first = false;
      if (v === undefined) return;
    }
    set(node, prop, v);
  });
  if (!alive(node)) return;
  const v = node.host.getProp(node.h, prop);
  (node.last ??= new Map()).set(prop, v);
  if (!Object.is(v, $.untrack(get))) setv(v);
}
