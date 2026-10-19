// Vite config helper for bundles built on @svelters/native-runtime: one self-contained ES module
// (dist/app.js) like packages/build, plus the aliases that expose Svelte's internal effects and task modules.
import path from 'node:path';
import { createRequire } from 'node:module';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import fs from 'node:fs';
import { createHash } from 'node:crypto';
import { compile as svelteCompile } from 'svelte/compiler';
import { extractNative, translateSheet } from '@svelters/style/build';
import { typesAliases } from '@svelters/types/vite';
import { formatDiagnostic } from '@svelters/style/runtime';

/** The specifier src/index.js imports for block/branch/destroy_effect/component_root. */
export const EFFECTS_SPECIFIER = 'svelte/internal/client/reactivity/effects';

/** Absolute path of this package's runtime entry. */
export const runtimePath = fileURLToPath(new URL('./src/index.js', import.meta.url));

/**
 * The native implementation of `@svelters/lists` (`ListView`): compiled components that import
 * the JS pipeline's list package get the same component API over the same host protocol.
 */
export const listsPath = fileURLToPath(new URL('./src/lists.js', import.meta.url));

/** Absolute path of Svelte's effects module (hidden by the package's exports map). */
export const effectsPath = path.join(
  path.dirname(createRequire(import.meta.url).resolve('svelte/internal/client')),
  'reactivity/effects.js',
);

/** Svelte's microtask queue (`queue_micro_task`, drained by `flushSync`), used by `boundary`. */
export const TASK_SPECIFIER = 'svelte/internal/client/dom/task';
export const taskPath = path.join(path.dirname(effectsPath), '../dom/task.js');

/** Svelte's component context (`getAllContexts` / `setContext` for list rows, src/lists.js). */
export const CONTEXT_SPECIFIER = 'svelte/internal/client/context';
export const contextPath = path.join(path.dirname(effectsPath), '../context.js');

/** Every hidden Svelte module src/ imports: specifier -> absolute path. */
export const HIDDEN_MODULES = { [EFFECTS_SPECIFIER]: effectsPath, [TASK_SPECIFIER]: taskPath, [CONTEXT_SPECIFIER]: contextPath };

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
        { find: /^svelte\/internal\/client\/dom\/task(\.js)?$/, replacement: taskPath },
        { find: /^svelte\/internal\/client\/context(\.js)?$/, replacement: contextPath },
        // generated components import the runtime from wherever the .svelte file lives
        { find: /^@svelters\/native-runtime$/, replacement: runtimePath },
        // virtualized lists: `import { ListView } from '@svelters/lists'` gets the native
        // implementation (same component API and host protocol, see src/lists.js)
        { find: /^@svelters\/(native-runtime\/)?lists$/, replacement: listsPath },
        // generated non-UI types: `import { File } from 'svelters:gtk/gio'`
        ...typesAliases(),
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

const registerPath = createRequire(import.meta.url).resolve('@svelters/renderer/style');
const STYLE_TAG = /<style(\s[^>]*)?>([\s\S]*?)<\/style>/;

/**
 * The X6 half of a component's `<style>` (ST1): `@native` blocks lifted out, Svelte scopes the
 * rest with `scope` (its own CSS compile, as the stock pipeline), and @svelters/style translates
 * it for `toolkit` without the setter declarations (the Rust compiler matches those itself).
 * Returns null when nothing remains for the toolkit's dialect.
 */
export function nativeSheets(code, file, toolkit, warn = () => {}) {
  const m = STYLE_TAG.exec(code);
  if (!m || /\blang\s*=/.test(m[1] ?? '')) return null;
  const { css, native, errors } = extractNative(m[2]);
  if (errors.length) throw new Error(`${file}: ${errors.join('; ')}`);
  const src = code.slice(0, m.index) + `<style${m[1] ?? ''}>${css}</style>` + code.slice(m.index + m[0].length);
  const scope = `svelte-${createHash('sha256').update(path.relative(repoRoot, file)).digest('hex').slice(0, 10)}`;
  let r;
  try {
    r = svelteCompile(src, { filename: file, css: 'external', cssHash: () => scope, experimental: { customRenderer: 'svelters-native' } });
  } catch (e) {
    // the native compiler reports the component's own problems; only the CSS half is skipped
    warn(`<style> not translated: Svelte could not compile the component for its CSS: ${String(e.message).split('\n')[0]}`);
    return null;
  }
  const t = translateSheet(r.css?.code ?? '', toolkit, { native: native[toolkit] ?? [], setters: false });
  for (const d of t.diagnostics) if (d.level !== 'info') warn(formatDiagnostic(d));
  if (!t.css.trim() && !t.dark.trim()) return null;
  return { scope, sheets: toolkit === 'qt' ? { qt: t.css, qtDark: t.dark } : { [toolkit]: t.css } };
}

/**
 * Vite plugin: compiles `.svelte` modules with the experimental Rust compiler (NC1) for one
 * backend, and `.svelte.js` modules (runes outside components: Svelte's `compileModule`).
 * Diagnostics become Vite errors/warnings with positions; the compiler's source map maps
 * generated code back to the source. `dev: true` keeps `$inspect` / `$inspect.trace` and labels
 * signals (Svelte's `dev` option); otherwise they are stripped.
 */
export function svelteNative({ backend = 'gtk', spec, dev = false } = {}) {
  const bin = compilerPath();
  return {
    name: 'svelters-native-compiler',
    enforce: 'pre',
    transform(code, id) {
      const file = id.split('?')[0];
      const isModule = file.endsWith('.svelte.js');
      if (!isModule && !file.endsWith('.svelte')) return null;
      const args = isModule ? ['--module', '--json', '--filename', file] : ['--backend', backend, '--json', '--filename', file];
      if (spec && !isModule) args.push('--spec', spec);
      // CSS declarations of <style> go to the X6 translator (GTK CSS / QSS), scoped like Svelte's
      const css = isModule ? null : nativeSheets(code, file, backend, (message) => this.warn({ message }));
      if (css) args.push('--css-scope', css.scope);
      if (dev) args.push('--dev');
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
        const more = errors.length > 1 ? `\n${errors.slice(1).map((x) => `  ${path.basename(file)}:${x.line}:${x.column}: ${x.code}: ${x.message}`).join('\n')}` : '';
        this.error({ message: `${d.code}${d.backend ? ` (${d.backend})` : ''}: ${d.message}${d.help ? ` (help: ${d.help})` : ''}${more}`, id, loc: { line: d.line, column: d.column - 1 } });
      }
      let js = res.js;
      if (css) {
        // imports are hoisted: the sheet is registered before the component module body runs
        js += `\nimport { registerStyles as $$registerStyles } from ${JSON.stringify(registerPath)};\n$$registerStyles(${JSON.stringify(path.relative(repoRoot, file))}, ${JSON.stringify(css.sheets)});\n`;
      }
      return { code: js, map: res.map };
    },
  };
}
