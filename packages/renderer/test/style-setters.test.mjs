// Setter rules and inline setter properties (ST1) against the TestHost op log: matching on
// creation and on class / id changes (class:), precedence, restore of the native value.
// SVELTERS_TEST_TOOLKIT=qt makes the TestHost answer the QSS dialect (Qt table: child attributes).
import assert from 'node:assert/strict';
import test from 'node:test';
import { createTestHost } from '@svelters/testhost';

const th = createTestHost();
const style = th.host.style.bind(th.host);
th.host.style = (op, h, text) => (op === 0 ? 1 : style(op, h, text)); // GTK CSS dialect
let reads = [];
const getProp = th.host.getProp.bind(th.host);
th.host.getProp = (h, name) => {
  reads.push(`get #${h} ${name}`);
  return getProp(h, name);
};
globalThis.__svelters_host = th.host;
const { default: r, root, registerStyles } = await import('../src/index.js');

const el = (tag, attrs = {}) => {
  const e = r.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) r.setAttribute(e, k, v);
  return e;
};
const flush = () => new Promise((res) => setImmediate(res));
const take = () => th.lines.splice(0).filter((l) => l.startsWith('set ') || l.startsWith('attr '));

const rule = (sel, spec, order, decls) => ({ sel: { tag: null, classes: [], id: null, ...sel }, spec, order, decls });

registerStyles('App.svelte', {
  gtk: '',
  qt: '',
  qtDark: '',
  rules: {
    gtk: [
      rule({ classes: ['toolbar'] }, [0, 1, 0], 0, [['spacing', 8], ['orientation', 'horizontal']]),
      rule({ tag: 'box', id: 'main' }, [1, 0, 1], 1, [['spacing', 12]]),
      rule({ tag: 'button' }, [0, 0, 1], 2, [['width-request', 80]]),
      rule({ classes: ['dense'] }, [0, 1, 0], 3, [['spacing', 2]]),
      // a class rule matching a kind it does not apply to is skipped silently
      rule({ classes: ['wide'] }, [0, 1, 0], 4, [['spacing', 99], ['hexpand', true]]),
    ],
    qt: [rule({ classes: ['toolbar'] }, [0, 1, 0], 0, [['spacing', 5]])],
  },
});

test('rules apply on creation; type selectors need no class; the native value is read first', async () => {
  const win = el('window');
  const box = el('box', { class: 'toolbar' });
  const b = el('button');
  r.insert(box, b, null);
  r.insert(win, box, null);
  reads = [];
  r.insert(root, win, null);
  await flush();
  const log = take();
  assert.ok(log.includes('set #2 spacing 8'), log.join('\n'));
  assert.ok(log.includes('set #2 orientation "horizontal"'), log.join('\n'));
  assert.ok(log.includes('set #3 widthRequest 80') || log.includes('set #3 width-request 80'), log.join('\n'));
  assert.ok(reads.some((x) => x.startsWith('get #2 ')), 'original captured with getProp');
  assert.ok(!log.some((l) => l.startsWith('set #1 ')), 'the window matches nothing');
});

test('class changes (class:) re-match; later equal-specificity rules win; dropping a rule restores', async () => {
  const win = el('window');
  const box = el('box', { class: 'toolbar' });
  r.insert(win, box, null);
  r.insert(root, win, null);
  await flush();
  take();
  const h = box.h;
  r.setAttribute(box, 'class', 'toolbar dense');
  await flush();
  assert.deepEqual(take(), [`set #${h} spacing 2`]);
  r.setAttribute(box, 'class', 'other');
  await flush();
  const log = take();
  // spacing and orientation go back to what getProp read before the first override (null here)
  assert.deepEqual(log.sort(), [`set #${h} orientation null`, `set #${h} spacing null`]);
});

test('id selectors outrank classes', async () => {
  const win = el('window');
  const box = el('box', { class: 'toolbar', id: 'main' });
  r.insert(win, box, null);
  r.insert(root, win, null);
  await flush();
  const log = take();
  assert.ok(log.includes(`set #${box.h} spacing 12`), log.join('\n'));
  assert.ok(!log.includes(`set #${box.h} spacing 8`), 'the lower rule is not written first');
});

test('explicit attribute > style= > rules; removing them falls back a layer', async () => {
  const win = el('window');
  const box = el('box', { class: 'toolbar', spacing: 4, style: 'spacing: 6px; color: red' });
  r.insert(win, box, null);
  r.insert(root, win, null);
  await flush();
  const h = box.h;
  let log = take();
  assert.ok(log.includes(`set #${h} spacing 4`), log.join('\n'));
  assert.ok(!log.includes(`set #${h} spacing 6`) && !log.includes(`set #${h} spacing 8`), log.join('\n'));
  r.removeAttribute(box, 'spacing');
  await flush();
  log = take();
  assert.ok(log.includes(`set #${h} spacing 6`), `inline next: ${log.join(' | ')}`);
  r.setAttribute(box, 'style', 'color: red');
  await flush();
  assert.deepEqual(take(), [`set #${h} spacing 8`]);
  r.setAttribute(box, 'spacing', 1);
  await flush();
  assert.deepEqual(take(), [`set #${h} spacing 1`]);
});

test('style: directives with setter properties take the inline path', async () => {
  const win = el('window');
  const lab = el('label');
  r.insert(win, lab, null);
  r.insert(root, win, null);
  await flush();
  take();
  r.setAttribute(lab, 'style', 'hexpand: true; xalign: 0.25;');
  await flush();
  assert.deepEqual(take().sort(), [`set #${lab.h} hexpand true`, `set #${lab.h} xalign 0.25`]);
  r.removeAttribute(lab, 'style');
  await flush();
  assert.deepEqual(take().sort(), [`set #${lab.h} hexpand null`, `set #${lab.h} xalign null`]);
});

test('a class rule on a kind it does not apply to is skipped for that prop only', async () => {
  const win = el('window');
  const lab = el('label', { class: 'wide' });
  r.insert(win, lab, null);
  r.insert(root, win, null);
  await flush();
  assert.deepEqual(take(), [`set #${lab.h} hexpand true`]);
});

test('re-registering a sheet without rules restores what its rules set', async () => {
  const win = el('window');
  const box = el('box', { class: 'toolbar' });
  r.insert(win, box, null);
  r.insert(root, win, null);
  await flush();
  take();
  registerStyles('App.svelte', { gtk: '', qt: '', qtDark: '' });
  await flush();
  const log = take();
  assert.ok(log.includes(`set #${box.h} spacing null`), log.join('\n'));
});
