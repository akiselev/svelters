// The styling guide must say what the style tables say: its setter-property tables are read from
// docs/site and compared with STYLE_TABLES (generated from metadata/style/<toolkit>.yaml), and
// the generated reference pages list every property, so the docs cannot drift from the code.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import test from 'node:test';
import { STYLE_TABLES } from '../src/props.generated.js';
import { DEFAULT_TOKENS } from '../src/tokens.js';
import { appliedThrough, renderStylingPages } from '../../../docs/site/scripts/lib/styling.mjs';

const page = fs.readFileSync(new URL('../../../docs/site/src/content/docs/guides/styling.md', import.meta.url), 'utf8');

/** Rows `| `name` | applied through | applies to |` of the guide section `heading`. */
function tableRows(heading) {
  const start = page.indexOf(`\n${heading}\n`);
  assert.ok(start > 0, `section ${heading} found`);
  const end = page.indexOf('\n#', start + heading.length + 2);
  return page
    .slice(start, end < 0 ? undefined : end)
    .split('\n')
    .filter((l) => l.startsWith('| `'))
    .map((l) => l.split('|').slice(1, -1).map((c) => c.trim()));
}

for (const [tk, heading] of [['gtk', '### GTK setter properties'], ['qt', '### Qt setter properties']]) {
  test(`the guide's ${tk} setter properties equal the ${tk} style table`, () => {
    const documented = Object.fromEntries(tableRows(heading).map(([n, via, kinds]) => [n.replace(/`/g, ''), { via, kinds }]));
    const table = Object.entries(STYLE_TABLES[tk].props).filter(([, e]) => e.via !== 'css');
    assert.deepEqual(Object.keys(documented).sort(), table.map(([n]) => n).sort(), 'same property set');
    for (const [n, e] of table) {
      assert.equal(documented[n].via, appliedThrough(tk, e), `${n}: applied through`);
      assert.equal(documented[n].kinds, (e.via === 'setter' ? e.kinds : e.parents).map((k) => `\`${k}\``).join(', '), `${n}: applies to`);
    }
  });
}

test('the generated reference pages list every property of their toolkit', () => {
  const pages = renderStylingPages();
  for (const [tk, t] of Object.entries(STYLE_TABLES)) {
    const md = pages.get(`api/styling/${tk}.md`);
    assert.ok(md, `api/styling/${tk}.md`);
    for (const [n, e] of Object.entries(t.props)) assert.ok(md.includes(`| \`${n}\` | ${appliedThrough(tk, e)} |`), `${tk}: ${n}`);
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
