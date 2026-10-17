// Handles in creation order: window 1, box 2, entry 3, label 4.
const h = globalThis.__svelters_host;
const win = h.create('window');
h.setProp(win, 'title', 'Sync');
const box = h.create('box');
const entry = h.create('entry');
const label = h.create('label');
h.insert(box, entry, null);
h.insert(box, label, null);
h.insert(win, box, null);
h.listen(entry, 'key-press', true);
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
  const text = args[1];
  note(`key ${text}`);
  if (text === 'a') return true; // consumed: the entry never sees it
  if (text === 'c') throw new Error('key boom');
  if (text === 'd') {
    // Closing from inside the handler raises close-request natively: a sync signal inside a sync handler.
    h.call(win, 'close', []);
    return null;
  }
  if (text === 'e') veto = false;
  return null;
};
globalThis.__svelters_dispatch = (_h, signal, _args, deferred) => note(`async ${signal} deferred=${deferred}`);

// requestAnimationFrame on the toolkit's frame path
let frames = 0;
const tick = () => {
  frames += 1;
  if (frames < 3) requestAnimationFrame(tick);
};
requestAnimationFrame(tick);
const cancelled = requestAnimationFrame(() => note('cancelled frame ran'));
cancelAnimationFrame(cancelled);
h.call(win, 'present', []);
