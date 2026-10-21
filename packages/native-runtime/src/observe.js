// Native property observation for Svelte (plans/research/REACTIVITY.md sections 2, 5.5 and 7).
//
// The host coalesces native change announcements per (handle, prop), reads and compares them at the
// checkpoint and calls `globalThis.__svelters_changed([h, prop, value, h, prop, value, ...])` once per
// checkpoint, before the queued signal events. This module turns that batch into Svelte reactivity:
//
// - `trackProp(h, prop)` subscribes the running effect (a `createSubscriber` per (handle, prop)): the
//   effect re-runs after the next delivered change. Outside an effect, and inside `untrack`, it does
//   nothing. The caller then reads live (`getProp`), so a missed announcement can make a value stale
//   but never wrong at read time.
// - `observeProp(h, prop, fn)` calls `fn(value)` once per delivered change (`value` is `undefined` for a
//   proportional property: read it live if needed) and returns the unsubscribe function. Element
//   bindings (`bind:`) are built on it.
// - `invalidateProp(h, prop)` re-runs the readers of (h, prop) now: our own writes, whose native echo
//   the host drops.
//
// Ownership: the table is keyed by handle number and never holds a wrapper or a node, so observing
// keeps nothing alive (section 5.5). The subscribing effects hold the wrapper; their teardown (one
// microtask later, as `createSubscriber`) calls `host.observe(h, prop, false)`. Every subscription and
// every `observeProp` registration holds one host-side reference; the host refcounts them per
// (h, prop) and the backend connects once.
//
// `@svelters/types` reaches this tracker through `installTracker` (its core has no dependencies), so
// generated getters (`get text() { return observed(this, 'text'); }`) become reactive once the runtime
// is loaded and stay plain reads without it.
import { untrack } from './core/index.js';
import { subscriber } from './core/internal.js';
import { installTracker } from '@svelters/types/core';

const host = () => globalThis.__svelters_host;

/**
 * The key of a property: GTK spells `cursor-position`, Qt `cursorPosition`, and JS may use either;
 * they all name one native property (the host resolves them to one id).
 */
const canon = (prop) => String(prop).replace(/[-_]/g, '').toLowerCase();

// host object -> Map<handle, Map<canon prop, Entry>> (tests run several hosts in one process).
const tables = new WeakMap();

function table(hs = host()) {
  let t = tables.get(hs);
  if (!t) tables.set(hs, (t = new Map()));
  return t;
}

/**
 * One (handle, prop): `updates` are the version bumps of its live subscriptions (native and
 * local-only), `fns` the `observeProp` registrations, `nativeSub` / `localSub` the subscribe functions.
 */
function entry(h, prop, create) {
  const t = table();
  const key = canon(prop);
  let props = t.get(h);
  const e = props?.get(key);
  if (e || !create) return e;
  if (!props) t.set(h, (props = new Map()));
  const n = { h, prop, updates: new Set(), fns: new Set(), nativeSub: null, localSub: null, signalSub: null };
  props.set(key, n);
  return n;
}

function forget(e) {
  if (e.updates.size || e.fns.size) return;
  const t = table();
  const props = t.get(e.h);
  if (props?.get(canon(e.prop)) !== e) return;
  props.delete(canon(e.prop));
  if (props.size === 0) t.delete(e.h);
}

/** `host.observe` that never throws: a dead handle or a host without observation answers false. */
function hostObserve(h, prop, on) {
  try {
    return host()?.observe?.(h, prop, on) ?? false;
  } catch {
    return false;
  }
}

function makeSub(e, native) {
  return subscriber((update) => {
    e.updates.add(update);
    const observing = native && hostObserve(e.h, e.prop, true);
    return () => {
      if (observing) hostObserve(e.h, e.prop, false);
      e.updates.delete(update);
      if (native) e.nativeSub = null;
      else e.localSub = null;
      forget(e);
    };
  });
}

/**
 * Subscribes the running effect to native changes of `prop` of handle `h` (a no-op outside an effect
 * and inside `untrack`). The caller reads the value itself, live.
 */
export function trackProp(h, prop) {
  const e = entry(h, prop, true);
  (e.nativeSub ??= makeSub(e, true))();
  forget(e);
}

/**
 * Like `trackProp` without a native observation: only `invalidateProp` (our own writes) re-runs the
 * effect. For `set_only` properties, which only our setters change.
 */
export function trackLocal(h, prop) {
  const e = entry(h, prop, true);
  (e.localSub ??= makeSub(e, false))();
  forget(e);
}

/**
 * Subscribes the running effect to the native `signal` of `obj` as the accessor `prop` (a property with
 * no native notification of its own, such as Qt `QTextDocument.plainText` after `contentsChanged`). The
 * listener holds the wrapper only while an effect is subscribed (`NativeObject.on`).
 */
export function trackSignal(obj, prop, signal) {
  const h = obj?.svlHandle;
  if (typeof h !== 'number') return;
  const e = entry(h, prop, true);
  (e.signalSub ??= subscriber((update) => {
    e.updates.add(update);
    let off = null;
    try {
      off = obj.on(signal, () => update());
    } catch {
      // a dead object: nothing to listen to
    }
    return () => {
      off?.();
      e.updates.delete(update);
      e.signalSub = null;
      forget(e);
    };
  }))();
  forget(e);
}

/** Re-runs the effects reading (h, prop) now: after our own write, whose native echo the host drops. */
export function invalidateProp(h, prop) {
  const e = entry(h, prop, false);
  if (e) for (const u of [...e.updates]) u();
}

/**
 * Calls `fn(value)` once per delivered change of `prop` of handle `h` (`value` is `undefined` for a
 * proportional property) until the returned function is called. `unsubscribe.observed` says whether
 * the host observes the property natively (false: no change notification for it on this backend, or
 * a dead handle; `fn` is then never called).
 */
export function observeProp(h, prop, fn) {
  if (typeof fn !== 'function') throw new TypeError('observeProp(h, prop, fn): fn must be a function');
  const e = entry(h, prop, true);
  const reg = { fn };
  e.fns.add(reg);
  const observed = hostObserve(h, prop, true);
  let done = false;
  const unsubscribe = () => {
    if (done) return;
    done = true;
    e.fns.delete(reg);
    if (observed) hostObserve(h, prop, false);
    forget(e);
  };
  unsubscribe.observed = observed;
  return unsubscribe;
}

/** The host's batch: `[h, prop, value, h, prop, value, ...]`, one triple per changed (h, prop). */
export function changed(batch) {
  if (!Array.isArray(batch)) return;
  for (let i = 0; i + 1 < batch.length; i += 3) {
    const e = entry(batch[i], batch[i + 1], false);
    if (!e) continue;
    const value = batch[i + 2];
    for (const u of [...e.updates]) u();
    for (const reg of [...e.fns]) {
      if (!e.fns.has(reg)) continue;
      try {
        reg.fn(value);
      } catch (err) {
        console.error(err);
      }
    }
  }
}

/** Number of (handle, prop) entries with a subscription or a callback (tests). */
export function trackedCount(hs = host()) {
  let n = 0;
  for (const props of table(hs).values()) n += props.size;
  return n;
}

globalThis.__svelters_changed = changed;
installTracker({ track: trackProp, local: trackLocal, signal: trackSignal, invalidate: invalidateProp, untrack });
