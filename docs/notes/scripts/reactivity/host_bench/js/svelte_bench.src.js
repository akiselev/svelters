// Svelte scenario: a reactive getter over a native property (createSubscriber), read by one
// render effect, with the native value changed N times (one notify::value event each).
//   MODE 'subscriber': every event calls update() -> the effect re-runs per event (no coalescing)
//   MODE 'coalesced':  only the last event of the burst calls update() (what host-side
//                      coalescing per (handle, prop) per checkpoint would deliver)
// Globals N and MODE are prepended by the harness.
import * as $ from 'svelte/internal/client';
import { createSubscriber } from 'svelte/reactivity';

const h = globalThis.__svelters_host;
const adj = h.construct('Gtk.Adjustment', { upper: 1e12, 'step-increment': 1 });

const listeners = new Map(); // `${handle}\0${signal}` -> Set<fn>
globalThis.__svelters_dispatch = (hh, signal, args) => {
  const set = listeners.get(`${hh}\0${signal}`);
  if (set) for (const fn of set) fn(args);
};
function on(handle, signal, fn) {
  const k = `${handle}\0${signal}`;
  let set = listeners.get(k);
  if (!set) {
    listeners.set(k, (set = new Set()));
    h.listen(handle, signal, true);
  }
  set.add(fn);
  return () => {
    set.delete(fn);
    if (set.size === 0) {
      listeners.delete(k);
      h.listen(handle, signal, false);
    }
  };
}

let events = 0;
const subscribe = createSubscriber((update) =>
  on(adj, 'value-changed', () => {
    events++;
    if (MODE === 'subscriber' || events === N) update();
  }),
);
const value = () => {
  subscribe();
  return h.getProp(adj, 'value');
};

let runs = 0;
let last = 0;
$.effect_root(() => {
  $.template_effect(() => {
    last = value();
    runs++;
  });
});
$.flush();

for (let i = 1; i <= N; i++) h.call(adj, 'set_value', [i]);
setTimeout(() => console.log(`[${MODE}] N=${N} events ${events}, effect runs ${runs}, last ${last}`), 0);
