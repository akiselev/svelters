#!/usr/bin/env node
// svelters-build [--watch] [dir]
//
// Builds the app in `dir` (default: the current directory; relative paths resolve against the
// directory the command was started from) with the app's own vite.config.js, which is expected
// to export `svelteConfig(import.meta.dirname)` from @svelters/build.
//
// --watch keeps building on every source change (Vite `build --watch`). It writes dist/app.js and
// dist/app.js.map in place without emptying dist/, so a host running with `--watch` never sees a
// missing bundle. Builds are dev builds: unminified, with source maps.
import path from 'node:path';
import { build } from 'vite';

const args = process.argv.slice(2);
const watch = args.includes('--watch');
const positional = args.filter((a) => !a.startsWith('--'));
const unknown = args.filter((a) => a.startsWith('--') && a !== '--watch');
if (unknown.length || positional.length > 1) {
  console.error('usage: svelters-build [--watch] [dir]');
  process.exit(2);
}
const base = process.env.INIT_CWD ?? process.cwd();
const dir = path.resolve(base, positional[0] ?? '.');

const inline = {
  root: dir,
  configFile: path.join(dir, 'vite.config.js'),
  logLevel: 'warn',
};
if (watch) inline.build = { watch: {}, emptyOutDir: false };

try {
  const result = await build(inline);
  if (!watch) {
    console.error(`[svelters-build] built ${path.join(dir, 'dist/app.js')}`);
  } else {
    let started = Date.now();
    result.on('event', (e) => {
      if (e.code === 'START') started = Date.now();
      else if (e.code === 'BUNDLE_END') {
        console.error(`[svelters-build] rebuilt in ${Date.now() - started} ms`);
        e.result?.close();
      } else if (e.code === 'ERROR') {
        console.error(`[svelters-build] build failed:\n${e.error?.stack ?? e.error}`);
        e.result?.close();
      }
    });
    console.error(`[svelters-build] watching ${dir}`);
    const stop = () => result.close().finally(() => process.exit(0));
    process.on('SIGTERM', stop);
    process.on('SIGINT', stop);
  }
} catch (e) {
  console.error(`[svelters-build] build failed:\n${e?.stack ?? e}`);
  process.exit(1);
}
