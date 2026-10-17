// Vite plugin that takes component <style> blocks out of the Svelte compile as data.
//
// vite-plugin-svelte compiles with `css: 'external'` and makes each component import a virtual
// stylesheet module (`App.svelte?svelte&type=style&lang.css`). A browser build would hand that to
// the CSS pipeline and a <style> tag. Here the module becomes JavaScript instead:
//
//   registerStyles("src/App.svelte", { gtk, qt, qtDark });
//
// with the stylesheet translated for every toolkit (@svelters/style). Svelte's scoping hash is
// already in the `class` of the component's elements, so the renderer only forwards classes.
// `@native gtk|qt { }` blocks are lifted out before the compile (Svelte would scope them and
// prune the toolkit's own selectors as unused) and appended verbatim to their toolkit's sheet.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { DEFAULT_TOKEN_NAMES, formatDiagnostic } from '@svelters/style/runtime';
import { extractNative, translateAll } from '@svelters/style/build';

const STYLE_ID = /\?svelte&type=style&lang\.css$/;
const PREFIX = '\0svelters-style:';
const registerPath = fileURLToPath(import.meta.resolve('@svelters/renderer/style'));

/**
 * @param {{ strict?: boolean, verbose?: boolean, tokens?: string[] }} [options]
 *   strict: diagnostics of level warning and error fail the build. verbose: also print `info`.
 *   tokens: extra `--sv-*` token names the app defines with setTokens().
 */
export function stylePlugin(options = {}) {
  const strict = options.strict ?? process.env.SVELTERS_STYLE_STRICT === '1';
  const verbose = options.verbose ?? process.env.SVELTERS_STYLE_VERBOSE === '1';
  /** filename -> { gtk: string[], qt: string[] } lifted out by the preprocessor */
  const natives = new Map();
  let root = process.cwd();
  const ids = new Map();
  const originals = new Map();

  const locate = (filename, cssText, offset) => {
    let line = 1;
    try {
      const src = fs.readFileSync(filename, 'utf8');
      const tag = src.search(/<style[\s>]/);
      if (tag >= 0) line = src.slice(0, tag).split('\n').length;
    } catch {
      // no source: report relative to the stylesheet
    }
    if (offset === undefined) return { line };
    const before = cssText.slice(0, offset).split('\n');
    return { line: line + before.length - 1, column: before.at(-1).length };
  };

  /** The preprocessor half: runs inside vite-plugin-svelte before the compile. */
  const preprocessor = {
    name: 'svelters-native-blocks',
    style({ content, filename, attributes }) {
      if (!filename || attributes.lang) return undefined;
      const { css, native, errors } = extractNative(content);
      natives.set(filename, native);
      if (errors.length) throw new Error(`${filename}: ${errors.join('; ')}`);
      return { code: css };
    },
  };

  const plugin = {
    name: 'svelters:style',
    enforce: 'pre',
    configResolved(c) {
      root = c.root;
    },
    resolveId(id) {
      if (!STYLE_ID.test(id)) return undefined;
      // The virtual id must not end in .css (Vite would run it through the CSS pipeline) nor
      // contain .svelte (vite-plugin-svelte would compile it), so it is just a number.
      let n = ids.get(id);
      if (n === undefined) {
        n = ids.size;
        ids.set(id, n);
        originals.set(n, id);
      }
      return `${PREFIX}${n}.js`;
    },
    async load(id) {
      if (!id.startsWith(PREFIX)) return undefined;
      const original = originals.get(Number(id.slice(PREFIX.length, -'.js'.length)));
      const filename = original.split('?')[0];
      let info = this.getModuleInfo(filename)?.meta.svelte?.css;
      if (!info) {
        const resolved = await this.resolve(filename);
        if (resolved?.id) info = this.getModuleInfo(resolved.id)?.meta.svelte?.css;
      }
      const cssText = info?.code ?? '';
      if (!cssText.trim()) return { code: 'export {};', moduleSideEffects: false };

      const { sheets, diagnostics } = translateAll(cssText, {
        native: natives.get(filename) ?? { gtk: [], qt: [] },
        knownTokens: options.tokens ? [...DEFAULT_TOKEN_NAMES, ...options.tokens] : undefined,
      });
      const seen = new Set();
      const rel = path.relative(root, filename);
      for (const d of diagnostics) {
        if (d.level === 'info' && !verbose) continue;
        const key = `${d.toolkit}|${d.message}|${d.offset}`;
        if (seen.has(key)) continue;
        seen.add(key);
        const loc = locate(filename, cssText, d.offset);
        const payload = { message: formatDiagnostic(d), id: filename, loc: { file: filename, ...loc }, plugin: 'svelters:style' };
        if (strict && d.level !== 'info') this.error(payload);
        else this.warn(payload);
      }
      const code =
        `import { registerStyles } from ${JSON.stringify(registerPath)};\n` +
        `registerStyles(${JSON.stringify(rel)}, ${JSON.stringify(sheets)});\n`;
      return { code, moduleSideEffects: true };
    },
  };
  return { plugin, preprocessor };
}
