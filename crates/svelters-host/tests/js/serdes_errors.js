// Every failure the host reports before a value reaches the backend, with its error class.
const h = globalThis.__svelters_host;
const t = h.create('thing');
const report = (f) => {
  try {
    f();
    console.log('no error');
  } catch (e) {
    console.log(`${e.name} ${e.message}`);
  }
};
report(() => h.setProp(t, 'spacing', '8px'));
report(() => h.setProp(t, 'spacing', 3.5));
report(() => h.setProp(t, 'spacing', 5000));
report(() => h.setProp(t, 'spacing', null));
report(() => h.setProp(t, 'spacing', undefined));
report(() => h.setProp(t, 'label', 8));
report(() => h.setProp(t, 'orientation', 'hidden'));
report(() => h.setProp(t, 'align', 'sideways'));
report(() => h.setProp(t, 'geometry', { x: 1, y: 2, width: 3 }));
report(() => h.setProp(t, 'geometry', { x: 1, y: 2, width: 'wide', height: 4 }));
report(() => h.setProp(t, 'geometry', 5));
report(() => h.setProp(t, 'enabled', 'yes'));
report(() => h.setProp(t, 'data', 'bytes'));
report(() => h.setProp(t, 'tags', ['a', 2]));
report(() => h.setProp(t, 'color', { red: 2, green: 0, blue: 0, alpha: 1 }));
report(() => h.call(t, 'setBoth', [1, 'a', 'extra']));
report(() => h.call(t, 'setBoth', ['x']));
report(() => h.setProp(t, 'when', 'yesterday'));
report(() => h.setProp(t, 'readonly', 1));
report(() => h.setProp(t, 'nosuch', 1));
report(() => h.setProp(999, 'spacing', 1));
report(() => h.setProp(t, 'loose', undefined));
report(() => h.call(t, 'fail', []));
