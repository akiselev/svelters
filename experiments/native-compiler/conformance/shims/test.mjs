// Stand-in for Svelte's tests/runtime-runes/test.ts as `_config.js` files import it
// (`import { test, ok } from '../../test'`): `test` is the identity, `ok` as upstream.
export const test = (config) => config;

export function ok(value) {
  if (!value) throw new Error(`Expected truthy value, got ${value}`);
}
