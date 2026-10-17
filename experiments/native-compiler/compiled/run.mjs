// Runs every compiled bundle on the Node TestHost with the case's script and checks:
//   existing fixtures: op log == the hand-written equivalent's (../handwritten/<name>/expected.oplog,
//     i.e. the compiler emits the intended code shape) and native state == the JS pipeline's
//     (tools/oplog-compare.mjs, at every event/gone boundary and at the end);
//   own fixtures: op log == <dir>/expected.<backend>.oplog, or, when <dir>/expected.<backend>.diagnostics
//     exists, that compilation was rejected with exactly those diagnostic codes.
// UPDATE=1 rewrites the own fixtures' expectations (review the diff!).
import fs from 'node:fs';
import path from 'node:path';
import { runBundle } from '@svelters/testhost';
import { compare } from '../../../tools/oplog-compare.mjs';
import { BACKENDS, cases } from './cases.mjs';

const here = import.meta.dirname;
const update = process.env.UPDATE === '1';
const only = process.argv.slice(2);
let failed = false;
const DEFAULTS = { label: '' };

for (const backend of BACKENDS) {
  for (const c of cases()) {
    if (only.length && !only.includes(c.name)) continue;
    const dir = path.join(here, 'out', backend, c.name);
    const diagFile = path.join(dir, 'diagnostics.txt');
    const tag = `${backend}/${c.name}`;
    if (c.own) {
      const expDiag = path.join(c.dir, `expected.${backend}.diagnostics`);
      if (fs.existsSync(diagFile) || fs.existsSync(expDiag)) {
        const got = fs.existsSync(diagFile) ? fs.readFileSync(diagFile, 'utf8') : '(compiled)\n';
        if (update) fs.writeFileSync(expDiag, got);
        else if (!fs.existsSync(expDiag) || fs.readFileSync(expDiag, 'utf8') !== got) {
          failed = true;
          console.log(`FAIL ${tag}: diagnostics differ\n${got}`);
          continue;
        }
        console.log(`PASS ${tag}: rejected as expected`);
        continue;
      }
    } else if (fs.existsSync(diagFile)) {
      failed = true;
      console.log(`FAIL ${tag}: did not compile\n${fs.readFileSync(diagFile, 'utf8')}`);
      continue;
    }
    const script = JSON.parse(fs.readFileSync(c.script, 'utf8'));
    let actual;
    try {
      actual = await runBundle({ bundlePath: path.join(dir, 'dist/app.js'), script });
    } catch (e) {
      failed = true;
      console.log(`FAIL ${tag}: runtime error: ${e.stack ?? e}`);
      continue;
    }
    fs.writeFileSync(path.join(dir, 'actual.oplog'), actual);
    if (c.own) {
      const exp = path.join(c.dir, `expected.${backend}.oplog`);
      if (update) fs.writeFileSync(exp, actual);
      const ok = fs.existsSync(exp) && fs.readFileSync(exp, 'utf8') === actual;
      if (!ok) failed = true;
      console.log(`${ok ? 'PASS' : 'FAIL'} ${tag}: ${ok ? 'op log matches' : 'op log differs from ' + path.basename(exp)}`);
      continue;
    }
    const hand = fs.readFileSync(c.handwritten, 'utf8');
    const theirs = fs.readFileSync(c.jsExpected, 'utf8');
    const r = compare(theirs, actual, { defaults: DEFAULTS });
    const sameShape = actual === hand;
    if (!sameShape || !r.equal) failed = true;
    console.log(`${sameShape && r.equal ? 'PASS' : 'FAIL'} ${tag}: ${sameShape ? 'op log == hand-written' : 'op log != hand-written'}; ${actual === theirs ? 'identical to' : r.equal ? 'state-equal with' : 'STATE DIFFERS from'} JS pipeline (${r.boundaries} boundaries, ops ${r.opsB} / ${r.opsA})`);
    for (const p of r.problems) console.log(`  ${p}`);
  }
}
process.exit(failed ? 1 : 0);
