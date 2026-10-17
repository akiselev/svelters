const h = globalThis.__svelters_host;
const win = h.create('window');
const lbl = h.create('label');
h.insert(win, lbl, null);
const set = (s) => h.setProp(lbl, 'label', s);
set('start');

setTimeout(() => {
  set('t50');
  Promise.resolve().then(() => set('t50-micro'));
  setTimeout(() => set('t50+10'), 10);
}, 50);
setTimeout(() => set('t20'), 20);
const cancelled = setTimeout(() => set('never'), 30);
clearTimeout(cancelled);

Promise.resolve().then(() => set('p1')).then(() => set('p2'));
queueMicrotask(() => set('mt'));
(async () => {
  await new Promise((r) => setTimeout(r, 40));
  set('async-40');
})();

globalThis.__svelters_dispatch = () => {};
