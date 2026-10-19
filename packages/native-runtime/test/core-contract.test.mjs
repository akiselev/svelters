// RUNTIME.md phase 1: `@svelters/native-runtime/core` exports exactly the names the compiler emits
// (read off crates/svelters-compiler/src), and only the core adapter imports Svelte's internals.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import * as core from '../src/core/index.js';
import { emittedNames } from './core-contract.mjs';

const src = path.resolve(import.meta.dirname, '../src');

test('core exports exactly the contract the compiler emits', () => {
  assert.deepEqual(Object.keys(core).sort(), emittedNames());
  for (const n of emittedNames()) assert.equal(typeof core[n], 'function', n);
});

test('only src/core/svelte.js imports svelte/internal', () => {
  const files = [];
  const walk = (d) => {
    for (const e of fs.readdirSync(d, { withFileTypes: true })) {
      const p = path.join(d, e.name);
      if (e.isDirectory()) walk(p);
      else if (e.name.endsWith('.js')) files.push(p);
    }
  };
  walk(src);
  const importers = files.filter((f) => /from\s+['"]svelte\/internal/.test(fs.readFileSync(f, 'utf8')));
  assert.deepEqual(importers.map((f) => path.relative(src, f)), [path.join('core', 'svelte.js')]);
});
