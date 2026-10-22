// One `svelters-e2e` worker (AU2b): a child process that runs whole test files, one at a time,
// against one app process of its own. Between tests the app is reset (`app.reset {seed}`: a fresh
// host on a fresh virtual clock) instead of relaunched; a test that kills or hangs the app fails
// alone, and the next one gets a new process.
//
// Messages from the parent: {type: 'run', file, opts} and {type: 'exit'}. To the parent:
// {type: 'test', …result} per test and {type: 'done', file, error?} per file.

import { existsSync } from 'node:fs';
import { dirname, isAbsolute, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';
import { launch, AutomationError } from '../index.js';
import { beginFile, fixtureNames, seedsOf } from './api.js';

/** The worker's app: the launch config it was started with and whether it is untouched. */
let current = null;

/** `$SVELTERS_RUN`, or `target/debug/svelters-run` in the cwd or a parent. */
export function findBin(env = process.env) {
  if (env.SVELTERS_RUN) return env.SVELTERS_RUN;
  for (let d = process.cwd(); ; d = dirname(d)) {
    const exe = resolve(d, 'target', 'debug', process.platform === 'win32' ? 'svelters-run.exe' : 'svelters-run');
    if (existsSync(exe)) return exe;
    if (dirname(d) === d) return null;
  }
}

function launchConfig(use, file, opts) {
  const rel = (p) => (p === undefined || isAbsolute(p) ? p : resolve(dirname(file), p));
  return {
    bin: use.bin ? rel(use.bin) : opts.bin ?? findBin(),
    app: rel(use.app),
    clock: use.clock ?? 'virtual',
    stepMs: use.stepMs,
    args: use.args ?? [],
  };
}

async function drop() {
  const c = current;
  current = null;
  if (!c) return;
  if (c.app.child?.exitCode === null) {
    await Promise.race([c.app.close(), new Promise((r) => setTimeout(r, 3_000))]).catch(() => {});
    if (c.app.child.exitCode === null) c.app.child.kill('SIGKILL');
  } else c.app.client.close();
}

/** The worker's app for `cfg`, at `seed`: launched, or reset when `reset` and it was used. */
async function appFor(cfg, seed, reset) {
  const key = JSON.stringify(cfg);
  if (current && (current.key !== key || current.app.child?.exitCode !== null)) await drop();
  if (!current) {
    if (!cfg.bin) throw new AutomationError('runner.no_host', 'no svelters-run found: build it (cargo build -p svelters-automation --features run --bin svelters-run) or set SVELTERS_RUN');
    const app = await launch({ ...cfg, seed, stderr: 'pipe' });
    current = { key, app, fresh: true, seed };
  } else if (reset && !(current.fresh && current.seed === seed)) {
    await current.app.reset({ seed });
    current.seed = seed;
  }
  return current;
}

const errorJson = (e) => ({
  message: e?.message ?? String(e),
  stack: e?.stack,
  code: e?.code,
  expected: e?.expected,
  actual: e?.actual,
});

/** One iteration of a test at `seed`. */
async function once(t, cfg, seed, file) {
  const names = fixtureNames(t.fn);
  const wantsApp = names === null || names.includes('app');
  const c = await appFor(cfg, seed, wantsApp);
  c.fresh = false;
  const app = c.app;
  const fixtures = {
    app,
    seed,
    mount: (component, o = {}) => {
      const target = component instanceof URL || isAbsolute(String(component)) ? component : resolve(dirname(file), String(component));
      return app.mount(target, { seed, ...o });
    },
  };
  let timer;
  const timeout = t.use.timeout ?? 30_000;
  try {
    await Promise.race([
      t.fn(fixtures),
      new Promise((_, reject) => {
        timer = setTimeout(() => reject(new AutomationError('runner.timeout', `test timed out after ${timeout} ms`)), timeout);
      }),
    ]);
  } catch (e) {
    // A dead or hung app is not the next test's problem.
    const tail = app.child?.stderrTail?.slice(-20) ?? [];
    if (e?.code === 'runner.timeout' || e?.code === 'transport.closed' || app.child?.exitCode !== null) await drop();
    e.appStderr = tail;
    throw e;
  } finally {
    clearTimeout(timer);
  }
}

async function runTest(t, file, opts) {
  const base = { type: 'test', file, title: t.title };
  if (t.mode === 'skip' || (opts.grep && !new RegExp(opts.grep).test(t.title))) return { ...base, status: 'skipped', seeds: [], duration: 0 };
  const seeds = seedsOf(t.use);
  const cfg = launchConfig(t.use, file, opts);
  const retries = opts.retries ?? t.use.retries ?? 0;
  const started = Date.now();
  let flaky = false;
  for (const seed of seeds) {
    let attempt = 0;
    for (;;) {
      try {
        await once(t, cfg, seed, file);
        if (attempt > 0) flaky = true;
        break;
      } catch (e) {
        if (attempt < retries) {
          attempt++;
          continue;
        }
        return { ...base, status: 'failed', seeds, seed, attempts: attempt + 1, error: errorJson(e), appStderr: e?.appStderr ?? [], duration: Date.now() - started };
      }
    }
  }
  return { ...base, status: flaky ? 'flaky' : 'passed', seeds, duration: Date.now() - started };
}

async function runFile(file, opts) {
  const reg = beginFile(file);
  await import(pathToFileURL(file).href);
  const tests = reg.tests.some((t) => t.mode === 'only') ? reg.tests.map((t) => (t.mode === 'only' ? t : { ...t, mode: 'skip' })) : reg.tests;
  for (const t of tests) process.send(await runTest(t, file, opts));
}

process.on('message', async (msg) => {
  if (msg.type === 'run') {
    try {
      await runFile(msg.file, msg.opts);
      process.send({ type: 'done', file: msg.file });
    } catch (e) {
      process.send({ type: 'done', file: msg.file, error: errorJson(e) });
    }
  } else if (msg.type === 'exit') {
    await drop();
    process.disconnect();
  }
});
