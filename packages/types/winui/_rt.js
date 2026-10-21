// Helpers of the generated `svelters:winui/<namespace>` modules (N4), on top of ../src/core.js. Hand-written; plain
// ES2022, no top-level side effects.
//
// WinRT overloads differ by argument count and the backend picks one at run time. The host pads omitted optional
// arguments with null up to the widest overload, and recognizes a trailing `{ signal }` only right after them, so an
// async call is normalized here: split off the options, then pad to the widest overload.
import { callAsync, callStaticAsync, wrapObject } from '../src/core.js';

function isPlain(x) {
  if (x === null || typeof x !== 'object' || Array.isArray(x)) return false;
  const p = Object.getPrototypeOf(x);
  return p === Object.prototype || p === null;
}

/** `[args, options]`: a trailing plain object holding only `signal` is the options of an async method. */
export function splitOptions(args, arities) {
  const max = Math.max(...arities);
  const last = args[args.length - 1];
  const opts = args.length > 0 && isPlain(last) && Object.keys(last).every((k) => k === 'signal')
    && (args.length === max + 1 || arities.includes(args.length - 1));
  const a = opts ? args.slice(0, -1) : args.slice();
  while (a.length < max) a.push(null);
  return [a, opts ? last : undefined];
}

/** An async instance method: a Promise of its result (shaped by `shape`); `{ signal }` cancels. */
export function instanceAsync(self, method, args, arities, shape) {
  const [a, o] = splitOptions(args, arities);
  return callAsync(self, method, a, o, shape);
}

/** An async static. */
export function staticAsync(native, method, args, arities, shape) {
  const [a, o] = splitOptions(args, arities);
  return callStaticAsync(native, method, a, o, shape);
}

/**
 * Whether constructor arguments are a props object of this class (`map`: prop name -> wire name): none, or one plain
 * object whose keys are all props. A record argument (`new SolidColorBrush({ a: 255, r: 0, g: 0, b: 0 })`) is not.
 */
export function isPropsFor(args, map) {
  if (args.length === 0) return true;
  if (args.length !== 1 || !isPlain(args[0])) return false;
  const keys = Object.keys(args[0]);
  return keys.every((k) => k in map || k.replace(/-([a-z])/g, (_, c) => c.toUpperCase()) in map);
}

/** An `object` result: a native object (a node with a handle) wraps as `Root` unless it already has a wrapper. */
export function wrapAny(Root, r) {
  if (r !== null && typeof r === 'object' && typeof r.svlHandle === 'number' && !(r instanceof Root)) return wrapObject(Root, r);
  return r;
}

/** A map result with each value converted. */
export function mapValues(m, fn) {
  if (m === null || m === undefined) return m;
  const out = {};
  for (const k of Object.keys(m)) out[k] = fn(m[k]);
  return out;
}
