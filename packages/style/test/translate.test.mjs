// Golden tests for the translator. `UPDATE=1 node --test test/` rewrites test/golden/*.golden.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import test from 'node:test';
import { compile } from 'svelte/compiler';
import { formatDiagnostic, inlineToNative, parseDeclarations } from '../src/decls.js';
import { DEFAULT_TOKENS, paletteOf, renderGtkTokens, renderQss, resolveTokens } from '../src/tokens.js';
import { extractNative, translateAll, translateSheet } from '../src/translate.js';

const dir = new URL('.', import.meta.url);
const read = (p) => fs.readFileSync(new URL(p, dir), 'utf8');

function styleOf(name) {
  const src = read(`cases/${name}.svelte`);
  const m = /<style>([\s\S]*)<\/style>/.exec(src);
  const { css, native, errors } = extractNative(m[1]);
  const markup = src.replace(m[0], `<style>${css}</style>`);
  const r = compile(markup, { filename: `${name}.svelte`, css: 'external', cssHash: ({ name: n }) => `svelte-${n.toLowerCase()}`, experimental: { customRenderer: '/x/renderer.js' } });
  return { css: r.css?.code ?? '', native, errors };
}

function golden(name, text) {
  const file = new URL(`golden/${name}.golden`, dir);
  if (process.env.UPDATE) {
    fs.mkdirSync(new URL('golden/', dir), { recursive: true });
    fs.writeFileSync(file, text);
  }
  assert.equal(text, fs.readFileSync(file, 'utf8'), `golden ${name} differs (UPDATE=1 to refresh)`);
}

function report(name) {
  const { css, native, errors } = styleOf(name);
  assert.deepEqual(errors, []);
  const gtk = translateSheet(css, 'gtk', { native: native.gtk });
  const qt = translateSheet(css, 'qt', { native: native.qt });
  const diags = [...gtk.diagnostics, ...qt.diagnostics].map(formatDiagnostic);
  return [
    '=== gtk ===', gtk.css, '=== qt ===', qt.css, '=== qt dark ===', qt.dark, '=== diagnostics ===', diags.join('\n'), '',
  ].join('\n');
}

test('card: portable subset, states, tokens, dark', () => golden('card', report('card')));
test('unsupported: every drop is diagnosed and names the toolkit', () => {
  const text = report('unsupported');
  golden('unsupported', text);
  const diags = text.split('=== diagnostics ===')[1].trim().split('\n');
  for (const d of diags) assert.match(d, /^(error|warning|info) \[(gtk|qt)\] /);
});
test('native: blocks pass through verbatim to their own toolkit only', () => golden('native', report('native')));

test('qt output never contains constructs QSS cannot parse', () => {
  for (const c of ['card', 'unsupported', 'native']) {
    const { css, native } = styleOf(c);
    const qt = translateSheet(css, 'qt', { native: native.qt });
    const text = (qt.css + qt.dark).replace(/\$\{[^}]*\}/g, 'X');
    assert.doesNotMatch(text.replace(/@native[\s\S]*/, ''), /:where|:is\(|var\(|--[a-z]|@media|@keyframes|oklch|calc\(/, c);
  }
});

test('translateAll bundles sheets and diagnostics', () => {
  const { css } = styleOf('card');
  const r = translateAll(css);
  assert.ok(r.sheets.gtk.includes('@media (prefers-color-scheme: dark)'));
  assert.ok(r.sheets.qtDark.includes('border-color: #666'));
  assert.deepEqual(r.diagnostics.filter((d) => d.level === 'error' && d.toolkit === 'gtk'), []);
});

test('inline style: gtk keeps var(), qt gets placeholders, opacity goes to the widget API', () => {
  const s = 'color: var(--sv-accent); font-weight: bold; opacity: 0.5; background-color: #fff';
  const g = inlineToNative(s, 'gtk');
  assert.equal(g.text, 'color: var(--sv-accent);\nfont-weight: bold;\nbackground-color: #fff;\nbackground-image: none;');
  assert.equal(g.opacity, '0.5');
  const q = inlineToNative(s, 'qt', 'QPushButton');
  assert.equal(q.text, 'color: ${--sv-accent};\nfont-weight: bold;\nbackground-color: #fff;\nborder: none;');
  assert.equal(q.opacity, '0.5');
  const l = inlineToNative('background: red', 'qt', 'QLabel');
  assert.equal(l.text, 'background: red;');
  const bad = inlineToNative('display: none; color: red', 'qt');
  assert.equal(bad.text, 'color: red;');
  assert.match(formatDiagnostic(bad.diagnostics[0]), /^warning \[qt\] property 'display' is not supported/);
});

test('parseDeclarations handles parentheses, quotes and trailing semicolons', () => {
  assert.deepEqual(parseDeclarations('a: b(c;d); font-family: "x;y", serif;; --k:1'), [
    { prop: 'a', value: 'b(c;d)' },
    { prop: 'font-family', value: '"x;y", serif' },
    { prop: '--k', value: '1' },
  ]);
});

test('tokens: gtk provider text, qss rendering, palette', () => {
  const t = resolveTokens('dark', { '--sv-accent': '#ff0000', '--sv-radius': null });
  assert.equal(t['--sv-accent'], '#ff0000');
  assert.ok(!('--sv-radius' in t));
  assert.ok(renderGtkTokens(t).startsWith(':root {\n  --sv-bg: #242424;'));
  const tpl = 'a {\n  color: ${--sv-fg};\n  border-radius: ${--sv-radius};\n  margin: ${--sv-nope|3px};\n  padding: ${--sv-nope};\n}';
  assert.equal(renderQss(tpl, t), 'a {\n  color: #ffffff;\n  margin: 3px;\n}');
  assert.deepEqual(paletteOf(DEFAULT_TOKENS.light).slice(0, 2), ['window=#f6f5f4', 'button=#ffffff']);
});
