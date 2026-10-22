// svelters-e2e's self-tests (AU2b). The CLI runs on the mock host (test/mock-host.mjs, no Rust
// build) here, and on the real host (`svelters-run`, the counter fixture) when SVELTERS_RUN is set.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { fixtureNames, seedsOf } from '../runner/api.js';
import { discover, parseArgs, replayCommand } from '../runner/cli.js';

const here = (p) => fileURLToPath(new URL(p, import.meta.url));
const cli = here('../bin/svelters-e2e.mjs');

function run(args, env = {}) {
  const r = spawnSync(process.execPath, [cli, ...args], { encoding: 'utf8', env: { ...process.env, GITHUB_ACTIONS: '', SEED: '', ITERATIONS: '', ...env }, timeout: 120_000 });
  return { code: r.status, out: r.stdout, err: r.stderr };
}

test('seeds are seed..seed+iterations, and SEED/ITERATIONS override them', () => {
  assert.deepEqual(seedsOf({}, {}), [0]);
  assert.deepEqual(seedsOf({ seed: 3, iterations: 3 }, {}), [3, 4, 5]);
  assert.deepEqual(seedsOf({ seed: 3, iterations: 3 }, { SEED: '4', ITERATIONS: '1' }), [4]);
  assert.throws(() => seedsOf({ seed: -1 }, {}));
});

test('fixture names come from the destructured parameter', () => {
  assert.deepEqual(fixtureNames(async ({ mount }) => mount), ['mount']);
  assert.deepEqual(fixtureNames(async ({ app, seed: s = 1 }) => [app, s]), ['app', 'seed']);
  assert.equal(fixtureNames(async (f) => f), null);
});

test('args, discovery and the replay command', () => {
  const o = parseArgs(['a', '--workers', '3', '--seed', '7', '--shard', '2/4', '--reporter', 'dot,junit:x.xml']);
  assert.deepEqual([o.paths, o.workers, o.seed, o.shard, o.reporters], [['a'], 3, 7, [2, 4], ['dot', 'junit:x.xml']]);
  assert.throws(() => parseArgs(['--shard', '5/4']));
  const files = discover([here('e2e-mock')]).map((f) => f.split('/').slice(-2).join('/'));
  assert.deepEqual(files, ['fail/failing.e2e.js', 'pass/counter.e2e.js', 'pass/mount.e2e.ts']);
  assert.equal(replayCommand({ seed: 4, file: '/x/a.e2e.js', title: 'fails (once)' }).replace(/ \S*a\.e2e\.js/, ' a.e2e.js'), `SEED=4 ITERATIONS=1 svelters-e2e a.e2e.js --grep '^fails \\(once\\)$'`);
});

test('the runner passes the mock suite on two workers, one app each', () => {
  const dir = mkdtempSync(join(tmpdir(), 'svelters-e2e-'));
  try {
    const log = join(dir, 'log');
    const r = run([here('e2e-mock/pass'), '--workers', '2', '--reporter', `list,json:${join(dir, 'r.json')},junit:${join(dir, 'r.xml')}`], { MOCK_LOG: log });
    assert.equal(r.code, 0, r.out + r.err);
    assert.match(r.out, /5 passed, 0 failed, 0 flaky, 1 skipped/);
    assert.match(r.out, /runs once per seed \[seeds 3\.\.5\]/);
    const json = JSON.parse(readFileSync(join(dir, 'r.json'), 'utf8'));
    assert.equal(json.counts.passed, 5);
    assert.match(readFileSync(join(dir, 'r.xml'), 'utf8'), /<testsuites tests="6" failures="0">/);
    const lines = readFileSync(log, 'utf8').trim().split('\n');
    // One process per file (two files, two workers), reset between tests, never relaunched.
    assert.equal(lines.filter((l) => l.startsWith('launch')).length, 2, lines.join('\n'));
    assert.ok(lines.includes('reset 4') && lines.includes('reset 5') && lines.includes('reset 9'), lines.join('\n'));
    assert.ok(lines.some((l) => /^mount 0 Panel\.svelte$/.test(l)), lines.join('\n'));
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('a failure prints its seed and replay command; crashes and hangs fail alone', () => {
  const r = run([here('e2e-mock/fail'), '--reporter', 'list,github']);
  assert.equal(r.code, 1, r.out + r.err);
  assert.match(r.out, /2 passed, 3 failed/);
  assert.match(r.out, /FAILED with SVELTERS_SEED=4 \(seeds 3\.\.5\)/);
  assert.match(r.out, /replay: SEED=4 ITERATIONS=1 svelters-e2e \S*failing\.e2e\.js --grep '\^fails on seed 4\$'/);
  assert.match(r.out, /✓ \S+ › passes after a crash/);
  assert.match(r.out, /✓ \S+ › passes after a hang/);
  assert.match(r.out, /runner\.timeout: test timed out after 500 ms/);
  assert.match(r.out, /^::error file=\S*failing\.e2e\.js,title=fails on seed 4::/m);

  // The replay runs that one seed of that one test.
  const replay = run([here('e2e-mock/fail/failing.e2e.js'), '--grep', '^fails on seed 4$'], { SEED: '4', ITERATIONS: '1' });
  assert.equal(replay.code, 1);
  assert.match(replay.out, /0 passed, 1 failed, 0 flaky, 4 skipped/);
  assert.match(replay.out, /\[seed 4\]/);
  // And a seed that passes passes.
  assert.equal(run([here('e2e-mock/fail/failing.e2e.js'), '--grep', '^fails on seed 4$', '--seed', '5']).code, 0);
});

test('retries rerun the same seed', () => {
  const r = run([here('e2e-mock/fail/failing.e2e.js'), '--grep', '^fails on seed 4$', '--retries', '1', '--reporter', 'dot']);
  assert.equal(r.code, 1);
  assert.match(r.out, /2 attempts/);
});

// The real host: `SVELTERS_RUN=target/debug/svelters-run pnpm --filter @svelters/test test`.
const bin = process.env.SVELTERS_RUN;
test('the runner on svelters-run: the counter fixture and mount', { skip: !bin || !existsSync(bin) ? 'set SVELTERS_RUN to a built svelters-run' : false, timeout: 600_000 }, () => {
  const r = run([here('e2e'), '--workers', '2']);
  assert.equal(r.code, 0, r.out + r.err);
  assert.match(r.out, /6 passed, 0 failed/);
});
