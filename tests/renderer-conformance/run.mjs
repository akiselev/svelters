// Renderer conformance: build each scenario bundle, run it under Node and under quickjs-ng,
// and compare the op streams with each other and with expected/<run>.txt.
// usage: node run.mjs [--node-only] [--update] [--only=prod,dev]
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { bundle } from './build.mjs';

const here = path.dirname(fileURLToPath(import.meta.url));
const args = process.argv.slice(2);
const nodeOnly = args.includes('--node-only');
const update = args.includes('--update');
const only = args.find((a) => a.startsWith('--only='))?.slice(7).split(',');

const RUNS = [
  { name: 'prod', entry: 'apps/all.mjs', opts: {} },
  { name: 'dev', entry: 'apps/all.mjs', opts: { dev: true } },
  { name: 'values2', entry: 'apps/v2.mjs', opts: {} },
  { name: 'hmr', entry: 'apps/hmr.mjs', opts: { dev: true, hmr: true } },
];

// Read-call counters depend on Svelte internals and are not part of the contract.
const normalize = (t) => t.replace(/; reads: .*$/gm, '').replace(/[ \t]+$/gm, '');
const stripIds = (t) => t.replace(/#\d+/g, '#');

function findQjs() {
  if (process.env.QJS) return process.env.QJS;
  const p = path.join(here, '.qjs/qjs');
  if (fs.existsSync(p)) return p;
  const r = spawnSync('bash', [path.join(here, 'get-qjs.sh')], { encoding: 'utf8' });
  if (r.status !== 0) throw new Error(`could not obtain qjs: ${r.stderr}`);
  return r.stdout.trim();
}

function firstDiff(a, b) {
  const x = a.split('\n');
  const y = b.split('\n');
  for (let i = 0; i < Math.max(x.length, y.length); i++) {
    if (x[i] !== y[i]) return `line ${i + 1}\n  expected: ${x[i]}\n  actual:   ${y[i]}`;
  }
  return 'identical';
}

fs.mkdirSync(path.join(here, 'out'), { recursive: true });
fs.mkdirSync(path.join(here, 'expected'), { recursive: true });
const qjs = nodeOnly ? null : findQjs();
let failed = 0;

for (const run of RUNS) {
  if (only && !only.includes(run.name)) continue;
  const out = path.join(here, 'out', `${run.name}.mjs`);
  const warnings = await bundle(path.join(here, run.entry), out, run.opts);
  const node = spawnSync(process.execPath, [out], { encoding: 'utf8', cwd: here, env: { ...process.env, ONLY: '' } });
  if (node.status !== 0) {
    console.error(`FAIL ${run.name}: node exited ${node.status}\n${node.stderr}`);
    failed++;
    continue;
  }
  const nodeText = normalize(node.stdout);
  const problems = [];

  if (qjs) {
    const q = spawnSync(qjs, ['--std', path.join(here, 'qjs-run.mjs'), out], { encoding: 'utf8', cwd: here });
    if (q.status !== 0) problems.push(`qjs exited ${q.status}: ${q.stderr}`);
    else {
      const qText = normalize(q.stdout);
      if (qText !== nodeText) problems.push(`node vs qjs differ at ${firstDiff(nodeText, qText)}`);
    }
  }

  const goldenPath = path.join(here, 'expected', `${run.name}.txt`);
  if (update) fs.writeFileSync(goldenPath, nodeText);
  else {
    const golden = fs.readFileSync(goldenPath, 'utf8');
    if (stripIds(golden) !== stripIds(nodeText)) problems.push(`op stream differs from expected at ${firstDiff(golden, nodeText)}`);
    else if (golden !== nodeText) console.log(`note ${run.name}: node ids differ from expected (structure identical)`);
  }

  if (problems.length) {
    failed++;
    console.error(`FAIL ${run.name}\n  ${problems.join('\n  ')}`);
  } else {
    console.log(`PASS ${run.name}${update ? ' (updated)' : ''} (${nodeText.split('\n').length} lines${qjs ? ', node == qjs' : ', node only'}${warnings.length ? '; warnings: ' + warnings.join(',') : ''})`);
  }
}
process.exit(failed ? 1 : 0);
