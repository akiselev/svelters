// Smoke test: the counter fixture's elements (GTK window, box, label, button, entry; their Qt
// counterparts main-window, box-layout, label, push-button, line-edit) have pages, listing properties
// and events. Runs on the generated pages, and on dist/ when it was built.
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
const COUNTER = {
  gtk: ['window', 'box', 'label', 'button', 'entry'],
  qt: ['main-window', 'box-layout', 'label', 'push-button', 'line-edit'],
};

for (const tk of ['gtk', 'qt']) {
  for (const tag of COUNTER[tk]) {
    test(`${tk}: <${tag}> page lists properties and events`, () => {
      const md = pages.get(`api/${tk}/${tag}.md`);
      assert.ok(md, `no page for ${tk}/${tag}`);
      assert.match(md, /\n## Properties\n/);
      assert.match(md, /\n## Events\n/);
      assert.match(md, /Annotated &amp; tested/);
      assert.match(md, /### Property details/);
      // at least one real property row and one event row
      assert.match(md, /\| \[`[\w-]+`\]\(#property-/);
      assert.ok(/\| \[`[\w-]+`\]\(#event-/.test(md) || /### Inherited events/.test(md), 'no events listed');
      assert.match(md, /Toolkit support/);
    });
  }
}

test('portable elements link between GTK and Qt', () => {
  const shared = COUNTER.gtk.filter((t) => COUNTER.qt.includes(t));
  assert.ok(shared.length > 0, 'the counter shares a tag between GTK and Qt');
  for (const tag of shared) {
    const gtk = pages.get(`api/gtk/${tag}.md`);
    assert.ok(gtk.includes(`/api/qt/${tag}/`), `gtk/${tag} does not link to qt/${tag}`);
    const qt = pages.get(`api/qt/${tag}.md`);
    assert.ok(qt.includes(`/api/gtk/${tag}/`), `qt/${tag} does not link to gtk/${tag}`);
  }
});

test('dangerous members are warned on (metadata/dangerous, docs/notes/dangerous-members.md)', () => {
  const win = pages.get('api/gtk/window.md');
  assert.match(win, /\n## Dangerous members\n/);
  for (const m of ['set_titlebar', 'set_child', 'set_application', 'set_display', 'set_startup_id']) {
    assert.match(win, new RegExp('`' + m + '` <span class="svl-danger"'), `window method ${m} not marked`);
  }
  assert.match(win, /\[`application`\]\(#property-application\)/);
  assert.match(win, /:::danger\[Dangerous: corrupts the widget tree\]\nCalling `set_titlebar`/);
  // inherited: an application window lists Window's and Widget's dangerous members
  const appwin = pages.get('api/gtk/application-window.md');
  assert.match(appwin, /\[`set_application`\]\(\/api\/gtk\/window\/#method-set-application\) <small>from Window<\/small>/);
  assert.match(appwin, /\[`set_parent`\]\([^)]*#method-set-parent\) <small>from Widget<\/small>/);
  // Qt: backend-owned but not reachable today
  const mw = pages.get('api/qt/main-window.md');
  assert.match(mw, /`setCentralWidget` <span class="svl-danger"/);
  assert.match(mw, /Calling `setCentralWidget` is not possible from app code today/);
  // a member that is safe stays unmarked
  assert.doesNotMatch(win, /`present` <span class="svl-danger"/);
});

const dist = path.join(site, 'dist');
test('built site has the counter pages (skipped when dist/ is absent)', { skip: !fs.existsSync(dist) }, () => {
  for (const tk of ['gtk', 'qt']) {
    for (const tag of COUNTER[tk]) {
      const f = path.join(dist, 'api', tk, tag, 'index.html');
      assert.ok(fs.existsSync(f), `${f} missing`);
      const html = fs.readFileSync(f, 'utf8');
      assert.match(html, /id="properties"/);
      assert.match(html, /id="events"/);
    }
  }
  assert.ok(fs.existsSync(path.join(dist, 'pagefind')), 'search index missing');
});
