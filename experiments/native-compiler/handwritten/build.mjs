// Builds <name>/dist/app.js for every hand-written component (or the names given as arguments).
import path from 'node:path';
import { build } from 'vite';
import { nativeConfig } from '@svelters/native-runtime/vite';

const NAMES = ['counter', 'if-toggle', 'keyed-list', 'keyed-stress', 'native-close', 'timers'];

const here = import.meta.dirname;
const names = process.argv.slice(2).length ? process.argv.slice(2) : NAMES;
for (const name of names) {
  await build(nativeConfig(path.join(here, name), { entry: 'main.js' }));
  console.log(`built ${name}/dist/app.js`);
}
