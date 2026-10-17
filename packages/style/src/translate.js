// Build-time translation of a component's (Svelte-scoped) stylesheet to GTK CSS and QSS.
//
// Input is `compile(..., { css: 'external' }).css.code`: class and element selectors carrying
// Svelte's scope hash, `:where(.svelte-h)` on later compounds, native-looking nesting.
// Output per toolkit is a stylesheet the toolkit is known to parse, plus diagnostics for
// everything that was dropped. For Qt this matters twice: QSS stops parsing at the first
// construct it does not understand, so the translator only ever emits constructs from a closed
// set (docs/notes/styling-gtk-css-qss.md section 2.2).
import { parseCss } from 'svelte/compiler';
import { diag, parseDeclarations, translateDecls } from './decls.js';
import { KINDS } from './kinds.generated.js';
import { DEFAULT_TOKEN_NAMES, PSEUDO } from './vocab.js';

export { KINDS };

const flatSelector = (children) => (children.length === 1 && children[0].children.length === 1 ? children[0].children[0] : null);

/**
 * Pulls `@native <toolkit> { ... }` blocks out of a stylesheet source. Svelte would scope them
 * and prune the toolkit's own selectors as unused, so they are removed before the compile and
 * handed back verbatim, per toolkit.
 * @returns {{ css: string, native: { gtk: string[], qt: string[] }, errors: string[] }}
 */
export function extractNative(source) {
  const native = { gtk: [], qt: [] };
  const errors = [];
  let out = '';
  let i = 0;
  const re = /@native\s+([a-z]+)\s*\{/g;
  for (;;) {
    re.lastIndex = i;
    const m = re.exec(source);
    if (!m) {
      out += source.slice(i);
      break;
    }
    // skip matches inside comments
    const lastOpen = source.lastIndexOf('/*', m.index);
    if (lastOpen >= i && source.indexOf('*/', lastOpen) > m.index) {
      out += source.slice(i, m.index + m[0].length);
      i = m.index + m[0].length;
      continue;
    }
    out += source.slice(i, m.index);
    let depth = 1;
    let k = m.index + m[0].length;
    let quote = '';
    while (k < source.length && depth > 0) {
      const ch = source[k];
      if (quote) {
        if (ch === '\\') k++;
        else if (ch === quote) quote = '';
      } else if (ch === '"' || ch === "'") quote = ch;
      else if (ch === '/' && source[k + 1] === '*') {
        const e = source.indexOf('*/', k + 2);
        k = e < 0 ? source.length : e + 1;
      } else if (ch === '{') depth++;
      else if (ch === '}') depth--;
      k++;
    }
    if (depth > 0) {
      errors.push(`@native ${m[1]} block is not closed`);
      i = source.length;
      break;
    }
    const body = source.slice(m.index + m[0].length, k - 1).trim();
    if (m[1] in native) native[m[1]].push(body);
    else errors.push(`@native ${m[1]}: unknown toolkit (use gtk or qt)`);
    // keep line numbers stable
    out += source.slice(m.index, k).replace(/[^\n]/g, '');
    i = k;
  }
  return { css: out, native, errors };
}

/**
 * Translates one stylesheet for one toolkit.
 * @param {string} cssText
 * @param {'gtk' | 'qt'} toolkit
 * @param {{ native?: string[], knownTokens?: Iterable<string>, kinds?: typeof KINDS }} [opts]
 * @returns {{ css: string, dark: string, diagnostics: import('./decls.js').Diagnostic[] }}
 */
export function translateSheet(cssText, toolkit, opts = {}) {
  const kinds = opts.kinds ?? KINDS;
  const knownTokens = new Set(opts.knownTokens ?? DEFAULT_TOKEN_NAMES);
  const diagnostics = [];
  const dg = (level, message, node) => diagnostics.push(diag(level, toolkit, message, node?.start));
  const ast = parseCss(cssText);
  const base = [];
  const dark = [];

  /** @returns {{ text: string, qtClass: string | null } | null} */
  function compound(rel, ctx) {
    let text = '';
    let qtClass = null;
    for (const s of rel.selectors) {
      switch (s.type) {
        case 'TypeSelector': {
          if (s.name === '*') {
            text += '*';
            break;
          }
          const k = kinds[s.name];
          if (!k) {
            dg('error', `unknown element <${s.name}> in selector; rule dropped`, s);
            return null;
          }
          if (!k[toolkit]) {
            dg('warning', `<${s.name}> is not an element on ${toolkit}; rule dropped`, s);
            return null;
          }
          const amb = k.ambiguous?.includes(toolkit);
          if (toolkit === 'gtk') text += amb ? `${k.gtk.css}.sv-k-${s.name}` : k.gtk.css;
          else {
            text += amb ? `.sv-k-${s.name}` : `.${k.qt}`;
            qtClass = k.qt;
          }
          break;
        }
        case 'ClassSelector':
          text += `.${s.name}`;
          break;
        case 'IdSelector':
          text += `#${s.name}`;
          break;
        case 'AttributeSelector':
          dg('error', `attribute selector [${s.name}] is not portable; rule dropped (use a class, or @native ${toolkit} { })`, s);
          return null;
        case 'NestingSelector':
          text += ctx.parent ?? '';
          break;
        case 'PseudoClassSelector': {
          if (s.name === 'where' || s.name === 'is') {
            const one = s.args && flatSelector(s.args.children);
            if (!one) {
              dg('error', `:${s.name}() with a selector list is not supported on ${toolkit}; rule dropped`, s);
              return null;
            }
            const r = compound(one, ctx);
            if (!r) return null;
            text += r.text;
            qtClass = r.qtClass ?? qtClass;
            break;
          }
          if (s.name === 'not') {
            const one = s.args && flatSelector(s.args.children);
            if (toolkit === 'gtk' && one && one.selectors.every((x) => x.type === 'ClassSelector' || x.type === 'PseudoClassSelector')) {
              const r = compound(one, ctx);
              if (!r) return null;
              text += `:not(${r.text})`;
              break;
            }
            if (toolkit === 'qt' && one?.selectors.length === 1 && one.selectors[0].type === 'PseudoClassSelector' && PSEUDO.qt[one.selectors[0].name]) {
              const m = PSEUDO.qt[one.selectors[0].name];
              text += `:!${Array.isArray(m) ? m[0] : m}`;
              break;
            }
            dg('error', `:not(...) of this form is not expressible on ${toolkit}; rule dropped`, s);
            return null;
          }
          let m = PSEUDO[toolkit][s.name];
          if (!m) {
            dg('error', `pseudo-class :${s.name} is not supported on ${toolkit}; rule dropped`, s);
            return null;
          }
          if (Array.isArray(m)) {
            dg('warning', m[1], s);
            m = m[0];
          }
          text += `:${m}`;
          break;
        }
        case 'PseudoElementSelector':
          dg('error', `pseudo-element ::${s.name} is not supported; rule dropped (parts need @native ${toolkit} { })`, s);
          return null;
        default:
          dg('error', `selector node ${s.type} is not supported; rule dropped`, s);
          return null;
      }
    }
    return { text, qtClass };
  }

  function complex(cx, ctx) {
    let text = '';
    let qtClass = null;
    for (const rel of cx.children) {
      const comb = rel.combinator?.name;
      if (comb && toolkit === 'qt' && (comb === '+' || comb === '~')) {
        dg('error', `combinator '${comb}' never matches in QSS; rule dropped`, rel);
        return null;
      }
      const c = compound(rel, ctx);
      if (!c) return null;
      text += (comb ? (comb === ' ' ? ' ' : ` ${comb} `) : '') + c.text;
      qtClass = c.qtClass;
    }
    return { text, qtClass };
  }

  function rule(node, ctx, out) {
    const sels = [];
    for (const cx of node.prelude.children) {
      const s = complex(cx, ctx);
      if (s) sels.push(s);
    }
    const decls = [];
    const nested = [];
    for (const ch of node.block.children) {
      if (ch.type === 'Declaration') decls.push({ prop: ch.property.startsWith('--') ? ch.property : ch.property.toLowerCase(), value: ch.value, node: ch });
      else nested.push(ch);
    }
    if (sels.length && decls.length) {
      // Qt: one subject class for the border rule, only when all selectors agree.
      const classes = new Set(sels.map((s) => s.qtClass));
      const qtClass = classes.size === 1 ? [...classes][0] : null;
      const r = translateDecls(decls, toolkit, { where: 'rule', qtClass, knownTokens, offset: node.start });
      diagnostics.push(...r.diagnostics);
      if (r.lines.length) {
        out.push(`${sels.map((s) => s.text).join(',\n')} {\n${r.lines.map((l) => `  ${l}`).join('\n')}\n}`);
      }
    }
    for (const n of nested) {
      if (n.type === 'Rule') for (const s of sels) rule(n, { parent: s.text }, out);
      else atrule(n, out);
    }
  }

  function atrule(node, out) {
    if (node.name === 'media') {
      const q = node.prelude.replace(/\s+/g, ' ').trim();
      const inner = [];
      const dest = toolkit === 'qt' && /^\(prefers-color-scheme:\s*dark\)$/.test(q) ? dark : inner;
      for (const c of node.block.children) (c.type === 'Rule' ? rule(c, {}, dest) : atrule(c, dest));
      if (toolkit === 'gtk') {
        if (/^\(prefers-(color-scheme|contrast|reduced-motion):\s*[\w-]+\)$/.test(q)) out.push(`@media ${q} {\n${inner.join('\n')}\n}`);
        else dg('error', `@media ${q} is not supported on gtk (only prefers-color-scheme, prefers-contrast, prefers-reduced-motion)`, node);
      } else if (dest !== dark) {
        dg('error', `@media ${q} is not supported on qt (only prefers-color-scheme: dark)`, node);
      }
      return;
    }
    if (node.name === 'keyframes') {
      if (toolkit === 'gtk') out.push(cssText.slice(node.start, node.end));
      else dg('warning', `@keyframes ${node.prelude} dropped: QSS has no animations`, node);
      return;
    }
    if (node.name === 'native') {
      if (node.prelude.trim() === toolkit && node.block) out.push(cssText.slice(node.block.start + 1, node.block.end - 1).trim());
      return;
    }
    dg('error', `@${node.name} is not supported on ${toolkit}; dropped`, node);
  }

  for (const n of ast.children) (n.type === 'Rule' ? rule(n, {}, base) : atrule(n, base));
  for (const text of opts.native ?? []) base.push(text);
  const join = (a) => (a.length ? `${a.join('\n')}\n` : '');
  return { css: join(base), dark: join(dark), diagnostics };
}

/** Translates for every toolkit; the result is what `registerStyles` carries. */
export function translateAll(cssText, { native = { gtk: [], qt: [] }, knownTokens } = {}) {
  const gtk = translateSheet(cssText, 'gtk', { native: native.gtk, knownTokens });
  const qt = translateSheet(cssText, 'qt', { native: native.qt, knownTokens });
  return {
    sheets: { gtk: gtk.css, qt: qt.css, qtDark: qt.dark },
    diagnostics: [...gtk.diagnostics, ...qt.diagnostics],
  };
}

export { parseDeclarations };
