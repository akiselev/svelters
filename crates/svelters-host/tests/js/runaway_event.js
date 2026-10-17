const h = globalThis.__svelters_host;
const w = h.create('window');
h.listen(w, 'spin', true);
h.listen(w, 'ping', true);
globalThis.__svelters_dispatch = (_h, signal) => {
  if (signal === 'spin') {
    for (;;) {}
  }
  h.setProp(w, 'title', 'alive');
};
