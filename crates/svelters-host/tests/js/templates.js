// W-C2: one template (a box with a label and a button that listens to `clicked`), instantiated
// twice, against the same tree built op by op (tests/fast_ops.rs compares the logs).
const h = globalThis.__svelters_host;
const win = h.create('window');
const T = h.template({
  hash: 'box-label-button',
  nodes: ['box', 0, null, 'label', 1, null, 'button', 1, null],
  props: [1, 'label', 'Name', 0, 'spacing', 4, 2, 'label', 'Go', 0, 'homogeneous', true],
  listens: [2, 'clicked', 0],
});
if (h.template({ hash: 'box-label-button', nodes: [] }) !== T) throw new Error('same hash, same template');
h.templateRelease(T);
for (let i = 0; i < 2; i++) {
  const r = h.instantiate(T);
  const root = typeof r === 'number' ? r : r[0];
  h.insert(win, root, null);
}
globalThis.__svelters_dispatch = () => {};
