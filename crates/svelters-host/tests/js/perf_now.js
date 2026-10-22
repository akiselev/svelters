// Scripted mode: performance.now() reads the virtual clock (the prelude replaces QuickJS-NG's own
// wall-clock `performance`), so a 1 s timer sees exactly 1000 ms more than at the start.
const h = globalThis.__svelters_host;
const win = h.create('window');
const lbl = h.create('label');
h.insert(win, lbl, null);
const t0 = performance.now();
setTimeout(() => h.setProp(lbl, 'label', `elapsed ${performance.now() - t0}`), 1000);

globalThis.__svelters_dispatch = () => {};
