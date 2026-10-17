// Node module hooks for running Svelte's runtime samples on the native runtime:
// - the hidden Svelte effects module (same alias as packages/native-runtime/vite.js);
// - `.svelte` files load the output the runner already compiled with svelters-compile
//   (`<dir>/_compiled/<file>.js`, see run.mjs);
// - the imports Svelte's `_config.js` files make into Svelte's test tree (`../../test`,
//   `../../../helpers.js`, `../../../../src/index-client.js`) go to local shims / the installed Svelte.
import fs from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { EFFECTS_SPECIFIER, effectsPath } from '@svelters/native-runtime/vite';

const here = path.dirname(fileURLToPath(import.meta.url));
const svelteClient = path.join(path.dirname(createRequire(import.meta.url).resolve('svelte/internal/client')), '../../index-client.js');
const shim = (f) => ({ url: pathToFileURL(path.join(here, 'shims', f)).href, shortCircuit: true });

export async function resolve(specifier, context, next) {
  if (specifier === EFFECTS_SPECIFIER) return { url: pathToFileURL(effectsPath).href, shortCircuit: true };
  if (/^(\.\.\/)+test(\.js|\.ts)?$/.test(specifier)) return shim('test.mjs');
  if (/^(\.\.\/)+helpers(\.js)?$/.test(specifier)) return shim('helpers.mjs');
  if (/^(\.\.\/)+suite(\.js|\.ts)?$/.test(specifier)) return shim('suite.mjs');
  if (/^(\.\.\/)+src\/index-client(\.js)?$/.test(specifier)) return { url: pathToFileURL(svelteClient).href, shortCircuit: true };
  return next(specifier, context);
}

export async function load(url, context, next) {
  if (url.startsWith('file:') && url.endsWith('.svelte')) {
    const file = fileURLToPath(url);
    const compiled = path.join(path.dirname(file), '_compiled', path.basename(file) + '.js');
    if (!fs.existsSync(compiled)) throw new Error(`${path.basename(file)} was not compiled (no ${compiled})`);
    return { format: 'module', source: fs.readFileSync(compiled, 'utf8'), shortCircuit: true };
  }
  return next(url, context);
}
