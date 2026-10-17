// Declaration-level translation shared by the build-time sheet translator and the renderer's
// inline `style` handling. No imports besides vocab.js; safe to bundle into QuickJS apps.
import { LAYOUT_HINTS, MATRIX, QT_PLAIN_BOX, valueProblem } from './vocab.js';

/** @typedef {{ level: 'error' | 'warning' | 'info', toolkit: string, message: string, offset?: number }} Diagnostic */

export const diag = (level, toolkit, message, offset) => ({ level, toolkit, message, ...(offset === undefined ? {} : { offset }) });
export const formatDiagnostic = (d) => `${d.level} [${d.toolkit}] ${d.message}`;

/** Splits `a: b; c: d(e;f)` into [{prop, value}], honouring parentheses and quotes. */
export function parseDeclarations(text) {
  const out = [];
  let depth = 0;
  let quote = '';
  let cur = '';
  const flush = () => {
    const i = cur.indexOf(':');
    if (i > 0) {
      const prop = cur.slice(0, i).trim();
      const value = cur.slice(i + 1).trim();
      if (prop && value) out.push({ prop: prop.startsWith('--') ? prop : prop.toLowerCase(), value });
    }
    cur = '';
  };
  for (const ch of text) {
    if (quote) {
      if (ch === quote) quote = '';
      cur += ch;
    } else if (ch === '"' || ch === "'") {
      quote = ch;
      cur += ch;
    } else if (ch === '(') {
      depth++;
      cur += ch;
    } else if (ch === ')') {
      depth = Math.max(0, depth - 1);
      cur += ch;
    } else if (ch === ';' && depth === 0) flush();
    else cur += ch;
  }
  flush();
  return out;
}

/** Rewrites every `var(--name[, fallback])` in `value` with `fn(name, fallback | null)`. */
export function replaceVars(value, fn) {
  let out = '';
  let i = 0;
  while (i < value.length) {
    const j = value.indexOf('var(', i);
    if (j < 0 || (j > 0 && /[\w-]/.test(value[j - 1]))) {
      if (j < 0) {
        out += value.slice(i);
        break;
      }
      out += value.slice(i, j + 4);
      i = j + 4;
      continue;
    }
    out += value.slice(i, j);
    let depth = 1;
    let k = j + 4;
    while (k < value.length && depth > 0) {
      if (value[k] === '(') depth++;
      else if (value[k] === ')') depth--;
      k++;
    }
    const inner = value.slice(j + 4, k - 1);
    const c = inner.indexOf(',');
    const name = (c < 0 ? inner : inner.slice(0, c)).trim();
    const fb = c < 0 ? null : inner.slice(c + 1).trim();
    out += fn(name, fb);
    i = k;
  }
  return out;
}

const BORDER_PROP = /^border(-(top|right|bottom|left))?(-(width|style|color))?$/;

/**
 * Translates declarations for one toolkit.
 *
 * `where`: 'rule' (stylesheet) or 'inline' (`style=`). `qtClass`: the Qt class of the rule's
 * subject when known (decides whether QSS needs `border: none` to paint a background).
 * `knownTokens`: a Set of token names; unknown `--sv-*` references get a warning.
 *
 * Returns the declaration lines (`prop: value;`, one per line, Qt tokens as `${--sv-x}`
 * placeholders), the inline opacity (applied through the widget API, not CSS), and diagnostics.
 */
export function translateDecls(decls, toolkit, { where = 'rule', qtClass = null, knownTokens = null, offset } = {}) {
  /** @type {string[]} */
  const lines = [];
  /** @type {Diagnostic[]} */
  const diagnostics = [];
  let opacity = null;
  const warn = (m) => diagnostics.push(diag('warning', toolkit, m, offset));
  const err = (m) => diagnostics.push(diag('error', toolkit, m, offset));
  let hasBg = false;
  let hasBorder = false;
  let hasBgColor = false;

  for (const { prop, value } of decls) {
    if (prop.startsWith('--')) {
      if (toolkit === 'qt') err(`custom property ${prop} cannot be declared in QSS; define tokens with setTokens() or the token set`);
      else lines.push(`${prop}: ${value};`);
      continue;
    }
    if (prop === 'opacity' && where === 'inline') {
      const n = Number.parseFloat(value);
      if (!Number.isFinite(n)) err(`opacity '${value}' is not a number`);
      else opacity = String(Math.max(0, Math.min(1, value.trim().endsWith('%') ? n / 100 : n)));
      continue;
    }
    const row = MATRIX[prop];
    if (!row) {
      const hint = LAYOUT_HINTS[prop];
      warn(`property '${prop}' is not supported${hint ? ` (${hint})` : ' by the portable vocabulary: put it in @native ' + toolkit + ' { }'}`);
      continue;
    }
    const level = row[toolkit];
    if (level === 'no' || (level === 'inline' && where !== 'inline')) {
      warn(`property '${prop}' is not supported on ${toolkit}${row.note ? `: ${row.note}` : ''}`);
      continue;
    }
    // Resolve var() first so the value check sees what the toolkit will see.
    let bad = false;
    let v = replaceVars(value, (name, fb) => {
      if (knownTokens && name.startsWith('--sv-') && !knownTokens.has(name) && fb === null) {
        warn(`token ${name} is not in the default token set; define it with setTokens() before use`);
      }
      if (toolkit === 'gtk') return `var(${name}${fb === null ? '' : `, ${fb}`})`;
      if (!name.startsWith('--sv-')) {
        err(`var(${name}) in ${prop}: only --sv-* tokens exist on Qt (QSS has no variables)`);
        bad = true;
        return '';
      }
      return '${' + name + (fb === null ? '' : `|${fb}`) + '}';
    });
    if (bad) continue;
    const problem = valueProblem(toolkit, prop, v.replace(/\$\{[^}]*\}/g, 'X'));
    if (problem) {
      err(problem);
      continue;
    }
    if (toolkit === 'qt') {
      if (/^background(-color)?$/.test(prop) && !/^\s*(none|transparent)?\s*$/.test(v)) hasBg = true;
      if (BORDER_PROP.test(prop)) hasBorder = true;
    } else if (prop === 'background-color') hasBgColor = true;
    lines.push(`${prop}: ${v};`);
  }
  if (toolkit === 'gtk' && hasBgColor) {
    // The theme paints buttons and entries with a background-image gradient that hides a colour.
    lines.push('background-image: none;');
    diagnostics.push(diag('info', toolkit, `background-color needs 'background-image: none' to show over the theme's gradients; added`, offset));
  }
  if (toolkit === 'qt' && hasBg && !hasBorder && !(qtClass && QT_PLAIN_BOX.has(qtClass))) {
    lines.push('border: none;');
    diagnostics.push(diag('info', toolkit, `background needs a border in QSS to paint on buttons and inputs; added 'border: none'`, offset));
  }
  return { lines, opacity, diagnostics };
}

/**
 * The native form of an inline `style` string: `text` is the declaration text for `toolkit`
 * (newline separated, QSS token placeholders unresolved), `opacity` goes to the widget API.
 */
export function inlineToNative(styleText, toolkit, qtClass = null) {
  const r = translateDecls(parseDeclarations(styleText), toolkit, { where: 'inline', qtClass });
  return { text: r.lines.join('\n'), opacity: r.opacity, diagnostics: r.diagnostics };
}
