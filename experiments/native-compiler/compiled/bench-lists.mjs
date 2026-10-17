// Timing comparison for virtualized lists on the real toolkits: the lists-stress fixture
// (`<ListView>`, 1k / 10k rows) built by the JS pipeline (fixtures/lists-stress/dist) and by the
// NC1 compiler (out/<backend>/lists-stress, `node build.mjs lists-stress`), plus the eager
// `{#each}` baseline bench/keyed-bench (1000 rows of box + label + button) on both pipelines.
// Each step is one click, timed by the counter driver (`--timing`: wall time from the click to idle).
// Runs every configuration RUNS times and prints the median per step.
// Needs `cargo build -p counter` (release is closer to P5 numbers: SVELTERS_COUNTER=target/release/counter),
// `pnpm -r --filter "./fixtures/*" build`, `pnpm --dir bench/keyed-bench build`, a display (xvfb-run).
import fs from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { build } from 'vite';
import { nativeConfig, svelteNative } from '@svelters/native-runtime/vite';

const here = import.meta.dirname;
const repo = path.resolve(here, '../../..');
const bin = process.env.SVELTERS_COUNTER ?? path.join(repo, 'target/debug/counter');
const RUNS = Number(process.env.RUNS ?? 3);
const out = path.join(here, 'out/bench');
fs.mkdirSync(out, { recursive: true });

// lists-stress handles: fill1k #4, fill10k #5, append #7, prepend #8, removeMiddle #10, reverse #12,
// rename #13, sortDesc #14, odd #15, clear #16, scrollEnd #18, scrollStart #19
const click = (h) => [{ event: [h, 'clicked', []] }, { checkpoint: true }];
const listSteps = [[4, 'fill 1k'], [16, 'clear'], [5, 'fill 10k'], [7, 'append'], [8, 'prepend'], [10, 'removeMiddle'], [12, 'reverse'], [13, 'rename'], [14, 'sortDesc'], [18, 'scrollEnd'], [19, 'scrollStart'], [15, 'odd'], [16, 'clear']];
// keyed-bench handles: Fill #4, Update #5, Reverse #6, Swap #7, Clear #8
const eachSteps = [[4, 'fill 1k'], [5, 'update'], [6, 'reverse'], [7, 'swap'], [8, 'clear']];

function writeScript(name, steps) {
  const p = path.join(out, `${name}.json`);
  fs.writeFileSync(p, JSON.stringify([{ checkpoint: true }, ...steps.flatMap(([h]) => click(h))]));
  return p;
}

async function compileBench(backend) {
  const dir = path.join(out, `${backend}-keyed-bench`);
  fs.mkdirSync(dir, { recursive: true });
  const src = path.join(repo, 'bench/keyed-bench/src/App.svelte');
  fs.writeFileSync(path.join(dir, 'main.js'), `import * as $n from '@svelters/native-runtime';\nimport App from ${JSON.stringify(src)};\n\n$n.mount(App, { props: {} });\n`);
  await build(nativeConfig(dir, { entry: 'main.js', outDir: 'dist', plugins: [svelteNative({ backend })] }));
  return dir;
}

function timeRun(backend, fixture, script) {
  const report = path.join(out, 'report.json');
  fs.rmSync(report, { force: true });
  try {
    execFileSync(bin, ['--backend', backend, '--fixture', fixture, '--script', script, '--report', report, '--end', 'quit', '--timeout', '120', '--timing'], { stdio: ['ignore', 'ignore', 'pipe'] });
  } catch (e) {
    if (!fs.existsSync(report)) throw new Error(`${backend} ${fixture}: ${String(e.stderr).slice(-500)}`);
  }
  const r = JSON.parse(fs.readFileSync(report, 'utf8'));
  if (!r.ok) throw new Error(`${backend} ${fixture}: ${JSON.stringify(r.failures ?? r.errors)}`);
  return r.steps.map((s) => s.wall_ms);
}

const median = (xs) => xs.slice().sort((a, b) => a - b)[Math.floor(xs.length / 2)];

const listScript = writeScript('lists', listSteps);
const eachScript = writeScript('each', eachSteps);
for (const backend of ['gtk', 'qt']) {
  const configs = [
    ['ListView', listSteps, listScript, path.join(repo, 'fixtures/lists-stress'), path.join(here, 'out', backend, 'lists-stress')],
    ['{#each}', eachSteps, eachScript, path.join(repo, 'bench/keyed-bench'), await compileBench(backend)],
  ];
  for (const [what, steps, script, jsDir, ncDir] of configs) {
    const runs = { js: [], nc: [] };
    for (let i = 0; i < RUNS; i++) {
      runs.js.push(timeRun(backend, jsDir, script));
      runs.nc.push(timeRun(backend, ncDir, script));
    }
    console.log(`\n${backend} ${what} (median of ${RUNS}, ms)    JS pipeline   compiled`);
    steps.forEach(([, label], i) => {
      const a = median(runs.js.map((r) => r[i]));
      const b = median(runs.nc.map((r) => r[i]));
      console.log(`  ${label.padEnd(14)} ${a.toFixed(0).padStart(10)} ${b.toFixed(0).padStart(10)}`);
    });
  }
}
