// Compiles every case for every backend with the Rust compiler (svelteNative plugin) into
// out/<backend>/<name>/dist/app.js. A case whose component is rejected on a backend (diagnostics) is
// recorded in out/<backend>/<name>/diagnostics.txt instead; run.mjs checks those expectations.
import fs from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { build } from 'vite';
import { nativeConfig, svelteNative } from '@svelters/native-runtime/vite';
import { BACKENDS, cases } from './cases.mjs';

const here = import.meta.dirname;
const only = process.argv.slice(2);
// Cases with extra scripts are compared live against the JS pipeline's bundle: build it first.
for (const c of cases()) {
  if ((only.length && !only.includes(c.name)) || !c.extraScripts?.length) continue;
  execFileSync('pnpm', ['run', '--silent', 'build'], { cwd: path.dirname(c.jsExpected), stdio: ['ignore', 'ignore', 'inherit'] });
  console.log(`built JS pipeline ${c.name}`);
}
for (const backend of BACKENDS) {
  for (const c of cases()) {
    if (only.length && !only.includes(c.name)) continue;
    const dir = path.join(here, 'out', backend, c.name);
    fs.rmSync(dir, { recursive: true, force: true });
    fs.mkdirSync(dir, { recursive: true });
    fs.writeFileSync(path.join(dir, 'main.js'), `import * as $n from '@svelters/native-runtime';\nimport App from ${JSON.stringify(c.source)};\n\n$n.mount(App, { props: {} });\n`);
    try {
      await build(nativeConfig(dir, { entry: 'main.js', outDir: 'dist', plugins: [svelteNative({ backend })] }));
      console.log(`built ${backend}/${c.name}`);
    } catch (e) {
      // keep only the diagnostics (no stack, repo-relative paths) so expectations are stable
      const repo = path.resolve(here, '../../..') + '/';
      const lines = String(e.message)
        .split('\n')
        .filter((l) => l.trim() && !/^\s+at /.test(l) && !l.startsWith('Build failed'))
        .map((l) => l.split(repo).join('').replace(/^RolldownError: /, ''));
      fs.writeFileSync(path.join(dir, 'diagnostics.txt'), lines.join('\n') + '\n');
      console.log(`rejected ${backend}/${c.name}: ${String(e.message).split('\n')[0]}`);
    }
  }
}
