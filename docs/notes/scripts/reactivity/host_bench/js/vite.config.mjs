// Bundles svelte_bench.src.js with the vendored Svelte runtime into one ES module for the host.
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const repo = path.resolve(here, '../../../../../..');
const svelte = path.join(repo, 'packages/build/node_modules/svelte');

export default {
  root: here,
  logLevel: 'warn',
  resolve: { conditions: ['browser'], alias: [
      { find: /^svelte\/internal\/client$/, replacement: path.join(svelte, 'src/internal/client/index.js') },
      { find: /^svelte\/reactivity$/, replacement: path.join(svelte, 'src/reactivity/index-client.js') },
    ] },
  define: { 'process.env.NODE_ENV': '"production"' },
  build: {
    target: 'es2022',
    minify: false,
    outDir: path.join(here, 'dist'),
    emptyOutDir: true,
    lib: { entry: path.join(here, 'svelte_bench.src.js'), formats: ['es'], fileName: () => 'svelte_bench.js' },
  },
};
