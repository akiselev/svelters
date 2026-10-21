// R1 scenario: the real tracker (packages/native-runtime/src/observe.js) over the real host batch.
// A render effect reads Gtk.Adjustment `value` through `trackProp` + `getProp`; the module then changes
// the value natively N times (GI `gtk_adjustment_set_value`, each emitting notify::value -> host
// prop_changed). The host coalesces them into one dirty entry, reads it once at the checkpoint and
// delivers one `__svelters_changed` batch: the effect re-runs once.
//   MODE 'batched':   as above
//   MODE 'unobserved': the same N changes with no observation (the cost of the GI calls alone)
// Globals N and MODE are prepended by the harness.
import * as $ from 'svelte/internal/client';
import { trackProp } from '../../../../../../packages/native-runtime/src/observe.js';

const h = globalThis.__svelters_host;
const adj = h.construct('Gtk.Adjustment', { upper: 1e12, 'step-increment': 1 });

let runs = 0;
let last = 0;
let batches = 0;
let entries = 0;
const deliver = globalThis.__svelters_changed;
globalThis.__svelters_changed = (b) => {
  batches++;
  entries += b.length / 3;
  deliver(b);
};
$.effect_root(() => {
  $.template_effect(() => {
    if (MODE === 'batched') trackProp(adj, 'value');
    last = h.getProp(adj, 'value');
    runs++;
  });
});
$.flush();

const t0 = Date.now();
for (let i = 1; i <= N; i++) h.call(adj, 'set_value', [i]);
const loadMs = Date.now() - t0;
setTimeout(() => console.log(`[${MODE}] N=${N} set_value loop ${loadMs} ms, batches ${batches}, entries ${entries}, effect runs ${runs}, last ${last}`), 0);
