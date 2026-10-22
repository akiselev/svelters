// The test API of svelters' own runner (AU2b, E2E-INSPECTOR §2.2, §6 Q2). A test file registers
// its tests on import; `svelters-e2e` (runner/cli.js) loads it in a worker and runs them.
//
//   import { test, expect } from '@svelters/test';
//   test.use({ app: '../fixtures/counter-egui', seed: 3, iterations: 5 });
//   test('counts', async ({ app }) => { … });
//   test('a component alone', async ({ mount }) => { const c = await mount('./Panel.svelte'); … });
//
// Options (`test.use`, `test.describe`'s scope, or a test's own second argument): `app` (an app
// dir, `App.svelte` or `dist/app.js`; relative paths are the test file's), `bin` (the host binary,
// default `$SVELTERS_RUN` or `target/debug/svelters-run`), `clock` (`virtual` | `real`), `stepMs`,
// `args`, `seed` (default 0), `iterations` (default 1), `timeout` (wall ms per iteration, default
// 30 000), `retries`. The environment's `SEED`/`ITERATIONS` override seed and iterations, as for
// `#[svelters_automation::test]`.

const KEY = Symbol.for('svelters.test.registry');

/** The registry the runner reads (one per process, shared by every copy of this module). */
export function registry() {
  globalThis[KEY] ??= { tests: [], scopes: [{ title: [], use: {} }], file: null };
  return globalThis[KEY];
}

/** Clears the registry and loads `file`'s tests into it (the worker). */
export function beginFile(file) {
  const r = registry();
  r.tests = [];
  r.scopes = [{ title: [], use: {} }];
  r.file = file;
  return r;
}

function scope() {
  const s = registry().scopes;
  return s[s.length - 1];
}

function add(mode, title, a, b) {
  const [opts, fn] = typeof a === 'function' ? [{}, a] : [a ?? {}, b];
  if (typeof fn !== 'function') throw new TypeError(`test(${JSON.stringify(title)}): want a function`);
  const sc = scope();
  registry().tests.push({
    title: [...sc.title, String(title)].join(' › '),
    mode,
    use: { ...sc.use, ...opts },
    fn,
  });
}

/** Registers a test: `test(title, fn)` or `test(title, options, fn)`. */
export function test(title, a, b) {
  add('run', title, a, b);
}
test.skip = (title, a, b) => add('skip', title, a, b);
/** Only the file's `only` tests run (per file). */
test.only = (title, a, b) => add('only', title, a, b);
/** Options for the rest of the file (or the enclosing `describe`). */
test.use = (opts) => Object.assign(scope().use, opts);
/** A titled group with its own `test.use` scope. */
test.describe = (title, fn) => {
  const parent = scope();
  const r = registry();
  r.scopes.push({ title: [...parent.title, String(title)], use: { ...parent.use } });
  try {
    fn();
  } finally {
    r.scopes.pop();
  }
};

/**
 * The fixtures a test function destructures (`async ({ app, mount }) => …`), read from its source
 * as Playwright does, so a `mount`-only test skips the reset that `mount` would redo. `null` when
 * the parameter is not a plain destructuring (then every fixture is prepared).
 */
export function fixtureNames(fn) {
  const m = /^[^(]*\(\s*\{([^}]*)\}/.exec(Function.prototype.toString.call(fn));
  if (!m) return null;
  return m[1]
    .split(',')
    .map((p) => p.split(':')[0].split('=')[0].trim())
    .filter(Boolean);
}

/** The seeds of one test: `seed..seed+iterations`, overridden by `SEED`/`ITERATIONS`. */
export function seedsOf(use, env = process.env) {
  const int = (v, what) => {
    const n = Number(v);
    if (!Number.isInteger(n) || n < 0) throw new TypeError(`${what}: want a non-negative integer, got ${JSON.stringify(v)}`);
    return n;
  };
  const seed = env.SEED !== undefined && env.SEED !== '' ? int(env.SEED, 'SEED') : int(use.seed ?? 0, 'seed');
  const iterations = env.ITERATIONS !== undefined && env.ITERATIONS !== '' ? int(env.ITERATIONS, 'ITERATIONS') : int(use.iterations ?? 1, 'iterations');
  return Array.from({ length: Math.max(1, iterations) }, (_, i) => seed + i);
}
