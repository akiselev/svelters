// Bundles tracker_bench.src.js with @svelters/native-runtime's tracker (src/observe.js) and Svelte's
// client runtime into one ES module for the host (R1 "batched" scenario).
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { HIDDEN_MODULES } from '../../../../../../packages/native-runtime/vite.js';

const here = path.dirname(fileURLToPath(import.meta.url));
const repo = path.resolve(here, '../../../../../..');
const svelte = path.dirname(HIDDEN_MODULES['svelte/internal/client/reactivity/effects']).replace(/\/reactivity$/, '');

export default {
  root: here,
  logLevel: 'warn',
  resolve: {
    conditions: ['browser'],
    alias: [
      ...Object.entries(HIDDEN_MODULES).map(([find, replacement]) => ({ find, replacement })),
      { find: /^svelte\/internal\/client$/, replacement: path.join(svelte, 'index.js') },
      { find: /^@svelters\/types\/core$/, replacement: path.join(repo, 'packages/types/src/core.js') },
    ],
  },
  define: { 'process.env.NODE_ENV': '"production"' },
  build: {
    target: 'es2022',
    minify: false,
    outDir: path.join(here, 'dist'),
    emptyOutDir: false,
    lib: { entry: path.join(here, 'tracker_bench.src.js'), formats: ['es'], fileName: () => 'tracker_bench.js' },
  },
};
