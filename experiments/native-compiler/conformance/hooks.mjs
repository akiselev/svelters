// Node module hooks for running Svelte's runtime samples on the native runtime:
// - the hidden Svelte modules, `svelte` itself and `@svelters/native-runtime/kinds` (out/_kinds.js) (same aliases as packages/native-runtime/vite.js:
//   `svelte` is src/svelte.js, Svelte's client API with a native `mount`/`unmount`);
// - `.svelte` and `.svelte.js` files load the output the runner already compiled with svelters-compile
//   (`<dir>/_compiled/<file>.js`, see run.mjs);
// - the imports Svelte's `_config.js` files make into Svelte's test tree (`../../test`,
//   `../../../helpers.js`, `../../../../src/index-client.js`, `vitest`) go to local shims / the installed
//   Svelte.
import fs from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { HIDDEN_MODULES, KINDS_SPECIFIER, svelteFacadePath } from '@svelters/native-runtime/vite';

const here = path.dirname(fileURLToPath(import.meta.url));
const svelteClient = path.join(path.dirname(createRequire(import.meta.url).resolve('svelte/internal/client')), '../../index-client.js');
const shim = (f) => ({ url: pathToFileURL(path.join(here, 'shims', f)).href, shortCircuit: true });

export async function resolve(specifier, context, next) {
  if (Object.hasOwn(HIDDEN_MODULES, specifier)) return { url: pathToFileURL(HIDDEN_MODULES[specifier]).href, shortCircuit: true };
  // as the `svelte` alias of packages/native-runtime/vite.js: native mount/unmount
  if (specifier === 'svelte') return { url: pathToFileURL(svelteFacadePath).href, shortCircuit: true };
  // the dom spec's runtime tables (spreads, <svelte:element>), written by run.mjs
  if (specifier === KINDS_SPECIFIER) return { url: pathToFileURL(path.join(here, 'out', '_kinds.js')).href, shortCircuit: true };
  if (/^(\.\.\/)+test(\.js|\.ts)?$/.test(specifier)) return shim('test.mjs');
  if (/^(\.\.\/)+helpers(\.js)?$/.test(specifier)) return shim('helpers.mjs');
  if (/^(\.\.\/)+suite(\.js|\.ts)?$/.test(specifier)) return shim('suite.mjs');
  if (specifier === 'vitest') return shim('vitest.mjs');
  // `./state.svelte` for `state.svelte.js`, as Vite's extension resolution in Svelte's test setup
  if (/^\.\.?\//.test(specifier) && specifier.endsWith('.svelte') && context.parentURL?.startsWith('file:')) {
    const file = fileURLToPath(new URL(specifier, context.parentURL));
    if (!fs.existsSync(file) && fs.existsSync(`${file}.js`)) return { url: pathToFileURL(`${file}.js`).href, shortCircuit: true };
  }
  if (/^(\.\.\/)+src\/index-client(\.js)?$/.test(specifier)) return { url: pathToFileURL(svelteClient).href, shortCircuit: true };
  return next(specifier, context);
}

export async function load(url, context, next) {
  if (url.startsWith('file:') && (url.endsWith('.svelte') || url.endsWith('.svelte.js'))) {
    const file = fileURLToPath(url);
    const compiled = path.join(path.dirname(file), '_compiled', path.basename(file) + '.js');
    if (!fs.existsSync(compiled)) throw new Error(`${path.basename(file)} was not compiled (no ${compiled})`);
    return { format: 'module', source: fs.readFileSync(compiled, 'utf8'), shortCircuit: true };
  }
  return next(url, context);
}
