// Probe (runtime research): resolve every `svelte` / `svelte/...` import to STOCK svelte@5.57.2
// instead of the vendored fork, including the hidden modules native-runtime aliases.
// Usage (from packages/native-runtime):
//   node --import ../../docs/notes/scripts/runtime-research/stock-svelte-register.mjs --test test/*.test.mjs
import path from 'node:path';
import { pathToFileURL, fileURLToPath } from 'node:url';

const repo = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../../..');
const stock = path.join(repo, 'node_modules/.pnpm/svelte@5.57.2/node_modules/svelte');
const client = path.join(stock, 'src/internal/client');
const hidden = {
  'svelte/internal/client/reactivity/effects': path.join(client, 'reactivity/effects.js'),
  'svelte/internal/client/dom/task': path.join(client, 'dom/task.js'),
  'svelte/internal/client/context': path.join(client, 'context.js'),
};
const parent = pathToFileURL(path.join(stock, '..', 'probe.js')).href; // resolve from beside stock svelte

export async function resolve(specifier, context, next) {
  if (Object.hasOwn(hidden, specifier)) return { url: pathToFileURL(hidden[specifier]).href, shortCircuit: true };
  if (specifier === 'svelte' || specifier.startsWith('svelte/')) {
    const r = await next(specifier, { ...context, parentURL: parent });
    if (!r.url.includes('svelte@5.57.2/')) throw new Error(`not stock: ${r.url}`);
    return r;
  }
  return next(specifier, context);
}
