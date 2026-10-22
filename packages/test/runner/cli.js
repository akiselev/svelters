// `svelters-e2e`: svelters' own test runner (AU2b, E2E-INSPECTOR §6 Q2). Dependency-free.
//
//   svelters-e2e [paths…] [--workers N] [--grep RE] [--seed N] [--iterations N] [--retries N]
//                [--shard I/N] [--bin PATH] [--reporter list,dot,json[:file],junit[:file],github]
//                [--list]
//
// It finds `*.e2e.{js,mjs,ts,mts}` under the paths (default `.`; `node_modules`, `target`, `dist`
// and dot directories are skipped), shares the files out to `--workers` child processes (one app
// process per worker, reset between tests), and prints each failure's seed and the command that
// replays it (`SEED=s ITERATIONS=1 svelters-e2e <file> --grep <title>`), as
// `#[svelters_automation::test]` does. `.ts` files run on Node's type stripping. Exit status: 0
// when nothing failed, 1 when a test failed, 2 for a usage error.

import { fork } from 'node:child_process';
import { availableParallelism } from 'node:os';
import { readdirSync, statSync, writeFileSync, mkdirSync } from 'node:fs';
import { dirname, join, relative, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const USAGE = `usage: svelters-e2e [paths…] [--workers N] [--grep RE] [--seed N] [--iterations N] [--retries N]
                    [--shard I/N] [--bin PATH] [--reporter list,dot,json[:file],junit[:file],github] [--list]`;

const TEST_FILE = /\.e2e\.(js|mjs|ts|mts)$/;
const SKIP_DIRS = new Set(['node_modules', 'target', 'dist']);

export function parseArgs(argv) {
  const o = { paths: [], reporters: null };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    const val = () => {
      if (i + 1 >= argv.length) throw new UsageError(`${a} wants a value`);
      return argv[++i];
    };
    const int = (v) => {
      const n = Number(v);
      if (!Number.isInteger(n) || n < 0) throw new UsageError(`${a} wants a non-negative integer`);
      return n;
    };
    switch (a) {
      case '--workers': case '-j': o.workers = Math.max(1, int(val())); break;
      case '--grep': case '-g': o.grep = val(); break;
      case '--seed': o.seed = int(val()); break;
      case '--iterations': o.iterations = int(val()); break;
      case '--retries': o.retries = int(val()); break;
      case '--bin': o.bin = resolve(val()); break;
      case '--reporter': o.reporters = val().split(',').filter(Boolean); break;
      case '--shard': {
        const m = /^(\d+)\/(\d+)$/.exec(val());
        if (!m || +m[1] < 1 || +m[1] > +m[2]) throw new UsageError('--shard wants I/N with 1 <= I <= N');
        o.shard = [+m[1], +m[2]];
        break;
      }
      case '--list': o.list = true; break;
      case '-h': case '--help': throw new UsageError('');
      default:
        if (a.startsWith('-')) throw new UsageError(`unknown flag ${a}`);
        o.paths.push(a);
    }
  }
  return o;
}

class UsageError extends Error {}

/** The test files under `paths` (files given by name are taken as they are), sorted. */
export function discover(paths) {
  const out = new Set();
  const walk = (p) => {
    for (const e of readdirSync(p, { withFileTypes: true })) {
      if (e.isDirectory()) {
        if (!SKIP_DIRS.has(e.name) && !e.name.startsWith('.')) walk(join(p, e.name));
      } else if (TEST_FILE.test(e.name)) out.add(resolve(p, e.name));
    }
  };
  for (const p of paths.length ? paths : ['.']) {
    const st = statSync(p);
    if (st.isDirectory()) walk(p);
    else out.add(resolve(p));
  }
  return [...out].sort();
}

// ---- reporters ----

const rel = (f) => relative(process.cwd(), f) || f;
const ms = (d) => (d >= 1000 ? `${(d / 1000).toFixed(1)}s` : `${d}ms`);
const quoteArg = (s) => (/^[\w./:=@-]+$/.test(s) ? s : `'${s.replace(/'/g, `'\\''`)}'`);
const escapeRe = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

/** The command that replays one failing seed of one test. */
export function replayCommand(r) {
  return `SEED=${r.seed} ITERATIONS=1 svelters-e2e ${quoteArg(rel(r.file))} --grep ${quoteArg(`^${escapeRe(r.title)}$`)}`;
}

function failureText(r) {
  const lines = [`${rel(r.file)} › ${r.title}`, `  FAILED with SVELTERS_SEED=${r.seed}${r.seeds.length > 1 ? ` (seeds ${r.seeds[0]}..${r.seeds.at(-1)})` : ''}${r.attempts > 1 ? `, ${r.attempts} attempts` : ''}`];
  lines.push(...String(r.error?.stack ?? r.error?.message).split('\n').map((l) => `  ${l}`));
  if (r.appStderr?.length) lines.push('  app stderr (last lines):', ...r.appStderr.map((l) => `    ${l}`));
  lines.push(`  replay: ${replayCommand(r)}`);
  return lines.join('\n');
}

const xml = (s) => String(s).replace(/[<>&"]/g, (c) => ({ '<': '&lt;', '>': '&gt;', '&': '&amp;', '"': '&quot;' })[c]);

function writeOut(path, text, out) {
  if (!path) return out.write(text);
  mkdirSync(dirname(resolve(path)), { recursive: true });
  writeFileSync(path, text);
}

export function makeReporter(name, out = process.stdout) {
  const [kind, path] = name.split(/:(.*)/s);
  switch (kind) {
    case 'list':
      return {
        test(r) {
          const mark = { passed: '✓', failed: '✗', skipped: '-', flaky: '!' }[r.status];
          const seeds = r.seeds.length > 1 ? ` [seeds ${r.seeds[0]}..${r.seeds.at(-1)}]` : r.seeds.length ? ` [seed ${r.seeds[0]}]` : '';
          out.write(`  ${mark} ${rel(r.file)} › ${r.title}${r.status === 'skipped' ? '' : seeds} (${ms(r.duration)})${r.status === 'flaky' ? ' flaky' : ''}\n`);
        },
        end(sum) {
          if (sum.failed.length) out.write(`\n${sum.failed.map(failureText).join('\n\n')}\n`);
          out.write(`\n${sum.counts.passed} passed, ${sum.counts.failed} failed, ${sum.counts.flaky} flaky, ${sum.counts.skipped} skipped (${ms(sum.duration)})\n`);
        },
      };
    case 'dot':
      return {
        test(r) {
          out.write({ passed: '.', failed: 'F', skipped: '-', flaky: '!' }[r.status]);
        },
        end(sum) {
          out.write('\n');
          if (sum.failed.length) out.write(`\n${sum.failed.map(failureText).join('\n\n')}\n`);
          out.write(`\n${sum.counts.passed} passed, ${sum.counts.failed} failed (${ms(sum.duration)})\n`);
        },
      };
    case 'json':
      return {
        end(sum) {
          writeOut(path, JSON.stringify({ counts: sum.counts, duration: sum.duration, results: sum.results.map((r) => ({ ...r, replay: r.status === 'failed' ? replayCommand(r) : undefined })) }, null, 2) + '\n', out);
        },
      };
    case 'junit':
      return {
        end(sum) {
          const byFile = Map.groupBy(sum.results, (r) => rel(r.file));
          const suites = [...byFile].map(([file, rs]) => {
            const cases = rs.map((r) => {
              const body =
                r.status === 'failed'
                  ? `<failure message="${xml(r.error?.message ?? '')}">${xml(failureText(r))}</failure>`
                  : r.status === 'skipped'
                    ? '<skipped/>'
                    : '';
              return `    <testcase classname="${xml(file)}" name="${xml(r.title)}" time="${(r.duration / 1000).toFixed(3)}">${body}</testcase>`;
            });
            const f = rs.filter((r) => r.status === 'failed').length;
            const s = rs.filter((r) => r.status === 'skipped').length;
            return `  <testsuite name="${xml(file)}" tests="${rs.length}" failures="${f}" skipped="${s}">\n${cases.join('\n')}\n  </testsuite>`;
          });
          writeOut(path, `<?xml version="1.0" encoding="UTF-8"?>\n<testsuites tests="${sum.results.length}" failures="${sum.counts.failed}">\n${suites.join('\n')}\n</testsuites>\n`, out);
        },
      };
    case 'github':
      // Annotations on the run's summary (GitHub Actions workflow commands).
      return {
        end(sum) {
          const esc = (s) => String(s).replace(/%/g, '%25').replace(/\r/g, '%0D').replace(/\n/g, '%0A');
          for (const r of sum.failed) out.write(`::error file=${rel(r.file)},title=${esc(r.title)}::${esc(`${r.error?.message}\nreplay: ${replayCommand(r)}`)}\n`);
          for (const r of sum.results.filter((r) => r.status === 'flaky')) out.write(`::warning file=${rel(r.file)},title=${esc(r.title)}::flaky (passed on retry)\n`);
        },
      };
    default:
      throw new UsageError(`unknown reporter ${kind}`);
  }
}

// ---- the pool ----

const WORKER = fileURLToPath(new URL('./worker.js', import.meta.url));

function workerArgs(files) {
  // Node 22 strips types behind a flag (default from 23.6).
  const [major, minor] = process.versions.node.split('.').map(Number);
  const needsFlag = files.some((f) => /\.m?ts$/.test(f)) && (major < 23 || (major === 23 && minor < 6));
  return needsFlag ? ['--experimental-strip-types', '--disable-warning=ExperimentalWarning'] : [];
}

/** Runs `files` on `workers` processes; resolves with every result. */
export function runFiles(files, opts, onResult) {
  const queue = [...files];
  const results = [];
  const n = Math.max(1, Math.min(opts.workers ?? Math.max(1, Math.floor(availableParallelism() / 2)), files.length));
  const env = { ...process.env };
  if (opts.seed !== undefined) env.SEED = String(opts.seed);
  if (opts.iterations !== undefined) env.ITERATIONS = String(opts.iterations);
  const execArgv = workerArgs(files);
  const wopts = { grep: opts.grep, retries: opts.retries, bin: opts.bin };
  return new Promise((done) => {
    let live = n;
    for (let i = 0; i < n; i++) {
      const w = fork(WORKER, [], { env: { ...env, SVELTERS_E2E_WORKER: String(i) }, execArgv, stdio: ['ignore', 'inherit', 'inherit', 'ipc'] });
      let file = null;
      const next = () => {
        file = queue.shift() ?? null;
        if (file) w.send({ type: 'run', file, opts: wopts });
        else w.send({ type: 'exit' });
      };
      w.on('message', (m) => {
        if (m.type === 'test') {
          results.push(m);
          onResult(m);
        } else if (m.type === 'done') {
          if (m.error) {
            const r = { type: 'test', file: m.file, title: '(file failed to load)', status: 'failed', seeds: [], seed: '-', error: m.error, duration: 0 };
            results.push(r);
            onResult(r);
          }
          next();
        }
      });
      w.on('exit', (code) => {
        if (file) {
          const r = { type: 'test', file, title: '(worker exited)', status: 'failed', seeds: [], seed: '-', error: { message: `the worker exited (${code}) while running this file` }, duration: 0 };
          results.push(r);
          onResult(r);
          file = null;
        }
        if (--live === 0) done(results);
      });
      next();
    }
  });
}

export async function main(argv = process.argv.slice(2), out = process.stdout) {
  let opts, reporters;
  try {
    opts = parseArgs(argv);
    opts.reporters ??= process.env.GITHUB_ACTIONS ? ['list', 'github'] : ['list'];
    reporters = opts.reporters.map((r) => makeReporter(r, out));
  } catch (e) {
    if (!(e instanceof UsageError)) throw e;
    process.stderr.write(`${e.message ? `svelters-e2e: ${e.message}\n` : ''}${USAGE}\n`);
    return 2;
  }
  let files = discover(opts.paths);
  if (opts.shard) files = files.filter((_, i) => i % opts.shard[1] === opts.shard[0] - 1);
  if (opts.list) {
    for (const f of files) out.write(`${rel(f)}\n`);
    return 0;
  }
  if (!files.length) {
    process.stderr.write('svelters-e2e: no *.e2e.{js,mjs,ts,mts} files found\n');
    return 1;
  }
  const started = Date.now();
  const results = await runFiles(files, opts, (r) => reporters.forEach((rep) => rep.test?.(r)));
  const counts = { passed: 0, failed: 0, flaky: 0, skipped: 0 };
  for (const r of results) counts[r.status]++;
  const sum = { results, counts, failed: results.filter((r) => r.status === 'failed'), duration: Date.now() - started };
  for (const rep of reporters) rep.end?.(sum);
  return counts.failed ? 1 : 0;
}
