const h = globalThis.__svelters_host;
const win = h.create('window');
const btn = h.create('button');
h.insert(win, btn, null);
const lbl = h.create('label');
h.insert(win, lbl, null);
h.listen(btn, 'clicked', true);
let clicks = 0;
globalThis.__svelters_dispatch = (_h, signal) => {
  if (signal !== 'clicked') return;
  clicks++;
  h.setProp(lbl, 'label', `clicked ${clicks}`);
  Promise.resolve().then(() => h.setProp(lbl, 'label', `flushed ${clicks}`));
};
