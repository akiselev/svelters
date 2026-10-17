// Build-time translation of a component's (Svelte-scoped) stylesheet to GTK CSS and QSS.
//
// Input is `compile(..., { css: 'external' }).css.code`: class and element selectors carrying
// Svelte's scope hash, `:where(.svelte-h)` on later compounds, native-looking nesting.
// Output per toolkit is a stylesheet the toolkit is known to parse, plus diagnostics for
// everything that was dropped. For Qt this matters twice: QSS stops parsing at the first
// construct it does not understand, so the translator only ever emits constructs from a closed
// set (docs/notes/styling-gtk-css-qss.md section 2.2).
//
// Declarations of setter and child-attr properties (the style table, props.generated.js) are not
// CSS: they become setter rules `{ sel: { tag, classes, id }, spec, order, decls }` that the
// renderer matches against elements and applies with setProp / setChildAttr. Their selector must
// be one compound of the toolkit's `setterSelectors` (type, .class, #id); anything else is an
// error naming the property.
import { parseCss } from 'svelte/compiler';
import { diag, parseDeclarations, translateDecls } from './decls.js';
import { KINDS } from './kinds.generated.js';
import { STYLE_TABLES, appliesTo } from './props.js';
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
 * @returns {{ css: string, dark: string, rules: SetterRule[], diagnostics: import('./decls.js').Diagnostic[] }}
 */
export function translateSheet(cssText, toolkit, opts = {}) {
  const kinds = opts.kinds ?? KINDS;
  const knownTokens = new Set(opts.knownTokens ?? DEFAULT_TOKEN_NAMES);
  const diagnostics = [];
  const dg = (level, message, node) => diagnostics.push(diag(level, toolkit, message, node?.start));
  const ast = parseCss(cssText);
  const base = [];
  const dark = [];
  /** @type {SetterRule[]} */
  const rules = [];
  let order = 0;
  const allowed = new Set(STYLE_TABLES[toolkit]?.setterSelectors ?? []);

  /**
   * The `{ tag, classes, id }` of a selector a setter rule accepts (one compound of type, class
   * and id selectors; Svelte's `:where(.svelte-h)` is flattened), or the reason it is not one.
   */
  function setterSelector(cx) {
    if (cx.children.length !== 1) return { why: 'combinators are not supported' };
    const sel = { tag: null, classes: [], id: null };
    const walk = (list) => {
      for (const x of list) {
        if (x.type === 'TypeSelector' && x.name !== '*' && allowed.has('type')) sel.tag = x.name;
        else if (x.type === 'ClassSelector' && allowed.has('class')) sel.classes.push(x.name);
        else if (x.type === 'IdSelector' && allowed.has('id')) sel.id = x.name;
        else if (x.type === 'PseudoClassSelector' && (x.name === 'where' || x.name === 'is') && x.args && flatSelector(x.args.children)) {
          const why = walk(flatSelector(x.args.children).selectors);
          if (why) return why;
        } else {
          const what = x.type === 'PseudoClassSelector' ? `:${x.name}` : x.type === 'PseudoElementSelector' ? `::${x.name}` : x.type === 'TypeSelector' ? x.name : x.type;
          return `${what} is not supported`;
        }
      }
      return null;
    };
    const why = walk(cx.children[0].selectors);
    if (why) return { why };
    if (!sel.tag && !sel.id && !sel.classes.length) return { why: 'the selector selects nothing specific' };
    return { sel };
  }

  /** Records the setter declarations of a rule, one setter rule per selector. */
  function setterRules(node, setters, ctx) {
    const kinds = [...allowed].join(', ');
    const names = setters.map((x) => `'${x.name}'`).join(', ');
    if (ctx.parent !== undefined || ctx.media) {
      dg('error', `${names}: setter properties cannot be inside ${ctx.media ? '@media' : 'a nested rule'} (they are written with setProp, not CSS); rule dropped`, node);
      return;
    }
    for (const cx of node.prelude.children) {
      const r = setterSelector(cx);
      if (r.why) {
        dg('error', `${names}: setter properties only take ${kinds} selectors (${r.why}); dropped for this selector`, cx);
        continue;
      }
      const decls = [];
      for (const x of setters) {
        if (r.sel.tag && x.entry.via === 'setter' && !appliesTo(x.entry, r.sel.tag)) {
          dg('error', `'${x.name}' does not apply to <${r.sel.tag}> on ${toolkit}; declaration dropped`, cx);
          continue;
        }
        decls.push([x.name, x.value]);
      }
      if (!decls.length) continue;
      const spec = [r.sel.id ? 1 : 0, r.sel.classes.length, r.sel.tag ? 1 : 0];
      rules.push({ sel: r.sel, spec, order: order++, decls });
    }
  }

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
      if (r.setters.length) setterRules(node, r.setters, ctx);
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
      for (const c of node.block.children) (c.type === 'Rule' ? rule(c, { media: true }, dest) : atrule(c, dest));
      if (toolkit === 'gtk') {
        if (/^\(prefers-(color-scheme|contrast|reduced-motion):\s*[\w-]+\)$/.test(q)) {
          if (inner.length) out.push(`@media ${q} {\n${inner.join('\n')}\n}`);
        } else dg('error', `@media ${q} is not supported on gtk (only prefers-color-scheme, prefers-contrast, prefers-reduced-motion)`, node);
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
  return { css: join(base), dark: join(dark), rules, diagnostics };
}

/**
 * @typedef {{ sel: { tag: string | null, classes: string[], id: string | null }, spec: [number, number, number],
 *   order: number, decls: [string, unknown][] }} SetterRule
 * `spec`: (ids, classes, types); `decls`: [style property, native value] in source order.
 */

/**
 * Translates for every toolkit; the result is what `registerStyles` carries: the sheets, plus
 * `rules` (setter rules per toolkit) when the stylesheet has any.
 */
export function translateAll(cssText, { native = { gtk: [], qt: [] }, knownTokens } = {}) {
  const gtk = translateSheet(cssText, 'gtk', { native: native.gtk, knownTokens });
  const qt = translateSheet(cssText, 'qt', { native: native.qt, knownTokens });
  const sheets = { gtk: gtk.css, qt: qt.css, qtDark: qt.dark };
  if (gtk.rules.length || qt.rules.length) sheets.rules = { gtk: gtk.rules, qt: qt.rules };
  return { sheets, diagnostics: [...gtk.diagnostics, ...qt.diagnostics] };
}

export { parseDeclarations };
