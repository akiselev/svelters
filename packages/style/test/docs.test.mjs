// The styling guide's support matrix must say what the translator does: this reads the table from
// docs/site and compares it with MATRIX, so the page cannot drift from the code.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import test from 'node:test';
import { DEFAULT_TOKENS } from '../src/tokens.js';
import { MATRIX } from '../src/vocab.js';

const page = fs.readFileSync(new URL('../../../docs/site/src/content/docs/guides/styling.md', import.meta.url), 'utf8');

function matrixRows() {
  const start = page.indexOf('## Support matrix');
  const end = page.indexOf('`yes` is translated', start);
  assert.ok(start > 0 && end > start, 'support matrix section found');
  return page
    .slice(start, end)
    .split('\n')
    .filter((l) => l.startsWith('| `'))
    .map((l) => l.split('|').slice(1, -1).map((c) => c.trim()));
}

test('the documented support matrix equals MATRIX', () => {
  const documented = {};
  for (const [props, gtk, qt, note] of matrixRows()) {
    for (const m of props.matchAll(/`([^`]+)`/g)) documented[m[1]] = { gtk, qt, note };
  }
  assert.deepEqual(Object.keys(documented).sort(), Object.keys(MATRIX).sort(), 'same property set');
  for (const [name, row] of Object.entries(MATRIX)) {
    assert.equal(documented[name].gtk, row.gtk, `${name}: gtk`);
    assert.equal(documented[name].qt, row.qt, `${name}: qt`);
    assert.equal(documented[name].note, row.note ?? '', `${name}: note`);
  }
});

test('the documented default tokens equal the built-in sets', () => {
  const start = page.indexOf('| Token | Light | Dark |');
  const rows = page
    .slice(start)
    .split('\n')
    .slice(2)
    .filter((l) => l.startsWith('| `'))
    .map((l) => l.split('|').slice(1, -1).map((c) => c.trim().replace(/`/g, '')));
  const doc = new Map();
  for (const [name, light, dark] of rows) {
    if (name.includes(' to ')) {
      const [a] = name.split(' to ');
      const prefix = a.replace(/\d+$/, '');
      light.split(', ').forEach((v, i) => doc.set(`${prefix}${i + 1}`, [v, dark === 'same' ? v : dark]));
    } else if (name.includes(', ')) {
      name.split(', ').forEach((n, i) => doc.set(n, [light.split(', ')[i], dark === 'same' ? light.split(', ')[i] : dark.split(', ')[i]]));
    } else doc.set(name, [light, dark]);
  }
  for (const name of Object.keys(DEFAULT_TOKENS.light)) {
    assert.ok(doc.has(name), `${name} documented`);
    assert.deepEqual(doc.get(name), [DEFAULT_TOKENS.light[name], DEFAULT_TOKENS.dark[name]], name);
  }
});
