// Value-model behaviour of the renderer against the TestHost: attribute coercion from host
// descriptors, object-valued props, and the id path for hot ops.
import test from 'node:test';
import assert from 'node:assert/strict';
import { createTestHost } from '@svelters/testhost';

const th = createTestHost();
globalThis.__svelters_host = th.host;
const { default: r, root } = await import('../src/index.js');

const el = (tag, attrs = {}) => {
  const e = r.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) r.setAttribute(e, k, v);
  return e;
};
const take = () => th.lines.splice(0).join('\n');

test('attribute strings are coerced from the host descriptors, not from a list in the renderer', () => {
  const win = el('window');
  const lbl = el('label', { wrap: '', 'margin-top': '3', opacity: '0.5', label: '7' });
  r.insert(win, lbl, null);
  r.insert(root, win, null);
  const out = take();
  assert.match(out, /set #\d+ wrap true/, 'a bare boolean attribute is true');
  assert.match(out, /set #\d+ margin-top 3\n/, 'integer property: "3" is 3');
  assert.match(out, /set #\d+ opacity 0.5/, 'number property');
  assert.match(out, /set #\d+ label "7"/, 'a string property keeps its text');
  // What cannot be coerced is passed through for the host to reject with a precise message.
  r.setAttribute(lbl, 'margin-top', '3px');
  assert.match(take(), /margin-top "3px"/);
  r.remove(win);
});

test('object-valued props (records, arrays) reach the host as they are', () => {
  const win = el('window');
  const box = el('box');
  r.insert(win, box, null);
  r.insert(root, win, null);
  take();
  r.setAttribute(box, 'geometry', { x: 1, y: 2, width: 3, height: 4 });
  r.setAttribute(box, 'tags', ['a', 'b']);
  assert.match(take(), /^set #\d+ geometry \{"x":1,"y":2,"width":3,"height":4\}\nset #\d+ tags \["a","b"\]$/);
  assert.throws(() => r.setAttribute(box, 'cb', () => {}), /functions are not valid host prop values/);
  r.remove(win);
});

test('hot ops go through cached ids when the host offers them', () => {
  const seen = { setProp: [], create: [], call: [] };
  const restore = [];
  for (const name of ['setProp', 'create', 'call']) {
    const orig = th.host[name];
    th.host[name] = (...a) => {
      seen[name].push(name === 'create' ? a[0] : a[1]);
      return orig(...a);
    };
    restore.push(() => (th.host[name] = orig));
  }
  try {
    const win = el('window', { title: 'ids' });
    const box = el('box', { spacing: '2' });
    r.insert(win, box, null);
    r.insert(root, win, null);
    take();
    assert.ok(seen.create.length > 0 && seen.create.every((k) => typeof k === 'number'), `kinds by id: ${seen.create}`);
    assert.ok(seen.setProp.length > 0 && seen.setProp.every((p) => typeof p === 'number'), `props by id: ${seen.setProp}`);
    assert.ok(seen.call.length > 0 && seen.call.every((m) => typeof m === 'number'), `methods by id: ${seen.call}`);
    r.remove(win);
  } finally {
    restore.forEach((f) => f());
  }
});

test('a host without id support still gets names', async () => {
  const bare = createTestHost();
  for (const k of ['kindId', 'propId', 'methodId', 'propType']) delete bare.host[k];
  const { default: r2, root: root2 } = await import('../src/index.js?bare');
  const saved = globalThis.__svelters_host;
  globalThis.__svelters_host = bare.host;
  try {
    const win = r2.createElement('window');
    const box = r2.createElement('box');
    r2.setAttribute(box, 'spacing', '4');
    r2.insert(win, box, null);
    r2.insert(root2, win, null);
    assert.match(bare.lines.join('\n'), /set #2 spacing "4"/, 'no descriptors: the text stays text');
    r2.remove(win);
    await new Promise((res) => setImmediate(res)); // the renderer destroys detached widgets in a microtask
  } finally {
    globalThis.__svelters_host = saved;
  }
});
