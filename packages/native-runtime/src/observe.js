// STAND-IN (milestone R2) for R1's `observe.js`. The R1 lane owns this file and replaces it at merge
// with its tracker (host batching through `__svelters_changed`, createSubscriber-based getters).
// Only the API R2 codes against is here, with the same shape:
//   observeProp(h, prop, fn) -> unsubscribe   `fn(value)` once per delivered change (`undefined` for
//                                             proportional props: the reader reads live)
//   trackProp(h, prop)                         inside an effect: re-run it when (h, prop) changes
// implemented over today's R1a path: `host.observe(h, prop, on)` (refcounted per (h, prop) by the host),
// one `globalThis.__svelters_prop_changed(h, prop)` per native notification, and a `getProp` read-back.
// Stand-in limits: no coalescing, no compare (every notification is delivered), values are always read,
// and a `trackProp` subscription lasts until the handle dies.
import * as $ from './core/index.js';

const host = () => globalThis.__svelters_host;

/** h -> Map(prop -> { fns: Set<{fn}>, version: Source|null, hs }) */
const subs = new Map();

function entry(h, prop, create) {
  let byProp = subs.get(h);
  if (!byProp) {
    if (!create) return null;
    subs.set(h, (byProp = new Map()));
  }
  let e = byProp.get(prop);
  if (!e && create) {
    const hs = host();
    byProp.set(prop, (e = { fns: new Set(), version: null, hs }));
    hs.observe(h, prop, true);
  }
  return e ?? null;
}

function release(h, prop, e) {
  const byProp = subs.get(h);
  if (!byProp || byProp.get(prop) !== e) return;
  byProp.delete(prop);
  if (byProp.size === 0) subs.delete(h);
  try {
    e.hs.observe(h, prop, false);
  } catch {
    // the handle died: the host already dropped its observations
  }
}

/** Calls `fn(value)` for every delivered change of `prop` of `h`; returns the unsubscribe function. */
export function observeProp(h, prop, fn) {
  const e = entry(h, prop, true);
  const rec = { fn };
  e.fns.add(rec);
  return () => {
    if (!e.fns.delete(rec)) return;
    if (e.fns.size === 0 && !e.version) release(h, prop, e);
  };
}

/** Inside an effect: makes it depend on `prop` of `h` (a no-op outside effects and in `untrack`). */
export function trackProp(h, prop) {
  if (!$.effect_tracking()) return;
  const e = entry(h, prop, true);
  e.version ??= $.state(0);
  $.get(e.version);
}

globalThis.__svelters_prop_changed = (h, prop) => {
  const e = entry(h, prop, false);
  if (!e) return;
  if (e.version) $.set(e.version, $.untrack(() => $.get(e.version)) + 1);
  if (e.fns.size === 0) return;
  let v;
  try {
    v = e.hs.getProp(h, prop);
  } catch {
    return; // died since the notification was queued
  }
  for (const rec of [...e.fns]) rec.fn(v);
};
