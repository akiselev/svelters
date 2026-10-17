const h = globalThis.__svelters_host;
const win = h.create('window');
h.listen(win, 'close-request', true);
const lbl = h.create('label');
h.insert(win, lbl, null);
globalThis.__svelters_dispatch_sync = (_h, _signal, args) => {
  switch (args[0]) {
    case 'veto':
      return true;
    case 'throw':
      throw new Error('handler boom');
    case 'ops':
      h.setProp(lbl, 'label', 'immediate');
      Promise.resolve().then(() => h.setProp(lbl, 'label', 'microtask'));
      return true;
    case 'undef':
      return undefined;
    case 'object':
      return { a: 1 };
    default:
      return null;
  }
};
