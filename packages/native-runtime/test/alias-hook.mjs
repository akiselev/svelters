import { pathToFileURL } from 'node:url';
import { EFFECTS_SPECIFIER, effectsPath } from '../vite.js';

export async function resolve(specifier, context, next) {
  if (specifier === EFFECTS_SPECIFIER) return { url: pathToFileURL(effectsPath).href, shortCircuit: true };
  return next(specifier, context);
}
