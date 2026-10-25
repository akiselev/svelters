// The general-op twin of fast_ops.js.
const h = globalThis.__svelters_host;
const win = h.create('window');
const box = h.create('box');
h.setProp(box, 'spacing', 6);
h.setProp(box, 'homogeneous', true);
h.insert(win, box, null);
const lbl = h.create('label');
h.setProp(lbl, 'label', 'hello');
h.setProp(lbl, 'lines', 3);
h.setProp(lbl, 'opacity', 0.5);
h.setProp(lbl, 'opacity', 1);
h.setProp(lbl, 'label', 'héllo wörld');
h.setProp(lbl, 'label', 'again');
h.insert(box, lbl, null);
const btn = h.create('button');
h.insert(box, btn, lbl);
h.listen(btn, 'clicked', true);
h.listen(btn, 'clicked', false);
h.listen(btn, 'clicked', true);
for (const [prop, v] of [['lines', 2.5], ['lines', '7'], ['label', 42], ['label', {}], ['no-such-prop', 1]]) {
  try {
    h.setProp(lbl, prop, v);
    console.log(`ok ${v}`);
  } catch (e) {
    console.log(`${e.name}: ${e.message}`);
  }
}
