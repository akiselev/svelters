// GIO services on the generic path (N1): the generated `Gio.File` class over the GI invoker. Sync calls,
// out parameters, GError -> NativeError, `_async`/`_finish` folded into Promises (resolve, reject,
// abort), a call-scope callback, enumeration and a file monitor's signal. Replaces the hand-written
// `gio-file` service of S1/S2.
// __DIR__ is replaced by the test with a scratch directory holding a.txt, b.txt and c.txt.
import { File } from './types/gtk/gio/File.js';

const log = (...a) => console.log(a.join(' '));
const text = (bytes) => String.fromCharCode(...bytes);
const DIR = '__DIR__';

async function main() {
  const f = File.newForPath(DIR + '/a.txt');

  // sync: strings, bool
  log('path', f.getPath() === DIR + '/a.txt', f.getUri().startsWith('file://'));
  log('exists', f.queryExists());
  // out parameters: {ret, contents, etagOut}
  const r = f.loadContents();
  log('loadContents', Object.keys(r).join(), r.ret, r.contents instanceof Uint8Array, JSON.stringify(text(r.contents)), r.etagOut === null || typeof r.etagOut === 'string');

  // sync GError -> NativeError (domain, GIOErrorEnum member name, message)
  const missing = File.newForPath(DIR + '/missing.txt');
  log('exists missing', missing.queryExists());
  try {
    missing.loadContents();
    log('no error');
  } catch (e) {
    log('sync error', e.name, e.domain, e.code, e instanceof NativeError, /missing\.txt/.test(e.message));
  }

  // async: g_file_load_contents_async + _finish -> Promise<{ret, contents, etagOut}>
  const pending = File.newForPath(DIR + '/b.txt').loadContentsAsync();
  log('async returns a promise', pending instanceof Promise);
  const data = await pending;
  log('async result', Object.keys(data).join(), JSON.stringify(text(data.contents)));

  // async failure rejects with the same NativeError shape
  try {
    await missing.loadContentsAsync();
    log('no rejection');
  } catch (e) {
    log('async rejection', e.name, e.domain, e.code);
  }

  // AbortSignal -> g_cancellable_cancel -> GIO completes with `cancelled`
  const ac = new AbortController();
  const aborted = File.newForPath(DIR + '/c.txt').loadContentsAsync({ signal: ac.signal });
  ac.abort();
  try {
    await aborted;
    log('abort: finished before the cancel');
  } catch (e) {
    log('abort', e.name, e.domain, e.code);
  }

  // an async result that is an object: a FileInfo, wrapped as its generated class
  const info = await f.queryInfoAsync('standard::size,standard::name', 'none');
  log('queryInfoAsync', info.constructor.name, info.getSize(), info.getName());

  // enumeration: FileEnumerator.next_file until null (main.js, the three files and the staged types/)
  const dir = File.newForPath(DIR);
  const en = dir.enumerateChildren('standard::name', 'none');
  const names = [];
  for (let i; (i = en.nextFile()) !== null; ) names.push(i.getName());
  log('enumerate', names.length, JSON.stringify(names.filter((n) => n.endsWith('.txt')).sort()));

  // call-scope callback: g_file_measure_disk_usage reports progress synchronously
  const progress = [];
  const usage = dir.measureDiskUsage('none', null, (reporting, size, dirs, files) => progress.push([reporting, files]));
  log('measureDiskUsage', usage.ret, usage.numFiles >= 3, progress.length > 0);

  // a file monitor's `changed` signal; the test creates new.txt once we say MONITORING
  const mon = dir.monitorDirectory('none');
  const events = [];
  const off = mon.on('changed', (file, other, event) => {
    events.push(event);
    if (event === 'created') log('EVENT created', file.getPath().endsWith('/new.txt'));
  });
  log('MONITORING');
  await new Promise((resolve) => {
    const poll = () => (events.includes('created') ? resolve() : setTimeout(poll, 10));
    poll();
  });
  off();
  mon.cancel();
  log('ALL DONE');
}

main().catch((e) => log('FAILED', e && e.stack ? e.stack : e));
