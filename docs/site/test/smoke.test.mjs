// Smoke test: the counter fixture's elements (window, box, label, button, entry) have pages for both
// toolkits, listing properties and events. Runs on the generated pages, and on dist/ when it was built.
import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { renderSite } from '../scripts/lib/render.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const site = path.resolve(here, '..');
const root = path.resolve(site, '../..');
const irDir = path.join(site, '.generated/ir');

if (!fs.existsSync(path.join(irDir, 'gtk.json')) || !fs.existsSync(path.join(irDir, 'qt.json'))) {
  const r = spawnSync('python3', [path.join(root, 'tools/docs/build_ir.py')], { stdio: 'inherit' });
  assert.equal(r.status, 0, 'build_ir.py failed');
}
const irs = ['gtk', 'qt'].map((tk) => JSON.parse(fs.readFileSync(path.join(irDir, `${tk}.json`), 'utf8')));
const { pages } = renderSite(irs, { base: '/', order: ['gtk', 'qt'] });
const COUNTER = ['window', 'box', 'label', 'button', 'entry'];

for (const tk of ['gtk', 'qt']) {
  for (const tag of COUNTER) {
    test(`${tk}: <${tag}> page lists properties and events`, () => {
      const md = pages.get(`api/${tk}/${tag}.md`);
      assert.ok(md, `no page for ${tk}/${tag}`);
      assert.match(md, /\n## Properties\n/);
      assert.match(md, /\n## Events\n/);
      assert.match(md, /Annotated &amp; tested/);
      assert.match(md, /### Property details/);
      // at least one real property row and one event row
      assert.match(md, /\| \[`[A-Za-z]+`\]\(#property-/);
      assert.ok(/\| \[`[\w-]+`\]\(#event-/.test(md) || /### Inherited events/.test(md), 'no events listed');
      assert.match(md, /Toolkit support/);
    });
  }
}

test('portable elements link between GTK and Qt', () => {
  for (const tag of COUNTER) {
    const gtk = pages.get(`api/gtk/${tag}.md`);
    assert.ok(gtk.includes(`/api/qt/${tag}/`), `gtk/${tag} does not link to qt/${tag}`);
    const qt = pages.get(`api/qt/${tag}.md`);
    assert.ok(qt.includes(`/api/gtk/${tag}/`), `qt/${tag} does not link to gtk/${tag}`);
  }
});

const dist = path.join(site, 'dist');
test('built site has the counter pages (skipped when dist/ is absent)', { skip: !fs.existsSync(dist) }, () => {
  for (const tk of ['gtk', 'qt']) {
    for (const tag of COUNTER) {
      const f = path.join(dist, 'api', tk, tag, 'index.html');
      assert.ok(fs.existsSync(f), `${f} missing`);
      const html = fs.readFileSync(f, 'utf8');
      assert.match(html, /id="properties"/);
      assert.match(html, /id="events"/);
    }
  }
  assert.ok(fs.existsSync(path.join(dist, 'pagefind')), 'search index missing');
});
