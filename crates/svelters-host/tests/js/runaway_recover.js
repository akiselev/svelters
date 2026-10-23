// FX6: an interrupted handler skips its `finally` (QuickJS's interrupt is uncatchable); the host then
// calls `__svelters_recover` once, which the next event reports.
const h = globalThis.__svelters_host;
const w = h.create('window');
h.listen(w, 'spin', true);
h.listen(w, 'ping', true);
let inside = false;
let recovered = 0;
globalThis.__svelters_recover = () => {
  recovered++;
};
globalThis.__svelters_dispatch = (_h, signal) => {
  if (signal === 'spin') {
    inside = true;
    try {
      for (;;) {}
    } finally {
      inside = false;
    }
  }
  h.setProp(w, 'title', `inside=${inside} recovered=${recovered}`);
};
