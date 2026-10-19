#!/usr/bin/env node
// Asserts the Svelte runtime internals the native runtime depends on (plans/research/RUNTIME.md
// phase 0): everything packages/native-runtime/src/core/svelte.js (the core adapter, the only module
// that imports `svelte/internal/*`) takes from the installed `svelte`. Run by the native-runtime
// package's `pnpm test`. A Svelte bump fails here first, loudly, instead of breaking components
// silently: audit the adapter against the new version, then update PINNED.
//
//   node tools/check-svelte-internals.mjs
import fs from 'node:fs';
import path from 'node:path';
import { createRequire, register } from 'node:module';
import { pathToFileURL } from 'node:url';

/** The Svelte version the adapter was audited against. */
const PINNED = '5.57.2';

const repo = path.resolve(import.meta.dirname, '..');
const runtimeDir = path.join(repo, 'packages/native-runtime');
const require = createRequire(path.join(runtimeDir, 'package.json'));
// the hidden-module aliases of vite.js, for Node
register(pathToFileURL(path.join(runtimeDir, 'test/alias-hook.mjs')));

const failures = [];
const check = (ok, msg) => {
  if (!ok) failures.push(msg);
};

const pkgPath = path.join(path.dirname(require.resolve('svelte/package.json')));
const version = JSON.parse(fs.readFileSync(path.join(pkgPath, 'package.json'), 'utf8')).version;
check(version === PINNED, `svelte is ${version}, the core adapter was audited against ${PINNED}: audit src/core/svelte.js, then update PINNED`);
const client = path.join(pkgPath, 'src/internal/client');
const src = (rel) => fs.readFileSync(path.join(client, rel), 'utf8');

// 1. the compiler-facing contract: every name the compiler emits is a function of svelte/internal/client
const $ = await import(pathToFileURL(require.resolve('svelte/internal/client')).href);
const { emittedNames } = await import(pathToFileURL(path.join(runtimeDir, 'test/core-contract.mjs')).href);
for (const n of emittedNames()) check(typeof $[n] === 'function', `svelte/internal/client has no function \`${n}\` (the compiler emits $.${n})`);
for (const n of ['flush', 'tick', 'clsx']) check(typeof $[n] === 'function', `svelte/internal/client has no function \`${n}\``);
check('active_effect' in $, 'svelte/internal/client no longer exports `active_effect`');

// 2. the hidden modules the adapter imports (vite.js HIDDEN_MODULES)
const hidden = {
  'reactivity/effects.js': ['block', 'branch', 'component_root', 'destroy_effect'],
  'dom/task.js': ['queue_micro_task'],
  'context.js': ['getAllContexts', 'setContext'],
};
for (const [file, names] of Object.entries(hidden)) {
  let m = null;
  try {
    m = await import(pathToFileURL(path.join(client, file)).href);
  } catch (e) {
    check(false, `${file}: ${e.message}`);
    continue;
  }
  for (const n of names) check(typeof m[n] === 'function', `${file} has no function \`${n}\``);
}

// 3. effect flag bits the adapter sets and reads
const core = await import(pathToFileURL(path.join(runtimeDir, 'src/core/svelte.js')).href);
const constants = await import(pathToFileURL(path.join(client, 'constants.js')).href);
for (const n of ['BOUNDARY_EFFECT', 'DESTROYED', 'DESTROYING']) {
  check(constants[n] === core[n], `constants.js ${n} is ${constants[n]}, the adapter uses ${core[n]}`);
}

// 4. the members Svelte's runtime uses on `effect.b`: the public members of its Boundary class
const boundary = src('dom/blocks/boundary.js');
const body = /export class Boundary \{([\s\S]*?)\n\}/.exec(boundary)?.[1];
check(body, 'dom/blocks/boundary.js has no `export class Boundary`');
if (body) {
  const members = new Set();
  for (const m of body.matchAll(/^\t(?:get |set |async )?([A-Za-z_$][\w$]*)\s*(?:[=;(])/gm)) if (m[1] !== 'constructor') members.add(m[1]);
  const ours = new Set(core.BOUNDARY_MEMBERS);
  for (const m of members) check(ours.has(m), `Boundary has a member \`${m}\` the adapter's boundary_handler lacks`);
  for (const m of ours) check(members.has(m), `Boundary no longer has \`${m}\` (adapter BOUNDARY_MEMBERS)`);
  const handler = core.boundary_handler({ error() {} });
  for (const m of ours) check(m in handler, `boundary_handler() lacks \`${m}\``);
}

// 5. the effect fields the adapter reads and rewrites (effect tree links, flags, boundary)
const effects = src('reactivity/effects.js');
const literal = /var effect = \{([\s\S]*?)\};/.exec(effects)?.[1] ?? '';
for (const f of ['f', 'b', 'first', 'last', 'next', 'prev', 'parent']) {
  check(new RegExp(`^\\s*${f}\\s*[:,]`, 'm').test(literal), `create_effect's effect object has no field \`${f}\``);
}

if (failures.length) {
  console.error(`check-svelte-internals: ${failures.length} problem(s) with svelte ${version}:\n  ${failures.join('\n  ')}`);
  process.exit(1);
}
console.log(`check-svelte-internals: svelte ${version} provides everything the core adapter uses (${emittedNames().length} contract names, ${Object.values(hidden).flat().length} hidden-module functions, 3 flags, ${core.BOUNDARY_MEMBERS.length} Boundary members)`);
