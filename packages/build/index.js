import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { svelte } from '@sveltejs/vite-plugin-svelte';

const here = path.dirname(fileURLToPath(import.meta.url));

/**
 * Vite config for one fixture/app. Output: dist/app.js, a single self-contained ES module
 * (Svelte runtime + renderer or DOM shim + app) with no imports and no Node built-ins.
 *
 * backend 'renderer' (default): Svelte custom renderer (vendored fork), `custom-renderer` export
 * condition, components compiled with experimental.customRenderer -> @svelters/renderer.
 * backend 'shim': stock-style DOM output (fragments: 'tree') on the @svelters/dom shim.
 * Select with SVELTERS_BACKEND=shim|renderer.
 */
export function svelteConfig(root, backend = process.env.SVELTERS_BACKEND ?? 'renderer') {
  if (backend !== 'renderer' && backend !== 'shim') throw new Error(`unknown SVELTERS_BACKEND ${backend}`);
  const renderer = backend === 'renderer';
  const compilerOptions = renderer
    ? { experimental: { customRenderer: fileURLToPath(import.meta.resolve('@svelters/renderer')) } }
    : { fragments: 'tree' };
  return {
    root,
    resolve: {
      conditions: renderer ? ['custom-renderer', 'browser'] : ['browser'],
      alias: { '@app': path.join(root, 'src/App.svelte') },
    },
    plugins: [
      svelte({
        compilerOptions,
        onwarn(warning, handler) {
          if (warning.code.startsWith('a11y_')) return;
          handler(warning);
        },
      }),
    ],
    build: {
      target: 'es2022',
      minify: false,
      sourcemap: true,
      cssCodeSplit: false,
      modulePreload: false,
      emptyOutDir: true,
      lib: { entry: path.join(here, `entry-${backend}.js`), formats: ['es'], fileName: () => 'app.js' },
    },
  };
}
