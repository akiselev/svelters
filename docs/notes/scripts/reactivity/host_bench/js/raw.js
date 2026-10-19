// Raw host scenario (no Svelte). Uses `value-changed`, not `notify::value`: the GTK backend cannot
// listen to detailed signals of reflected kinds yet (reflect.rs find_signal uses g_signal_lookup). Globals N and MODE are prepended by the harness.
//   MODE 'none': listen to notify::value, handler does nothing
//   MODE 'read': handler reads the value back with getProp (GTK notify carries no value)
//   MODE 'skip': not listened: the backend never connects, nothing is posted
const h = globalThis.__svelters_host;
const now = () => (globalThis.performance?.now ? performance.now() : Date.now());
const adj = h.construct('Gtk.Adjustment', { upper: 1e12, 'step-increment': 1 });

let calls = 0;
let sum = 0;
globalThis.__svelters_dispatch = (hh, signal) => {
  calls++;
  if (MODE === 'read') sum += h.getProp(adj, 'value');
};
if (MODE !== 'skip') h.listen(adj, 'value-changed', true);

// host op costs, measured in JS
let t = now();
for (let i = 0; i < 20000; i++) sum += h.getProp(adj, 'value');
const getPropUs = ((now() - t) * 1000) / 20000;

t = now();
for (let i = 1; i <= N; i++) h.call(adj, 'set_value', [i]);
const setUs = ((now() - t) * 1000) / N;
console.log(`[${MODE}] N=${N} getProp ${getPropUs.toFixed(2)} us/op; call(set_value) incl. notify+post_event ${setUs.toFixed(2)} us/op`);

// report after the timed checkpoint (the harness runs a second one)
setTimeout(() => console.log(`[${MODE}] delivered ${calls} events (sum ${sum > 0})`), 0);
