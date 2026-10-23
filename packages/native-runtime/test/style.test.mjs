// The X6 styling runtime (src/style.js) through compiled-shape components: class / style / id
// forwarding, stylesheet registration, tokens and scheme, against the TestHost. (Ported from the
// retired @svelters/renderer's style tests, RT1.)
import assert from 'node:assert/strict';
import test from 'node:test';
import * as $ from 'svelte/internal/client';
import { createTestHost } from '@svelters/testhost';

const th = createTestHost();
globalThis.__svelters_host = th.host;
const $n = await import('../src/index.js');
const { registerStyles, setTokens, setColorScheme, getTokens } = await import('../src/style.js');

const flush = () => new Promise((res) => setImmediate(res));
const take = () => th.lines.splice(0).join('\n');

/** Mounts a window holding the elements `body(container)` creates; returns their nodes. */
function mount(body) {
  let out;
  $n.mount(($$anchor, $$props) => {
    $.push($$props, true);
    const w = $n.toplevel('window', 'present');
    out = body($n.slot(w, null));
    $n.append($$anchor, w);
    $.pop();
  });
  return out;
}

const button = (c, attrs = {}) => {
  const b = $n.element('button');
  for (const [k, v] of Object.entries(attrs)) $n.attr(b, k, v);
  $n.append(c, b);
  return b;
};

test('apps without styling send no style ops', async () => {
  mount((c) => button(c));
  await flush();
  assert.ok(!take().includes('style'));
});

test('class / id / style are forwarded once, diffed, and coalesced', async () => {
  const b = mount((c) => button(c, { class: 'primary svelte-x', id: 'ok', style: 'color: red; opacity: 0.5; display: none' }));
  await flush();
  const log = take();
  assert.match(log, /style sheet "\*" null\nstyle scheme "light" 0\n/, 'first use initialises the backend once');
  assert.match(log, /style #\d+ classes "primary svelte-x"/);
  assert.match(log, /style #\d+ name "ok"/);
  assert.match(log, /style #\d+ inline "color: red;"/);
  assert.match(log, /style #\d+ opacity "0.5"/);
  // the same value again sends nothing; two changes in one job send one op each
  $n.attr(b, 'class', 'primary svelte-x');
  $n.attr(b, 'style', 'color: blue');
  $n.attr(b, 'style', 'color: red; opacity: 0.5; display: none');
  await flush();
  assert.equal(take(), '');
  $n.attr(b, 'class', 'primary svelte-x active');
  $n.attr(b, 'style', 'color: blue');
  await flush();
  const after = take();
  assert.match(after, /classes "primary svelte-x active"/);
  assert.match(after, /inline "color: blue;"/);
  assert.match(after, /opacity null/);
  $n.attr(b, 'class', null);
  $n.attr(b, 'style', null);
  await flush();
  assert.match(take(), /classes null[\s\S]*inline null/);
});

test('registered sheets mark ambiguous kinds and push the right dialect', async () => {
  registerStyles('App.svelte', { gtk: 'button.sv-k-button { color: red; }\n', qt: '.QPushButton { color: red; }\n', qtDark: '' });
  const log = take();
  assert.match(log, /style sheet "App.svelte" "button.sv-k-button \{ color: red; \}\\n"/, 'the test host speaks CSS');
  mount((c) => {
    button(c);
    const tb = $n.element('toggle-button');
    $n.attr(tb, 'class', 'x');
    $n.append(c, tb);
  });
  await flush();
  const l2 = take();
  assert.match(l2, /classes "sv-k-window"/);
  assert.match(l2, /classes "sv-k-button"/);
  assert.match(l2, /classes "sv-k-toggle-button x"/);
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

test('U3: utility classes on a non-egui backend warn once, semantic classes never', async () => {
  const warned = [];
  const warn = console.warn;
  console.warn = (m) => warned.push(String(m));
  try {
    mount((c) => button(c, { class: 'row text-input top-bar border flex sv-k-button p-x' }));
    await flush();
    assert.deepEqual(warned.filter((w) => w.includes('egui only')), [], 'semantic classes are not utilities');
    mount((c) => {
      button(c, { class: 'row hover:bg-list-hover' });
      button(c, { class: 'p-4 w-[120px] part-thumb:bg-accent' });
    });
    await flush();
    const u = warned.filter((w) => w.includes('egui only'));
    assert.equal(u.length, 1, 'one warning per process');
    assert.match(u[0], /'hover:bg-list-hover' is a plain class on gtk/);
  } finally {
    console.warn = warn;
  }
  take();
});
