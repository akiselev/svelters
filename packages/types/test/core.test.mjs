// The object core and the generated classes against a fake host (the real host is covered by the
// GTK integration tests): identity, construction, outs, values, listeners, disposal.
import assert from 'node:assert/strict';
import { beforeEach, test } from 'node:test';
import { NativeObject, NativeValue, WRAP, wrapObject, DisposedError } from '../src/core.js';
import { ListStore } from '../gtk/gio/ListStore.js';
import { File } from '../gtk/gio/File.js';
import { ListModel } from '../gtk/gio/ListModel.js';
import { FontDescription } from '../gtk/pango/FontDescription.js';
import { TextBuffer } from '../gtk/gtk/TextBuffer.js';

let log;
let nextHandle;
let results;

class Node {
  constructor(h) {
    this.h = h;
  }
  get svlHandle() {
    return this.h;
  }
}

function installHost() {
  log = [];
  nextHandle = 100;
  results = new Map();
  const nodes = new Map();
  const node = (h) => nodes.get(h) ?? (nodes.set(h, new Node(h)), nodes.get(h));
  globalThis.__svelters_host = {
    construct(kind, props) {
      log.push(['construct', kind, props]);
      return node(++nextHandle);
    },
    callStatic(kind, method, args) {
      log.push(['callStatic', kind, method, args]);
      const r = results.get(`${kind}.${method}`);
      return typeof r === 'function' ? r(args, node) : (r ?? node(++nextHandle));
    },
    call(h, method, args) {
      log.push(['call', h, method, args.map((a) => (a && typeof a === 'object' && 'svlHandle' in a ? `#${a.svlHandle}` : a))]);
      const r = results.get(method);
      return typeof r === 'function' ? r(args, node) : r;
    },
    getProp(h, name) {
      log.push(['getProp', h, name]);
      return results.get(`prop:${name}`);
    },
    setProp(h, name, v) {
      log.push(['setProp', h, name, v]);
    },
    listen(h, signal, on) {
      log.push(['listen', h, signal, on]);
    },
    destroy(h) {
      log.push(['destroy', h]);
    },
  };
  return node;
}

beforeEach(() => {
  delete globalThis.__svelters_dispatch;
  installHost();
});

test('new Class(props) constructs with native property names and type arguments', () => {
  const store = new ListStore({ itemType: File });
  assert.deepEqual(log[0], ['construct', 'Gio.ListStore', { 'item-type': 'Gio.File' }]);
  assert.equal(store.svlHandle, 101);
  assert.ok(store instanceof NativeObject);
  assert.throws(() => new ListStore({ nope: 1 }), /no property 'nope'/);
  assert.throws(() => new File(), /cannot be constructed/);
});

test('one JS object per native object, and a more specific declared class upgrades the wrapper', () => {
  const node = installHost();
  const n = node(7);
  const a = wrapObject(ListModel, n);
  assert.equal(wrapObject(ListModel, n), a, 'same node, same wrapper');
  assert.equal(wrapObject(ListModel, node(7)), a);
  assert.ok(!(a instanceof ListStore));
  const b = wrapObject(ListStore, n);
  assert.equal(b, a, 'identity is kept');
  assert.ok(b instanceof ListStore, 'and the wrapper gained the richer class');
  assert.equal(wrapObject(ListStore, null), null);
});

test('methods call by native name, statics by native type, returns are wrapped as the declared class', () => {
  const node = installHost();
  const store = new ListStore({ itemType: 'Gio.File' });
  results.set('get_n_items', 3);
  assert.equal(store.getNItems(), 3);
  results.set('get_item', (_a, nodeOf) => nodeOf(55));
  const item = store.getObject(0);
  assert.equal(item.svlHandle, 55);
  assert.equal(store.getObject(0), item, 'the same node gives the same wrapper');
  store.append(item);
  assert.deepEqual(log.at(-1), ['call', 101, 'append', ['#55']]);
  results.set('Gio.File.new_for_path', (_a, nodeOf) => nodeOf(77));
  const f = File.newForPath('/tmp');
  assert.ok(f instanceof File);
  assert.deepEqual(log.at(-1), ['callStatic', 'Gio.File', 'new_for_path', ['/tmp']]);
  void node;
});

test('out parameters come back as an object, trailing nullable parameters default to null', () => {
  const f = wrapObject(File, installHost()(9));
  // the host keys out values by the native parameter name; the module renames them
  results.set('load_contents', () => ({ ret: true, contents: new Uint8Array([1, 2]), etag_out: 'e' }));
  const r = f.loadContents();
  assert.deepEqual(log.at(-1), ['call', 9, 'load_contents', [null]]);
  assert.equal(r.ret, true);
  assert.deepEqual([...r.contents], [1, 2]);
  assert.equal(r.etagOut, 'e');
});

test('async members return a Promise, pass { signal } last and shape the finish result', async () => {
  const node = installHost();
  const f = wrapObject(File, node(9));
  results.set('load_contents_async', () => Promise.resolve({ ret: true, contents: new Uint8Array([7]), etag_out: null }));
  const p = f.loadContentsAsync();
  assert.ok(p instanceof Promise);
  assert.deepEqual(log.at(-1), ['call', 9, 'load_contents_async', []]);
  const r = await p;
  assert.deepEqual(Object.keys(r), ['ret', 'contents', 'etagOut']);
  assert.equal(r.etagOut, null);
  const signal = new AbortController().signal;
  results.set('query_info_async', (args) => Promise.resolve(node(55)));
  const info = await f.queryInfoAsync('standard::size', 'none', 0, { signal });
  assert.deepEqual(log.at(-1).slice(0, 3), ['call', 9, 'query_info_async']);
  assert.equal(log.at(-1)[3].length, 4, 'options travel after the declared arguments');
  assert.equal(info.constructor.name, 'FileInfo');
  // a rejection passes through unchanged
  results.set('delete_async', () => Promise.reject(new Error('nope')));
  await assert.rejects(f.deleteAsync(), /nope/);
  // async statics go through callStatic
  results.set('Gio.File.new_tmp_async', () => Promise.resolve({ ret: node(60), iostream: node(61) }));
  const t = await File.newTmpAsync(null);
  assert.equal(t.ret.constructor.name, 'File');
  assert.equal(t.iostream.constructor.name, 'FileIOStream');
  assert.throws(() => f.loadContentsAsync(42), /options must be an object/);
});

test('callback arguments that are objects arrive as generated classes', async () => {
  const node = installHost();
  const { CustomFilter } = await import('../gtk/gtk/CustomFilter.js');
  const cf = wrapObject(CustomFilter, node(909));
  let seen;
  results.set('set_filter_func', (args) => args[0](node(70)));
  cf.setFilterFunc((item) => (seen = item));
  assert.equal(seen.constructor.name, 'Object');
  assert.equal(seen.svlHandle, 70);
  results.set('set_filter_func', (args) => args[0]);
  assert.equal(cf.setFilterFunc(null), undefined);
});

test('property accessors read and write through getProp / setProp', () => {
  const tb = new TextBuffer({ text: 'hi' });
  assert.deepEqual(log[0], ['construct', 'Gtk.TextBuffer', { text: 'hi' }]);
  results.set('prop:text', 'hello');
  assert.equal(tb.text, 'hello');
  tb.text = 'bye';
  assert.deepEqual(log.at(-1), ['setProp', 101, 'text', 'bye']);
});

test('values are immutable wrappers with native equality and string form', () => {
  const fake = (h) => ({ get __svlBoxed() { return [h, 1]; }, dispose() { log.push(['release', h]); } });
  results.set('Pango.FontDescription.from_string', () => fake(500));
  const fd = FontDescription.fromString('Sans 12');
  assert.ok(fd instanceof NativeValue);
  assert.deepEqual(fd.__svlBoxed, [500, 1]);
  assert.equal(typeof fd.setFamily, 'undefined', 'mutators are hidden');
  assert.equal(typeof fd.getFamily, 'function');
  results.set('equal', true);
  assert.equal(fd.equals(FontDescription.fromString('Sans 12')), true);
  results.set('to_string', 'Sans 12');
  assert.equal(fd.toString(), 'Sans 12');
  fd.dispose();
  assert.equal(fd.__svlBoxed, null);
  assert.throws(() => fd.getFamily(), DisposedError);
  assert.deepEqual(log.at(-1), ['release', 500]);
});

test('on() listens once per signal, shapes dispatch, chains to the previous dispatcher and returns its disposer', () => {
  const seen = [];
  globalThis.__svelters_dispatch = (h, s, a) => seen.push(['prev', h, s, a]);
  const store = new ListStore({ itemType: File });
  const got = [];
  const off1 = store.on('items-changed', (p, r, a) => got.push([p, r, a]));
  const off2 = store.on('itemsChanged', () => got.push('second'));
  assert.deepEqual(log.filter((e) => e[0] === 'listen'), [['listen', 101, 'items-changed', true]], 'one native connection');
  globalThis.__svelters_dispatch(101, 'items-changed', [0, 0, 1], false, 0);
  assert.deepEqual(got, [[0, 0, 1], 'second']);
  globalThis.__svelters_dispatch(999, 'clicked', [], false, 0);
  assert.deepEqual(seen, [['prev', 999, 'clicked', []]]);
  off1();
  off2();
  assert.deepEqual(log.at(-1), ['listen', 101, 'items-changed', false]);
  globalThis.__svelters_dispatch(101, 'items-changed', [0, 0, 1], false, 0);
  assert.equal(got.length, 2);
});

test('dispose() releases once, makes the wrapper stale and stops listeners', () => {
  const store = new ListStore({ itemType: File });
  store.on('items-changed', () => {});
  store.dispose();
  store.dispose();
  assert.equal(log.filter((e) => e[0] === 'destroy').length, 1);
  assert.equal(store.disposed, true);
  assert.throws(() => store.getNItems(), DisposedError);
  assert.ok(WRAP);
});
