// Handles in creation order: window 1, box 2, entry 3, label 4, key controller 5.
const h = globalThis.__svelters_host;
const win = h.create('window');
h.setProp(win, 'title', 'Sync');
const box = h.create('box');
const entry = h.create('entry');
const label = h.create('label');
const key = h.create('EventControllerKey');
h.insert(box, entry, null);
h.insert(box, label, null);
h.insert(win, box, null);
h.insert(entry, key, null);
h.listen(key, 'key-pressed', true);
h.listen(win, 'close-request', true);

const log = [];
let veto = true;
const note = (s) => {
  log.push(s);
  h.setProp(label, 'label', log.join('|'));
};

globalThis.__svelters_dispatch_sync = (_h, signal, args) => {
  if (signal === 'close-request') {
    note(`close veto=${veto}`);
    return veto ? true : null;
  }
  const keyval = args[0];
  note(`key ${keyval}`);
  if (keyval === 97) return true;
  if (keyval === 99) throw new Error('key boom');
  if (keyval === 100) {
    // Writing the entry raises `changed` natively, and a native hook raises a second key-pressed
    // from inside that emission: a sync signal inside a sync handler.
    h.setProp(entry, 'text', 'typed');
    return true;
  }
  if (keyval === 101) veto = false;
  return null;
};
globalThis.__svelters_dispatch = (_h, signal, _args, deferred) => note(`async ${signal} deferred=${deferred}`);

// requestAnimationFrame on the GDK frame clock
let frames = 0;
const tick = () => {
  frames += 1;
  if (frames < 3) requestAnimationFrame(tick);
};
requestAnimationFrame(tick);
const cancelled = requestAnimationFrame(() => note('cancelled frame ran'));
cancelAnimationFrame(cancelled);
globalThis.__frames = () => frames;
h.call(win, 'present', []);
