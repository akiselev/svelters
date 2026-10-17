#!/usr/bin/env node
// Checks a widget gallery fixture (fixtures/gallery-<toolkit>):
//  1. every page in src/pages/ starts with a `<!-- kinds: A B -->` header (read by tools/metadata/coverage.py);
//  2. every page has a screenshot test line in examples/counter/tests/gallery.rs;
//  3. the bundle opens every page in the TestHost without a JS error (clicking each sidebar button);
//  4. the op log holds every line of `style-setters.expected` (the setter section of the styling
//     page: rules and inline setter properties written with setProp / setChildAttr), with the
//     TestHost answering the toolkit's style dialect.
// Usage: node fixtures/gallery-common/check-pages.mjs <fixtureDir>
import fs from 'node:fs';
import path from 'node:path';
import { runBundle } from '../../packages/testhost/src/index.js';
import { pages, SIDEBAR_FIRST_HANDLE } from './pages.mjs';

const dir = path.resolve(process.argv[2] ?? '.');
const tk = path.basename(dir).replace(/^gallery-/, '');
const list = pages(dir);
const fail = (m) => {
  console.error(`FAIL gallery-${tk}: ${m}`);
  process.exit(1);
};
if (!list.length) fail('no pages in src/pages');
const tests = fs.readFileSync(path.join(dir, '../../examples/counter/tests/gallery.rs'), 'utf8');
for (const p of list) {
  const src = fs.readFileSync(path.join(dir, 'src/pages', `${p}.svelte`), 'utf8');
  if (!/<!--\s*kinds:\s*[^>]+-->/.test(src)) fail(`${p}.svelte has no <!-- kinds: ... --> header`);
  if (!new RegExp(`gallery_page!\\(\\s*\\w+\\s*,\\s*"${tk}"\\s*,\\s*"${p}"`).test(tests)) fail(`no gallery_page!(..., "${tk}", "${p}", ...) line in examples/counter/tests/gallery.rs`);
}
const script = [{ checkpoint: true }];
list.forEach((_, i) => script.push({ event: [SIDEBAR_FIRST_HANDLE + i, 'clicked', []] }, { checkpoint: true }));
let log;
try {
  log = await runBundle({ bundlePath: path.join(dir, 'dist/app.js'), script, styleDialect: tk === 'qt' ? 'qss' : 'css' });
} catch (e) {
  fail(`opening the pages in the TestHost threw: ${e?.stack ?? e}`);
}
const expected = path.join(dir, 'style-setters.expected');
if (fs.existsSync(expected)) {
  const lines = log.split('\n');
  for (const want of fs.readFileSync(expected, 'utf8').split('\n').filter((l) => l.trim() && !l.startsWith('#'))) {
    const re = new RegExp(`^${want.replace(/[.*+?^${}()|[\]\\]/g, '\\$&').replace(/#N/g, '#\\d+')}$`);
    if (!lines.some((l) => re.test(l))) fail(`style-setters.expected: no op matching "${want}"`);
  }
}
console.log(`PASS gallery-${tk} (${list.length} pages)`);
