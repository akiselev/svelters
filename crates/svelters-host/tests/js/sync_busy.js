const h = globalThis.__svelters_host;
const win = h.create('window');
h.listen(win, 'close-request', true);
const lbl = h.create('label');
h.insert(win, lbl, null);
globalThis.__svelters_dispatch_sync = () => {
  // The backend raises another sync signal from inside this handler.
  h.setProp(lbl, 'label', 'from sync');
  return true;
};
globalThis.__svelters_dispatch = (_h, _signal, _args, deferred) => {
  h.call(lbl, 'seen', [deferred]);
};
