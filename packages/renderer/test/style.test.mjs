// class / style / id forwarding, stylesheet registration, tokens and scheme, against the TestHost.
import assert from 'node:assert/strict';
import test from 'node:test';
import { createTestHost } from '@svelters/testhost';

const th = createTestHost();
globalThis.__svelters_host = th.host;
const { default: r, root, registerStyles, setTokens, setColorScheme, getTokens } = await import('../src/index.js');

const el = (tag, attrs = {}) => {
  const e = r.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) r.setAttribute(e, k, v);
  return e;
};
const flush = () => new Promise((res) => setImmediate(res));
const take = () => th.lines.splice(0).join('\n');

test('apps without styling send no style ops', () => {
  const win = el('window', { title: 'T' });
  const b = el('button');
  r.insert(win, b, null);
  r.insert(root, win, null);
  assert.ok(!take().includes('style'));
});

test('class / id / style are forwarded once, diffed, and coalesced', async () => {
  const win = el('window');
  const b = el('button', { class: 'primary svelte-x', id: 'ok', style: 'color: red; opacity: 0.5; display: none' });
  r.insert(win, b, null);
  r.insert(root, win, null);
  const log = take();
  assert.match(log, /style sheet "\*" null\nstyle scheme "light" 0\n/, 'first use initialises the backend once');
  assert.match(log, /style #\d+ classes "primary svelte-x"/);
  assert.match(log, /style #\d+ name "ok"/);
  assert.match(log, /style #\d+ inline "color: red;"/);
  assert.match(log, /style #\d+ opacity "0.5"/);
  // the same value again sends nothing; two changes in one job send one op each
  r.setAttribute(b, 'class', 'primary svelte-x');
  r.setAttribute(b, 'style', 'color: blue');
  r.setAttribute(b, 'style', 'color: red; opacity: 0.5; display: none');
  await flush();
  assert.equal(take(), '');
  r.setAttribute(b, 'class', 'primary svelte-x active');
  r.setAttribute(b, 'style', 'color: blue');
  await flush();
  const after = take();
  assert.match(after, /classes "primary svelte-x active"/);
  assert.match(after, /inline "color: blue;"/);
  assert.match(after, /opacity null/);
  r.removeAttribute(b, 'class');
  r.removeAttribute(b, 'style');
  await flush();
  assert.match(take(), /classes null[\s\S]*inline null/);
});

test('registered sheets mark ambiguous kinds and push the right dialect', async () => {
  registerStyles('App.svelte', { gtk: 'button.sv-k-button { color: red; }\n', qt: '.QPushButton { color: red; }\n', qtDark: '' });
  const log = take();
  assert.match(log, /style sheet "App.svelte" "button.sv-k-button \{ color: red; \}\\n"/, 'the test host speaks CSS');
  const win = el('window');
  const b = el('button');
  const tb = el('togglebutton', { class: 'x' });
  r.insert(win, b, null);
  r.insert(win, tb, null);
  r.insert(root, win, null);
  await flush();
  const l2 = take();
  assert.match(l2, /classes "sv-k-window"/);
  assert.match(l2, /classes "sv-k-button"/);
  assert.match(l2, /classes "sv-k-togglebutton x"/);
});

test('tokens and scheme batch into one refresh; GTK rewrites only the token provider', async () => {
  setTokens({ '--sv-accent': '#ff0000' });
  setTokens({ '--sv-radius': '2px' });
  setColorScheme('dark');
  assert.equal(take(), '', 'nothing is sent before the job ends');
  await flush();
  const log = take();
  const tokenSheets = log.split('\n').filter((l) => l.startsWith('style sheet "@tokens"'));
  assert.equal(tokenSheets.length, 1);
  assert.ok(tokenSheets[0].includes('--sv-accent: #ff0000;') && tokenSheets[0].includes('--sv-bg: #242424;'));
  assert.match(log, /style scheme "dark" 0/);
  assert.equal(getTokens()['--sv-radius'], '2px');
  setTokens({ '--sv-accent': '#ff0000' });
  await flush();
  assert.equal(take(), '', 'an unchanged token set sends nothing');
  setColorScheme('light');
  setTokens({ '--sv-accent': null, '--sv-radius': null });
  await flush();
  assert.match(take(), /--sv-accent: #3584e4;/);
});
