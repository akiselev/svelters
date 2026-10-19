// N1 lifetime cases of GI closures and folded async calls, through the generated classes on the real host.
// __DIR__ is a scratch directory holding a.txt.
import { File } from './types/gtk/gio/File.js';
import { StringList } from './types/gtk/gtk/StringList.js';
import { StringObject } from './types/gtk/gtk/StringObject.js';
import { CustomFilter } from './types/gtk/gtk/CustomFilter.js';
import { FilterListModel } from './types/gtk/gtk/FilterListModel.js';
import { TextTagTable } from './types/gtk/gtk/TextTagTable.js';
import { TextTag } from './types/gtk/gtk/TextTag.js';
import { FileDialog } from './types/gtk/gtk/FileDialog.js';

const log = (...a) => console.log(a.join(' '));
const DIR = '__DIR__';
const tick = () => new Promise((r) => setTimeout(r, 20));

async function main() {
  // call scope: gtk_text_tag_table_foreach runs the function during the call; the closure ends with it
  const table = new TextTagTable();
  for (const name of ['b', 'a', 'c']) table.add(new TextTag({ name }));
  const seen = [];
  table.foreach((tag) => seen.push(tag.name));
  log('call scope', JSON.stringify(seen.sort()), seen.length === 3);

  // notified scope with a result: gtk_custom_filter_set_filter_func keeps the closure until its
  // GDestroyNotify; the item argument arrives as a handle and is rewrapped
  const list = new StringList({ strings: ['apple', 'banana', 'cherry'] });
  const filter = new CustomFilter();
  let calls = 0;
  filter.setFilterFunc((item) => {
    calls++;
    return StringObject.from(item).string.startsWith('b');
  });
  const model = new FilterListModel({ model: list, filter });
  log('notified', model.getNItems(), calls >= 3);
  list.append('blueberry');
  log('notified after append', model.getNItems());

  // the callback outlives its owner: disposing the filter wrapper releases the JS function while GTK
  // still holds the filter (inside the model). Later invocations get `false`, not a crash.
  filter.dispose();
  await tick();
  list.append('banana split');
  log('outlived owner', model.getNItems());
  // dropping the model finalizes the filter: its destroy notify frees the native closure
  model.dispose();
  list.dispose();

  // many concurrent async calls on one object and on many
  const a = File.newForPath(DIR + '/a.txt');
  const many = await Promise.all(Array.from({ length: 40 }, (_, i) => (i % 2 ? a : File.newForPath(DIR + '/a.txt')).loadContentsAsync()));
  log('concurrent', many.length, many.every((r) => r.contents.length === 5));

  // cancellation of many operations: each settles exactly once, cancelled or (if it won the race) done
  const results = await Promise.allSettled(
    Array.from({ length: 20 }, () => {
      const ac = new AbortController();
      const p = a.loadContentsAsync({ signal: ac.signal });
      ac.abort();
      return p;
    }),
  );
  log('cancel many', results.length, results.every((r) => (r.status === 'rejected' ? r.reason.code === 'cancelled' : r.value.ret === true)));

  // an already aborted signal cancels right after the start
  const pre = new AbortController();
  pre.abort();
  try {
    await a.loadContentsAsync({ signal: pre.signal });
    log('pre-aborted: finished');
  } catch (e) {
    log('pre-aborted', e.code);
  }

  // the object is disposed while its operation runs: the Promise rejects with `disposed`
  const g = File.newForPath(DIR + '/a.txt');
  const inflight = g.loadContentsAsync();
  g.dispose();
  try {
    await inflight;
    log('disposed: finished');
  } catch (e) {
    log('disposed mid-flight', e.domain, e.code);
  }

  // an async static constructor: g_file_new_tmp_async -> {ret: File, iostream}
  const tmp = await File.newTmpAsync(null);
  log('static async', tmp.ret.constructor.name, tmp.iostream.constructor.name, tmp.ret.queryExists());
  tmp.iostream.close(null);
  tmp.ret.delete(null);

  // another error domain: a cancelled file dialog rejects with GtkDialogError
  const dialog = new FileDialog({ title: 'Open' });
  const ac = new AbortController();
  const opening = dialog.open(null, { signal: ac.signal });
  await tick();
  ac.abort();
  try {
    await opening;
    log('dialog: resolved');
  } catch (e) {
    log('dialog', e.domain, e.code);
  }
  log('ALL DONE');
}

main().catch((e) => log('FAILED', e && e.stack ? e.stack : e));
