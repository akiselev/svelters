// The part of Svelte's tests/suite.ts that `_config.js` files import.
export function assert_ok(value) {
  if (!value) throw new Error(`Expected truthy value, got ${value}`);
}
