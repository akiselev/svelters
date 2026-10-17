import { pathToFileURL } from 'node:url';
import { HIDDEN_MODULES } from '../vite.js';

export async function resolve(specifier, context, next) {
  if (Object.hasOwn(HIDDEN_MODULES, specifier)) return { url: pathToFileURL(HIDDEN_MODULES[specifier]).href, shortCircuit: true };
  return next(specifier, context);
}
