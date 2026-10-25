// DZ0b2: in dev, the Vite plugin scopes every egui sheet with the svelters compiler (`--css`) and
// passes `file` and `map` to `registerStyles`, as the Rust loader does, so the engine's matched
// rules point into the component's `<style>`. Needs svelters-compile (CI builds
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
const PLAIN = '<box class="a" />\n\n<style>\n  .a { gap: 2px; }\n  .b { padding: 1px; }\n</style>\n';
const plugin = (dev) => svelteNative({ backend: 'egui', dev });
const ctx = { warn() {}, error(e) { throw new Error(e.message); } };
const FILE = path.resolve('src/StyleMap.svelte');

/** The `registerStyles(id, sheets)` sheets object the transform emitted. */
function sheets(code) {
  const m = /\$\$registerStyles\(("(?:[^"\\]|\\.)*"), (\{.*\})\);/.exec(code);
  assert.ok(m, code);
  return JSON.parse(m[2]);
}

for (const [name, src] of [['with @apply', SRC], ['without @apply', PLAIN]]) {
  test(`dev egui sheets carry file and map (${name})`, { skip: !haveCompiler && 'no svelters-compile' }, () => {
    checkMapped(sheets(plugin(true).transform.call(ctx, src, FILE).code), src);
  });
}

function checkMapped(s, src) {
  assert.equal(s.file, FILE);
  assert.match(s.map, /^\d+,\d+,\d+(;\d+,\d+,\d+)*$/);
  const style = src.slice(src.indexOf('<style>') + '<style>'.length);
  for (const run of s.map.split(';')) {
    const [o, at, len] = run.split(',').map(Number);
    assert.equal(s.css.slice(o, o + len), style.slice(at, at + len), `run ${run}: untouched style text`);
  }
  const runs = s.map.split(';').map((r) => r.split(',').map(Number));
  assert.ok(runs.some(([, at, len]) => style.slice(at, at + len).includes('padding: 1px')), s.map);
}

for (const [name, src] of [['with @apply', SRC], ['without @apply', PLAIN]]) {
  test(`release egui sheets carry no map (${name})`, { skip: !haveCompiler && 'no svelters-compile' }, () => {
    const s = sheets(plugin(false).transform.call(ctx, src, FILE).code);
    assert.ok(s.css.includes('padding: 1px'), s.css);
    assert.equal(s.file, undefined);
    assert.equal(s.map, undefined);
  });
}
