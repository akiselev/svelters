// Setter rules on a Qt-dialect host: child attributes (`stretch`) go to setChildAttr on the
// parent, after every insert, and an explicit `parent:` attribute wins.
import assert from 'node:assert/strict';
import test from 'node:test';
import { createTestHost } from '@svelters/testhost';

const th = createTestHost({ toolkit: 'qt' });
const style = th.host.style.bind(th.host);
th.host.style = (op, h, text) => (op === 0 ? 2 : style(op, h, text)); // QSS dialect
globalThis.__svelters_host = th.host;
const { default: r, root, registerStyles } = await import('../src/index.js');

const el = (tag, attrs = {}) => {
  const e = r.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) r.setAttribute(e, k, v);
  return e;
};
const flush = () => new Promise((res) => setImmediate(res));
const take = () => th.lines.splice(0).filter((l) => l.startsWith('set ') || l.startsWith('attr '));

registerStyles('App.svelte', {
  gtk: '',
  qt: '',
  qtDark: '',
  rules: {
    gtk: [],
    qt: [
      { sel: { tag: null, classes: ['grow'], id: null }, spec: [0, 1, 0], order: 0, decls: [['stretch', 2], ['minimum-width', 40]] },
      { sel: { tag: 'box-layout', classes: [], id: null }, spec: [0, 0, 1], order: 1, decls: [['contents-margins', { top: 1, right: 2, bottom: 1, left: 2 }]] },
    ],
  },
});

test('child attributes from rules reach the parent after insert; class changes reset them', async () => {
  const win = el('main-window');
  const box = el('box-layout');
  const b = el('push-button', { class: 'grow' });
  r.insert(box, b, null);
  r.insert(win, box, null);
  r.insert(root, win, null);
  await flush();
  let log = take();
  assert.ok(log.includes(`attr #${box.h} #${b.h} stretch 2`), log.join('\n'));
  assert.ok(log.includes(`set #${b.h} minimum-width 40`), log.join('\n'));
  assert.ok(log.includes(`set #${box.h} contents-margins {"top":1,"right":2,"bottom":1,"left":2}`), log.join('\n'));
  r.setAttribute(b, 'class', '');
  await flush();
  log = take();
  assert.ok(log.includes(`attr #${box.h} #${b.h} stretch null`), log.join('\n'));
});

test('an explicit parent: attribute wins over a rule', async () => {
  const win = el('main-window');
  const box = el('box-layout');
  const b = el('push-button', { class: 'grow', 'parent:stretch': 5 });
  r.insert(box, b, null);
  r.insert(win, box, null);
  r.insert(root, win, null);
  await flush();
  const log = take().filter((l) => l.includes('stretch'));
  assert.deepEqual(log, [`attr #${box.h} #${b.h} stretch 5`]);
});
