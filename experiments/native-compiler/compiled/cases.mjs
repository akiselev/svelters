// The compiled cases: the existing JS-pipeline fixtures (same App.svelte, same script) and the
// X1-only fixtures under ../fixtures (native binding, styling, slots, teardown).
import path from 'node:path';
import fs from 'node:fs';

const here = import.meta.dirname;
const repo = path.resolve(here, '../../..');
export const BACKENDS = ['gtk', 'qt'];

/** { name, source, script, jsExpected?, handwritten?, own } */
export function cases() {
  const out = [];
  for (const name of ['counter', 'if-toggle', 'keyed-list', 'keyed-stress', 'native-close', 'timers']) {
    out.push({
      name,
      source: path.join(repo, 'fixtures', name, 'src/App.svelte'),
      script: path.join(repo, 'fixtures', name, 'script.json'),
      jsExpected: path.join(repo, 'fixtures', name, 'expected.oplog'),
      handwritten: path.join(here, '../handwritten', name, 'expected.oplog'),
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
