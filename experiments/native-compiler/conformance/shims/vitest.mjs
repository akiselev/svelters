// The part of vitest that Svelte's `_config.js` files import (`vi.fn`, `expect` with the mock and
// equality matchers they use, chai's `assert`), so those samples run under plain Node.
import { assert, expect as chaiExpect } from 'chai';

export { assert };

function fn(impl = () => undefined) {
  const calls = [];
  const mock = function (...args) {
    calls.push(args);
    return impl.apply(this, args);
  };
  mock.mock = { calls };
  mock.mockClear = () => {
    calls.length = 0;
    return mock;
  };
  mock.mockReset = mock.mockClear;
  mock.mockImplementation = (f) => {
    impl = f;
    return mock;
  };
  mock._isMockFunction = true;
  return mock;
}

export const vi = {
  fn,
  spyOn(object, key) {
    const original = object[key];
    const spy = fn(function (...args) {
      return original.apply(this, args);
    });
    spy.mockRestore = () => {
      object[key] = original;
    };
    object[key] = spy;
    return spy;
  },
};

const calls = (m) => {
  if (!m?._isMockFunction) throw new TypeError(`${m} is not a vi.fn() mock`);
  return m.mock.calls;
};

function matchers(actual, negate) {
  const check = (pass, message) => {
    if (pass === negate) assert.fail(negate ? `expected not: ${message}` : message);
  };
  const deepEq = (a, b) => {
    try {
      chaiExpect(a).to.deep.equal(b);
      return true;
    } catch {
      return false;
    }
  };
  return {
    toHaveBeenCalled: () => check(calls(actual).length > 0, 'expected the mock to have been called'),
    toHaveBeenCalledTimes: (n) => check(calls(actual).length === n, `expected ${n} calls, got ${calls(actual).length}`),
    toHaveBeenCalledWith: (...args) =>
      check(calls(actual).some((c) => deepEq(c, args)), `expected a call with ${JSON.stringify(args)}, got ${JSON.stringify(calls(actual))}`),
    toBe: (v) => check(Object.is(actual, v), `expected ${String(actual)} to be ${String(v)}`),
    toEqual: (v) => check(deepEq(actual, v), `expected ${JSON.stringify(actual)} to equal ${JSON.stringify(v)}`),
    toStrictEqual: (v) => check(deepEq(actual, v), `expected ${JSON.stringify(actual)} to equal ${JSON.stringify(v)}`),
    toBeTruthy: () => check(!!actual, `expected ${String(actual)} to be truthy`),
    toBeFalsy: () => check(!actual, `expected ${String(actual)} to be falsy`),
    toBeUndefined: () => check(actual === undefined, `expected ${String(actual)} to be undefined`),
    toContain: (v) => check(actual?.includes?.(v) ?? false, `expected ${String(actual)} to contain ${String(v)}`),
  };
}

export function expect(actual) {
  const m = matchers(actual, false);
  m.not = matchers(actual, true);
  return m;
}
