// Compiles every case for every backend with the Rust compiler (svelteNative plugin) into
// out/<backend>/<name>/app.js. A case whose component is rejected on a backend (diagnostics) is
// recorded in out/<backend>/<name>/diagnostics.txt instead; run.mjs checks those expectations.
import fs from 'node:fs';
import path from 'node:path';
import { build } from 'vite';
import { nativeConfig, svelteNative } from '@svelters/native-runtime/vite';
import { BACKENDS, cases } from './cases.mjs';

const here = import.meta.dirname;
const only = process.argv.slice(2);
for (const backend of BACKENDS) {
  for (const c of cases()) {
    if (only.length && !only.includes(c.name)) continue;
    const dir = path.join(here, 'out', backend, c.name);
    fs.rmSync(dir, { recursive: true, force: true });
    fs.mkdirSync(dir, { recursive: true });
    fs.writeFileSync(path.join(dir, 'main.js'), `import * as $n from '@svelters/native-runtime';\nimport App from ${JSON.stringify(c.source)};\n\n$n.mount(App, { props: {} });\n`);
    try {
      await build({ ...nativeConfig(dir, { entry: 'main.js', outDir: '.', plugins: [svelteNative({ backend })] }), build: { ...nativeConfig(dir, { entry: 'main.js', outDir: '.' }).build, emptyOutDir: false } });
      console.log(`built ${backend}/${c.name}`);
    } catch (e) {
      fs.writeFileSync(path.join(dir, 'diagnostics.txt'), String(e.message) + '\n');
      console.log(`rejected ${backend}/${c.name}: ${String(e.message).split('\n')[0]}`);
    }
  }
}
