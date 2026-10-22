#!/usr/bin/env node
// Builds TK1 test cases: every `*.svelte` case in <dir> for each toolkit it targets.
//
//   svelters-cases <dir> [--out <dir>] [--toolkit gtk|qt|egui]... [name-substring...]
//
// Output: <out>/<toolkit>/<case>/dist/app.js (default <out>: <dir>/dist). A case that fails to
// build for one toolkit is reported and the others still build; the exit status is 1 if any failed
// (the runners then report that case as failing on that toolkit's backends).
import path from 'node:path';
import { buildCase, listCases } from '../index.js';

const args = process.argv.slice(2);
let dir = null;
let out = null;
const toolkits = [];
const filters = [];
for (let i = 0; i < args.length; i++) {
  const a = args[i];
  if (a === '--out') out = args[++i];
  else if (a === '--toolkit') toolkits.push(args[++i]);
  else if (a.startsWith('--')) throw new Error(`unknown option ${a}`);
  else if (dir === null) dir = a;
  else filters.push(a);
}
if (!dir) {
  console.error('usage: svelters-cases <dir> [--out <dir>] [--toolkit gtk|qt|egui]... [name-substring...]');
  process.exit(2);
}
dir = path.resolve(dir);
out = path.resolve(out ?? path.join(dir, 'dist'));

let failed = 0;
let built = 0;
for (const c of listCases(dir)) {
  if (filters.length && !filters.some((f) => c.name.includes(f))) continue;
  for (const tk of c.toolkits) {
    if (toolkits.length && !toolkits.includes(tk)) continue;
    try {
      await buildCase(c, tk, out);
      built++;
    } catch (e) {
      failed++;
      console.error(`${c.name} [${tk}]: ${String(e.message ?? e).split('\n').slice(0, 6).join('\n  ')}`);
    }
  }
}
console.log(`svelters-cases: ${built} built, ${failed} failed (${path.relative(process.cwd(), out) || '.'})`);
process.exit(failed ? 1 : 0);
