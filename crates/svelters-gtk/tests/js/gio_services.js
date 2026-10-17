// Callbacks, async methods and GError on real GIO calls, through the host and the GTK backend.
// __DIR__ is replaced by the test with a scratch directory holding a.txt, b.txt and c.txt.
const h = globalThis.__svelters_host;
const log = (...a) => console.log(a.join(' '));
const text = (bytes) => String.fromCharCode(...bytes);
const DIR = '__DIR__';

async function main() {
  const f = h.create('gio-file');

  // sync: strings, bool, bytes
  h.setProp(f, 'path', DIR + '/a.txt');
  log('path', h.getProp(f, 'path') === DIR + '/a.txt', h.getProp(f, 'uri').startsWith('file://'));
  log('exists', h.call(f, 'exists', []));
  const bytes = h.call(f, 'loadContents', []);
  log('loadContents', bytes instanceof Uint8Array, JSON.stringify(text(bytes)));

  // sync GError -> NativeError (domain, GIOErrorEnum member name, message)
  h.setProp(f, 'path', DIR + '/missing.txt');
  log('exists missing', h.call(f, 'exists', []));
  try {
    h.call(f, 'loadContents', []);
    log('no error');
  } catch (e) {
    log('sync error', e.name, e.domain, e.code, e instanceof NativeError, /missing\.txt/.test(e.message));
  }

  // async: g_file_load_contents_async -> Promise<Uint8Array>
  h.setProp(f, 'path', DIR + '/b.txt');
  const pending = h.call(f, 'loadContentsAsync', []);
  log('async returns a promise', pending instanceof Promise);
  const data = await pending;
  log('async result', JSON.stringify(text(data)));

  // async failure rejects with the same NativeError shape
  h.setProp(f, 'path', DIR + '/missing.txt');
  try {
    await h.call(f, 'loadContentsAsync', []);
    log('no rejection');
  } catch (e) {
    log('async rejection', e.name, e.domain, e.code);
  }

  // AbortSignal -> g_cancellable_cancel -> GIO completes with `cancelled`
  h.setProp(f, 'path', DIR + '/c.txt');
  const ac = new AbortController();
  const aborted = h.call(f, 'loadContentsAsync', [{ signal: ac.signal }]);
  ac.abort();
  try {
    await aborted;
    log('abort: finished before the cancel');
  } catch (e) {
    log('abort', e.name, e.domain, e.code);
  }

  // call-scope callback: g_file_enumerate_children, one JS call per child
  h.setProp(f, 'path', DIR);
  const names = [];
  const n = h.call(f, 'listNames', [(name) => { names.push(name); return true; }]);
  log('listNames', n, JSON.stringify(names.sort()));
  let first = null;
  h.call(f, 'listNames', [(name) => { first = name; return false; }]);
  log('listNames stops on false', first !== null);

  // notified-scope callback: g_file_monitor; the test creates a file once we say MONITORING
  const events = [];
  h.call(f, 'monitor', [(event, path) => { events.push(event); if (event === 'created') log('EVENT created', path.endsWith('/new.txt')); }]);
  log('MONITORING');
  await new Promise((resolve) => {
    const poll = () => (events.includes('created') ? resolve() : setTimeout(poll, 10));
    poll();
  });

  // unmonitor releases the callback natively; destroying the owner releases the rest
  h.call(f, 'unmonitor', []);
  const owner = h.create('gio-file');
  h.setProp(owner, 'path', DIR);
  h.call(owner, 'monitor', [() => {}]);
  h.destroy(owner);
  h.destroy(f);
  log('ALL DONE');
}

main().catch((e) => log('FAILED', e && e.stack ? e.stack : e));
