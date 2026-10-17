// Drives the renderer methods directly (no Svelte compile) against the TestHost.
import test from 'node:test';
import assert from 'node:assert/strict';
import { createTestHost } from '@svelters/testhost';

const th = createTestHost();
globalThis.__svelters_host = th.host;
const { default: r, root, dispatch } = await import('../src/index.js');

const el = (tag, attrs = {}) => {
  const e = r.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) r.setAttribute(e, k, v);
  return e;
};
const tick = () => new Promise((res) => setImmediate(res));
const take = () => th.lines.splice(0).join('\n');

test('lazy creation, label folding, ordering', () => {
  const win = el('window', { title: 'T' });
  const box = el('box', { spacing: '4' });
  const btn = el('button');
  const text = r.createTextNode('Hi');
  r.insert(btn, text, null);
  r.insert(box, btn, null);
  r.insert(win, box, null);
  assert.equal(take(), '', 'nothing native before attach to root');
  r.insert(root, win, null);
  assert.equal(
    take(),
    [
      'create window #1',
      'set #1 title "T"',
      'create box #2',
      'set #2 spacing 4',
      'create button #3',
      'set #3 label "Hi"',
      'insert #2 #3 before=end slot=default',
      'insert #1 #2 before=end slot=default',
      'call #1 present []',
    ].join('\n'),
  );
  r.setText(text, 'Yo');
  assert.equal(take(), 'set #3 label "Yo"');
});

test('dispatch returns handler results and the host event object', () => {
  const win = el('window');
  const b = el('button');
  r.insert(win, b, null);
  r.addEventListener(b, 'click', function (e) {
    if (e) e.defaultPrevented = true;
    return 'handled';
  });
  r.addEventListener(b, 'clicked', () => 42);
  r.insert(root, win, null);
  const lines = take().split('\n');
  const h = Number(lines.find((l) => l.startsWith('create button')).split('#')[1]);
  assert.ok(lines.includes(`listen #${h} clicked on`));
  assert.equal(lines.filter((l) => l.startsWith('listen')).length, 1, 'one listen per signal');
  const evt = { defaultPrevented: false };
  const out = dispatch(h, 'clicked', [evt]);
  assert.deepEqual(out.results, ['handled', 42]);
  assert.equal(out.event, evt);
  assert.equal(out.event.defaultPrevented, true);
  assert.deepEqual(dispatch(h, 'clicked', []).event, null);
});

test('sv-slot proxy routes native descendants to the parent slot', async () => {
  const win = el('window');
  const proxy = el('sv-slot', { name: 'titlebar' });
  const bar = el('headerbar');
  const body = el('box');
  r.insert(proxy, bar, null);
  r.insert(win, body, null);
  r.insert(win, proxy, body); // proxy before body in the tree
  r.insert(root, win, null);
  const out = take();
  assert.match(out, /insert #\d+ #\d+ before=end slot=titlebar/);
  assert.match(out, /insert #\d+ #\d+ before=end slot=default/);
  assert.ok(!out.includes('sv-slot'), 'proxy never reaches the host');
  // removing the proxy removes its widget
  r.remove(proxy);
  assert.match(take(), /^remove #\d+ #\d+$/);
  await tick();
  assert.match(take(), /^destroy #\d+$/);
});

test('move is remove + insert and is not destroyed; removal destroys children first', async () => {
  const win = el('window');
  const box = el('box');
  const a = el('label');
  const b = el('label');
  r.insert(box, a, null);
  r.insert(box, b, null);
  r.insert(win, box, null);
  r.insert(root, win, null);
  take();
  r.insert(box, b, a); // move b before a
  assert.match(take(), /^remove #\d+ #\d+\ninsert #\d+ #\d+ before=#\d+ slot=default$/);
  await tick();
  assert.equal(take(), '', 'moved node survives the sweep');
  r.remove(box);
  const rm = take();
  assert.match(rm, /^remove #\d+ #\d+$/);
  await tick();
  const d = take().split('\n');
  assert.equal(d.length, 3);
  assert.ok(d.every((l) => l.startsWith('destroy')));
});
