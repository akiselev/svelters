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
import { formatDiagnostic } from '@svelters/style/runtime';

/**
 * The runtime's alias table (`"svelters": { "aliases" }` in this package's package.json), shared with
 * the Rust module loader (crates/svelters-modules) so the two cannot drift. A key is a specifier
 * (matched with or without `.js`) or a pattern with one `*` (one path segment); a value is `./<path>`
 * in this package or `<package>/<path>`, a file inside a dependency found through node_modules
 * without its exports map (Svelte hides the internal modules the core adapter imports).
 */
export const ALIAS_TABLE = JSON.parse(fs.readFileSync(new URL('./package.json', import.meta.url), 'utf8')).svelters.aliases;

const here = path.dirname(fileURLToPath(import.meta.url));

/** The directory of package `name` as Node finds it from `from` (the first `node_modules/<name>` up). */
function packageDir(from, name) {
  for (let dir = from; ; dir = path.dirname(dir)) {
    const candidate = path.join(dir, 'node_modules', name);
    if (fs.existsSync(candidate)) return fs.realpathSync(candidate);
    if (path.dirname(dir) === dir) throw new Error(`@svelters/native-runtime: alias package ${name} is not installed (pnpm install)`);
  }
}

/** An alias table value as an absolute path (a pattern's `*` is kept). */
function aliasTarget(value) {
  if (value.startsWith('./')) return path.join(here, value.slice(2));
  const n = value.startsWith('@') ? 2 : 1;
  const parts = value.split('/');
  return path.join(packageDir(here, parts.slice(0, n).join('/')), ...parts.slice(n));
}

/** Every alias, resolved: specifier (or pattern) -> absolute path. */
export const ALIASES = Object.fromEntries(Object.entries(ALIAS_TABLE).map(([k, v]) => [k, aliasTarget(v)]));

/** The specifier src/core/svelte.js imports for block/branch/destroy_effect/component_root. */
export const EFFECTS_SPECIFIER = 'svelte/internal/client/reactivity/effects';

/** Absolute path of this package's runtime entry (`$n` in generated code). */
export const runtimePath = ALIASES['@svelters/native-runtime'];

/** The core generated code imports as `$` (`@svelters/native-runtime/core`, RUNTIME.md phase 1). */
export const corePath = ALIASES['@svelters/native-runtime/core'];

/** The implementation of `@svelters/lists` (`ListView`, virtualized native lists). */
export const listsPath = ALIASES['@svelters/lists'];

/** EM1: `@svelters/native-runtime/embed` (installs `__svelters_embed`), also `svelters:embed` (`root()`). */
export const embedPath = ALIASES['@svelters/native-runtime/embed'];

/** The X6 styling runtime (`registerStyles`, `setColorScheme`, `setTokens`): `@svelters/native-runtime/style`. */
export const stylePath = ALIASES['@svelters/native-runtime/style'];

/** Absolute path of Svelte's effects module (hidden by the package's exports map). */
export const effectsPath = ALIASES[EFFECTS_SPECIFIER];

/** Svelte's microtask queue (`queue_micro_task`, drained by `flushSync`), used by `boundary`. */
export const TASK_SPECIFIER = 'svelte/internal/client/dom/task';
export const taskPath = ALIASES[TASK_SPECIFIER];

/** Svelte's component context (`getAllContexts` / `setContext` for list rows, src/lists.js). */
export const CONTEXT_SPECIFIER = 'svelte/internal/client/context';
export const contextPath = ALIASES[CONTEXT_SPECIFIER];

/** Svelte's `capture` / `unset_context` (an `{#await}` callback renders under its block, as Svelte's). */
export const ASYNC_SPECIFIER = 'svelte/internal/client/reactivity/async';
export const asyncPath = ALIASES[ASYNC_SPECIFIER];

/** Svelte's batches: `is_flushing_sync` (a settled `{#await}` flushes unless a flushSync runs). */
export const BATCH_SPECIFIER = 'svelte/internal/client/reactivity/batch';
export const batchPath = ALIASES[BATCH_SPECIFIER];

/** Svelte's reaction state setters (FX6: `recover` after an aborted entry). */
export const RUNTIME_SPECIFIER = 'svelte/internal/client/runtime';
export const runtimeStatePath = ALIASES[RUNTIME_SPECIFIER];

/** Svelte's runtime flags (`enable_tracing_mode_flag`, for `@svelters/native-runtime/core/tracing`). */
export const FLAGS_SPECIFIER = 'svelte/internal/flags/index';
export const flagsPath = ALIASES[FLAGS_SPECIFIER];

/** Svelte's DEV array patches (`state_proxy_equality_mismatch`), installed by Svelte's DOM init. */
export const EQUALITY_SPECIFIER = 'svelte/internal/client/dev/equality';
export const equalityPath = ALIASES[EQUALITY_SPECIFIER];

/** Svelte's `loop` (TR1: the animation player's frame task). */
export const LOOP_SPECIFIER = 'svelte/internal/client/loop';
export const loopPath = ALIASES[LOOP_SPECIFIER];

/** Svelte's `set_should_intro` (TR1: `mount`'s `intro` option). */
export const RENDER_SPECIFIER = 'svelte/internal/client/render';
export const renderPath = ALIASES[RENDER_SPECIFIER];

/** Svelte's runtime constants (RT1: `STATE_SYMBOL`, what marks a `$state` proxy). */
export const CONSTANTS_SPECIFIER = 'svelte/internal/client/constants';
export const constantsPath = ALIASES[CONSTANTS_SPECIFIER];

/** Svelte's client entry (what `svelte` resolves to with the browser condition), for src/svelte.js. */
export const SVELTE_CLIENT_SPECIFIER = 'svelte/index-client';
export const svelteClientPath = ALIASES[SVELTE_CLIENT_SPECIFIER];

/** `svelte` as app code sees it: Svelte's client API with a native `mount`/`unmount` (src/svelte.js). */
export const svelteFacadePath = ALIASES.svelte;

/** Every hidden Svelte module the runtime imports (src/core/svelte.js, src/svelte.js): specifier -> absolute path. */
export const HIDDEN_MODULES = {
  [EFFECTS_SPECIFIER]: effectsPath,
  [TASK_SPECIFIER]: taskPath,
  [CONTEXT_SPECIFIER]: contextPath,
  [ASYNC_SPECIFIER]: asyncPath,
  [BATCH_SPECIFIER]: batchPath,
  [RUNTIME_SPECIFIER]: runtimeStatePath,
  [FLAGS_SPECIFIER]: flagsPath,
  [SVELTE_CLIENT_SPECIFIER]: svelteClientPath,
  [EQUALITY_SPECIFIER]: equalityPath,
  [LOOP_SPECIFIER]: loopPath,
  [RENDER_SPECIFIER]: renderPath,
  [CONSTANTS_SPECIFIER]: constantsPath,
};

/** TR1: `svelte/transition` and `svelte/animate` as app code sees them: native ports of Svelte's built-ins. */
export const transitionPath = ALIASES['svelte/transition'];
export const animatePath = ALIASES['svelte/animate'];

/** What generated code imports when a component uses `$inspect.trace` in dev. */
export const tracingPath = ALIASES['@svelters/native-runtime/core/tracing'];

const escapeRegExp = (s) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');

/** `resolve.alias` entries for the alias table: exact keys with an optional `.js`, `*` as one segment. */
export function runtimeAliases() {
  return Object.entries(ALIASES).map(([key, target]) => {
    if (!key.includes('*')) return { find: new RegExp(`^${escapeRegExp(key)}(\\.js)?$`), replacement: target };
    const [prefix, suffix] = key.split('*');
    return { find: new RegExp(`^${escapeRegExp(prefix)}([^/]+)${escapeRegExp(suffix)}$`), replacement: target.replace('*', '$1') };
  });
}

/**
 * Vite config for one app: `entry` (relative to `root`) -> `<root>/<outDir>/app.js`, es2022, not
 * minified, with a source map, resolved with the `browser` condition (Svelte's client build).
 */
export function nativeConfig(root, { entry = 'main.js', outDir = 'dist', plugins = [] } = {}) {
  return {
    root,
    plugins: [sveltersAssets(root), ...plugins],
    configFile: false,
    logLevel: 'warn',
    resolve: {
      conditions: ['browser'],
      alias: [
        // the runtime's alias table (package.json): Svelte's hidden modules for the core adapter, the
        // `svelte` facade with a native mount, the native transition ports, the runtime's own entry
        // points (generated components import them from wherever the .svelte file lives), the embed
        // entry, the native lists and the generated non-UI types (`svelters:gtk/gio`)
        ...runtimeAliases(),
        // the app's Rust exports (E0): `import { Simulation } from 'svelters:app/sim'` gets the stub the
        // app binary dumped into <root>/.svelters/app (svelters::exports::dump); the QuickJS host also
        // serves the same module itself when a bundle leaves the import external
        { find: /^svelters:app\/([a-z0-9_-]+)$/, replacement: path.join(path.resolve(root), '.svelters/app/$1.js') },
        // EV2b: the app's native behaviours (`#[svelters::behavior]`): `import { pick } from
        // 'svelters:behaviors/cad'` gets the module of Svelte actions the app binary dumped
        // (svelters_egui::dump: .svelters/app/behaviors/<ns>.js)
        { find: /^svelters:behaviors\/([a-z0-9_-]+)$/, replacement: path.join(path.resolve(root), '.svelters/app/behaviors/$1.js') },
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

/** AS1: the asset module, served by the host (`svelters:assets`); a bundle leaves it external. */
export const ASSETS_ID = 'svelters:assets';

// The files that are code, not assets (the Rust loader's rule, `transform.rs` `Kind::of`).
const CODE_FILE = /\.(svelte|m?js|cjs|json|tsx?|mts|css)$/;
const VIRTUAL = '\0svelters-asset:';

/**
 * AS1 (plans/research/ASSETS.md §1.4): asset imports give the same value as under the Rust module
 * loader. `import icon from './icon.svg'` (and `?url`) is the `Asset` handle of
 * `asset://app/<logical path>`; `?bytes` is a `Uint8Array` read through the host; `?raw` and
 * `?inline` stay Vite's (the same values). The logical path is relative to `root`, or
 * `node_modules/<package name>/<path in package>` for a dependency's file. `vite build` copies every
 * referenced asset to `<outDir>/assets/<logical path>`, which the runner mounts as root `app`.
 */
export function sveltersAssets(root) {
  const appRoot = fs.realpathSync(path.resolve(root));
  const files = new Map(); // logical path -> file
  const logical = (file) => {
    const real = fs.realpathSync(file);
    const rel = path.relative(appRoot, real);
    if (!rel.startsWith('..') && !path.isAbsolute(rel)) return rel.split(path.sep).join('/');
    for (let d = path.dirname(real); d !== path.dirname(d); d = path.dirname(d)) {
      const pj = path.join(d, 'package.json');
      if (fs.existsSync(pj)) {
        const name = JSON.parse(fs.readFileSync(pj, 'utf8')).name;
        if (name) return `node_modules/${name}/${path.relative(d, real).split(path.sep).join('/')}`;
      }
    }
    throw new Error(`${file}: outside the app root and in no package`);
  };
  return {
    name: 'svelters:assets',
    enforce: 'pre',
    async resolveId(source, importer, opts) {
      if (source === ASSETS_ID) return { id: ASSETS_ID, external: true };
      if (source.startsWith(VIRTUAL)) return source;
      const [spec, query] = source.split('?', 2);
      if (query !== undefined && query !== 'url' && query !== 'bytes') return null;
      if (query === undefined && CODE_FILE.test(spec)) return null;
      if (query === undefined && opts?.attributes?.type !== 'bytes' && !/\.[A-Za-z0-9]+$/.test(spec)) return null;
      const r = await this.resolve(spec, importer, { ...opts, skipSelf: true });
      if (!r || r.external || r.id.startsWith('\0')) return null;
      const file = r.id.split('?')[0];
      if (query === undefined && CODE_FILE.test(file) && opts?.attributes?.type !== 'bytes') return null;
      const kind = query === 'bytes' || opts?.attributes?.type === 'bytes' ? 'bytes' : 'url';
      return `${VIRTUAL}${kind}:${file}`;
    },
    load(id) {
      if (!id.startsWith(VIRTUAL)) return null;
      const rest = id.slice(VIRTUAL.length);
      const kind = rest.slice(0, rest.indexOf(':'));
      const file = rest.slice(kind.length + 1);
      const lp = logical(file);
      files.set(lp, file);
      this.addWatchFile(file);
      const ref = JSON.stringify(`asset://app/${lp}`);
      return kind === 'bytes'
        ? `import { bytes } from '${ASSETS_ID}';\nexport default bytes(${ref});\n`
        : `import { asset } from '${ASSETS_ID}';\nexport default asset(${ref});\n`;
    },
    generateBundle() {
      for (const [lp, file] of files) this.emitFile({ type: 'asset', fileName: `assets/${lp}`, source: fs.readFileSync(file) });
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
export function kindsModule({ backend = 'gtk', spec, specFragments = [] } = {}) {
  const args = ['--kinds', '--backend', backend];
  if (spec) args.push('--spec', spec);
  for (const f of specFragments) args.push('--spec-fragment', f);
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
/**
 * The element tags of a backend (metadata/semantics/<backend>.json) plus those of its spec fragments
 * (RA1 provider kinds), for selector checks.
 */
function tagsOf(backend, specFragments = []) {
  const key = [backend, ...specFragments].join('\0');
  if (!tagCache.has(key)) {
    let tags = null;
    try {
      tags = Object.keys(JSON.parse(fs.readFileSync(path.join(repoRoot, "metadata/semantics", `${backend}.json`), "utf8")).tags ?? {});
      for (const f of specFragments) tags.push(...Object.keys(JSON.parse(fs.readFileSync(f, 'utf8')).tags ?? {}));
    } catch {
      tags = null;
    }
    tagCache.set(key, tags);
  }
  return tagCache.get(key);
}

/** The component's `<style>` match, null without one or for `<style lang=...>` (left alone). */
function styleBlock(code) {
  const m = STYLE_TAG.exec(code);
  return !m || /\blang\s*=/.test(m[1] ?? '') ? null : m;
}

/** The scope class of a component's sheets: a hash of its path in the repository. */
function scopeOf(file) {
  return `svelte-${createHash('sha256').update(path.relative(repoRoot, file)).digest('hex').slice(0, 10)}`;
}

/** The `@native` blocks of a `<style>` body (`extractNative`), throwing on a malformed one. */
function nativeBlocks(styleText, file) {
  const r = extractNative(styleText);
  if (r.errors.length) throw new Error(`${file}: ${r.errors.join('; ')}`);
  return r;
}

export function nativeSheets(code, file, toolkit, warn = () => {}, specFragments = []) {
  const m = styleBlock(code);
  if (!m) return null;
  const { css, native } = nativeBlocks(m[2], file);
  const src = code.slice(0, m.index) + `<style${m[1] ?? ''}>${css}</style>` + code.slice(m.index + m[0].length);
  const scope = scopeOf(file);
  let r;
  try {
    r = svelteCompile(withoutBindings(src), { filename: file, css: 'external', cssHash: () => scope });
  } catch (e) {
    // the native compiler reports the component's own problems; only the CSS half is skipped
    warn(`<style> not translated: Svelte could not compile the component for its CSS: ${String(e.message).split('\n')[0]}`);
    return null;
  }
  // egui runs the Rust CSS engine in process (CS5e): it gets the scoped CSS as Svelte wrote it
  if (toolkit === 'egui') return r.css?.code?.trim() ? { scope, sheets: { css: r.css.code } } : null;
  return translatedSheets(r.css?.code ?? '', scope, native, toolkit, warn, specFragments);
}

/**
 * The X6 sheets of a stylesheet already scoped with `scope` (by stock Svelte in nativeSheets, or by the
 * svelters compiler's port of Svelte's CSS pipeline, `svelters-compile --css`): @svelters/style
 * translates it for `toolkit` without the setter declarations, plus the toolkit's `@native` blocks.
 * Null when nothing remains for the toolkit's dialect.
 */
export function translatedSheets(scopedCss, scope, native, toolkit, warn = () => {}, specFragments = []) {
  const t = translateSheet(scopedCss, toolkit, { native: native[toolkit] ?? [], setters: false, tags: tagsOf(toolkit, specFragments) });
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
 * signals (Svelte's `dev` option); otherwise they are stripped. `specFragments` (RA1): spec fragments
 * merged into the backend's spec (`svelters-compile --spec-fragment`), such as the `kinds.json` an
 * egui app binary dumps for its Rust-provided kinds. `utilities` (U4, egui): check utility classes,
 * `true` or `{ prefix, strictTokens, scanScript }` like the embed's `EmbedConfig::utilities`
 * (`svelters-compile --utilities ...`).
 */
export function svelteNative({ backend = 'gtk', spec, specFragments = [], dev = false, utilities = false } = {}) {
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
      return id === KINDS_ID ? kindsModule({ backend, spec, specFragments }) : null;
    },
    transform(code, id) {
      const file = id.split('?')[0];
      const isModule = file.endsWith('.svelte.js');
      if (!isModule && !file.endsWith('.svelte')) return null;
      const args = isModule ? ['--module', '--json', '--filename', file] : ['--backend', backend, '--json', '--filename', file];
      if (spec && !isModule) args.push('--spec', spec);
      if (!isModule) for (const f of specFragments) args.push('--spec-fragment', f);
      if (dev) args.push('--dev');
      if (utilities && !isModule) {
        args.push('--utilities');
        if (utilities.prefix) args.push('--utilities-prefix', utilities.prefix);
        if (utilities.strictTokens) args.push('--utilities-strict');
        if (utilities.scanScript) args.push('--utilities-scan-script');
      }
      const warn = (message) => this.warn({ message });
      const run = (extra) => {
        let out;
        try {
          out = execFileSync(bin, [...args, ...extra, '-'], { input: code, encoding: 'utf8', stdio: ['pipe', 'pipe', 'pipe'] });
        } catch (e) {
          out = e.stdout;
          if (!out) throw new Error(`svelters-compile failed: ${e.stderr || e.message}`);
        }
        return JSON.parse(out);
      };
      // CSS declarations of <style> go to the X6 translator (GTK CSS / QSS / decl), scoped like Svelte's.
      // Q3w: the svelters compiler scopes the <style> itself (`--css`, the Rust port of Svelte's CSS
      // pipeline, `@native` blocks lifted out); egui still takes stock Svelte's scoped CSS (CS5e).
      const style = isModule ? null : styleBlock(code);
      let css = null;
      let res;
      if (style && backend !== 'egui') {
        const { native } = nativeBlocks(style[2], file);
        const scope = scopeOf(file);
        res = run(['--css', '--css-scope', scope]);
        if (res.js != null) {
          css = res.css && !res.css.injected ? translatedSheets(res.css.code, scope, native, backend, warn, specFragments) : null;
          // nothing for the toolkit (only setter declarations, say): no scope class on the elements, as before
          if (!css) {
            const plain = run([]);
            res = { ...res, js: plain.js, map: plain.map };
          }
        }
      } else if (style && /@apply\s/.test(style[2])) {
        // U4: egui with `@apply`: the svelters compiler expands it before scoping (stock Svelte can't)
        const scope = scopeOf(file);
        res = run(['--css', '--css-scope', scope]);
        // DZ0b2 (dev): the component's path and the sheet's map to its `<style>` text, as the Rust
        // loader passes them, for matched-rule source offsets. Stock Svelte's sheets (below) have no map.
        const source = dev && res.css?.map != null ? { file, map: res.css.map } : {};
        css = res.css && !res.css.injected && res.css.code.trim() ? { scope, sheets: { css: res.css.code, ...source } } : null;
      } else {
        css = style ? nativeSheets(code, file, backend, warn, specFragments) : null;
        res = run(css ? ['--css-scope', css.scope] : []);
      }
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
