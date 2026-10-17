// Lifetime model: idempotent destroy, dead-handle errors, native death notifications.
const h = globalThis.__svelters_host;
const win = h.create('window');
const box = h.create('box');
const lbl = h.create('label');
h.insert(win, box, null);
h.insert(box, lbl, null);

// destroy is idempotent: the second call is a no-op, and the handle explains itself afterwards.
const tmp = h.create('label');
h.destroy(tmp);
h.destroy(tmp);
const attempt = (what, f) => {
  try {
    f();
    console.log(`${what}: ok`);
  } catch (e) {
    console.log(`${what}: ${e.message}`);
  }
};
attempt('get destroyed', () => h.getProp(tmp, 'label'));
attempt('set destroyed', () => h.setProp(tmp, 'label', 'x'));
attempt('get never issued', () => h.getProp(9999, 'label'));

globalThis.__svelters_dispatch = () => {};
globalThis.__svelters_gone = (handle, reason) => {
  console.log(`gone #${handle} ${reason}`);
  if (handle !== win) return;
  // By now the host has recorded the whole cascade, so the descendants are dead too.
  attempt('get descendant', () => h.getProp(lbl, 'label'));
  attempt('set descendant', () => h.setProp(lbl, 'label', 'x'));
  attempt('listen off descendant', () => h.listen(lbl, 'clicked', false));
  attempt('listen on descendant', () => h.listen(lbl, 'clicked', true));
  attempt('destroy descendant', () => h.destroy(lbl));
  attempt('destroy descendant again', () => h.destroy(lbl));
  attempt('remove dead', () => h.remove(box, lbl));
  attempt('insert dead', () => h.insert(win, lbl, null));
  attempt('call dead', () => h.call(win, 'present', []));
};
h.call(win, 'present', []);
