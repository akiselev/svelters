#!/usr/bin/env node
// Checks a widget gallery fixture (fixtures/gallery-<toolkit>):
//  1. every page in src/pages/ starts with a `<!-- kinds: A B -->` header (read by tools/metadata/coverage.py);
//  2. every page has a screenshot test line in examples/counter/tests/gallery.rs;
//  3. the bundle opens every page in the TestHost without a JS error (clicking each sidebar button).
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
try {
  await runBundle({ bundlePath: path.join(dir, 'dist/app.js'), script });
} catch (e) {
  fail(`opening the pages in the TestHost threw: ${e?.stack ?? e}`);
}
console.log(`PASS gallery-${tk} (${list.length} pages)`);
