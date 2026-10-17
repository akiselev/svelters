// Bundles the Svelte adapter (svelte-entry.mjs) into one ES module for qjs, with the production
// conditions native bundles use, and reports sizes. Run from the repo root:
//   node docs/notes/scripts/runtime-research/build-bench.mjs <outdir>
import { build } from '../../../../node_modules/.pnpm/esbuild@0.28.2/node_modules/esbuild/lib/main.js';
import path from 'node:path';
const here = import.meta.dirname;
const out = process.argv[2];
const stock = path.resolve(here, '../../../../node_modules/.pnpm/svelte@5.57.2/node_modules');
for (const [entry, file, minify] of [['svelte-entry.mjs', 'svelte-signals.mjs', false], ['svelte-entry.mjs', 'svelte-signals.min.mjs', true], ['mini-signals.mjs', 'mini-signals.min.mjs', true]]) {
  const r = await build({
    entryPoints: [path.join(here, entry)], bundle: true, format: 'esm', target: 'es2022', minify,
    conditions: ['custom-renderer', 'browser', 'production'], nodePaths: [stock],
    outfile: path.join(out, file), metafile: true, logLevel: 'warning',
    define: { 'process.env.NODE_ENV': '"production"' },
  });
  const bytes = Object.values(r.metafile.outputs)[0].bytes;
  console.log(`${file}: ${bytes} bytes, ${Object.keys(r.metafile.inputs).length} input modules`);
}
