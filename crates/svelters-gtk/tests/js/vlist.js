// Native protocol test for `virtual-list` without Svelte: JS plays the list controller by hand.
// Handles in creation order: window 1, box 2, status label 3, virtual list 4.
const h = globalThis.__svelters_host;
const H = (x) => ({ svlHandle: x });

const win = h.create('window');
h.setProp(win, 'title', 'vlist');
h.setProp(win, 'default-width', 320);
h.setProp(win, 'default-height', 320);
const box = h.create('box');
h.setProp(box, 'orientation', 'vertical');
const status = h.create('label');
const list = h.create('virtual-list');
h.setProp(list, 'vexpand', true);
h.insert(box, status, null);
h.insert(box, list, null);
h.insert(win, box, null);
for (const s of ['setup', 'bind', 'unbind', 'teardown', 'activate', 'select']) h.listen(list, s, true);

let listAlive = true;
const rows = new Map(); // row slot -> { box, label, id, gen }
const stats = { setups: 0, binds: 0, unbinds: 0, teardowns: 0, staleUnbinds: 0, selected: -2, activated: -2, deferredBinds: 0 };
const report = (extra = '') => {
  const bound = [...rows.values()].filter((r) => r.id >= 0);
  const ids = bound.map((r) => r.id);
  const text = Object.entries({
    ...stats,
    rows: rows.size,
    bound: bound.length,
    minId: ids.length ? Math.min(...ids) : -1,
    maxId: ids.length ? Math.max(...ids) : -1,
    count: listAlive ? h.call(list, 'count', []) : -1,
  })
    .map(([k, v]) => `${k}=${v}`)
    .join(';');
  h.setProp(status, 'label', text + extra);
};

globalThis.__svelters_dispatch_sync = (_h, signal, args) => {
  if (signal === 'setup') {
    const [row] = args;
    const b = h.create('box');
    const l = h.create('label');
    h.insert(b, l, null);
    rows.set(row, { box: b, label: l, id: -1, gen: 0 });
    h.call(list, 'setRow', [row, H(b)]);
    stats.setups++;
  } else if (signal === 'bind') {
    const [row, pos, id, gen] = args;
    const r = rows.get(row);
    r.id = id;
    r.gen = gen;
    h.setProp(r.label, 'label', `item ${id} at ${pos}`);
    stats.binds++;
  } else if (signal === 'unbind') {
    const [row, gen] = args;
    const r = rows.get(row);
    if (r && r.gen === gen) {
      r.id = -1;
      stats.unbinds++;
    } else stats.staleUnbinds++;
  } else if (signal === 'teardown') {
    const [row] = args;
    const r = rows.get(row);
    if (r) {
      rows.delete(row);
      h.destroy(r.label);
      h.destroy(r.box);
    }
    stats.teardowns++;
  }
  return null;
};
// Async signals, and sync ones that came late because JS was busy (deferred = true).
globalThis.__svelters_dispatch = (_h, signal, args, deferred) => {
  if (signal === 'select') stats.selected = args[0];
  else if (signal === 'activate') stats.activated = args[0];
  else if (deferred) {
    // A late unbind/teardown: same bookkeeping as the sync path.
    globalThis.__svelters_dispatch_sync(_h, signal, args);
    stats.deferredBinds++;
  }
};

const ids = (from, n) => Array.from({ length: n }, (_, i) => from + i);
const steps = [
  () => h.call(list, 'splice', [0, 0, ids(1, 10000)]),
  () => report(';step=filled'),
  () => h.call(list, 'scrollTo', [9999]),
  () => report(';step=scrolled-end'),
  () => h.call(list, 'scrollTo', [0]),
  () => report(';step=scrolled-start'),
  () => {
    // notifications the test counts natively: insert 1, remove 3, move one (remove + insert)
    h.call(list, 'splice', [5, 0, [10001]]);
    h.call(list, 'splice', [2, 3, []]);
    h.call(list, 'splice', [0, 1, []]);
    h.call(list, 'splice', [9, 0, [1]]);
  },
  () => report(';step=mutated'),
  () => h.setProp(list, 'selected', 3),
  () => report(';step=selected-by-js'),
  () => report(';step=selected-by-user'),
  () => h.setProp(list, 'layout', 'grid'),
  () => report(';step=grid'),
  () => h.call(list, 'splice', [0, h.call(list, 'count', []), []]),
  () => report(';step=cleared'),
  () => {
    h.remove(box, list);
    h.destroy(list);
    listAlive = false;
    for (const r of rows.values()) {
      h.destroy(r.label);
      h.destroy(r.box);
    }
    rows.clear();
  },
  () => report(';step=done'),
];
let i = 0;
const next = () => {
  if (i < steps.length) steps[i++]();
  if (i <= steps.length) setTimeout(next, 250);
};
setTimeout(next, 200);
h.call(win, 'present', []);
