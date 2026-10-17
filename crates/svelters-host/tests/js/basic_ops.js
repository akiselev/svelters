const h = globalThis.__svelters_host;
const win = h.create('window');
h.setProp(win, 'title', 'Hi');
const btn = h.create('button');
h.setProp(btn, 'label', 'go');
h.listen(btn, 'clicked', true);
h.insert(win, btn, null);
const lbl = h.create('label');
h.insert(win, lbl, btn, 'titlebar');
let n = 0;
globalThis.__svelters_dispatch = (_h, _signal, args) => {
  n += 1;
  Promise.resolve().then(() => h.setProp(lbl, 'label', `clicked ${n} ${JSON.stringify(args)}`));
};
h.call(win, 'present', []);
