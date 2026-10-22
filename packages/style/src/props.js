// The style property table (props.generated.js, from metadata/style/<toolkit>.yaml): which
// properties a toolkit has and how each is applied, value parsing for setter and child-attr
// properties, and setter-rule matching. Plain ES2022, no Node: the renderer bundles it.
import { STYLE_TABLES, TAG_SETS } from './props.generated.js';

export { STYLE_TABLES };

/** The table entry of `name` on `toolkit`, or null. */
export const propOf = (toolkit, name) => STYLE_TABLES[toolkit]?.props[name] ?? null;

const tagSets = TAG_SETS.map((s) => new Set(s));

/** Whether a setter (on the element) or child-attr (on its parent) entry applies to `tag`. */
export const appliesTo = (entry, tag) => entry.tags === undefined || tagSets[entry.tags].has(tag);

/** The tags an entry applies to (the parents' tags for a child attribute). */
export const tagsOf = (entry) => (entry.tags === undefined ? [] : TAG_SETS[entry.tags]);

/** `is a setter or child attribute`: applied through widget properties, not the dialect. */
export const isSetter = (entry) => entry?.via === 'setter' || entry?.via === 'child-attr';

function distance(a, b) {
  const d = Array.from({ length: a.length + 1 }, (_, i) => [i]);
  for (let j = 1; j <= b.length; j++) d[0][j] = j;
  for (let i = 1; i <= a.length; i++) {
    for (let j = 1; j <= b.length; j++) d[i][j] = Math.min(d[i - 1][j] + 1, d[i][j - 1] + 1, d[i - 1][j - 1] + (a[i - 1] === b[j - 1] ? 0 : 1));
  }
  return d[a.length][b.length];
}

/**
 * Why `name` is not a style property of `toolkit`, with the toolkit's closest names and, when
 * another toolkit has it, which one (names are native, so they differ between toolkits).
 */
export function unknownProp(toolkit, name) {
  // closest: names sharing all words with it (one word more at most, prefix first), then typos
  const words = name.split('-');
  const near = Object.keys(STYLE_TABLES[toolkit]?.props ?? {})
    .map((n) => {
      const w = n.split('-');
      const shares = (words.every((x) => w.includes(x)) && w.length <= words.length + 1) || (w.every((x) => words.includes(x)) && w.length < words.length);
      return [n, shares ? (n.startsWith(name) ? 0 : 1) : 2 + distance(n, name)];
    })
    .filter(([, d]) => d <= 4)
    .sort((a, b) => a[1] - b[1] || a[0].localeCompare(b[0]))
    .slice(0, 3)
    .map(([n]) => n);
  const elsewhere = Object.keys(STYLE_TABLES).filter((t) => t !== toolkit && STYLE_TABLES[t].props[name]);
  let msg = `property '${name}' is not ${/^[aeiou]/.test(toolkit) ? 'an' : 'a'} ${toolkit} style property`;
  if (elsewhere.length) msg += ` (it is one on ${elsewhere.join(', ')})`;
  if (near.length) msg += `; did you mean ${near.map((n) => `'${n}'`).join(', ')}?`;
  return msg;
}

const LENGTH = /^(-?\d+(?:\.\d+)?)([a-z]*)$/;

function length(toolkit, text) {
  const m = LENGTH.exec(text);
  if (!m) return null;
  const units = STYLE_TABLES[toolkit].units;
  if (m[2] && !(m[2] in units)) return null;
  return Number(m[1]) * (m[2] ? units[m[2]] : 1);
}

/**
 * Parses the CSS text of a setter or child-attr declaration into the value written natively.
 * Returns { value } or { error }.
 */
export function parseSetterValue(toolkit, name, entry, text) {
  const v = String(text).trim();
  if (/\bvar\(/.test(v)) return { error: `${name}: var() is not supported in a setter property (its value is written with setProp)` };
  const units = Object.keys(STYLE_TABLES[toolkit].units);
  const unitText = units.length ? `a number (unitless or in ${units.join(', ')})` : 'a number';
  switch (entry.type) {
    case 'length': {
      const n = length(toolkit, v);
      return n === null ? { error: `${name}: '${v}' is not a length (${unitText})` } : { value: Math.round(n) };
    }
    case 'int':
      return /^-?\d+$/.test(v) ? { value: Number(v) } : { error: `${name}: '${v}' is not an integer` };
    case 'number':
      return /^-?\d+(\.\d+)?$/.test(v) ? { value: Number(v) } : { error: `${name}: '${v}' is not a number` };
    case 'bool':
      return v === 'true' || v === 'false' ? { value: v === 'true' } : { error: `${name}: '${v}' is not true or false` };
    case 'enum':
      return entry.values.includes(v) ? { value: v } : { error: `${name}: '${v}' is not one of ${entry.values.join(', ')}` };
    case 'insets': {
      const parts = v.split(/\s+/).map((p) => length(toolkit, p));
      if (parts.length > 4 || parts.some((p) => p === null)) return { error: `${name}: '${v}' is not 1 to 4 lengths (top right bottom left, as CSS)` };
      const [t, r = t, b = t, l = r] = parts.map(Math.round);
      return { value: { top: t, right: r, bottom: b, left: l } };
    }
    case 'string':
      return { value: v.replace(/^(['"])(.*)\1$/, '$2') };
    default:
      return { error: `${name}: unknown value type ${entry.type}` };
  }
}

/**
 * Whether a setter rule's compound selector `{ tag, classes, id }` matches an element with tag
 * `tag`, class list `classes` (array) and widget name `id`.
 */
export function ruleMatches(sel, tag, classes, id) {
  if (sel.tag && sel.tag !== tag) return false;
  if (sel.id && sel.id !== id) return false;
  for (const c of sel.classes) if (!classes.includes(c)) return false;
  return true;
}

/** Compares two setter rules by precedence: specificity, then source order (ascending). */
export const byPrecedence = (a, b) => a.spec[0] - b.spec[0] || a.spec[1] - b.spec[1] || a.spec[2] - b.spec[2] || a.order - b.order;
