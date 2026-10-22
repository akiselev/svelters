// TK1 test cases: the build side. A case is one `.svelte` file with a header comment and an
// `@test` comment (plans/milestones/TK1-test-cases.md); this module finds the cases of a directory,
// reads the toolkits each one targets and builds one bundle per (case, toolkit):
//
//   <out>/<toolkit>/<case>/dist/app.js
//
// with `svelters:testkit` resolved to the kit of that toolkit (kit/<toolkit>/index.js). The runners
// (crates/svelters-testkit, examples/counter/tests/testkit.rs) read the same layout.
import fs from 'node:fs';
import path from 'node:path';
import { build } from 'vite';
import { svelteConfig } from '@svelters/build';

const here = import.meta.dirname;

/** The toolkits the kit supports, which are also the default `@toolkits` of a case. */
export const TOOLKITS = ['gtk', 'qt', 'egui'];

/** The kit module of `toolkit` (what `svelters:testkit` resolves to). */
export function kitPath(toolkit) {
  return path.join(here, 'kit', toolkit, 'index.js');
}

/** `{ name, file, toolkits }` of each case file (`*.svelte`, not `_`-prefixed helpers) in `dir`. */
export function listCases(dir) {
  return fs
    .readdirSync(dir)
    .filter((f) => f.endsWith('.svelte') && !f.startsWith('_'))
    .sort()
    .map((f) => {
      const file = path.join(dir, f);
      return { name: f.slice(0, -'.svelte'.length), file, toolkits: caseToolkits(fs.readFileSync(file, 'utf8')) };
    });
}

/** The `@toolkits` line of a case's header comment, else every kit toolkit. */
export function caseToolkits(src) {
  const m = /^\s*@toolkits\s+(.+)$/m.exec(src);
  if (!m) return [...TOOLKITS];
  return m[1].trim().split(/[\s,]+/).filter(Boolean);
}

/** Builds `case` for `toolkit` into `<out>/<toolkit>/<case.name>/dist/app.js`. */
export async function buildCase(c, toolkit, out) {
  const root = path.join(out, toolkit, c.name);
  fs.mkdirSync(root, { recursive: true });
  const config = svelteConfig(root, { toolkit, dev: false });
  config.resolve.alias = config.resolve.alias.filter((a) => String(a.find) !== String(/^@app$/));
  config.resolve.alias.unshift({ find: /^@app$/, replacement: c.file }, { find: /^svelters:testkit$/, replacement: kitPath(toolkit) });
  config.logLevel = 'error';
  await build(config);
  return path.join(root, 'dist', 'app.js');
}
