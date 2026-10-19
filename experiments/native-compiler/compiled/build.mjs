// Compiles every case for each of its targets (cases.mjs TARGETS) with the Rust compiler
// (svelteNative plugin) into out/<target>/<name>/dist/app.js. A case whose component is rejected
// on a target (diagnostics) is recorded in out/<target>/<name>/diagnostics.txt instead; run.mjs
// checks those expectations.
import fs from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { build } from 'vite';
import { nativeConfig, svelteNative } from '@svelters/native-runtime/vite';
import { cases, sourceOf, targetsOf } from './cases.mjs';

const here = import.meta.dirname;
const repo = path.resolve(here, '../../..');
const only = process.argv.slice(2);

// Cases with extra scripts are compared live against the fixture's own bundle: build it first.
for (const c of cases()) {
  if ((only.length && !only.includes(c.name)) || !c.extraScripts?.length) continue;
  execFileSync('pnpm', ['run', '--silent', 'build'], { cwd: path.dirname(c.jsExpected), stdio: ['ignore', 'ignore', 'inherit'] });
  console.log(`built fixture ${c.name}`);
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
