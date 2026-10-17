// The widget galleries (fixtures/gallery-{gtk,qt}) through the Rust compiler, each for its own
// backend:
//   1. every page (src/**/*.svelte) compiles (diagnostics listed with --list);
//   2. --run: the whole gallery is bundled (out/<backend>/gallery/dist/app.js) and run on the
//      Node TestHost with the gallery script (fixtures/gallery-common: open every page through its
//      sidebar button), and the native state is compared with the JS pipeline's bundle
//      (fixtures/gallery-<backend>/dist/app.js, `pnpm -r --filter "./fixtures/*" build`) at every
//      boundary (tools/oplog-compare.mjs).
//   node galleries.mjs [--list] [--run] [--expect N]   (--expect: fail when fewer than N pages compile)
import fs from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { build } from 'vite';
import { compilerPath, nativeConfig, svelteNative } from '@svelters/native-runtime/vite';
import { runBundle } from '@svelters/testhost';
import { compare } from '../../../tools/oplog-compare.mjs';
import { pages as galleryPages, SIDEBAR_FIRST_HANDLE } from '../../../fixtures/gallery-common/pages.mjs';

const here = import.meta.dirname;
const repo = path.resolve(here, '../../..');
const list = process.argv.includes('--list');
const runIt = process.argv.includes('--run');
const ei = process.argv.indexOf('--expect');
const expect = ei >= 0 ? Number(process.argv[ei + 1]) : null;
const bin = compilerPath();
let failed = false;

function files(dir) {
  const out = [];
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, e.name);
    if (e.isDirectory()) out.push(...files(p));
    else if (e.name.endsWith('.svelte')) out.push(p);
  }
  return out.sort();
}

let ok = 0;
let total = 0;
const codes = new Map();
for (const backend of ['gtk', 'qt']) {
  for (const file of files(path.join(repo, `fixtures/gallery-${backend}/src`))) {
    total++;
    let out;
    try {
      out = execFileSync(bin, ['--backend', backend, '--json', '--filename', file, file], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] });
    } catch (e) {
      out = e.stdout;
    }
    const res = JSON.parse(out);
    const errors = res.diagnostics.filter((d) => d.severity === 'error');
    const rel = path.relative(repo, file);
    if (!errors.length) {
      ok++;
      console.log(`ok   ${backend} ${rel}`);
      continue;
    }
    console.log(`FAIL ${backend} ${rel}: ${errors.length} error(s): ${[...new Set(errors.map((d) => d.code))].join(', ')}`);
    for (const d of errors) {
      codes.set(d.code, (codes.get(d.code) ?? 0) + 1);
      if (list) console.log(`       ${d.line}:${d.column} ${d.code}: ${d.message}${d.help ? ` (help: ${d.help})` : ''}`);
    }
  }
}
console.log(`\n${ok}/${total} gallery pages compile`);
if (codes.size) console.log(`errors by code: ${[...codes].sort((a, b) => b[1] - a[1]).map(([c, n]) => `${c} ${n}`).join(', ')}`);
if (expect != null && ok < expect) failed = true;

if (runIt) {
  for (const backend of ['gtk', 'qt']) {
    const fixture = path.join(repo, `fixtures/gallery-${backend}`);
    const dir = path.join(here, 'out', backend, 'gallery');
    fs.rmSync(dir, { recursive: true, force: true });
    fs.mkdirSync(dir, { recursive: true });
    fs.writeFileSync(path.join(dir, 'main.js'), `import * as $n from '@svelters/native-runtime';\nimport App from ${JSON.stringify(path.join(fixture, 'src/App.svelte'))};\n\n$n.mount(App, { props: {} });\n`);
    const tag = `gallery-${backend}`;
    try {
      await build(nativeConfig(dir, { entry: 'main.js', outDir: 'dist', plugins: [svelteNative({ backend })] }));
    } catch (e) {
      failed = true;
      console.log(`FAIL ${tag}: did not build: ${String(e.message).split('\n')[0]}`);
      continue;
    }
    const script = [{ checkpoint: true }];
    galleryPages(fixture).forEach((_, i) => script.push({ event: [SIDEBAR_FIRST_HANDLE + i, 'clicked', []] }, { checkpoint: true }));
    let ours;
    let theirs;
    try {
      ours = await runBundle({ bundlePath: path.join(dir, 'dist/app.js'), script });
      fs.writeFileSync(path.join(dir, 'actual.oplog'), ours);
    } catch (e) {
      failed = true;
      console.log(`FAIL ${tag}: compiled bundle threw on the TestHost: ${e?.stack ?? e}`);
      continue;
    }
    const js = path.join(fixture, 'dist/app.js');
    if (!fs.existsSync(js)) {
      console.log(`SKIP ${tag} comparison: ${path.relative(repo, js)} not built`);
      continue;
    }
    theirs = await runBundle({ bundlePath: js, script });
    fs.writeFileSync(path.join(dir, 'js.oplog'), theirs);
    const r = compare(theirs, ours, { defaults: { label: '' }, camelProps: true });
    if (!r.equal) failed = true;
    console.log(`${r.equal ? 'PASS' : 'FAIL'} ${tag}: ${ours === theirs ? 'op log identical to' : r.equal ? 'state-equal with' : 'STATE DIFFERS from'} JS pipeline (${r.boundaries} boundaries, ops ${r.opsB} / ${r.opsA})`);
    for (const p of r.problems.slice(0, 5)) console.log(`  ${p}`);
  }
}
process.exit(failed ? 1 : 0);
