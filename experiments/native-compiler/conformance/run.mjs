// Conformance of the NC1 compiler + @svelters/native-runtime against Svelte's own runtime tests
// (tests/runtime-runes/samples of stock Svelte 5.57.2, tag svelte@5.57.2, `dom` variant), on a jsdom host.
//
//   node run.mjs [--check] [--update-baseline] [name-substring...]
//
// For each sample: copy it to out/<name>, compile every .svelte file with
// `svelters-compile --spec metadata/semantics/dom.json --json`, then (if it compiled) run
// `_config.js` in a child process (run-one.mjs) and classify:
//   pass | rejected (compile diagnostics, by code) | runtime_error | mismatch | timeout
// Samples without a `dom` variant (mode/skip_mode excludes client, async gating) are not counted;
// `skip: true` samples are listed as skipped.
// Writes results.json (per sample) and REPORT.md. baseline.json is the ratchet: the samples that
// passed when it was last updated; --check fails if any of them no longer passes;
// --update-baseline rewrites it (refused if that would drop a passing sample, unless FORCE=1).
// expected.json lists the expected rejections (features outside the native subset or design, each
// category with its reason, its ref and the failure it must show); the score reports them apart
// from passes and failures, and never as passes.
// Env: UPSTREAM (default /tmp/claude-0/up; samples at <UPSTREAM>/svelte/packages/svelte/tests/
// runtime-runes/samples), SVELTERS_COMPILE (compiler binary), JOBS, TIMEOUT_MS.
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { execFile, spawn } from 'node:child_process';
import { compilerPath } from '@svelters/native-runtime/vite';

const here = import.meta.dirname;
const repo = path.resolve(here, '../../..');
const upstream = process.env.UPSTREAM || '/tmp/claude-0/up';
const samplesDir = path.join(upstream, 'svelte/packages/svelte/tests/runtime-runes/samples');
const spec = path.join(repo, 'metadata/semantics/dom.json');
const outDir = path.join(here, 'out');
const jobs = Number(process.env.JOBS) || os.availableParallelism();
const timeoutMs = Number(process.env.TIMEOUT_MS) || 20000;
const args = process.argv.slice(2);
const check = args.includes('--check');
const updateBaseline = args.includes('--update-baseline');
const filters = args.filter((a) => !a.startsWith('--'));
// The native compiler has no async mode (experimental.async), so samples run as in Svelte's
// SVELTE_NO_ASYNC=true CI job: async-* samples are excluded, configs pick their sync expectations.
process.env.SVELTE_NO_ASYNC ??= 'true';
const asyncMode = process.env.SVELTE_NO_ASYNC !== 'true';

// `pnpm test` passes --skip-missing: without the upstream samples or a built compiler the run is
// skipped with a message instead of failing (`pnpm run test:compiler` builds the compiler first).
const skipMissing = args.includes('--skip-missing');
const missing = (what) => {
  if (!skipMissing) return false;
  console.log(`SKIP conformance: ${what}`);
  return true;
};
if (!fs.existsSync(samplesDir)) {
  if (missing(`no Svelte samples at ${samplesDir} (clone sveltejs/svelte @ tag svelte@5.57.2 into $UPSTREAM/svelte)`)) process.exit(0);
  console.error(`no Svelte samples at ${samplesDir} (clone sveltejs/svelte @ tag svelte@5.57.2 into $UPSTREAM/svelte)`);
  process.exit(2);
}
let bin;
try {
  bin = compilerPath();
} catch (e) {
  if (missing(`${e.message}; \`pnpm run test:compiler\` builds it through tools/dev/cargo-slot`)) process.exit(0);
  throw e;
}

// ---------------------------------------------------------------------------------------------
// Which samples have a `dom` variant (shared.ts runtime_suite), from the config text: importing
// 1000 configs in one process would need the whole test environment.

function configFacts(text) {
  const list = (key) => {
    const m = new RegExp(`\\b${key}\\s*:\\s*\\[([^\\]]*)\\]`).exec(text);
    return m ? [...m[1].matchAll(/'([^']+)'|"([^"]+)"/g)].map((x) => x[1] ?? x[2]) : null;
  };
  return {
    mode: list('mode'),
    skip_mode: list('skip_mode'),
    skip: /^\s*skip\s*:\s*true/m.test(text),
    skip_no_async: /\bskip_no_async\s*:\s*true/.test(text),
    skip_async: /\bskip_async\s*:\s*true/.test(text),
    compileOptions: /\bcompileOptions\s*:/.test(text) ? (/\bcompileOptions\s*:\s*(\{[^}]*\})/.exec(text)?.[1] ?? '{...}').replace(/\s+/g, ' ') : null,
    error: /^\s*error\s*:/m.test(text),
    // compileOptions.dev (passed to svelters-compile as --dev)
    dev: /\bcompileOptions\s*:\s*\{[^}]*\bdev\s*:\s*true/.test(text),
  };
}

function domVariant(name, f) {
  if (!asyncMode && (f.skip_no_async || name.startsWith('async-'))) return false;
  if (asyncMode && f.skip_async) return false;
  if (f.skip_mode?.includes('client') || (f.mode && !f.mode.includes('client'))) return false;
  return true;
}

// ---------------------------------------------------------------------------------------------

function walk(dir, rel = '') {
  const out = [];
  for (const e of fs.readdirSync(path.join(dir, rel), { withFileTypes: true })) {
    const r = path.join(rel, e.name);
    if (e.name === '_output' || e.name === '_actual.json') continue;
    if (e.isDirectory()) out.push(...walk(dir, r));
    else out.push(r);
  }
  return out;
}

function compileFile(src, name, dev) {
  return new Promise((resolve) => {
    execFile(bin, ['--spec', spec, '--json', ...(dev ? ['--dev'] : []), '--filename', name, src], { maxBuffer: 64 << 20 }, (err, stdout, stderr) => {
      try {
        resolve(JSON.parse(stdout));
      } catch {
        resolve({ js: null, diagnostics: [{ severity: 'error', code: 'compiler_crash', message: (stderr || String(err)).split('\n').slice(0, 3).join(' ') }] });
      }
    });
  });
}

/** `unsupported_feature: {#snippet}`-style key for a diagnostic. */
function reasonKey(d) {
  if (d.code === 'unsupported_feature') {
    const m = /^(.*?) (?:is|are) not supported/.exec(d.message);
    let what = (m ? m[1] : d.message).replace(/`[^`]*`/g, '`…`');
    if (/^\$[a-z.]+$/.test(what) === false && /^\$/.test(what)) what = what.split(' ')[0];
    return `unsupported_feature: ${what}`;
  }
  if (d.code === 'unknown_element' || d.code === 'unknown_property' || d.code === 'unknown_event' || d.code === 'bind_no_plan') {
    const m = /<([^>]+)>|`([^`]+)`|bind:([\w-]+)/.exec(d.message);
    return `${d.code}${m ? `: ${m[1] ?? m[2] ?? m[3]}` : ''}`;
  }
  return d.code;
}

function runChild(dir) {
  return new Promise((resolve) => {
    const child = spawn(process.execPath, ['--conditions=browser', '--conditions=development', '--no-warnings', '--import', path.join(here, 'register.mjs'), path.join(here, 'run-one.mjs'), dir], {
      cwd: here,
      stdio: ['ignore', 'pipe', 'pipe'],
    });
    let stdout = '';
    let stderr = '';
    child.stdout.on('data', (d) => (stdout += d));
    child.stderr.on('data', (d) => (stderr += d));
    const timer = setTimeout(() => child.kill('SIGKILL'), timeoutMs);
    child.on('close', (code, signal) => {
      clearTimeout(timer);
      const line = stdout.split('\n').reverse().find((l) => l.startsWith('@@RESULT '));
      if (line) return resolve(JSON.parse(line.slice('@@RESULT '.length)));
      if (signal === 'SIGKILL') return resolve({ outcome: 'timeout', message: `no result after ${timeoutMs} ms` });
      const msg = stderr.split('\n').filter((l) => l.trim() && !/^\s+at /.test(l)).slice(0, 4).join(' ');
      resolve({ outcome: 'runtime_error', message: msg || `exit ${code}` });
    });
  });
}

async function prepare(name) {
  const src = path.join(samplesDir, name);
  const dst = path.join(outDir, name);
  const cfgPath = path.join(src, '_config.js');
  if (!fs.existsSync(cfgPath) || !fs.existsSync(path.join(src, 'main.svelte'))) return { outcome: 'not_dom', why: 'no _config.js/main.svelte' };
  const facts = configFacts(fs.readFileSync(cfgPath, 'utf8'));
  if (!domVariant(name, facts)) return { outcome: 'not_dom', why: 'no dom variant' };
  const base = { compileOptions: facts.compileOptions ?? undefined };
  if (facts.skip) return { ...base, outcome: 'skipped', why: 'skip: true upstream' };
  fs.rmSync(dst, { recursive: true, force: true });
  const files = walk(src);
  const diags = [];
  for (const f of files) {
    fs.mkdirSync(path.dirname(path.join(dst, f)), { recursive: true });
    fs.copyFileSync(path.join(src, f), path.join(dst, f));
  }
  for (const f of files) {
    if (/\.svelte\.ts$/.test(f)) {
      diags.push({ file: f, code: 'svelters_typescript_unsupported', message: `${f}: TypeScript .svelte.ts modules are not supported by the native compiler`, severity: 'error' });
      continue;
    }
    if (!f.endsWith('.svelte') && !f.endsWith('.svelte.js')) continue;
    const res = await compileFile(path.join(src, f), f, facts.dev);
    const errs = res.diagnostics.filter((d) => d.severity === 'error').map((d) => ({ file: f, code: d.code, message: d.message, line: d.line, column: d.column }));
    diags.push(...errs);
    if (res.js) {
      const out = path.join(dst, path.dirname(f), '_compiled', path.basename(f) + '.js');
      fs.mkdirSync(path.dirname(out), { recursive: true });
      fs.writeFileSync(out, res.js);
    }
  }
  if (diags.length) {
    if (facts.error) {
      // an expected compile error: the sample passes when we report the same message
      const r = await runChild(dst);
      if (r.outcome === 'pass') return { ...base, ...r };
    }
    const reasons = [...new Set(diags.map(reasonKey))];
    return { ...base, outcome: 'rejected', reason: reasons[0], reasons, diagnostics: diags.slice(0, 8) };
  }
  return { ...base, ...(await runChild(dst)) };
}

// ---------------------------------------------------------------------------------------------

const names = fs
  .readdirSync(samplesDir)
  .filter((n) => fs.statSync(path.join(samplesDir, n)).isDirectory())
  .filter((n) => !filters.length || filters.some((f) => n.includes(f)))
  .sort();
fs.mkdirSync(outDir, { recursive: true });

const results = {};
let next = 0;
let done = 0;
await Promise.all(
  Array.from({ length: jobs }, async () => {
    while (next < names.length) {
      const name = names[next++];
      results[name] = await prepare(name);
      done++;
      if (process.stdout.isTTY) process.stdout.write(`\r${done}/${names.length}`);
    }
  }),
);
if (process.stdout.isTTY) process.stdout.write('\n');

const sorted = Object.fromEntries(Object.keys(results).sort().map((k) => [k, results[k]]));
const counted = Object.entries(sorted).filter(([, r]) => r.outcome !== 'not_dom');
const by = (pred) => counted.filter(([, r]) => pred(r));
const count = (o) => by((r) => r.outcome === o).length;
const tally = (rows, key) => {
  const m = new Map();
  for (const [, r] of rows) for (const k of [].concat(key(r))) m.set(k, (m.get(k) ?? 0) + 1);
  return [...m].sort((a, b) => b[1] - a[1] || (a[0] < b[0] ? -1 : 1));
};
const passing = by((r) => r.outcome === 'pass').map(([n]) => n);

// ---- expected rejections (expected.json)
// Samples that need a feature outside the native language subset or design. Each category names
// why and where that is decided (`ref`), and how the failure must look (`match`): `reasons` are
// regexes over the compile-time rejection reasons (one must match), `runtime` regexes over the
// runtime error / mismatch message. A listed sample counts as an expected rejection only while it
// fails in that way; one that passes or fails differently is reported as "stale" and counts with
// the failures, so a real bug cannot hide behind the list. Expected rejections are never passes.
const expectedPath = path.join(here, 'expected.json');
const expectedList = fs.existsSync(expectedPath) ? JSON.parse(fs.readFileSync(expectedPath, 'utf8')) : { categories: {}, samples: {} };
// A sample entry is its category's name, or `{ category, match }` when the failure is specific to
// the sample (a generic assertion message): `match` then pins that exact failure.
const hasMatch = (m) => m?.reasons?.length || m?.runtime?.length;
for (const [k, c] of Object.entries(expectedList.categories)) if (!c.why || !c.ref) throw new Error(`expected.json: category ${k} needs why and ref`);
const expectedEntry = (n) => {
  const e = expectedList.samples[n];
  if (e === undefined) return null;
  const k = typeof e === 'string' ? e : e.category;
  const c = expectedList.categories[k];
  if (!c) throw new Error(`expected.json: ${n}: unknown category ${k}`);
  const match = (typeof e === 'object' && e.match) || c.match;
  if (!hasMatch(match)) throw new Error(`expected.json: ${n}: no match (category ${k} has none, so the entry needs its own)`);
  return { k, match };
};
for (const n of Object.keys(expectedList.samples)) expectedEntry(n);
const staleExpected = [];
for (const [n, r] of counted) {
  const entry = expectedEntry(n);
  if (!entry) continue;
  const { k, match: m } = entry;
  const hits =
    (r.outcome === 'rejected' && (m.reasons ?? []).some((re) => r.reasons.some((x) => new RegExp(re).test(x)))) ||
    ((r.outcome === 'runtime_error' || r.outcome === 'mismatch') && (m.runtime ?? []).some((re) => new RegExp(re).test(r.message ?? '')));
  if (hits) r.expected = k;
  else staleExpected.push([n, k, r]);
}
if (!filters.length) {
  const countedNames = new Set(counted.map(([n]) => n));
  for (const n of Object.keys(expectedList.samples)) if (!countedNames.has(n)) staleExpected.push([n, expectedEntry(n).k, { outcome: 'not a counted sample' }]);
}
const isExpected = (r) => r.expected !== undefined;
const expectedRows = by(isExpected);

if (filters.length) {
  for (const [n, r] of Object.entries(sorted)) console.log(`${r.outcome.padEnd(13)} ${n}${r.expected ? `  {expected: ${r.expected}}` : ''}${r.reason ? `  [${r.reason}]` : ''}${r.message ? `  ${process.env.VERBOSE ? r.message : r.message.split('\n')[0].slice(0, 200)}` : ''}`);
  for (const [n, k, r] of staleExpected) console.log(`stale expected.json entry: ${n} (${k}) is ${r.outcome}`);
  process.exit(0);
}

const summary = {
  samples: names.length,
  no_dom_variant: names.length - counted.length,
  dom_samples: counted.length,
  pass: count('pass'),
  rejected: count('rejected'),
  runtime_error: count('runtime_error'),
  mismatch: count('mismatch'),
  timeout: count('timeout'),
  skipped: count('skipped'),
  // the score: pass + expected_rejection + failing + skipped = dom_samples
  expected_rejection: expectedRows.length,
  failing: by((r) => r.outcome !== 'pass' && r.outcome !== 'skipped' && !isExpected(r)).length,
};
{
  // one sample per line (diffable); messages shortened
  const head = { svelte: '5.57.2 (tag svelte@5.57.2) tests/runtime-runes/samples', variant: 'dom', svelte_no_async: !asyncMode, summary };
  const rows = Object.entries(sorted).map(([n, r]) => `  ${JSON.stringify(n)}:${JSON.stringify({ ...r, message: r.message?.slice(0, 600), diagnostics: r.diagnostics?.slice(0, 3) })}`);
  fs.writeFileSync(path.join(here, 'results.json'), `${JSON.stringify(head).slice(0, -1)},"samples":{\n${rows.join(',\n')}\n}}\n`);
}

// ---- REPORT.md
const rejected = by((r) => r.outcome === 'rejected' && !isExpected(r));
const pct = (n) => `${((100 * n) / counted.length).toFixed(1)}%`;
const table = (rows, head) => [`| ${head} | samples |`, '| --- | ---: |', ...rows.map(([k, v]) => `| ${k.replace(/\|/g, '\\|')} | ${v} |`)].join('\n');
const failing = by((r) => (r.outcome === 'runtime_error' || r.outcome === 'mismatch' || r.outcome === 'timeout') && !isExpected(r));
function expectedSection() {
  const cats = Object.entries(expectedList.categories).map(([k, c]) => {
    const mine = expectedRows.filter(([, r]) => r.expected === k).map(([n]) => n);
    return { k, c, mine };
  });
  const head = ['| category | samples | why | decided in |', '| --- | ---: | --- | --- |', ...cats.map(({ k, c, mine }) => `| ${k} | ${mine.length} | ${c.why.replace(/\|/g, '\\|')} | ${c.ref.replace(/\|/g, '\\|')} |`)].join('\n');
  const lists = cats.filter(({ mine }) => mine.length).map(({ k, mine }) => `- ${k} (${mine.length}): ${mine.join(', ')}`).join('\n');
  const stale = staleExpected.length
    ? `\n\nListed in \`expected.json\` but not failing in the listed way (counted as passes or failures, not as expected rejections; fix or drop the entry):\n\n${staleExpected.map(([n, k, r]) => `- ${n} (${k}): ${r.outcome}${r.reason ? ` [${r.reason}]` : ''}`).join('\n')}`
    : '';
  return `${head}\n\n${lists}${stale}`;
}
function focusTable() {
  const groups = ['state', 'derived', 'effect', 'props', 'bindable', 'proxy', 'untrack', 'flush', 'each', 'if', 'binding', 'event', 'class'];
  const rows = groups.map((g) => {
    const mine = counted.filter(([n]) => n === g || n.startsWith(`${g}-`) || (g === 'bindable' && n.includes('bindable')));
    const c = (o) => mine.filter(([, r]) => r.outcome === o).length;
    return `| ${g} | ${mine.length} | ${c('pass')} | ${c('rejected')} | ${c('runtime_error')} | ${c('mismatch') + c('timeout')} |`;
  });
  return ['| prefix | samples | pass | rejected | runtime error | mismatch/timeout |', '| --- | ---: | ---: | ---: | ---: | ---: |', ...rows].join('\n');
}
const firstLine = (m) => (m ?? '').split('\n')[0].replace(/\|/g, '\\|').slice(0, 160);
const report = `# NC1 conformance: Svelte runtime-runes samples on the DOM test backend

Generated by \`node experiments/native-compiler/conformance/run.mjs\` (do not edit; harness design in run.mjs / run-one.mjs headers).
Svelte 5.57.2 (tag svelte@5.57.2, stock), \`tests/runtime-runes/samples\`, \`dom\` variant, assertions unchanged.

## Score

| | samples | share |
| --- | ---: | ---: |
| **pass** | **${summary.pass}** | ${pct(summary.pass)} |
| expected rejection (outside the native subset or design; \`expected.json\`) | ${summary.expected_rejection} | ${pct(summary.expected_rejection)} |
| failing (in the subset: rejected, runtime error, mismatch, timeout) | ${summary.failing} | ${pct(summary.failing)} |
| skipped upstream | ${summary.skipped} | ${pct(summary.skipped)} |
| **samples with a dom variant** | **${summary.dom_samples}** | |

Expected rejections are not passes. Of the ${summary.dom_samples - summary.expected_rejection - summary.skipped} samples that are neither
expected rejections nor skipped, ${summary.pass} pass (${((100 * summary.pass) / Math.max(1, summary.dom_samples - summary.expected_rejection - summary.skipped)).toFixed(1)}%).

## Outcomes

| outcome | samples | share |
| --- | ---: | ---: |
| pass | ${summary.pass} | ${pct(summary.pass)} |
| rejected at compile time | ${summary.rejected} | ${pct(summary.rejected)} |
| runtime error | ${summary.runtime_error} | ${pct(summary.runtime_error)} |
| assertion mismatch | ${summary.mismatch} | ${pct(summary.mismatch)} |
| timeout | ${summary.timeout} | ${pct(summary.timeout)} |
| skipped upstream (\`skip: true\`) | ${summary.skipped} | ${pct(summary.skipped)} |
| **samples with a dom variant** | **${summary.dom_samples}** | |

${summary.no_dom_variant} of the ${summary.samples} sample directories have no \`dom\` variant (server-only \`mode\`, \`skip_mode: ['client']\`, async gating) and are not counted.

## Rune / reactivity samples (by name prefix)

${focusTable()}

## Expected rejections

${expectedSection()}

## Compile-time rejections: first diagnostic per sample

Expected rejections are left out of this and the next three tables.

${table(tally(rejected, (r) => r.reason).slice(0, 40), 'reason')}

## Compile-time rejections: every reason a sample hits

A sample counts once per distinct reason, so this shows what fixing one feature would unblock at most.

${table(tally(rejected, (r) => r.reasons).slice(0, 40), 'reason')}

## Runtime errors and mismatches

${table(tally(failing, (r) => `${r.outcome}: ${firstLine(r.message).replace(/\d+/g, 'N').slice(0, 90)}`).slice(0, 40), 'outcome: first line (digits as N)')}

## Failing samples (runtime error / mismatch / timeout)

\`compileOptions.dev\` is passed through (\`svelters-compile --dev\`); other compile options are not
honoured. ${failing.filter(([, r]) => r.compileOptions).length} of the ${failing.length} failing samples set compile options.

| sample | outcome | compileOptions | message |
| --- | --- | --- | --- |
${failing.map(([n, r]) => `| ${n} | ${r.outcome} | ${(r.compileOptions ?? '').replace(/\|/g, '\\|')} | ${firstLine(r.message)} |`).join('\n')}

## Passing samples

${passing.join(', ')}
`;
fs.writeFileSync(path.join(here, 'REPORT.md'), report);

// ---- ratchet
const baselinePath = path.join(here, 'baseline.json');
const baseline = fs.existsSync(baselinePath) ? JSON.parse(fs.readFileSync(baselinePath, 'utf8')).pass : [];
const lost = baseline.filter((n) => !passing.includes(n));
const gained = passing.filter((n) => !baseline.includes(n));
console.log(`pass ${summary.pass} / ${summary.dom_samples}, expected rejections ${summary.expected_rejection}, failing ${summary.failing} (rejected ${summary.rejected}, runtime errors ${summary.runtime_error}, mismatches ${summary.mismatch}, timeouts ${summary.timeout}, skipped ${summary.skipped}); baseline ${baseline.length}: +${gained.length} -${lost.length}`);
if (staleExpected.length) console.log(`stale expected.json entries (not failing in the listed way): ${staleExpected.map(([n, , r]) => `${n} (${r.outcome})`).join(', ')}`);
if (lost.length) console.log(`no longer passing: ${lost.join(', ')}`);
if (gained.length) console.log(`newly passing: ${gained.join(', ')}`);
if (updateBaseline) {
  if (lost.length && process.env.FORCE !== '1') {
    console.error('refusing to update the baseline: it would drop passing samples (FORCE=1 to override)');
    process.exit(1);
  }
  fs.writeFileSync(baselinePath, JSON.stringify({ note: 'samples that pass; run.mjs --check fails if one stops passing', count: passing.length, pass: passing }, null, 1) + '\n');
}
if (check && lost.length) process.exit(1);
