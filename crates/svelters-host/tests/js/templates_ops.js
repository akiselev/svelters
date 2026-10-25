// The op-by-op twin of templates.js.
const h = globalThis.__svelters_host;
const win = h.create('window');
for (let i = 0; i < 2; i++) {
  const box = h.create('box');
  h.setProp(box, 'spacing', 4);
  h.setProp(box, 'homogeneous', true);
  const label = h.create('label');
  h.setProp(label, 'label', 'Name');
  h.insert(box, label, null);
  const button = h.create('button');
  h.setProp(button, 'label', 'Go');
  h.insert(box, button, null);
  h.listen(button, 'clicked', true);
  h.insert(win, box, null);
}
globalThis.__svelters_dispatch = () => {};
