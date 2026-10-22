// CS5e: a backend that runs the Rust CSS engine itself (egui; `SVL_STYLE_TARGET` answers 4) gets
// raw CSS: component sheets as Svelte scoped them, user classes, the id, the untranslated `style=`
// text and the app's token overrides as a `:root` sheet (DS1: the backend's theme owns the default
// tokens and the scheme). No setter rules run in JS.
import assert from 'node:assert/strict';
import test from 'node:test';
import * as $ from 'svelte/internal/client';
import { createTestHost } from '@svelters/testhost';

const th = createTestHost();
const style = th.host.style;
th.host.style = (op, h, text) => (op === 0 ? 4 : style(op, h, text));
globalThis.__svelters_host = th.host;
const $n = await import('../src/index.js');
const { registerStyles, setTokens, setColorScheme } = await import('../src/style.js');

const flush = () => new Promise((res) => setImmediate(res));
const take = () => th.lines.splice(0).join('\n');

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

test('sheets, classes, id and inline style reach an engine backend as CSS', async () => {
  registerStyles('App.svelte', { css: 'button.svelte-x:hover { background-color: red; width: 50%; }\n' });
  let log = take();
  assert.match(log, /style sheet "\*" null/, 'stale sheets cleared once');
  assert.ok(!/@tokens/.test(log), 'no default tokens: the backend owns them');
  assert.ok(!/style scheme/.test(log), 'no scheme until the app chooses one');
  assert.match(log, /style sheet "App.svelte" "button.svelte-x:hover \{ background-color: red; width: 50%; \}\\n"/);
  const b = mount((c) => {
    const b = $n.element('button');
    $n.attr(b, 'class', 'primary svelte-x');
    $n.attr(b, 'id', 'go');
    $n.attr(b, 'style', 'width: calc(100% - 8px); color: var(--sv-accent)');
    $n.append(c, b);
    return b;
  });
  await flush();
  log = take();
  assert.match(log, /style #\d+ classes "primary svelte-x"/, 'no sv-k- markers');
  assert.match(log, /style #\d+ name "go"/);
  assert.match(log, /style #\d+ inline "width: calc\(100% - 8px\); color: var\(--sv-accent\)"/, 'raw, untranslated');
  assert.ok(!/style #\d+ opacity/.test(log), 'opacity is the engine’s');
  $n.attr(b, 'class', 'primary svelte-x on');
  await flush();
  assert.match(take(), /classes "primary svelte-x on"/);
  setTokens({ '--sv-accent': '#ff0000' });
  await flush();
  log = take();
  assert.match(log, /style sheet "@tokens" ":root \{\\n  --sv-accent: #ff0000;\\n\}\\n"/, 'only the override');
  assert.ok(!/style scheme/.test(log));
  setColorScheme('dark');
  await flush();
  log = take();
  assert.match(log, /style scheme "dark"/, 'a chosen scheme reaches the backend');
  assert.ok(!/@tokens/.test(log), 'tokens unchanged');
});
