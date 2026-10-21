// ABI v2.6 host contract (tests/batch.rs): prints every `__svelters_changed` batch, every event and
// sync signal with the value the app last saw, and a job queued by each, so the order of delivery and
// job drains is visible. An `evt` event whose first argument is a string runs it as code (h, log, obj).
const h = globalThis.__svelters_host;
const log = (...a) => console.log(a.join(' '));
const seen = {};
const fmt = (x) =>
  x === undefined ? 'undefined'
  : typeof x === 'number' && Number.isNaN(x) ? 'NaN'
  : x !== null && typeof x === 'object' ? 'boxed'
  : JSON.stringify(x);

globalThis.__svelters_changed = (b) => {
  const out = [];
  for (let i = 0; i < b.length; i += 3) {
    seen[b[i + 1]] = b[i + 2];
    out.push(b[i], JSON.stringify(b[i + 1]), fmt(b[i + 2]));
  }
  log(`changed ${out.join(',')}`);
  queueMicrotask(() => log('job after changed'));
};
globalThis.__svelters_dispatch = (handle, signal, args) => {
  if (Array.isArray(args) && typeof args[0] === 'string') {
    new Function('h', 'log', 'obj', args[0])(h, log, handle);
    return;
  }
  log(`event #${handle} ${signal} sees ${fmt(seen.value)}`);
  queueMicrotask(() => log(`job after ${signal}`));
};
globalThis.__svelters_dispatch_sync = (handle, signal) => {
  log(`sync #${handle} ${signal} sees ${fmt(seen.value)}`);
  queueMicrotask(() => log('job after sync'));
  return null;
};

const obj = h.create('obj');
h.listen(obj, 'evt', true);
