// Vite config helper for bundles built on @svelters/native-runtime: one self-contained ES module
// (dist/app.js) like packages/build, plus the alias that exposes Svelte's internal effects module.
import path from 'node:path';
import { createRequire } from 'node:module';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import fs from 'node:fs';

/** The specifier src/index.js imports for block/branch/destroy_effect/component_root. */
export const EFFECTS_SPECIFIER = 'svelte/internal/client/reactivity/effects';

/** Absolute path of this package's runtime entry. */
export const runtimePath = fileURLToPath(new URL('./src/index.js', import.meta.url));

/** Absolute path of Svelte's effects module (hidden by the package's exports map). */
export const effectsPath = path.join(
  path.dirname(createRequire(import.meta.url).resolve('svelte/internal/client')),
  'reactivity/effects.js',
);

/**
 * Vite config for one app: `entry` (relative to `root`) -> `<root>/<outDir>/app.js`, es2022, not
 * minified, with a source map, resolved with the same conditions as packages/build.
 */
export function nativeConfig(root, { entry = 'main.js', outDir = 'dist', plugins = [] } = {}) {
  return {
    root,
    plugins,
    configFile: false,
    logLevel: 'warn',
    resolve: {
      conditions: ['custom-renderer', 'browser'],
      alias: [
        { find: /^svelte\/internal\/client\/reactivity\/effects(\.js)?$/, replacement: effectsPath },
        // generated components import the runtime from wherever the .svelte file lives
        { find: /^@svelters\/native-runtime$/, replacement: runtimePath },
      ],
      dedupe: ['svelte'],
    },
    build: {
      target: 'es2022',
      minify: false,
      sourcemap: true,
      cssCodeSplit: false,
      modulePreload: false,
      emptyOutDir: true,
      outDir: path.resolve(root, outDir),
      lib: { entry: path.resolve(root, entry), formats: ['es'], fileName: () => 'app.js' },
    },
  };
}

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');

/** The `svelters-compile` binary: $SVELTERS_COMPILE, else the newest of the workspace's release/debug builds. */
export function compilerPath() {
  if (process.env.SVELTERS_COMPILE) return process.env.SVELTERS_COMPILE;
  const found = ['target/release/svelters-compile', 'target/debug/svelters-compile']
    .map((p) => path.join(repoRoot, p))
    .filter((p) => fs.existsSync(p))
    .sort((a, b) => fs.statSync(b).mtimeMs - fs.statSync(a).mtimeMs);
  if (!found.length) throw new Error('svelters-compile not found: run `cargo build -p svelters-compiler` or set SVELTERS_COMPILE');
  return found[0];
}

/**
 * Vite plugin: compiles `.svelte` modules with the experimental Rust compiler (NC1) for one
 * backend. Diagnostics become Vite errors/warnings with positions; the compiler's source map
 * maps generated code back to the component.
 */
export function svelteNative({ backend = 'gtk', spec } = {}) {
  const bin = compilerPath();
  return {
    name: 'svelters-native-compiler',
    enforce: 'pre',
    transform(code, id) {
      if (!id.endsWith('.svelte')) return null;
      const args = ['--backend', backend, '--json', '--filename', id];
      if (spec) args.push('--spec', spec);
      args.push('-');
      let out;
      try {
        out = execFileSync(bin, args, { input: code, encoding: 'utf8', stdio: ['pipe', 'pipe', 'pipe'] });
      } catch (e) {
        out = e.stdout;
        if (!out) throw new Error(`svelters-compile failed: ${e.stderr || e.message}`);
      }
      const res = JSON.parse(out);
      for (const d of res.diagnostics.filter((d) => d.severity === 'warning')) {
        this.warn({ message: `${d.code}${d.backend ? ` (${d.backend})` : ''}: ${d.message}`, loc: { line: d.line, column: d.column - 1 }, pos: d.start });
      }
      const errors = res.diagnostics.filter((d) => d.severity === 'error');
      if (errors.length) {
        const d = errors[0];
        const more = errors.length > 1 ? `\n${errors.slice(1).map((x) => `  ${path.basename(id)}:${x.line}:${x.column}: ${x.code}: ${x.message}`).join('\n')}` : '';
        this.error({ message: `${d.code}${d.backend ? ` (${d.backend})` : ''}: ${d.message}${d.help ? ` (help: ${d.help})` : ''}${more}`, id, loc: { line: d.line, column: d.column - 1 } });
      }
      return { code: res.js, map: res.map };
    },
  };
}
