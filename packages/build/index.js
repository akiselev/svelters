import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { svelte } from '@sveltejs/vite-plugin-svelte';
import { stylePlugin } from './style-plugin.js';

const here = path.dirname(fileURLToPath(import.meta.url));

/**
 * Vite config for one fixture/app. Output: dist/app.js, a single self-contained ES module
 * (Svelte runtime + renderer or DOM shim + app) with no imports and no Node built-ins.
 *
 * backend 'renderer' (default): Svelte custom renderer (vendored fork), `custom-renderer` export
 * condition, components compiled with experimental.customRenderer -> @svelters/renderer.
 * backend 'shim': stock-style DOM output (fragments: 'tree') on the @svelters/dom shim.
 * Select with SVELTERS_BACKEND=shim|renderer.
 *
 * Renderer builds take component <style> blocks out of the compile as data (`style-plugin.js`):
 * `options.style` = { strict, verbose, tokens } (docs/site: styling guide).
 */
export function svelteConfig(root, backend = process.env.SVELTERS_BACKEND ?? 'renderer', options = {}) {
  if (backend !== 'renderer' && backend !== 'shim') throw new Error(`unknown SVELTERS_BACKEND ${backend}`);
  const renderer = backend === 'renderer';
  const compilerOptions = renderer
    ? { experimental: { customRenderer: fileURLToPath(import.meta.resolve('@svelters/renderer')) } }
    : { fragments: 'tree' };
  const styles = renderer ? stylePlugin(options.style) : null;
  return {
    root,
    resolve: {
      conditions: renderer ? ['custom-renderer', 'browser'] : ['browser'],
      alias: { '@app': path.join(root, 'src/App.svelte') },
    },
    plugins: [
      ...(styles ? [styles.plugin] : []),
      svelte({
        compilerOptions,
        ...(styles ? { preprocess: [styles.preprocessor] } : {}),
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
