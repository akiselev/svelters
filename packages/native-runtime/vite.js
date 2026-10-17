// Vite config helper for bundles built on @svelters/native-runtime: one self-contained ES module
// (dist/app.js) like packages/build, plus the alias that exposes Svelte's internal effects module.
import path from 'node:path';
import { createRequire } from 'node:module';

/** The specifier src/index.js imports for block/branch/destroy_effect/component_root. */
export const EFFECTS_SPECIFIER = 'svelte/internal/client/reactivity/effects';

/** Absolute path of Svelte's effects module (hidden by the package's exports map). */
export const effectsPath = path.join(
  path.dirname(createRequire(import.meta.url).resolve('svelte/internal/client')),
  'reactivity/effects.js',
);

/**
 * Vite config for one app: `entry` (relative to `root`) -> `<root>/<outDir>/app.js`, es2022, not
 * minified, with a source map, resolved with the same conditions as packages/build.
 */
export function nativeConfig(root, { entry = 'main.js', outDir = 'dist' } = {}) {
  return {
    root,
    configFile: false,
    logLevel: 'warn',
    resolve: {
      conditions: ['custom-renderer', 'browser'],
      alias: [{ find: /^svelte\/internal\/client\/reactivity\/effects(\.js)?$/, replacement: effectsPath }],
    },
    build: {
      target: 'es2022',
      minify: false,
      sourcemap: true,
      cssCodeSplit: false,
      modulePreload: false,
      emptyOutDir: true,
      outDir: path.resolve(root, outDir),
      lib: { entry: path.resolve(root, entry), formats: ['es'], fileName: () => 'app.js' },
    },
  };
}
