// Compiles every case for each of its targets (cases.mjs TARGETS) with the Rust compiler
// (svelteNative plugin) into out/<target>/<name>/dist/app.js. A case whose component is rejected
// on a target (diagnostics) is recorded in out/<target>/<name>/diagnostics.txt instead; run.mjs
// checks those expectations.
import fs from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { build } from 'vite';
import { nativeConfig, svelteNative } from '@svelters/native-runtime/vite';
import { TARGETS, cases, sourceOf, targetsOf } from './cases.mjs';

const here = import.meta.dirname;
const repo = path.resolve(here, '../../..');
const only = process.argv.slice(2);

/** Deep merge of plain objects (`b` wins). */
function merge(a, b) {
  if (a === null || b === null || typeof a !== 'object' || typeof b !== 'object' || Array.isArray(a) || Array.isArray(b)) return b;
  const out = { ...a };
  for (const [k, v] of Object.entries(b)) out[k] = k in a ? merge(a[k], v) : v;
  return out;
}

// specs derived from a real one plus an overlay (the testhost target)
for (const t of TARGETS) {
  if (!t.overlay) continue;
  const base = JSON.parse(fs.readFileSync(path.join(repo, `metadata/semantics/${t.backend}.json`), 'utf8'));
  fs.mkdirSync(path.dirname(t.spec), { recursive: true });
  fs.writeFileSync(t.spec, JSON.stringify(merge(base, JSON.parse(fs.readFileSync(t.overlay, 'utf8')))));
}

// Cases with extra scripts are compared live against the JS pipeline's bundle: build it first.
for (const c of cases()) {
  if ((only.length && !only.includes(c.name)) || !c.extraScripts?.length) continue;
  execFileSync('pnpm', ['run', '--silent', 'build'], { cwd: path.dirname(c.jsExpected), stdio: ['ignore', 'ignore', 'inherit'] });
  console.log(`built JS pipeline ${c.name}`);
}
for (const c of cases()) {
  if (only.length && !only.includes(c.name)) continue;
  for (const t of targetsOf(c)) {
    const dir = path.join(here, 'out', t.name, c.name);
    fs.rmSync(dir, { recursive: true, force: true });
    fs.mkdirSync(dir, { recursive: true });
    fs.writeFileSync(path.join(dir, 'main.js'), `import * as $n from '@svelters/native-runtime';\nimport App from ${JSON.stringify(sourceOf(c, t.name))};\n\n$n.mount(App, { props: {} });\n`);
    try {
      await build(nativeConfig(dir, { entry: 'main.js', outDir: 'dist', plugins: [svelteNative({ backend: t.backend, spec: t.spec })] }));
      console.log(`built ${t.name}/${c.name}`);
    } catch (e) {
      // keep only the diagnostics (no stack, repo-relative paths) so expectations are stable
      const lines = String(e.message)
        .replace(/\x1b\[[0-9;]*m/g, '') // vite colours its messages when CI is set
        .split('\n')
        .filter((l) => l.trim() && !/^\s+at /.test(l) && !l.startsWith('Build failed'))
        .map((l) => l.split(repo + '/').join('').replace(/^RolldownError: /, ''));
      fs.writeFileSync(path.join(dir, 'diagnostics.txt'), lines.join('\n') + '\n');
      console.log(`rejected ${t.name}/${c.name}: ${String(e.message).split('\n')[0]}`);
    }
  }
}
