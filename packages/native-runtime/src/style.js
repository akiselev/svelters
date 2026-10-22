// Styling for the native runtime: the `class` / `style` / `id` attributes of native elements, component
// stylesheets registered by the build (`registerStyles`), `--sv-*` tokens and the colour scheme.
//
// Everything reaches the backend through one host op, `host.style(op, handle, text)` (ABI v2.3,
// plans/architecture/backend-abi.md). The first call asks the backend which dialect it speaks
// (CSS for GTK, QSS for Qt); from then on only that dialect is rendered and sent.
//
// Nothing is sent until an app uses styling (a sheet is registered, or an element has a class,
// style or id), so apps without styles keep their op logs unchanged.
//
// A backend that runs the Rust CSS engine itself (egui, CS5e; `SVL_STYLE_TARGET` answers 4) gets
// raw CSS instead: the component sheets as Svelte scoped them (`css`), the user classes, the id, the
// `style=` text untranslated and the tokens as a `:root` sheet. It matches, cascades and restyles
// on its own, hover included; no setter rules run here.
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
  parseSetterValue,
  propOf,
  ruleMatches,
  inlineToNative,
  paletteOf,
  renderGtkTokens,
  renderQss,
  resolveTokens,
  usesTokens,
  byCascade,
  declDecls,
  expandDecl,
  parseDeclarations,
  replaceVars,
  selectorMatches,
} from '@svelters/style/runtime';
import { animatesOpacity } from './motion.js';

const OP = { TARGET: 0, CLASSES: 1, NAME: 2, INLINE: 3, OPACITY: 4, SHEET: 5, SCHEME: 6, DECLS: 7 };
/** `SVL_STYLE_TARGET` answers: 1 GTK CSS, 2 QSS, 3 the decl dialect of AppKit (a toolkit per code). */
const DECL_TOOLKITS = { 3: 'appkit' };
/** `SVL_STYLE_TARGET` answer of a backend running the Rust engine in process (egui, CS5e). */
const ENGINE = 4;
/** Properties an element inherits from its parent when no rule of its own sets them (as in CSS). */
const INHERITED = ['color', 'font-size', 'font-weight', 'font-family', 'font-style'];
/** Separators of the DECLS payload: blocks, then fields inside a block (state, then `prop:value`). */
const BLOCK = '\u0002';
const TOKENS_ID = '@tokens';

const host = () => globalThis.__svelters_host;

const state = {
  /** undefined: not asked yet; 'css' | 'qss' | 'decl' | 'engine' | null (no styling support). */
  target: undefined,
  declToolkit: null, // 'appkit' when target is 'decl'
  declRules: [], // decl rules of every sheet, in cascade order (ascending)
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

/** The runtime (index.js) tells this module how to enumerate live elements (for late sheet registration). */
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
  state.target = t === 1 ? 'css' : t === 2 ? 'qss' : t === ENGINE ? 'engine' : DECL_TOOLKITS[t] ? 'decl' : null;
  state.declToolkit = DECL_TOOLKITS[t] ?? null;
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
  // decl backends report the system appearance; it is the scheme unless the app chose one
  if (t === 'decl' && !state.schemeChosen) state.scheme = hs.style(OP.SCHEME, 0, null) === 1 ? 'dark' : 'light';
  state.tokens = resolveTokens(state.scheme, state.overrides);
  state.tokensKey = JSON.stringify(state.tokens);
  state.lastScheme = state.scheme;
  sendScheme();
  if (t === 'css' || t === 'engine') hs.style(OP.SHEET, 0, `${TOKENS_ID}${SEP}${renderGtkTokens(state.tokens)}`);
  return true;
}

function sendScheme() {
  const pal = state.target === 'qss' ? paletteOf(state.tokens) : [];
  state.paletteKey = pal.join(SEP);
  // VIS1: the egui engine follows its own context's dark mode (the host app's theme) until the app
  // chooses a scheme; 'auto' says so (the default 'light' would force light CSS in a dark app)
  const scheme = state.target === 'engine' && !state.schemeChosen ? 'auto' : state.scheme;
  host().style(OP.SCHEME, 0, [scheme, ...pal].join(SEP));
}

function collectDeclRules() {
  const all = [];
  for (const e of state.sheets.values()) for (const r of e.decl ?? []) all.push({ ...r, order: e.seq * 1e6 + r.order });
  state.declRules = all.sort(byCascade);
}

function sheetText(entry) {
  if (state.target === 'css') return entry.gtk ?? '';
  if (state.target === 'engine') return entry.css ?? '';
  let t = renderQss(entry.qt ?? '', state.tokens);
  if (state.scheme === 'dark' && entry.qtDark) t += renderQss(entry.qtDark, state.tokens);
  return t;
}

function pushSheet(id, entry) {
  if (state.target === 'decl') return;
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
  const entry = { gtk: sheets.gtk ?? '', qt: sheets.qt ?? '', qtDark: sheets.qtDark ?? '', css: sheets.css ?? '', rules: sheets.rules ?? null, decl: sheets.decl ?? null, seq: state.seq++, sent: undefined };
  state.sheets.set(id, entry);
  if (!ensureInit()) return;
  if (state.target === 'decl') {
    collectDeclRules();
    for (const el of state.elements()) markStyleDirty(el);
    return;
  }
  pushSheet(id, entry);
  if (state.target === 'engine') return; // the backend restyles its own nodes
  const rulesChanged = entry.rules !== null || (prev?.rules ?? null) !== null;
  if (rulesChanged) collectRules();
  if (first || rulesChanged) for (const el of state.elements()) markStyleDirty(el);
}

const toolkitOf = () => (state.target === 'css' ? 'gtk' : state.target === 'qss' ? 'qt' : state.target === 'decl' ? state.declToolkit : null);

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
  state.schemeChosen = true;
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
  if (state.target === 'css' || state.target === 'engine') {
    // GTK: one small provider; var() in component CSS resolves natively (the engine's too).
    host().style(OP.SHEET, 0, `${TOKENS_ID}${SEP}${renderGtkTokens(tokens)}`);
  }
  sendScheme();
  if (state.target === 'qss') {
    for (const [id, entry] of state.sheets) pushSheet(id, entry);
    for (const el of state.tokenEls) if (el.h != null) markStyleDirty(el);
  }
  if (state.target === 'decl') for (const el of state.elements()) if (el.sc) markStyleDirty(el);
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
  if (!hasAny && !el.sc && !(hasSheets() && (KINDS[el.tag]?.ambiguous || state.ruleTags.has(el.tag) || state.target === 'decl'))) return;
  if (!ensureInit()) return;
  if (state.target === 'decl') return applyDecl(el);
  if (state.target === 'engine') return applyEngine(el);
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
    // TR1: a running transition owns the opacity channel; this becomes its base value (written
    // back when the transition ends, src/motion.js `commit`)
    if (!animatesOpacity(el)) {
      nativeOf(hs, OP.OPACITY, el.h, opacity);
      if (el.vis) el.vis.sent.opacity = opacity === '' ? null : opacity;
    }
  }
  if (state.rules.length || sc.inlineSetters?.length || sc.setters) applySetters(el, sc, list, name);
}

/** Engine backends (CS5e): the user classes, the id and the raw `style=` text, each when it changed. */
function applyEngine(el) {
  const a = el.attrs;
  const hs = host();
  const sc = (el.sc ??= { classes: '', name: '', inline: '' });
  const classes = classList(a.class).join(' ');
  if (classes !== sc.classes) {
    sc.classes = classes;
    nativeOf(hs, OP.CLASSES, el.h, classes);
  }
  const name = a.id == null ? '' : String(a.id);
  if (name !== sc.name) {
    sc.name = name;
    nativeOf(hs, OP.NAME, el.h, name);
  }
  const inline = a.style == null ? '' : String(a.style).trim();
  if (inline !== sc.inline) {
    sc.inline = inline;
    nativeOf(hs, OP.INLINE, el.h, inline);
  }
}

/** The runtime's hooks into the element (index.js): explicit props and child attributes. */
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
      if (!hooks.hasProp(el, cur.entry.prop, cur.entry.native)) (hooks.restoreProp ?? hooks.writeProp)(el, cur.entry.prop, cur.orig);
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
  let els = [...dirty];
  dirty.clear();
  if (state.target === 'decl') els = withDescendants(els);
  for (const el of els) if (el.h != null) applyStyle(el);
}

/**
 * `els` and every live element below one of them, parents before children (descendant selectors
 * and inherited properties of a subtree depend on its ancestors).
 */
function withDescendants(els) {
  const want = new Set(els);
  const all = [...state.elements()];
  const depth = new Map();
  const depthOf = (el) => {
    let d = depth.get(el);
    if (d === undefined) {
      d = el.np ? depthOf(el.np) + 1 : 0;
      depth.set(el, d);
    }
    return d;
  };
  const under = new Map();
  const isUnder = (el) => {
    if (want.has(el)) return true;
    let r = under.get(el);
    if (r === undefined) {
      r = el.np ? isUnder(el.np) : false;
      under.set(el, r);
    }
    return r;
  };
  return all.filter(isUnder).sort((a, b) => depthOf(a) - depthOf(b));
}

// ---- the decl dialect (AppKit): a cascade engine over the native element tree ----

const declClasses = (el) => classList(el.attrs.class);

/** The native ancestor chain of `el`, outermost first, `el` last: [{ tag, classes, id }]. */
function chainOf(el) {
  const out = [];
  for (let n = el; n; n = n.np) out.push({ tag: n.tag, classes: declClasses(n), id: n.attrs.id == null ? null : String(n.attrs.id) });
  return out.reverse();
}

/** Inline `style=` of a decl element: [prop, value] (setter properties are left to the setter path). */
function inlineDecls(el, text) {
  const key = `decl|${text}`;
  let r = state.inlineCache.get(key);
  if (!r) {
    const d = declDecls(parseDeclarations(text), state.declToolkit, { where: 'inline' });
    for (const x of d.diagnostics) if (x.level !== 'info') warnOnce(`${formatDiagnostic(x)} (in style="${text}")`);
    const setters = [];
    for (const x of d.setters) {
      const v = parseSetterValue(state.declToolkit, x.prop, x.entry, x.value);
      if (v.error) warnOnce(`${v.error} (in style="${text}")`);
      else setters.push({ name: x.prop, entry: x.entry, value: v.value });
    }
    r = { decls: d.decls, setters };
    if (state.inlineCache.size > 2000) state.inlineCache.clear();
    state.inlineCache.set(key, r);
  }
  return r;
}

/**
 * Computes `el`'s declarations per state and sends them when they changed. The base block is the
 * cascade of the matching rules without a state (dark rules too while the scheme is dark), the
 * inherited text properties of the parent, and the inline style on top; a state block holds the
 * declarations of the matching rules with that state, minus what the inline style sets (inline
 * style wins over every rule, as on the web). `var()` is resolved here with the active tokens and
 * the custom properties set on the element and its ancestors.
 */
function applyDecl(el) {
  const sc = (el.sc ??= { decl: undefined, computed: null, vars: null, setters: null });
  const chain = chainOf(el);
  const scheme = state.scheme;
  const base = new Map();
  const states = new Map();
  for (const r of state.declRules) {
    if (r.dark && scheme !== 'dark') continue;
    if (!selectorMatches(r.sel, chain)) continue;
    const into = r.state ? states.get(r.state) ?? states.set(r.state, new Map()).get(r.state) : base;
    for (const [p, v] of r.decls) into.set(p, v);
  }
  const styleText = el.attrs.style == null ? '' : String(el.attrs.style).trim();
  const inline = styleText ? inlineDecls(el, styleText) : { decls: [], setters: [] };
  for (const [p, v] of inline.decls) base.set(p, v);
  // custom properties: the token set, then the ancestors', then the element's own
  const parentVars = el.np?.sc?.vars ?? null;
  let vars = parentVars ?? state.tokens;
  const own = [...base].filter(([p]) => p.startsWith('--'));
  if (own.length) {
    vars = { ...vars };
    for (const [p, v] of own) vars[p] = v;
  }
  sc.vars = vars === state.tokens ? null : vars;
  const resolve = (v) => replaceVars(v, (name, fb) => vars?.[name] ?? state.tokens?.[name] ?? (fb === null ? '' : resolve(fb)));
  const block = (m) => {
    const out = new Map();
    for (const [p, v] of m) {
      if (p.startsWith('--')) continue;
      for (const [q, w] of expandDecl(p, resolve(v))) out.set(q, w);
    }
    return out;
  };
  const computed = block(base);
  const parent = el.np?.sc?.computed ?? null;
  if (parent) for (const p of INHERITED) if (!computed.has(p) && parent.has(p)) computed.set(p, parent.get(p));
  sc.computed = computed;
  const inlineProps = new Set(inline.decls.map(([p]) => p));
  let text = [...computed].map(([p, v]) => `\u0001${p}:${v}`).join('');
  for (const st of ['hover', 'focus', 'checked', 'active', 'disabled']) {
    const m = states.get(st);
    if (!m) continue;
    for (const p of inlineProps) m.delete(p);
    const b = block(m);
    if (b.size) text += `${BLOCK}${st}${[...b].map(([p, v]) => `\u0001${p}:${v}`).join('')}`;
  }
  if (text !== sc.decl) {
    if (text || sc.decl) host().style(OP.DECLS, el.h, text || null);
    sc.decl = text;
  }
  if (inline.setters.length || sc.setters) {
    sc.inlineSetters = inline.setters;
    applySetters(el, sc, declClasses(el), el.attrs.id == null ? '' : String(el.attrs.id));
  }
}

/** The runtime calls this after inserting `el` under a native parent (descendant selectors change). */
export function styleInserted(el) {
  if (state.target === 'decl' && hasSheets()) markStyleDirty(el);
}

/** Forgets per-element style bookkeeping when the native widget is destroyed. */
export function forgetStyle(el) {
  el.sc = null;
  state.tokenEls.delete(el);
  dirty.delete(el);
}
