// Golden test: a small IR fixture through the output generator. Update with UPDATE=1 pnpm test.
import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { renderSite } from '../scripts/lib/render.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const fixture = path.resolve(here, '../../../tools/docs/ir/fixtures/mini.ir.json');
const goldenDir = path.join(here, 'golden');

const ir = JSON.parse(fs.readFileSync(fixture, 'utf8'));
const { pages } = renderSite([ir], {
  base: '/',
  repo: 'https://example.invalid/repo',
  branch: 'main',
  planned: [{ id: 'egui', name: 'egui', status: 'planned' }],
  order: ['demo'],
  screens: new Set(['demo/button']),
});

test('mini IR renders the expected page set', () => {
  assert.deepEqual([...pages.keys()].sort(), [
    'api/demo/button.md',
    'api/demo/index.md',
    'api/demo/types/demoalign.md',
    'api/demo/types/index.md',
    'api/demo/types/demowidget.md',
    'api/index.md',
    'api/matrix.md',
    'coverage.md',
    'index.mdx',
  ].sort());
});

test('mini IR output matches the golden files', () => {
  for (const [rel, text] of pages) {
    const file = path.join(goldenDir, rel);
    if (process.env.UPDATE) {
      fs.mkdirSync(path.dirname(file), { recursive: true });
      fs.writeFileSync(file, text);
      continue;
    }
    assert.ok(fs.existsSync(file), `missing golden ${rel} (run UPDATE=1)`);
    assert.equal(text, fs.readFileSync(file, 'utf8'), `golden mismatch: ${rel}`);
  }
});

test('dangerous members get a callout, a row mark and a per-kind list (own and inherited)', () => {
  const widget = pages.get('api/demo/types/demowidget.md');
  const button = pages.get('api/demo/button.md');
  // per-member callouts on the details
  assert.match(widget, /:::danger\[Dangerous: corrupts the widget tree\]\nCalling `show` is reachable from app code/);
  assert.match(widget, /\*\*Use instead:\*\* The `visible` attribute or \{#if\}\./);
  assert.match(widget, /:::danger\[Dangerous: owned by the backend\]\nWriting this property is not possible from app code today/);
  assert.match(widget, /<code>metadata\/dangerous\/demo.yaml#show<\/code>/);
  // prose is escaped, code spans are not
  assert.match(widget, /renderer's `show` attribute \(&lt;demo&gt;\)/);
  // row marks in the member tables
  assert.match(widget, /`show` <span class="svl-danger"/);
  assert.match(widget, /\(#property-visible\) <span class="svl-danger"/);
  // the section lists own members, and a subclass lists inherited ones with links to the owner
  assert.match(widget, /## Dangerous members\n\n:::danger\[2 dangerous members\]/);
  assert.match(button, /## Dangerous members/);
  assert.match(button, /\[`show`\]\(\/api\/demo\/types\/demowidget\/#method-show\) <small>from Widget<\/small> \| method \|/);
  assert.match(button, /property \(not reachable today\)/);
  // a kind without any has no section; the coverage page counts them
  assert.doesNotMatch(pages.get('api/demo/types/demoalign.md'), /Dangerous members/);
  assert.match(pages.get('coverage.md'), /\| Dangerous members \| 1 properties, 0 events, 1 methods/);
});

test('xrefs resolve, unknown html is neutralised, unresolved links lose their target', () => {
  const button = pages.get('api/demo/button.md');
  assert.match(button, /\(\/api\/demo\/button\/#property-label\)/);
  assert.match(button, /&lt;unknown&gt;/);
  assert.doesNotMatch(button, /xref:/);
  const widget = pages.get('api/demo/types/demowidget.md');
  assert.match(widget, /\[Align\]\(\/api\/demo\/types\/demoalign\/\)/);
  assert.match(button, /editUrl: "https:\/\/example.invalid\/repo\/edit\/main\/metadata\/overrides\/demo\/Button.yaml"/);
  assert.match(widget, /Raw ingested/);
  assert.match(button, /Annotated &amp; tested/);
});
