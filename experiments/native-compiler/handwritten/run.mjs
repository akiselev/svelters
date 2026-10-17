// Runs every built <name>/dist/app.js on the Node TestHost with <name>/script.json, then
//   1. compares the op log byte for byte with <name>/expected.oplog (UPDATE=1 rewrites it), and
//   2. compares the native states with the JS pipeline's fixtures/<name>/expected.oplog
//      (tools/oplog-compare.mjs) at every event/gone boundary and at the end.
// Needs node --experimental-vm-modules.
import fs from 'node:fs';
import path from 'node:path';
import { runBundle } from '@svelters/testhost';
import { compare } from '../../../tools/oplog-compare.mjs';

const NAMES = ['counter', 'if-toggle', 'keyed-list', 'keyed-stress', 'native-close', 'timers'];
// A label written "" is the native default: the JS renderer never writes it (README.md).
const DEFAULTS = { label: '' };

const here = import.meta.dirname;
const fixtures = path.join(here, '../../../fixtures');
const update = process.env.UPDATE === '1';
let failed = false;

for (const name of process.argv.slice(2).length ? process.argv.slice(2) : NAMES) {
  const dir = path.join(here, name);
  const script = JSON.parse(fs.readFileSync(path.join(dir, 'script.json'), 'utf8'));
  const actual = await runBundle({ bundlePath: path.join(dir, 'dist/app.js'), script });
  const ours = path.join(dir, 'expected.oplog');
  const theirs = fs.readFileSync(path.join(fixtures, name, 'expected.oplog'), 'utf8');

  let status = 'PASS';
  if (update) {
    fs.writeFileSync(ours, actual);
    status = 'UPDATED';
  } else if (!fs.existsSync(ours) || fs.readFileSync(ours, 'utf8') !== actual) {
    status = 'FAIL (op log differs from expected.oplog)';
    failed = true;
    console.log(actual);
  }
  const r = compare(theirs, actual, { defaults: DEFAULTS });
  if (!r.equal) failed = true;
  const same = actual === theirs ? 'identical log' : r.equal ? 'state-equal' : 'STATE DIFFERS';
  console.log(`${status} ${name}: ${same} vs JS pipeline, ${r.boundaries} boundaries, ops ours ${r.opsB} / JS ${r.opsA}`);
  for (const p of r.problems) console.log(`  ${p}`);
}
process.exit(failed ? 1 : 0);
