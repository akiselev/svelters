// The compiler-facing core contract (docs/notes/native-compiler/runtime-contract.md "Core API"):
// the `$.<name>` the Rust compiler can emit, read off its sources. Used by core-contract.test.mjs
// and tools/check-svelte-internals.mjs.
import fs from 'node:fs';
import path from 'node:path';

const repo = path.resolve(import.meta.dirname, '../../..');
const SRC = path.join(repo, 'crates/svelters-compiler/src');

function rustFiles(dir, out = []) {
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, e.name);
    if (e.isDirectory()) out.push(...rustFiles(p));
    else if (e.name.endsWith('.rs') && e.name !== 'tests.rs') out.push(p);
  }
  return out;
}

/** String literals of a Rust source line (comments dropped). */
function literals(line) {
  const out = [];
  let i = 0;
  while (i < line.length) {
    if (line[i] === '/' && line[i + 1] === '/') break;
    if (line[i] === '"') {
      let j = i + 1;
      let s = '';
      while (j < line.length && line[j] !== '"') {
        if (line[j] === '\\') {
          s += line[j + 1];
          j += 2;
        } else s += line[j++];
      }
      out.push(s);
      i = j + 1;
    } else if (line[i] === "'" && line[i + 2] === "'") i += 3; // char literal
    else i++;
  }
  return out;
}

/** The `$.` names the compiler emits (`update{pre}` expands to both forms). */
export function emittedNames() {
  const names = new Set();
  for (const f of rustFiles(SRC)) {
    for (const line of fs.readFileSync(f, 'utf8').split('\n')) {
      for (const lit of literals(line)) {
        for (const m of lit.matchAll(/\$\.([a-z_]+)(\{pre\}(_[a-z]+)?)?/g)) {
          if (m[2]) {
            names.add(`${m[1]}${m[3] ?? ''}`);
            names.add(`${m[1]}_pre${m[3] ?? ''}`);
          } else names.add(m[1]);
        }
      }
    }
  }
  return [...names].sort();
}
