// Qt async methods and callbacks through the host: a QDialog as a Promise (with AbortSignal -> cancel)
// and a QMenu item that runs a JS function. The Rust test triggers the menu actions from the Qt loop.
const h = globalThis.__svelters_host;
const log = (...a) => console.log(a.join(' '));

async function main() {
  const dlg = h.create('colordialog');

  // accept() -> finished(1): the Promise resolves with the result.
  const p = h.call(dlg, 'open', []);
  log('open returns a promise', p instanceof Promise);
  setTimeout(() => h.call(dlg, 'accept', []), 5);
  log('accepted', await p);

  // Completing inside the starting call is fine: the Promise settles a microtask later.
  const q = h.call(dlg, 'open', []);
  h.call(dlg, 'reject', []);
  log('rejected', await q);

  // An abort signal closes the dialog and rejects with a NativeError.
  const ac = new AbortController();
  const r = h.call(dlg, 'open', [{ signal: ac.signal }]);
  setTimeout(() => ac.abort(), 5);
  try {
    await r;
    log('abort: resolved');
  } catch (e) {
    log('abort', e instanceof NativeError, e.domain, e.message);
  }
  try {
    h.call(dlg, 'open', ['x']);
  } catch (e) {
    log('arity', e.name, e.message);
  }

  // Callbacks: the menu keeps the functions until it dies; the Rust test triggers the actions.
  const menu = h.create('menu');
  const seen = [];
  h.call(menu, 'addItem', ['Open', (checked) => seen.push('open:' + checked), 'Ctrl+O']);
  h.call(menu, 'addItem', ['Boom', () => { throw new Error('boom'); }, '']);
  try {
    h.call(menu, 'addItem', ['Bad', 42, '']);
  } catch (e) {
    log('not a function', e.name, e.message);
  }
  log('ready');
  for (let i = 0; i < 300 && seen.length === 0; i++) await new Promise((res) => setTimeout(res, 10));
  log('menu callbacks', seen.join());

  h.destroy(menu);
  h.destroy(dlg);
  log('done');
}
main().catch((e) => log('FAILED', e && e.message));
