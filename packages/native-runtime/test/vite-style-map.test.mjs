// DZ0b2: in dev, the Vite plugin's egui sheets scoped by the svelters compiler (`--css`, the
// `@apply` path) pass `file` and `map` to `registerStyles`, as the Rust loader does, so the
// engine's matched rules point into the component's `<style>`. Needs svelters-compile (CI builds
// it before `pnpm test`); skipped without it.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import path from 'node:path';
import { compilerPath, svelteNative } from '../vite.js';

let haveCompiler = true;
try {
  compilerPath();
} catch {
  haveCompiler = false;
}

const SRC = '<box class="a" />\n\n<style>\n  .a { @apply bg-surface; gap: 2px; }\n  .b { padding: 1px; }\n</style>\n';
const FILE = path.resolve('src/StyleMap.svelte');

/** The `registerStyles(id, sheets)` sheets object the transform emitted. */
function sheets(code) {
  const m = /\$\$registerStyles\(("(?:[^"\\]|\\.)*"), (\{.*\})\);/.exec(code);
  assert.ok(m, code);
  return JSON.parse(m[2]);
}

test('dev egui sheets from the svelters compiler carry file and map', { skip: !haveCompiler && 'no svelters-compile' }, () => {
  const out = svelteNative({ backend: 'egui', dev: true }).transform.call({ warn() {}, error(e) { throw new Error(e.message); } }, SRC, FILE);
  const s = sheets(out.code);
  assert.equal(s.file, FILE);
  assert.match(s.map, /^\d+,\d+,\d+(;\d+,\d+,\d+)*$/);
  const style = SRC.slice(SRC.indexOf('<style>') + '<style>'.length);
  for (const run of s.map.split(';')) {
    const [o, src, len] = run.split(',').map(Number);
    assert.equal(s.css.slice(o, o + len), style.slice(src, src + len), `run ${run}: untouched style text`);
  }
  const runs = s.map.split(';').map((r) => r.split(',').map(Number));
  assert.ok(runs.some(([, src, len]) => style.slice(src, src + len).includes('padding: 1px')), s.map);
});

test('release egui sheets carry no map', { skip: !haveCompiler && 'no svelters-compile' }, () => {
  const out = svelteNative({ backend: 'egui', dev: false }).transform.call({ warn() {}, error(e) { throw new Error(e.message); } }, SRC, FILE);
  const s = sheets(out.code);
  assert.equal(s.file, undefined);
  assert.equal(s.map, undefined);
});
