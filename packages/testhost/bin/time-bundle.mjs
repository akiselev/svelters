#!/usr/bin/env node
// Usage: node --experimental-vm-modules --no-warnings time-bundle.mjs <fixtureDir>
// Times each prefix of <fixtureDir>/script.json on V8 (TestHost) and prints the increment per
// event step, as a JS-engine comparison for the QuickJS time split (docs/notes/p5-integration.md).
import fs from 'node:fs';
import path from 'node:path';
import { runBundle } from '../src/index.js';

const dir = path.resolve(process.argv[2] ?? '.');
const bundlePath = path.join(dir, 'dist', 'app.js');
const script = JSON.parse(fs.readFileSync(path.join(dir, 'script.json'), 'utf8'));
const prefixes = [];
script.forEach((s, i) => {
  if (s.event) prefixes.push({ what: `${s.event[1]}#${s.event[0]}`, steps: script.slice(0, i + 2) });
});
let prev = 0;
for (const reps of [1, 2]) {
  prev = 0;
  for (const p of prefixes) {
    const t0 = performance.now();
    await runBundle({ bundlePath, script: p.steps });
    const dt = performance.now() - t0;
    if (reps === 2) console.log(`${p.what}: cumulative ${dt.toFixed(0)} ms, step ${(dt - prev).toFixed(0)} ms`);
    prev = dt;
  }
}
