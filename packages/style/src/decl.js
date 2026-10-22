// The `decl` dialect (ST2): toolkits with no stylesheet language (AppKit) get their component
// stylesheets as structured rules that the runtime cascade engine matches (native-runtime/style.js)
// and the backend applies natively. Plain ES2022 apart from `parseCss` in `translateDeclSheet`, which
// only runs at build time.
//
// A rule: { sel, state, dark, spec, order, decls }
//   sel    compounds from the outermost to the subject: [{ tag, classes, id, comb }], `comb` is the
//          combinator to the previous compound (' ' or '>'), null on the first.
//   state  null or one of STATES: a pseudo-class on the subject the backend tracks itself.
//   dark   from `@media (prefers-color-scheme: dark)`.
//   spec   [ids, classes + pseudo-classes, types] over the whole selector.
//   decls  [[property, value]] in source order; values keep var(--sv-*) (resolved at runtime).
import { diag, replaceVars } from './decls.js';
import { STYLE_TABLES, unknownProp } from './props.js';

/** Pseudo-classes of the subject the backend tracks (`focus-visible` is approximated by `focus`). */
export const STATES = ['hover', 'active', 'focus', 'disabled', 'checked'];
const STATE_ALIAS = { 'focus-visible': 'focus' };

const flat = (children) => (children.length === 1 && children[0].children.length === 1 ? children[0].children[0] : null);

// ---- values ----

const NAMED_COLORS = new Set([
  'transparent', 'black', 'white', 'red', 'green', 'blue', 'gray', 'grey', 'orange', 'yellow', 'purple', 'pink', 'brown',
  'cyan', 'magenta', 'currentcolor',
]);
const COLOR = /^(#[0-9a-f]{3,4}|#[0-9a-f]{6}|#[0-9a-f]{8}|rgba?\([^)]*\)|hsla?\([^)]*\)|[a-z]+)$/i;
const LENGTH = /^-?\d*\.?\d+(px|pt)?$/;
const NUMBER = /^-?\d*\.?\d+%?$/;

/** A problem with `value` for a property of decl grammar `type`, or null. `X` stands for a var(). */
export function declValueProblem(prop, type, value, values) {
  const v = value.trim().toLowerCase();
  if (v === 'x' || v === 'initial' || v === 'unset' || v === 'inherit') return null;
  const isColor = (t) => t === 'x' || (COLOR.test(t) && (t.startsWith('#') || t.includes('(') || NAMED_COLORS.has(t)));
  const isLength = (t) => t === 'x' || t === '0' || LENGTH.test(t);
  switch (type) {
    case 'color':
      return isColor(v) ? null : `'${value}' is not a colour for ${prop} (hex, rgb(), hsl() or a basic name)`;
    case 'length':
      return isLength(v) || v === 'auto' || v === 'none' ? null : `'${value}' is not a length for ${prop} (px or pt)`;
    case 'number':
      return NUMBER.test(v) ? null : `'${value}' is not a number for ${prop}`;
    case 'enum':
      return values.includes(v) ? null : `'${value}' is not one of ${values.join(', ')} for ${prop}`;
    case 'font-weight':
      return /^(normal|bold|lighter|bolder|[1-9]00)$/.test(v) ? null : `'${value}' is not a font weight`;
    case 'string':
      return null;
    case 'border': {
      if (v === 'none' || v === '0') return null;
      for (const p of splitTop(v)) if (!(isLength(p) || p === 'solid' || p === 'none' || isColor(p))) return `'${p}' in border is not a width, solid/none or a colour`;
      return null;
    }
    case 'shadow': {
      if (v === 'none') return null;
      const parts = splitTop(v);
      const lens = parts.filter(isLength);
      if (parts.includes('inset')) return 'inset shadows are not supported';
      if (lens.length < 2 || lens.length > 3) return `box-shadow takes x, y and an optional blur (no spread) and a colour`;
      return null;
    }
    default:
      return null;
  }
}

/** Splits a value on top-level whitespace (parentheses kept together). */
export function splitTop(v) {
  const out = [];
  let depth = 0;
  let cur = '';
  for (const ch of v) {
    if (ch === '(') depth++;
    if (ch === ')') depth--;
    if (/\s/.test(ch) && depth === 0) {
      if (cur) out.push(cur);
      cur = '';
    } else cur += ch;
  }
  if (cur) out.push(cur);
  return out;
}

/**
 * Checks the declarations of one rule (or an inline style) for a decl toolkit. Setter properties are
 * returned apart (`setters`, the compiler or the setter path applies them); everything else becomes
 * `[prop, value]` with var() kept. Diagnostics name the toolkit.
 */
export function declDecls(decls, toolkit, { knownTokens = null, offset, where = 'rule' } = {}) {
  const table = STYLE_TABLES[toolkit];
  const out = [];
  const setters = [];
  const diagnostics = [];
  const warn = (m) => diagnostics.push(diag('warning', toolkit, m, offset));
  const err = (m) => diagnostics.push(diag('error', toolkit, m, offset));
  for (const { prop, value } of decls) {
    if (prop.startsWith('--')) {
      out.push([prop, value]); // a custom property: tokens can be set per element subtree
      continue;
    }
    const entry = table.props[prop];
    if (!entry) {
      warn(unknownProp(toolkit, prop));
      continue;
    }
    if (entry.via !== 'css') {
      setters.push({ prop, value, entry });
      continue;
    }
    const v = replaceVars(value, (name, fb) => {
      if (knownTokens && name.startsWith('--sv-') && !knownTokens.has(name) && fb === null) {
        warn(`token ${name} is not in the default token set; define it with setTokens() before use`);
      }
      return 'X';
    });
    const problem = declValueProblem(prop, entry.type, v, entry.values ?? []);
    if (problem) {
      err(`${problem}; declaration dropped${where === 'inline' ? ' (in style=)' : ''}`);
      continue;
    }
    out.push([prop, value.trim()]);
  }
  return { decls: out, setters, diagnostics };
}

/**
 * Translates one (Svelte-scoped) stylesheet to decl rules.
 * @param {string} cssText
 * @param {string} toolkit a toolkit whose style table has dialect `decl`
 * @param {{ parseCss: Function, tags?: Iterable<string>, knownTokens?: Iterable<string> }} opts
 */
export function translateDeclSheet(cssText, toolkit, opts) {
  const tags = opts.tags ? new Set(opts.tags) : null;
  const knownTokens = opts.knownTokens ? new Set(opts.knownTokens) : null;
  const diagnostics = [];
  const dg = (level, message, node) => diagnostics.push(diag(level, toolkit, message, node?.start));
  const rules = [];
  let order = 0;

  /** One compound: { tag, classes, id, pseudo[] } or null (diagnosed). */
  function compound(rel) {
    const c = { tag: null, classes: [], id: null, pseudo: [] };
    const walk = (list) => {
      for (const s of list) {
        if (s.type === 'TypeSelector') {
          if (s.name === '*') continue;
          if (tags && !tags.has(s.name)) {
            dg('warning', `<${s.name}> is not an element on ${toolkit}; rule dropped`, s);
            return false;
          }
          c.tag = s.name;
        } else if (s.type === 'ClassSelector') c.classes.push(s.name);
        else if (s.type === 'IdSelector') c.id = s.name;
        else if (s.type === 'PseudoClassSelector' && (s.name === 'where' || s.name === 'is')) {
          const one = s.args && flat(s.args.children);
          if (!one) {
            dg('error', `:${s.name}() with a selector list is not supported on ${toolkit}; rule dropped`, s);
            return false;
          }
          if (!walk(one.selectors)) return false;
        } else if (s.type === 'PseudoClassSelector') {
          const name = STATE_ALIAS[s.name] ?? s.name;
          if (!STATES.includes(name)) {
            dg('error', `pseudo-class :${s.name} is not supported on ${toolkit} (${STATES.map((x) => `:${x}`).join(', ')}); rule dropped`, s);
            return false;
          }
          c.pseudo.push(name);
        } else {
          const what = s.type === 'AttributeSelector' ? `attribute selector [${s.name}]` : s.type === 'PseudoElementSelector' ? `::${s.name}` : s.type;
          dg('error', `${what} is not supported on ${toolkit}; rule dropped`, s);
          return false;
        }
      }
      return true;
    };
    return walk(rel.selectors) ? c : null;
  }

  /** A complex selector -> { sel, state, spec } or null. */
  function complex(cx) {
    const sel = [];
    let state = null;
    const spec = [0, 0, 0];
    for (let i = 0; i < cx.children.length; i++) {
      const rel = cx.children[i];
      const comb = rel.combinator?.name ?? null;
      if (comb && comb !== ' ' && comb !== '>') {
        dg('error', `combinator '${comb}' is not supported on ${toolkit} (descendant and child only); rule dropped`, rel);
        return null;
      }
      const c = compound(rel);
      if (!c) return null;
      if (c.pseudo.length) {
        if (i !== cx.children.length - 1) {
          dg('error', `:${c.pseudo[0]} is only supported on the last compound on ${toolkit}; rule dropped`, rel);
          return null;
        }
        if (c.pseudo.length > 1) {
          dg('error', `one state pseudo-class per selector on ${toolkit}; rule dropped`, rel);
          return null;
        }
        state = c.pseudo[0];
      }
      spec[0] += c.id ? 1 : 0;
      spec[1] += c.classes.length + c.pseudo.length;
      spec[2] += c.tag ? 1 : 0;
      sel.push({ tag: c.tag, classes: c.classes, id: c.id, comb: i === 0 ? null : comb });
    }
    return { sel, state, spec };
  }

  function rule(node, dark) {
    const decls = [];
    for (const ch of node.block.children) {
      if (ch.type === 'Declaration') decls.push({ prop: ch.property.startsWith('--') ? ch.property : ch.property.toLowerCase(), value: ch.value });
      else if (ch.type === 'Rule') dg('error', `nested rules are not supported on ${toolkit}; dropped`, ch);
      else atrule(ch, dark);
    }
    if (!decls.length) return;
    const r = declDecls(decls, toolkit, { knownTokens, offset: node.start });
    diagnostics.push(...r.diagnostics);
    if (!r.decls.length) return;
    for (const cx of node.prelude.children) {
      const s = complex(cx);
      if (s) rules.push({ ...s, dark, order: order++, decls: r.decls });
    }
  }

  function atrule(node, dark) {
    if (node.name === 'media') {
      const q = node.prelude.replace(/\s+/g, ' ').trim();
      const m = /^\(prefers-color-scheme:\s*(dark|light)\)$/.exec(q);
      if (!m) {
        dg('error', `@media ${q} is not supported on ${toolkit} (only prefers-color-scheme)`, node);
        return;
      }
      // light rules apply in both schemes before the dark ones: model `light` as plain rules
      for (const c of node.block.children) (c.type === 'Rule' ? rule(c, m[1] === 'dark') : atrule(c, dark));
      return;
    }
    if (node.name === 'native') return; // other toolkits' escapes
    dg('error', `@${node.name} is not supported on ${toolkit}; dropped`, node);
  }

  const ast = opts.parseCss(cssText);
  for (const n of ast.children) (n.type === 'Rule' ? rule(n, false) : atrule(n, false));
  return { rules, diagnostics };
}

// ---- runtime half: matching and cascading (no parser) ----

/** Whether compound `c` matches element `el` ({ tag, classes: string[], id }). */
const compoundMatches = (c, el) => (!c.tag || c.tag === el.tag) && (!c.id || c.id === el.id) && c.classes.every((x) => el.classes.includes(x));

/**
 * Whether `sel` matches the element at the end of `chain` (outermost ancestor first, the element
 * last; each { tag, classes, id }).
 */
export function selectorMatches(sel, chain) {
  let i = chain.length - 1;
  if (!compoundMatches(sel[sel.length - 1], chain[i])) return false;
  for (let k = sel.length - 1; k > 0; k--) {
    const comb = sel[k].comb;
    const want = sel[k - 1];
    if (comb === '>') {
      i--;
      if (i < 0 || !compoundMatches(want, chain[i])) return false;
    } else {
      do i--;
      while (i >= 0 && !compoundMatches(want, chain[i]));
      if (i < 0) return false;
    }
  }
  return true;
}

/** Cascade order: specificity, then source order (ascending; later wins). */
export function byCascade(a, b) {
  for (let i = 0; i < 3; i++) if (a.spec[i] !== b.spec[i]) return a.spec[i] - b.spec[i];
  return a.order - b.order;
}

/** Expands shorthands into longhands the backend applies (border, background). */
export function expandDecl(prop, value) {
  if (prop === 'background') return [['background-color', value]];
  if (prop === 'border') {
    const v = value.trim().toLowerCase();
    if (v === 'none' || v === '0') return [['border-width', '0']];
    const out = [];
    for (const p of splitTop(value)) {
      const q = p.toLowerCase();
      if (q === 'solid') continue;
      if (q === 'none') out.push(['border-width', '0']);
      else if (/^-?\d*\.?\d+(px|pt)?$/.test(q) || q === '0') out.push(['border-width', p]);
      else out.push(['border-color', p]);
    }
    return out;
  }
  if (prop === 'border-style' && value.trim() === 'none') return [['border-width', '0']];
  if (prop === 'border-style') return [];
  return [[prop, value]];
}
