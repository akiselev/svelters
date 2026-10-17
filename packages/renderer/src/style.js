// Styling for the renderer: the `class` / `style` / `id` attributes of native elements, component
// stylesheets registered by the build (`registerStyles`), `--sv-*` tokens and the colour scheme.
//
// Everything reaches the backend through one host op, `host.style(op, handle, text)` (ABI v2.3,
// plans/architecture/backend-abi.md). The first call asks the backend which dialect it speaks
// (CSS for GTK, QSS for Qt); from then on only that dialect is rendered and sent.
//
// Nothing is sent until an app uses styling (a sheet is registered, or an element has a class,
// style or id), so apps without styles keep their op logs unchanged.
//
// Setter properties (the style table: `spacing`, `hexpand`, `minimum-width`, ...) are not CSS.
// Stylesheets carry them as setter rules (`registerStyles(id, { ..., rules })`), matched here
// against each element's tag, classes and id whenever those change; `style=` / `style:` carry them
// inline. They are written with setProp (setChildAttr for child attributes). Precedence: an explicit
// attribute > style= / style: > rules (specificity, then source order) > the native value, which
// is read with getProp before a rule first overrides it and written back when no rule sets it.
import {
  KINDS,
  SEP,
  appliesTo,
  byPrecedence,
  formatDiagnostic,
  propOf,
  ruleMatches,
  inlineToNative,
  paletteOf,
  renderGtkTokens,
  renderQss,
  resolveTokens,
  usesTokens,
} from '@svelters/style/runtime';

const OP = { TARGET: 0, CLASSES: 1, NAME: 2, INLINE: 3, OPACITY: 4, SHEET: 5, SCHEME: 6 };
const TOKENS_ID = '@tokens';

const host = () => globalThis.__svelters_host;

const state = {
  /** undefined: not asked yet; 'css' | 'qss' | null (no styling support). */
  target: undefined,
  init: false,
  sheets: new Map(), // id -> { gtk, qt, qtDark, rules, seq, sent }
  seq: 0, // registration counter: later sheets win ties in rule order
  rules: [], // setter rules of the active toolkit, every sheet, by precedence (ascending)
  ruleTags: new Set(), // tags named by a type selector of a setter rule
  scheme: 'light',
  overrides: {},
  tokens: null, // active token set once initialised
  tokensKey: '',
  paletteKey: '',
  tokenEls: new Set(), // elements whose Qt inline style depends on tokens
  inlineCache: new Map(),
  warned: new Set(),
  queued: false,
  elements: () => [],
};

/** The renderer tells this module how to enumerate live elements (for late sheet registration). */
export function bindElements(fn) {
  state.elements = fn;
}

function targetOf() {
  if (state.target !== undefined) return state.target;
  const hs = host();
  if (typeof hs?.style !== 'function') {
    state.target = null;
    return null;
  }
  const t = hs.style(OP.TARGET, 0, null);
  state.target = t === 1 ? 'css' : t === 2 ? 'qss' : null;
  return state.target;
}

const warnOnce = (msg) => {
  if (state.warned.has(msg)) return;
  state.warned.add(msg);
  console.warn(`[svelters/style] ${msg}`);
};

/** Sends the one-time setup: clear stale sheets (hot reload), scheme + palette, token layer. */
function ensureInit() {
  if (state.init) return state.target !== null;
  state.init = true;
  const t = targetOf();
  if (t === null) {
    warnOnce('this host has no styling support: class, style and stylesheets are ignored');
    return false;
  }
  const hs = host();
  hs.style(OP.SHEET, 0, null); // clears every sheet a previous bundle registered
  state.tokens = resolveTokens(state.scheme, state.overrides);
  state.tokensKey = JSON.stringify(state.tokens);
  state.lastScheme = state.scheme;
  sendScheme();
  if (t === 'css') hs.style(OP.SHEET, 0, `${TOKENS_ID}${SEP}${renderGtkTokens(state.tokens)}`);
  return true;
}

function sendScheme() {
  const pal = state.target === 'qss' ? paletteOf(state.tokens) : [];
  state.paletteKey = pal.join(SEP);
  host().style(OP.SCHEME, 0, [state.scheme, ...pal].join(SEP));
}

function sheetText(entry) {
  if (state.target === 'css') return entry.gtk ?? '';
  let t = renderQss(entry.qt ?? '', state.tokens);
  if (state.scheme === 'dark' && entry.qtDark) t += renderQss(entry.qtDark, state.tokens);
  return t;
}

function pushSheet(id, entry) {
  const text = sheetText(entry);
  if (entry.sent === text) return;
  entry.sent = text;
  host().style(OP.SHEET, 0, text ? `${id}${SEP}${text}` : id);
}

/** Whether any component stylesheet is registered (type selectors then need kind markers). */
export const hasSheets = () => state.sheets.size > 0;

/**
 * Registers a component's translated stylesheets (`{ gtk, qt, qtDark }`); called by the module
 * the build generates for every `<style>` block. Registering an id again replaces it.
 */
export function registerStyles(id, sheets) {
  const first = state.sheets.size === 0;
  const prev = state.sheets.get(id);
  const entry = { gtk: sheets.gtk ?? '', qt: sheets.qt ?? '', qtDark: sheets.qtDark ?? '', rules: sheets.rules ?? null, seq: state.seq++, sent: undefined };
  state.sheets.set(id, entry);
  if (!ensureInit()) return;
  pushSheet(id, entry);
  const rulesChanged = entry.rules !== null || (prev?.rules ?? null) !== null;
  if (rulesChanged) collectRules();
  if (first || rulesChanged) for (const el of state.elements()) markStyleDirty(el);
}

const toolkitOf = () => (state.target === 'css' ? 'gtk' : state.target === 'qss' ? 'qt' : null);

/** Rebuilds the active toolkit's setter rules from every registered sheet. */
function collectRules() {
  const tk = toolkitOf();
  const all = [];
  state.ruleTags.clear();
  for (const e of state.sheets.values()) {
    for (const r of e.rules?.[tk] ?? []) {
      all.push({ ...r, order: e.seq * 1e6 + r.order });
      if (r.sel.tag) state.ruleTags.add(r.sel.tag);
    }
  }
  state.rules = all.sort(byPrecedence);
}

/** Overrides tokens app-wide (`null` removes an override). Takes effect at the end of the job. */
export function setTokens(overrides) {
  for (const [k, v] of Object.entries(overrides)) {
    if (!k.startsWith('--')) throw new TypeError(`token names start with '--': ${k}`);
    if (v === null || v === undefined) delete state.overrides[k];
    else state.overrides[k] = String(v);
  }
  scheduleRefresh();
}

/** The active token set. */
export const getTokens = () => resolveTokens(state.scheme, state.overrides);

/** Switches the colour scheme ('light' | 'dark'): tokens, palette and dark sheets follow. */
export function setColorScheme(scheme) {
  if (scheme !== 'light' && scheme !== 'dark') throw new TypeError(`colour scheme must be 'light' or 'dark', not ${scheme}`);
  if (state.scheme === scheme) return;
  state.scheme = scheme;
  scheduleRefresh();
}

export const getColorScheme = () => state.scheme;

function scheduleRefresh() {
  if (state.queued) return;
  state.queued = true;
  Promise.resolve().then(refresh);
}

/** Applies pending token / scheme changes in one batch. */
export function refresh() {
  state.queued = false;
  if (!state.init) return; // nothing was sent yet: the first use picks the values up
  if (state.target === null) return;
  const tokens = resolveTokens(state.scheme, state.overrides);
  const key = JSON.stringify(tokens);
  const schemeChanged = state.lastScheme !== state.scheme;
  state.tokens = tokens;
  if (key === state.tokensKey && !schemeChanged) return;
  state.tokensKey = key;
  state.lastScheme = state.scheme;
  if (state.target === 'css') {
    // GTK: one small provider; var() in component CSS resolves natively.
    host().style(OP.SHEET, 0, `${TOKENS_ID}${SEP}${renderGtkTokens(tokens)}`);
  }
  sendScheme();
  if (state.target === 'qss') {
    for (const [id, entry] of state.sheets) pushSheet(id, entry);
    for (const el of state.tokenEls) if (el.h != null) markStyleDirty(el);
  }
}

// ---- element hooks ----

/** Parses the class attribute into a deduplicated list. */
function classList(value) {
  if (value === null || value === undefined || value === false) return [];
  const out = [];
  for (const c of String(value).split(/\s+/)) if (c && !out.includes(c)) out.push(c);
  return out;
}

function inlineOf(el, text) {
  const key = `${state.target}|${el.tag}|${text}`;
  let r = state.inlineCache.get(key);
  if (!r) {
    const toolkit = state.target === 'css' ? 'gtk' : 'qt';
    const qtClass = KINDS[el.tag]?.qt ?? null;
    r = inlineToNative(text, toolkit, qtClass);
    for (const d of r.diagnostics) if (d.level !== 'info') warnOnce(`${formatDiagnostic(d)} (in style="${text}")`);
    if (state.inlineCache.size > 2000) state.inlineCache.clear();
    state.inlineCache.set(key, r);
  }
  return r;
}

const nativeOf = (hs, op, h, value) => hs.style(op, h, value === '' || value === undefined ? null : value);

/**
 * Brings the backend up to date with `el`'s class / id / style attributes. `el.sc` remembers what
 * was sent, so unchanged parts cost nothing.
 */
export function applyStyle(el) {
  if (el.h == null) return;
  const a = el.attrs;
  const hasAny = a.class != null || a.style != null || a.id != null;
  if (!hasAny && !el.sc && !(hasSheets() && (KINDS[el.tag]?.ambiguous || state.ruleTags.has(el.tag)))) return;
  if (!ensureInit()) return;
  const hs = host();
  const sc = (el.sc ??= { classes: '', name: '', inline: '', opacity: '', setters: null });

  const list = classList(a.class);
  if (hasSheets() && KINDS[el.tag]?.ambiguous) list.unshift(`sv-k-${el.tag}`);
  const classes = list.join(' ');
  if (classes !== sc.classes) {
    sc.classes = classes;
    nativeOf(hs, OP.CLASSES, el.h, classes);
  }
  const name = a.id == null ? '' : String(a.id);
  if (name !== sc.name) {
    sc.name = name;
    nativeOf(hs, OP.NAME, el.h, name);
  }
  const styleText = a.style == null ? '' : String(a.style).trim();
  if (styleText !== sc.src) {
    sc.src = styleText;
    const r = styleText ? inlineOf(el, styleText) : { text: '', opacity: null };
    sc.tpl = r.text;
    sc.inlineSetters = r.setters ?? [];
    if (state.target === 'qss' && usesTokens(r.text)) state.tokenEls.add(el);
    else state.tokenEls.delete(el);
    sc.opacity0 = r.opacity ?? '';
  }
  const inline = state.target === 'qss' ? renderQss(sc.tpl ?? '', state.tokens) : (sc.tpl ?? '');
  if (inline !== sc.inline) {
    sc.inline = inline;
    nativeOf(hs, OP.INLINE, el.h, inline);
  }
  const opacity = sc.opacity0 ?? '';
  if (opacity !== sc.opacity) {
    sc.opacity = opacity;
    nativeOf(hs, OP.OPACITY, el.h, opacity);
  }
  if (state.rules.length || sc.inlineSetters?.length || sc.setters) applySetters(el, sc, list, name);
}

/** The renderer's hooks into the element (index.js): explicit props and child attributes. */
let hooks = null;
export function bindSetterHooks(h) {
  hooks = h;
}

/**
 * Setter and child-attr values from the matching rules and the inline style, applied as a diff
 * against what was applied before (`sc.setters`: key -> { entry, name, value }). An explicit
 * attribute of the same prop (or `parent:` attribute) wins and is left alone.
 */
function applySetters(el, sc, classes, id) {
  const tk = toolkitOf();
  /** @type {Map<string, { entry: any, name: string, value: unknown }>} */
  const want = new Map();
  const put = (name, value) => {
    const entry = propOf(tk, name);
    if (!entry) return false;
    if (entry.via === 'setter' && !appliesTo(entry, el.tag)) return false;
    const key = entry.via === 'setter' ? `p:${entry.prop}` : `a:${entry.attr}`;
    if (entry.via === 'setter' ? hooks.hasProp(el, entry.prop, entry.native) : hooks.hasChildAttr(el, entry.attr)) return true;
    want.set(key, { entry, name, value });
    return true;
  };
  for (const r of state.rules) if (ruleMatches(r.sel, el.tag, classes, id)) for (const [n, v] of r.decls) put(n, v);
  for (const x of sc.inlineSetters ?? []) {
    if (!put(x.name, x.value)) warnOnce(`'${x.name}' does not apply to <${el.tag}> on ${tk} (in style="${el.attrs.style}")`);
  }
  const had = sc.setters ?? new Map();
  for (const [key, cur] of had) {
    if (want.has(key)) continue;
    // no rule sets it any more: back to the native value read before the first override
    if (cur.entry.via === 'setter') {
      if (!hooks.hasProp(el, cur.entry.prop, cur.entry.native)) hooks.writeProp(el, cur.entry.prop, cur.orig);
    } else if (!hooks.hasChildAttr(el, cur.entry.attr)) hooks.writeChildAttr(el, cur.entry.attr, null);
  }
  const next = new Map();
  for (const [key, w] of want) {
    const cur = had.get(key);
    const orig = cur ? cur.orig : w.entry.via === 'setter' ? hooks.readProp(el, w.entry.prop) : null;
    next.set(key, { ...w, orig });
    if (cur && sameValue(cur.value, w.value)) continue;
    if (w.entry.via === 'setter') hooks.writeProp(el, w.entry.prop, w.value);
    else hooks.writeChildAttr(el, w.entry.attr, w.value);
  }
  sc.setters = next.size ? next : null;
}

const sameValue = (a, b) => a === b || (typeof a === 'object' && a !== null && JSON.stringify(a) === JSON.stringify(b));

/**
 * The child attributes rules and inline style set on `el` (sent again after every insert, like
 * the `parent:` attributes; an explicit one wins).
 */
export function styleChildAttrs(el) {
  const out = [];
  if (!el.sc?.setters) return out;
  for (const w of el.sc.setters.values()) if (w.entry.via === 'child-attr') out.push([w.entry.attr, w.value]);
  return out;
}

/**
 * An explicit attribute took `prop` over: forget the rule's claim (without restoring; the explicit
 * value stands). Removing that attribute later re-matches the rules (`markStyleDirty`).
 */
export function explicitPropSet(el, prop) {
  const m = el.sc?.setters;
  if (!m) return;
  for (const [key, w] of m) if (w.entry.via === 'setter' && (w.entry.prop === prop || w.entry.native === prop)) m.delete(key);
}

const dirty = new Set();
let dirtyQueued = false;

/** Applies `el`'s style at the end of the current job (so `style:a style:b` becomes one update). */
export function markStyleDirty(el) {
  if (el.h == null) return;
  dirty.add(el);
  if (dirtyQueued) return;
  dirtyQueued = true;
  Promise.resolve().then(flushDirty);
}

function flushDirty() {
  dirtyQueued = false;
  const els = [...dirty];
  dirty.clear();
  for (const el of els) if (el.h != null) applyStyle(el);
}

/** Forgets per-element style bookkeeping when the native widget is destroyed. */
export function forgetStyle(el) {
  el.sc = null;
  state.tokenEls.delete(el);
  dirty.delete(el);
}
