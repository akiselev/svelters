// Runs the compiled bundles on the REAL toolkits (GTK 4 / Qt 6 under a display, e.g. xvfb-run)
// through the counter example's driver: scripted `clicked` becomes a real click, `changed` real
// typing into the entry, `gone` a native window close. Checks per case and backend:
//   - the run succeeded with no host errors;
//   - existing fixtures: the native widget tree read back from the toolkit equals the JS
//     pipeline's (the same fixture built by packages/build, run by the same driver);
//   - own fixtures: the tree equals <dir>/expected.<backend>.native.json (UPDATE=1 records it).
// Screenshots: out/<backend>/<name>/native.png. Needs `cargo build -p counter`.
import fs from 'node:fs';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { BACKENDS, cases } from './cases.mjs';

const here = import.meta.dirname;
const repo = path.resolve(here, '../../..');
const bin = path.join(repo, 'target/debug/counter');
const update = process.env.UPDATE === '1';
const only = process.argv.slice(2);
let failed = false;

function run(backend, fixtureDir, script, report, png) {
  const args = ['--backend', backend, '--fixture', fixtureDir, '--script', script, '--report', report, '--end', 'quit', '--timeout', '60'];
  if (png) args.push('--screenshot', png);
  try {
    execFileSync(bin, args, { stdio: ['ignore', 'ignore', 'pipe'], encoding: 'utf8' });
  } catch (e) {
    if (!fs.existsSync(report)) return { ok: false, errors: [String(e.stderr || e.message).slice(-2000)] };
  }
  return JSON.parse(fs.readFileSync(report, 'utf8'));
}

// keep what the app controls: class, depth, text
const tree = (r) => (r.dump ?? []).map(({ class: c, d, text }) => ({ class: c, d, text }));

for (const backend of BACKENDS) {
  for (const c of cases()) {
    if (only.length && !only.includes(c.name)) continue;
    const dir = path.join(here, 'out', backend, c.name);
    const tag = `${backend}/${c.name}`;
    if (!fs.existsSync(path.join(dir, 'dist/app.js'))) {
      console.log(`SKIP ${tag}: rejected at compile time`);
      continue;
    }
    // scripts with `sync`/`deferred` steps (toolkit callbacks the driver cannot cause) have a
    // clicks-only twin for the real toolkits; the toolkit then calls setup/bind/... itself
    const script = c.nativeScript ?? c.script;
    const ours = run(backend, dir, script, path.join(dir, 'native.json'), path.join(dir, 'native.png'));
    const problems = [];
    if (!ours.ok) problems.push(`run failed: ${JSON.stringify(ours.failures ?? ours.errors)}`);
    if (ours.errors?.length) problems.push(`host errors: ${JSON.stringify(ours.errors)}`);
    let against = '';
    if (!c.own) {
      const theirs = run(backend, path.join(repo, 'fixtures', c.name), script, path.join(dir, 'native-js.json'), null);
      if (!theirs.ok) problems.push(`JS pipeline run failed on ${backend}: ${JSON.stringify(theirs.failures ?? theirs.errors)}`);
      else if (JSON.stringify(tree(ours)) !== JSON.stringify(tree(theirs))) {
        problems.push(`native tree differs from the JS pipeline's:\n  ours:   ${JSON.stringify(tree(ours))}\n  theirs: ${JSON.stringify(tree(theirs))}`);
      }
      against = 'native tree == JS pipeline on the same toolkit';
    } else {
      const exp = path.join(c.dir, `expected.${backend}.native.json`);
      const got = JSON.stringify(tree(ours), null, 1) + '\n';
      if (update) fs.writeFileSync(exp, got);
      else if (!fs.existsSync(exp) || fs.readFileSync(exp, 'utf8') !== got) problems.push(`native tree differs from ${path.basename(exp)}:\n${got}`);
      against = 'native tree == expected';
    }
    if (problems.length) failed = true;
    const t = ours.first_frame_ms != null ? ` (first frame ${Math.round(ours.first_frame_ms)} ms, load ${Math.round(ours.load_ms)} ms)` : '';
    console.log(`${problems.length ? 'FAIL' : 'PASS'} ${tag}: ${problems.length ? problems.join('\n  ') : against}${t}`);
  }
}
process.exit(failed ? 1 : 0);
