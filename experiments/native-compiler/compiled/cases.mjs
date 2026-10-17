// The compiled cases: the existing JS-pipeline fixtures (same App.svelte, same script) and the
// NC1-only fixtures under ../fixtures (native binding, styling, slots, teardown).
import path from 'node:path';
import fs from 'node:fs';

const here = import.meta.dirname;
const repo = path.resolve(here, '../../..');
export const BACKENDS = ['gtk', 'qt'];

/** { name, source, script, jsExpected?, handwritten?, own } */
export function cases() {
  const out = [];
  // virtual-list / lists-stress: `<ListView>` from @svelters/lists (virtualized native list); the
  // native pipeline resolves it to @svelters/native-runtime/lists. No hand-written equivalent.
  const lists = ['virtual-list', 'lists-stress', 'lists-recycle'];
  for (const name of ['counter', 'if-toggle', 'keyed-list', 'keyed-stress', 'native-close', 'timers', ...lists]) {
    out.push({
      name,
      source: path.join(repo, 'fixtures', name, 'src/App.svelte'),
      script: path.join(repo, 'fixtures', name, 'script.json'),
      jsExpected: path.join(repo, 'fixtures', name, 'expected.oplog'),
      handwritten: lists.includes(name) ? null : path.join(here, '../handwritten', name, 'expected.oplog'),
      // Rejected at compile time by the backend spec (the JS pipeline fails at runtime instead).
      rejectOn: name === 'if-toggle' ? { qt: 'unknown_element' } : {},
      // The hand-written equivalent predates master's bind:this version of this fixture: only
      // the JS-pipeline state comparison applies.
      handwrittenStale: name === 'native-close',
      nativeScript: fs.existsSync(path.join(here, 'scripts', `${name}.native.json`)) ? path.join(here, 'scripts', `${name}.native.json`) : undefined,
      // further scripts run live on both pipelines' bundles (JS: fixtures/<name>/dist/app.js)
      extraScripts: fs.existsSync(path.join(repo, 'fixtures', name, 'script.deferred.json')) ? [path.join(repo, 'fixtures', name, 'script.deferred.json')] : [],
    });
  }
  const own = path.join(here, '../fixtures');
  if (fs.existsSync(own)) {
    for (const name of fs.readdirSync(own).sort()) {
      const dir = path.join(own, name);
      if (!fs.existsSync(path.join(dir, 'App.svelte'))) continue;
      out.push({ name, source: path.join(dir, 'App.svelte'), script: path.join(dir, 'script.json'), dir, own: true });
    }
  }
  return out;
}
