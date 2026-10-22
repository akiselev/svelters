// Vite config and plugin for apps compiled by the svelters compiler (svelters-compile) onto
// @svelters/native-runtime: one self-contained ES module (dist/app.js; @svelters/build wraps it for an
// app directory), the aliases that resolve the runtime's modules from any component, and the
// Svelte modules the core adapter (src/core/svelte.js) imports that Svelte's exports map hides.
import path from 'node:path';
import { createRequire } from 'node:module';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import fs from 'node:fs';
import { createHash } from 'node:crypto';
import { compile as svelteCompile, parse as svelteParse } from 'svelte/compiler';
import { extractNative, translateSheet } from '@svelters/style/build';
import { typesAliases } from '@svelters/types/vite';
import { formatDiagnostic } from '@svelters/style/runtime';

/** The specifier src/core/svelte.js imports for block/branch/destroy_effect/component_root. */
export const EFFECTS_SPECIFIER = 'svelte/internal/client/reactivity/effects';

/** Absolute path of this package's runtime entry (`$n` in generated code). */
export const runtimePath = fileURLToPath(new URL('./src/index.js', import.meta.url));

/** The core generated code imports as `$` (`@svelters/native-runtime/core`, RUNTIME.md phase 1). */
export const corePath = fileURLToPath(new URL('./src/core/index.js', import.meta.url));

/** The implementation of `@svelters/lists` (`ListView`, virtualized native lists). */
export const listsPath = fileURLToPath(new URL('./src/lists.js', import.meta.url));

/** EM1: `@svelters/native-runtime/embed` (installs `__svelters_embed`), also `svelters:embed` (`root()`). */
export const embedPath = fileURLToPath(new URL('./src/embed.js', import.meta.url));

/** The X6 styling runtime (`registerStyles`, `setColorScheme`, `setTokens`): `@svelters/native-runtime/style`. */
export const stylePath = fileURLToPath(new URL('./src/style.js', import.meta.url));

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

/** Svelte's `capture` / `unset_context` (an `{#await}` callback renders under its block, as Svelte's). */
export const ASYNC_SPECIFIER = 'svelte/internal/client/reactivity/async';
export const asyncPath = path.join(path.dirname(effectsPath), 'async.js');

/** Svelte's batches: `is_flushing_sync` (a settled `{#await}` flushes unless a flushSync runs). */
export const BATCH_SPECIFIER = 'svelte/internal/client/reactivity/batch';
export const batchPath = path.join(path.dirname(effectsPath), 'batch.js');

/** Svelte's runtime flags (`enable_tracing_mode_flag`, for `@svelters/native-runtime/core/tracing`). */
export const FLAGS_SPECIFIER = 'svelte/internal/flags/index';
export const flagsPath = path.join(path.dirname(effectsPath), '../../flags/index.js');

/** Svelte's DEV array patches (`state_proxy_equality_mismatch`), installed by Svelte's DOM init. */
export const EQUALITY_SPECIFIER = 'svelte/internal/client/dev/equality';
export const equalityPath = path.join(path.dirname(effectsPath), '../dev/equality.js');

/** Svelte's `loop` (TR1: the animation player's frame task). */
export const LOOP_SPECIFIER = 'svelte/internal/client/loop';
export const loopPath = path.join(path.dirname(effectsPath), '../loop.js');

/** Svelte's `set_should_intro` (TR1: `mount`'s `intro` option). */
export const RENDER_SPECIFIER = 'svelte/internal/client/render';
export const renderPath = path.join(path.dirname(effectsPath), '../render.js');

/** Svelte's client entry (what `svelte` resolves to with the browser condition), for src/svelte.js. */
export const SVELTE_CLIENT_SPECIFIER = 'svelte/index-client';
export const svelteClientPath = path.join(path.dirname(effectsPath), '../../../index-client.js');

/** `svelte` as app code sees it: Svelte's client API with a native `mount`/`unmount` (src/svelte.js). */
export const svelteFacadePath = fileURLToPath(new URL('./src/svelte.js', import.meta.url));

/** Every hidden Svelte module the runtime imports (src/core/svelte.js, src/svelte.js): specifier -> absolute path. */
export const HIDDEN_MODULES = {
  [EFFECTS_SPECIFIER]: effectsPath,
  [TASK_SPECIFIER]: taskPath,
  [CONTEXT_SPECIFIER]: contextPath,
  [ASYNC_SPECIFIER]: asyncPath,
  [BATCH_SPECIFIER]: batchPath,
  [FLAGS_SPECIFIER]: flagsPath,
  [SVELTE_CLIENT_SPECIFIER]: svelteClientPath,
  [EQUALITY_SPECIFIER]: equalityPath,
  [LOOP_SPECIFIER]: loopPath,
  [RENDER_SPECIFIER]: renderPath,
};

/** TR1: `svelte/transition` and `svelte/animate` as app code sees them: native ports of Svelte's built-ins. */
export const transitionPath = fileURLToPath(new URL('./src/transition.js', import.meta.url));
export const animatePath = fileURLToPath(new URL('./src/animate.js', import.meta.url));

/** What generated code imports when a component uses `$inspect.trace` in dev. */
export const tracingPath = fileURLToPath(new URL('./src/core/tracing.js', import.meta.url));

const escapeRegExp = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

/**
 * Vite config for one app: `entry` (relative to `root`) -> `<root>/<outDir>/app.js`, es2022, not
 * minified, with a source map, resolved with the `browser` condition (Svelte's client build).
 */
export function nativeConfig(root, { entry = 'main.js', outDir = 'dist', plugins = [] } = {}) {
  return {
    root,
    plugins,
    configFile: false,
    logLevel: 'warn',
    resolve: {
      conditions: ['browser'],
      alias: [
        // the hidden Svelte modules of the core adapter
        ...Object.entries(HIDDEN_MODULES).map(([find, replacement]) => ({ find: new RegExp(`^${escapeRegExp(find)}(\\.js)?$`), replacement })),
        // `import { mount, flushSync } from 'svelte'`: Svelte's client API, mount/unmount native
        { find: /^svelte$/, replacement: svelteFacadePath },
        // TR1: Svelte's built-in transitions read the DOM (getComputedStyle); the native ports read
        // the runtime node's visuals and the host's geometry
        { find: /^svelte\/transition$/, replacement: transitionPath },
        { find: /^svelte\/animate$/, replacement: animatePath },
        // generated components import the runtime from wherever the .svelte file lives
        { find: /^@svelters\/native-runtime$/, replacement: runtimePath },
        { find: /^@svelters\/native-runtime\/core$/, replacement: corePath },
        { find: /^@svelters\/native-runtime\/core\/tracing$/, replacement: tracingPath },
        // the styling API (`setColorScheme`, `setTokens`), also for apps that do not depend on it
        { find: /^@svelters\/native-runtime\/style$/, replacement: registerPath },
        // EM1: the embed entry and `import { root } from 'svelters:embed'` (one module instance)
        { find: /^(@svelters\/native-runtime\/embed|svelters:embed)$/, replacement: embedPath },
        // virtualized lists: `import { ListView } from '@svelters/lists'` gets the native
        // implementation (same component API and host protocol, see src/lists.js)
        { find: /^@svelters\/(native-runtime\/)?lists$/, replacement: listsPath },
        // generated non-UI types: `import { File } from 'svelters:gtk/gio'`
        ...typesAliases(),
        // the app's Rust exports (E0): `import { Simulation } from 'svelters:app/sim'` gets the stub the
        // app binary dumped into <root>/.svelters/app (svelters::exports::dump); the QuickJS host also
        // serves the same module itself when a bundle leaves the import external
        { find: /^svelters:app\/([a-z0-9_-]+)$/, replacement: path.join(path.resolve(root), '.svelters/app/$1.js') },
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
  const exe = process.platform === 'win32' ? '.exe' : '';
  const found = [`target/release/svelters-compile${exe}`, `target/debug/svelters-compile${exe}`]
    .map((p) => path.join(repoRoot, p))
    .filter((p) => fs.existsSync(p))
    .sort((a, b) => fs.statSync(b).mtimeMs - fs.statSync(a).mtimeMs);
  if (!found.length) throw new Error('svelters-compile not found: run `cargo build -p svelters-compiler` or set SVELTERS_COMPILE');
  return found[0];
}

const registerPath = stylePath;

/** What a component that resolves attributes at run time imports (spreads, `<svelte:element>`). */
export const KINDS_SPECIFIER = '@svelters/native-runtime/kinds';
const KINDS_ID = '\0svelters-kinds';

/**
 * The runtime spec tables for one backend (or spec file): `svelters-compile --kinds`, the module
 * `@svelters/native-runtime/kinds` resolves to (docs/notes/native-compiler/runtime-contract.md
 * "Spread attributes").
 */
export function kindsModule({ backend = 'gtk', spec } = {}) {
  const args = ['--kinds', '--backend', backend];
  if (spec) args.push('--spec', spec);
  return execFileSync(compilerPath(), args, { encoding: 'utf8', stdio: ['ignore', 'pipe', 'pipe'] });
}
const STYLE_TAG = /<style(\s[^>]*)?>([\s\S]*?)<\/style>/;

/**
 * `src` with every `bind:` directive blanked (same length, so positions hold). Stock Svelte only
 * knows the DOM bindings (`bind:text` on a native element is `bind_invalid_name`); a binding never
 * changes which elements a selector matches, so the CSS of the blanked component is the same.
 */
export function withoutBindings(src) {
  let ast;
  try {
    ast = svelteParse(src, { modern: true });
  } catch {
    return src;
  }
  const ranges = [];
  const walk = (n) => {
    if (Array.isArray(n)) for (const x of n) walk(x);
    else if (n && typeof n === 'object') {
      if (n.type === 'BindDirective' && typeof n.start === 'number') ranges.push([n.start, n.end]);
      for (const [k, v] of Object.entries(n)) if (k !== 'instance' && k !== 'module' && k !== 'css') walk(v);
    }
  };
  walk(ast.fragment);
  let out = src;
  for (const [a, b] of ranges.sort((x, y) => y[0] - x[0])) out = out.slice(0, a) + ' '.repeat(b - a) + out.slice(b);
  return out;
}

/**
 * The X6 half of a component's `<style>` (ST1): `@native` blocks lifted out, stock Svelte scopes
 * the rest with `scope` (a CSS-only compile of the component: `css: 'external'`, its bindings
 * blanked, see withoutBindings), and @svelters/style translates it for `toolkit` without the
 * setter declarations (the Rust compiler matches those itself). Returns null when nothing remains
 * for the toolkit's dialect.
 */
const tagCache = new Map();
/** The element tags of a backend (metadata/semantics/<backend>.json), for selector checks. */
function tagsOf(backend) {
  if (!tagCache.has(backend)) {
    let tags = null;
    try {
      tags = Object.keys(JSON.parse(fs.readFileSync(path.join(repoRoot, "metadata/semantics", `${backend}.json`), "utf8")).tags ?? {});
    } catch {
      tags = null;
    }
    tagCache.set(backend, tags);
  }
  return tagCache.get(backend);
}

export function nativeSheets(code, file, toolkit, warn = () => {}) {
  const m = STYLE_TAG.exec(code);
  if (!m || /\blang\s*=/.test(m[1] ?? '')) return null;
  const { css, native, errors } = extractNative(m[2]);
  if (errors.length) throw new Error(`${file}: ${errors.join('; ')}`);
  const src = code.slice(0, m.index) + `<style${m[1] ?? ''}>${css}</style>` + code.slice(m.index + m[0].length);
  const scope = `svelte-${createHash('sha256').update(path.relative(repoRoot, file)).digest('hex').slice(0, 10)}`;
  let r;
  try {
    r = svelteCompile(withoutBindings(src), { filename: file, css: 'external', cssHash: () => scope });
  } catch (e) {
    // the native compiler reports the component's own problems; only the CSS half is skipped
    warn(`<style> not translated: Svelte could not compile the component for its CSS: ${String(e.message).split('\n')[0]}`);
    return null;
  }
  const t = translateSheet(r.css?.code ?? '', toolkit, { native: native[toolkit] ?? [], setters: false, tags: tagsOf(toolkit) });
  for (const d of t.diagnostics) if (d.level !== 'info') warn(formatDiagnostic(d));
  // decl dialect (AppKit): structured rules for the runtime cascade engine
  if (t.declRules) return t.declRules.length ? { scope, sheets: { decl: t.declRules } } : null;
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
    // the spec tables of attributes resolved at run time (spreads, <svelte:element>): generated by the
    // compiler from the same spec, imported only by components that need them
    resolveId(id) {
      return id === KINDS_SPECIFIER ? KINDS_ID : null;
    },
    load(id) {
      return id === KINDS_ID ? kindsModule({ backend, spec }) : null;
    },
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
