// Attribute coercion driven by the backend's type descriptors.
//
// Svelte passes static attributes as strings (`spacing="8"`, a bare `disabled` as ""). Whether
// "8" means the number 8 or the text "8" depends on the property, so the answer comes from the
// host's descriptors (`host.propType`), not from a list in the renderer. Anything the descriptors
// do not call for is left alone; the host validates what it receives and reports a precise error.
import { propType } from './hostops.js';

const INTEGER = /^\s*[+-]?\d+\s*$/;
const NUMBER = /^\s*[+-]?(\d+\.?\d*|\.\d+)([eE][+-]?\d+)?\s*$/;

/** `value` converted for a prop of coarse type `type` (only strings are ever converted). */
export function coerceByType(type, value) {
  if (typeof value !== 'string') return value;
  switch (type) {
    case 'integer':
      return INTEGER.test(value) ? Number(value) : value;
    case 'number':
      return NUMBER.test(value) ? Number(value) : value;
    case 'boolean':
      if (value === '' || value === 'true') return true;
      if (value === 'false') return false;
      return value;
    default:
      return value;
  }
}

/** The value to send for attribute `name` on an element of kind `tag`. */
export function coerceAttribute(host, tag, name, value) {
  if (typeof value === 'function') throw new TypeError(`<${tag}>.${name}: functions are not valid host prop values`);
  if (typeof value !== 'string') return value;
  const type = propType(host, tag, name);
  return type ? coerceByType(type, value) : value;
}
