import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { nativeConfig, svelteNative } from '@svelters/native-runtime/vite';

const here = path.dirname(fileURLToPath(import.meta.url));

/**
 * Vite config for one app (a fixture, a gallery, a bench): `<root>/src/App.svelte` compiled with the
 * svelters compiler (`svelters-compile`, through @svelters/native-runtime/vite) for one toolkit,
 * bundled with the native runtime into `<root>/dist/app.js`, a single self-contained ES module with
 * no imports and no Node built-ins, plus its source map.
 *
 * The toolkit comes from the app's package.json, `"svelters": { "toolkit": "qt" }` (default `gtk`;
 * also `appkit`, `winui`), or `options.toolkit`. `options.spec` (or `"svelters": { "spec": "..." }`,
 * relative to the app) compiles against another semantics file than the toolkit's builtin one.
 * `options.dev` keeps `$inspect` and labels signals (Svelte's `dev`). `options.specFragments` (or
 * `"svelters": { "specFragments": [...] }`, relative to the app) merges spec fragments into the
 * toolkit's spec: an egui app lists the `kinds.json` its binary dumps (RA1 provider kinds), e.g.
 * `svelteConfig(dir, { specFragments: ['.svelters/app/kinds.json'] })`.
 */
export function svelteConfig(root, options = {}) {
  let meta = {};
  try {
    meta = JSON.parse(fs.readFileSync(path.join(root, 'package.json'), 'utf8')).svelters ?? {};
  } catch {
    // no package.json: the defaults
  }
  const backend = options.toolkit ?? meta.toolkit ?? 'gtk';
  const spec = options.spec ?? (meta.spec ? path.resolve(root, meta.spec) : undefined);
  const specFragments = (options.specFragments ?? meta.specFragments ?? []).map((f) => path.resolve(root, f));
  const config = nativeConfig(root, {
    entry: path.relative(root, path.join(here, 'entry.js')),
    outDir: 'dist',
    plugins: [svelteNative({ backend, spec, specFragments, dev: options.dev ?? false })],
  });
  config.resolve.alias.unshift({ find: /^@app$/, replacement: path.join(root, 'src/App.svelte') });
  return config;
}
