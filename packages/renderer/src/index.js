// @svelters/renderer: a Svelte custom renderer (sveltejs/svelte#18042 API, vendored fork) that
// drives a SveltersHost. It keeps a JS tree of fragment/element/text/comment nodes and
// translates it into host ops. Comments, text, fragments and `sv-slot` proxies stay JS-only;
// only elements become native widgets, created lazily when first inserted under a native parent.
// Mapping rules and the dispatch return shape: docs/notes/p1-js-lane.md. Plain ES2022, no globals.
import { createRenderer } from 'svelte/renderer';
import { BINDINGS, TOPLEVELS } from './bindings.generated.js';
import { coerceAttribute } from './coerce.js';
import { kindRef, methodRef, propRef } from './hostops.js';
import { applyStyle, bindElements, bindSetterHooks, explicitPropSet, forgetStyle, markStyleDirty, styleChildAttrs } from './style.js';

export { BINDINGS, TOPLEVELS };
export { getColorScheme, getTokens, registerStyles, setColorScheme, setTokens } from './style.js';

/**
 * Tags whose Text children are folded into a prop, with candidate prop names: the first one the
 * host resolves wins (`button` is GTK/Qt `label` and WinUI `content`; names stay native per toolkit).
 */
export const TEXT_PROPS = {
  label: ['label'],
  button: ['label', 'content'],
  'text-block': ['text'],
  'check-box': ['content'],
  'toggle-button': ['content'],
};
/** Kinds whose Text children are folded into a prop (see TEXT_PROPS). */
export const LABEL_KINDS = new Set(Object.keys(TEXT_PROPS));

/** DOM-ish event name -> native signal. Anything not listed passes through unchanged. */
export const EVENT_TO_SIGNAL = { click: 'clicked', change: 'changed', close: 'close-request' };
const SIGNAL_TO_EVENT = Object.fromEntries(Object.entries(EVENT_TO_SIGNAL).map(([e, s]) => [s, e]));
/**
 * The native signal for a DOM-ish event type on `tag`. A mapped name (`click` -> `clicked`) is used
 * unless the host resolves only the unmapped one (WinUI's events keep their native names: `click`).
 */
const signalFor = (type, tag) => {
  const mapped = EVENT_TO_SIGNAL[type];
  if (mapped === undefined) return type;
  if (tag !== undefined && !resolves(tag, 'signalId', mapped) && resolves(tag, 'signalId', type)) return type;
  return mapped;
};
const eventFor = (signal) => SIGNAL_TO_EVENT[signal] ?? signal;

/** Attributes that stay JS-side and never become host props. */
const JS_ONLY_ATTRS = new Set(['class', 'style', 'id', 'show']);
/** Attributes handled by style.js: sent to the backend as classes, inline declarations and the widget name. */
const STYLE_ATTRS = new Set(['class', 'style', 'id']);

const PROXY_TAG = 'sv-slot';

/**
 * Attributes named `parent:<name>` are per-child attributes owned by the native parent (a grid
 * cell, a tab label, a dock area): `<entry parent:row={1} parent:columnSpan={2}>`. They never
 * become props of the child; they are sent with `setChildAttr` once the child is inserted, again
 * on every change, and again when it is inserted into another parent.
 */
const CHILD_ATTR_PREFIX = 'parent:';

/** `listen_ex` flags (ABI v2.2). */
export const LISTEN_AFTER = 1;
export const LISTEN_CAPTURE = 2;

/** Listener flags for `addEventListener` options: `capture` (Svelte's `on<event>capture`) and `after`. */
const flagsOf = (options) => {
  if (options === true) return LISTEN_CAPTURE;
  if (!options || typeof options !== 'object') return 0;
  return (options.capture ? LISTEN_CAPTURE : 0) | (options.after ? LISTEN_AFTER : 0);
};

/**
 * Thrown by the native-access methods of a node (`call`, `getProp`) when it has no live native
 * widget. `reason` is 'destroyed' once the renderer destroyed it, or the native death reason.
 */
export class StaleHandleError extends Error {
  constructor(tag, reason) {
    super(`<${tag}> is dead: ${reason}`);
    this.name = 'StaleHandleError';
    this.reason = reason;
  }
}

/** The live native handle of `el`, or a StaleHandleError / Error saying why there is none. */
function liveHandle(el) {
  if (el.kind !== 'element' || isProxy(el)) throw new TypeError(`<${el.tag ?? el.kind}> has no native widget`);
  if (el.dead) throw new StaleHandleError(el.tag, el.dead);
  if (el.h == null) throw new Error(`<${el.tag}> has no native widget yet: it is created when inserted into the window tree`);
  return el.h;
}

class WNode {
  constructor(kind, tag, value) {
    this.kind = kind; // 'fragment' | 'element' | 'text' | 'comment'
    this.tag = tag ?? null;
    this.value = value ?? null; // text / comment data
    this.parent = null;
    // children are a doubly linked list so that insert/remove/sibling lookups are O(1)
    this.first = null;
    this.last = null;
    this.prev = null;
    this.next = null;
    if (kind === 'element') {
      this.attrs = {}; // raw attribute values, for getAttribute/hasAttribute
      this.props = new Map(); // host props, insertion-ordered
      this.listeners = {}; // event type -> [{ f, flags }]
      this.counts = {}; // `${native signal}\0${flags}` -> listener count
      this.signals = new Map(); // `${native signal}\0${flags}` -> { sig, flags } with >= 1 listener
      this.cattrs = new Map(); // `parent:` attributes, by name
      this.slot = null; // from data-slot / slot
      this.h = null; // native handle once realized
      this.dead = null; // why the native widget is gone: 'destroyed' or a native death reason
      this.np = null; // native parent this element is inserted under
      this.dead = null; // reason the native object died (see `gone`), cleared when re-realized
      this.lastLabel = '';
      this.mounted = false; // toplevel kinds: made transient and shown (or deferred) by `mountTop`
      this.shown = false; // toplevel kinds: the show policy ran
      this.tparent = null; // toplevel kinds: the enclosing toplevel this one is transient for
      this.pending = null; // toplevel kinds: nested toplevels waiting for this one to be shown
      this.sc = null; // what styling has been sent for this element (style.js)
    }
  }

  /** Native handle of a realized element, else null. */
  get handle() {
    return this.h ?? null;
  }

  /** What the host converts a node argument to (`call(h, 'set_child', [node])`); null when dead. */
  get svlHandle() {
    return this.h ?? null;
  }

  /** Set a native property through the bound node (`bind:this`); throws StaleHandleError when dead. */
  setProp(name, value) {
    const hs = host();
    hs.setProp(liveHandle(this), propRef(hs, this.tag, name), value);
  }

  /** Call a native method through the bound node (`bind:this`); throws StaleHandleError when dead. */
  call(method, ...args) {
    return host().call(liveHandle(this), method, args);
  }

  /** Read a native property through the bound node (`bind:this`); throws StaleHandleError when dead. */
  getProp(name) {
    return host().getProp(liveHandle(this), name);
  }
}

const host = () => globalThis.__svelters_host;
const byHandle = new Map(); // handle -> WNode, realized elements only
const borrowedNodes = new Map(); // handle -> WeakRef<WNode> for native objects the app did not create
const sweepSet = new Set(); // elements natively detached since the last sweep
let sweepQueued = false;

export const root = new WNode('element', 'svelters-root');
bindElements(() => byHandle.values());
// style.js applies setter rules through these (explicit attributes win over rules)
bindSetterHooks({
  hasProp: (el, prop, native) => el.props.has(prop) || el.props.has(native),
  hasChildAttr: (el, name) => el.cattrs.has(name),
  readProp: (el, prop) => host().getProp(el.h, propRef(host(), el.tag, prop)),
  writeProp: (el, prop, value) => host().setProp(el.h, propRef(host(), el.tag, prop), value),
  writeChildAttr: (el, name, value) => {
    if (el.h != null && el.np?.h != null) host().setChildAttr(el.np.h, el.h, name, value);
  },
});

// Borrowed handles (native objects a getter or call returned, such as a parent, a root or an
// entry's inner text) are wrapped in nodes that are never part of the tree. They are held weakly
// and forgotten natively once the node is collected; a handle that already has a realized element
// (an object we created) resolves to that element, so identity holds across getters.
const releaseBorrowed = (h) => {
  if (borrowedNodes.get(h)?.deref()) return; // a newer wrapper took over the handle
  borrowedNodes.delete(h);
  try {
    host().destroy(h);
  } catch {
    // the host is shutting down
  }
};
const borrowedRegistry = typeof FinalizationRegistry === 'function' ? new FinalizationRegistry(releaseBorrowed) : null;

/** The node for native handle `h`: the realized element, or a borrowed node (one per handle). */
export function nodeFor(h) {
  const el = byHandle.get(h);
  if (el) return el;
  const live = borrowedNodes.get(h)?.deref();
  if (live) return live;
  const n = new WNode('element', 'borrowed');
  n.borrowed = true;
  n.h = h;
  borrowedNodes.set(h, typeof WeakRef === 'function' ? new WeakRef(n) : { deref: () => n });
  borrowedRegistry?.register(n, h);
  return n;
}

const noBorrowed = (what) => {
  throw new Error(`a borrowed native object cannot be ${what} through the tree`);
};

const isProxy = (n) => n.kind === 'element' && n.tag === PROXY_TAG;

/** The TOPLEVELS entry of an element of a toplevel kind (a window or dialog), else undefined. */
const topSpec = (n) => (n.kind === 'element' && !n.borrowed && Object.hasOwn(TOPLEVELS, n.tag) ? TOPLEVELS[n.tag] : undefined);
const proxySlot = (n) => (typeof n.attrs.name === 'string' ? n.attrs.name : null);

/** Native element descendants of `p` in document order, looking through sv-slot proxies. */
function flat(p, slot, out) {
  for (let c = p.first; c; c = c.next) {
    if (c.kind !== 'element') continue;
    if (isProxy(c)) flat(c, proxySlot(c) ?? slot, out);
    else out.push({ el: c, slot: slot ?? c.slot });
  }
  return out;
}

/** Native parent and slot for element `n`: nearest non-proxy ancestor, innermost proxy name first. */
function placement(n) {
  let slot = null;
  let cur = n.parent;
  while (cur && isProxy(cur)) {
    slot ??= proxySlot(cur);
    cur = cur.parent;
  }
  return { parent: cur, slot: slot ?? n.slot };
}

function isAttached(el) {
  let n = el;
  while (n.parent) n = n.parent;
  return n === root || n.outlet === true;
}

/**
 * A realized element that is not part of the window tree: a mount target for content a backend
 * hosts itself (a recycled list row). Its children are inserted natively as they are mounted, and
 * it is never swept. The owner passes it to a native call (`view.call('setRow', row, outlet)`) and
 * ends it with `releaseOutlet`.
 */
export function outlet(tag = 'box') {
  const el = new WNode('element', tag);
  el.outlet = true;
  realize(el);
  return el;
}

/** Destroys an outlet and everything mounted in it (the native widget handles are released). */
export function releaseOutlet(el) {
  el.outlet = false;
  destroyTree(el);
}

const enqueue = (fn) => {
  if (typeof queueMicrotask === 'function') queueMicrotask(fn);
  else Promise.resolve().then(fn);
};

function markDetached(el) {
  sweepSet.add(el);
  if (!sweepQueued) {
    sweepQueued = true;
    enqueue(sweep);
  }
}

function destroyTree(el) {
  for (let c = el.first; c; c = c.next) if (c.kind === 'element') destroyTree(c);
  if (el.h != null) {
    byHandle.delete(el.h);
    forgetStyle(el);
    host().destroy(el.h);
    el.h = null;
    el.dead = 'destroyed';
    el.np = null;
    el.lastLabel = '';
    el.mounted = el.shown = false;
    el.tparent = el.pending = null;
  }
}

// Native widgets removed from the tree are destroyed once the current job ends, unless they were
// re-attached (a move is remove + insert within one flush).
function sweep() {
  sweepQueued = false;
  const els = [...sweepSet];
  sweepSet.clear();
  for (const el of els) if (!isAttached(el)) destroyTree(el);
}

function labelText(el) {
  let s = '';
  for (let c = el.first; c; c = c.next) if (c.kind === 'text') s += c.value;
  return s;
}

/** Whether the host resolves `name` as a member of `tag` (`fn` is propId/signalId/methodId). Unknown without ids. */
function resolves(tag, fn, name) {
  const hs = host();
  if (typeof hs.kindId !== 'function' || typeof hs[fn] !== 'function') return false;
  const kid = kindRef(hs, tag);
  if (typeof kid !== 'number') return false;
  try {
    return hs[fn](kid, name) != null;
  } catch {
    return false;
  }
}

const textPropCache = new Map();
/** The prop Text children of `tag` set: the first candidate the host resolves (the first without ids). */
function textProp(tag) {
  let p = textPropCache.get(tag);
  if (p === undefined) {
    const names = TEXT_PROPS[tag];
    p = names.find((n) => resolves(tag, 'propId', n)) ?? names[0];
    if (names.length === 1 || typeof host().propId === 'function') textPropCache.set(tag, p);
  }
  return p;
}

function updateLabel(el) {
  if (el.kind !== 'element' || el.h == null || !LABEL_KINDS.has(el.tag)) return;
  const s = labelText(el);
  if (s !== el.lastLabel) {
    el.lastLabel = s;
    host().setProp(el.h, propRef(host(), el.tag, textProp(el.tag)), s);
  }
}

// Create the native widget (replaying stored props/listeners) and, if it was just created,
// insert its native children in order. Existing handles are kept (re-attach after a move).
function realize(el) {
  if (el.h != null) return;
  const hs = host();
  el.dead = null;
  el.h = hs.create(kindRef(hs, el.tag));
  el.dead = null;
  byHandle.set(el.h, el);
  for (const [k, v] of el.props) hs.setProp(el.h, propRef(hs, el.tag, k), v);
  for (const { sig, flags } of el.signals.values()) nativeListen(el, sig, true, flags);
  applyStyle(el);
  updateLabel(el);
  for (const { el: c, slot } of flat(el, null, [])) {
    realize(c);
    if (topSpec(c)) {
      mountTop(c);
      continue;
    }
    hs.insert(el.h, c.h, null, slot ?? undefined);
    c.np = el;
    applyChildAttrs(c);
  }
}

/** The nearest enclosing element of a toplevel kind, or null for a toplevel mounted at the root. */
function topAncestor(el) {
  for (let p = el.parent; p; p = p.parent) if (topSpec(p)) return p;
  return null;
}

/** The first of `names` the host resolves as a method of `tag` (the last one when none does). */
function pickMethod(tag, names) {
  const hs = host();
  if (names.length === 1 || typeof hs.methodId !== 'function') return names[0];
  const kid = kindRef(hs, tag);
  if (typeof kid === 'number') {
    for (const n of names) {
      try {
        if (hs.methodId(kid, n) != null) return n;
      } catch {
        // not a method of this kind
      }
    }
  }
  return names[names.length - 1];
}

/**
 * A realized toplevel element (window or dialog) joins the app: it becomes transient for the
 * enclosing toplevel (`host.insert(enclosing, el)`, see backend-abi.md "Toplevels in the tree")
 * and is shown with its policy, after the enclosing one was shown.
 */
function mountTop(el) {
  if (el.mounted) return;
  el.mounted = true;
  const anc = topAncestor(el);
  if (anc?.h != null) {
    host().insert(anc.h, el.h, null);
    el.tparent = anc;
    if (!anc.shown) {
      (anc.pending ??= []).push(el);
      return;
    }
  }
  showTop(el);
}

/** The `show` policy: 'modal' (dialog default), 'modeless' (window default) or 'none' (the app opens it). */
function showTop(el) {
  const spec = topSpec(el);
  const mode = el.attrs.show ?? (spec.dialog ? 'modal' : 'modeless');
  if (mode !== 'modal' && mode !== 'modeless' && mode !== 'none') {
    throw new Error(`<${el.tag} show="${mode}">: expected modal, modeless or none`);
  }
  el.shown = true;
  if (mode !== 'none') {
    const m = pickMethod(el.tag, mode === 'modal' ? spec.open : spec.modeless);
    if (mode === 'modal' && m === 'present' && spec.modalProp && !el.props.has('modal')) setNativeProp(el, 'modal', true);
    host().call(el.h, methodRef(host(), el.tag, m), []);
  }
  const waiting = el.pending;
  el.pending = null;
  if (waiting) for (const c of waiting) if (c.h != null && c.mounted) showTop(c);
}

/** First native element in or under `s` (looking through sv-slot proxies), or null. */
function firstFlat(s) {
  if (s.kind !== 'element') return null;
  if (!isProxy(s)) return topSpec(s) ? null : s;
  for (let c = s.first; c; c = c.next) {
    const e = firstFlat(c);
    if (e) return e;
  }
  return null;
}

/** The native element that follows `n` in `flat(parent)` order, or null. */
function nextFlat(n, parent) {
  let cur = n;
  for (;;) {
    for (let s = cur.next; s; s = s.next) {
      const e = firstFlat(s);
      if (e) return e;
    }
    cur = cur.parent;
    if (!cur || cur === parent || !isProxy(cur)) return null;
  }
}

function nativeInsert(parent, el, slot) {
  let before = null;
  for (let c = nextFlat(el, parent); c; c = nextFlat(c, parent)) {
    if (c.np === parent && placement(c).slot === slot) {
      before = c.h;
      break;
    }
  }
  host().insert(parent.h, el.h, before, slot ?? undefined);
  el.np = parent;
  applyChildAttrs(el);
}

// Sends every `parent:` attribute of `el` to its native parent; called right after an insert.
function applyChildAttrs(el) {
  if (el.h == null || el.np?.h == null) return;
  for (const [name, value] of el.cattrs) host().setChildAttr(el.np.h, el.h, name, value);
  // child attributes from style rules / inline style (an explicit `parent:` one wins)
  for (const [name, value] of styleChildAttrs(el)) if (!el.cattrs.has(name)) host().setChildAttr(el.np.h, el.h, name, value);
}

function setChildAttr(el, name, value) {
  if (value === undefined) value = null;
  if (el.cattrs.has(name) && el.cattrs.get(name) === value) return;
  if (value === null) {
    el.cattrs.delete(name);
    if (el.sc) markStyleDirty(el); // a style rule may set it again
  } else el.cattrs.set(name, value);
  if (el.h != null && el.np?.h != null) host().setChildAttr(el.np.h, el.h, name, value);
}

function attachNative(el) {
  const { parent, slot } = placement(el);
  if (!parent) return;
  if (topSpec(el)) {
    if (parent === root || (parent.kind === 'element' && parent.h != null)) {
      realize(el);
      mountTop(el);
    }
  } else if (parent === root) {
    throw new Error(`only toplevel kinds (${Object.keys(TOPLEVELS).join(', ')}) may be mounted at top level, got <${el.tag}>`);
  } else if (parent.kind === 'element' && parent.h != null) {
    realize(el);
    nativeInsert(parent, el, slot);
  }
}

function childAttached(parent, n) {
  if (n.kind === 'text') updateLabel(parent);
  else if (n.kind === 'element') {
    if (isProxy(n)) for (const { el } of flat(n, null, [])) attachNative(el);
    else attachNative(n);
  }
}

function detach(n) {
  const p = n.parent;
  if (!p) return;
  const targets = isProxy(n) ? flat(n, null, []).map((e) => e.el) : n.kind === 'element' ? [n] : [];
  const places = targets.map(placement);
  if (n.prev) n.prev.next = n.next;
  else p.first = n.next;
  if (n.next) n.next.prev = n.prev;
  else p.last = n.prev;
  n.prev = n.next = null;
  n.parent = null;
  targets.forEach((t, i) => {
    const pl = places[i];
    if (!pl.parent || t.h == null) return;
    if (topSpec(t)) {
      if (t.tparent?.h != null) host().remove(t.tparent.h, t.h);
      t.tparent = null;
      t.mounted = false;
      markDetached(t);
    } else if (pl.parent === root) markDetached(t);
    else if (t.np === pl.parent && pl.parent.h != null) {
      host().remove(pl.parent.h, t.h);
      t.np = null;
      markDetached(t);
    }
  });
  if (n.kind === 'text' || n.kind === 'comment') updateLabel(p);
}

function setNativeProp(el, name, value) {
  if (el.props.has(name) && el.props.get(name) === value) return;
  if (!el.props.has(name)) explicitPropSet(el, name);
  el.props.set(name, value);
  if (el.h != null) host().setProp(el.h, propRef(host(), el.tag, name), value);
}

// `listen` for flags 0, `listen_ex` (a 4th argument) otherwise.
function nativeListen(el, sig, on, flags) {
  if (flags) host().listen(el.h, sig, on, flags);
  else host().listen(el.h, sig, on);
}

// listener bookkeeping per (native signal, flags): listen on 0 -> 1, off on 1 -> 0
function bumpListener(el, type, delta, flags) {
  if (el === root || isProxy(el)) return;
  const sig = signalFor(type, el.tag);
  const key = `${sig}\0${flags}`;
  const n = (el.counts[key] ?? 0) + delta;
  el.counts[key] = n;
  if (delta > 0 && n === 1) {
    el.signals.set(key, { sig, flags });
    if (el.h != null) nativeListen(el, sig, true, flags);
  } else if (delta < 0 && n === 0) {
    el.signals.delete(key);
    if (el.h != null) nativeListen(el, sig, false, flags);
  }
}

const nativeIn = {
  string: (v) => (typeof v === 'string' ? v : v !== null && typeof v === 'object' ? String(v.detail ?? '') : String(v ?? '')),
  bool: Boolean,
  number: Number,
  // Structured values (dates, records): passed through; the host's descriptors convert them.
  value: (v) => v,
};
const nativeOut = {
  string: (v) => (v == null ? '' : String(v)),
  bool: Boolean,
  number: (v) => (v == null ? 0 : Number(v)),
  value: (v) => v,
};

// `bind:<name>` on a native element (the fork's renderer.bind hook). The descriptor comes from the
// binding metadata. The backend already blocks the change signal during a programmatic set, so
// the echo is handled natively; the renderer only compares before setting (`el.props` mirrors the
// native value, also for natively made edits) and, if the descriptor says the native side
// normalizes, reads the value back and reports it.
function bind(el, name, get, set) {
  const spec = BINDINGS[el.tag]?.[name];
  if (!spec) {
    const known = Object.keys(BINDINGS[el.tag] ?? {});
    throw new Error(`bind:${name} is not supported on <${el.tag}>${known.length ? ` (bindable: ${known.join(', ')})` : ' (it has no bindable properties)'}`);
  }
  const twoWay = spec.direction === 'two_way';
  const handler = (arg) => {
    // `valueFrom: 'prop'`: the change signal carries no value (GTK `toggled`, `notify::*`), so
    // read the property back from the widget that just changed.
    const raw = spec.valueFrom === 'prop' ? (el.h != null ? host().getProp(el.h, spec.prop) : undefined) : arg;
    const value = nativeIn[spec.type](raw);
    if (spec.compare) el.props.set(spec.prop, value);
    set(value);
  };
  if (spec.signal) renderer.addEventListener(el, spec.signal, handler);
  return {
    update: twoWay
      ? (value) => {
          const next = nativeOut[spec.type](value);
          setNativeProp(el, spec.prop, next);
          if (spec.readBack && el.h != null) {
            const actual = host().getProp(el.h, spec.prop);
            if (actual !== next) {
              el.props.set(spec.prop, actual);
              set(actual);
            }
          }
        }
      : undefined,
    destroy() {
      if (spec.signal) renderer.removeEventListener(el, spec.signal, handler);
    },
  };
}

const renderer = createRenderer({
  createFragment: () => new WNode('fragment'),
  createElement: (name) => new WNode('element', name),
  createTextNode: (data) => new WNode('text', null, String(data)),
  createComment: (data) => new WNode('comment', null, String(data)),
  nodeType: (node) => node.kind,
  getNodeValue: (node) => node.value,
  getAttribute: (el, name) => (name in el.attrs ? el.attrs[name] : null),
  hasAttribute: (el, name) => name in el.attrs,
  setAttribute(el, key, value) {
    el.attrs[key] = value;
    if (key.startsWith(CHILD_ATTR_PREFIX)) {
      if (!isProxy(el)) setChildAttr(el, key.slice(CHILD_ATTR_PREFIX.length), value);
    } else if (key === 'slot' || key === 'data-slot') el.slot = String(value);
    else if (STYLE_ATTRS.has(key)) markStyleDirty(el);
    else if (!JS_ONLY_ATTRS.has(key) && !isProxy(el)) setNativeProp(el, key, coerceAttribute(host(), el.tag, key, value));
  },
  removeAttribute(el, name) {
    delete el.attrs[name];
    if (name.startsWith(CHILD_ATTR_PREFIX)) setChildAttr(el, name.slice(CHILD_ATTR_PREFIX.length), null);
    else if (name === 'slot' || name === 'data-slot') el.slot = null;
    else if (STYLE_ATTRS.has(name)) markStyleDirty(el);
    else if (el.props.has(name)) {
      el.props.delete(name);
      if (el.h != null) host().setProp(el.h, propRef(host(), el.tag, name), null);
      if (el.sc) markStyleDirty(el); // a style rule may set it again
    }
  },
  setText(node, text) {
    if (node.kind === 'element') {
      for (let c = node.first, nx; c; c = nx) {
        nx = c.next;
        detach(c);
      }
      if (text) renderer.insert(node, renderer.createTextNode(text), null);
    } else {
      node.value = String(text);
      if (node.parent) updateLabel(node.parent);
    }
  },
  getFirstChild: (el) => el.first,
  getLastChild: (el) => el.last,
  getNextSibling: (node) => node.next,
  getParent: (node) => node.parent,
  insert(parent, node, anchor) {
    let nodes = [node];
    if (node.kind === 'fragment') {
      nodes = [];
      for (let c = node.first; c; c = c.next) nodes.push(c);
      for (const c of nodes) c.parent = c.prev = c.next = null;
      node.first = node.last = null;
    }
    if (parent.borrowed) noBorrowed('inserted into');
    for (const n of nodes) {
      if (n.borrowed) noBorrowed('inserted');
      detach(n);
      if (anchor && anchor.parent !== parent) throw new Error('insert: anchor is not a child of parent');
      const before = anchor ?? null;
      const after = before ? before.prev : parent.last;
      n.prev = after;
      n.next = before;
      if (after) after.next = n;
      else parent.first = n;
      if (before) before.prev = n;
      else parent.last = n;
      n.parent = parent;
      childAttached(parent, n);
    }
  },
  remove: (node) => detach(node),
  // `options` is Svelte's `{ capture }` for `on<event>capture`, or `{ after, capture }` from `listen()`.
  addEventListener(target, type, handler, options) {
    if (target.borrowed) throw new Error('events of a borrowed native object cannot be listened to through the renderer');
    const flags = flagsOf(options);
    (target.listeners[type] ??= []).push({ f: handler, flags });
    bumpListener(target, type, 1, flags);
  },
  removeEventListener(target, type, handler, options) {
    const a = target.listeners[type];
    const flags = flagsOf(options);
    const i = a ? a.findIndex((l) => l.f === handler && l.flags === flags) : -1;
    if (i >= 0) {
      a.splice(i, 1);
      bumpListener(target, type, -1, flags);
    }
  },
  bind,
});

export default renderer;

/**
 * An attachment that listens to a native signal with phase flags:
 * `<entry {@attach listen('key-pressed', fn, { after: true })} />`, `{ capture: true }` for the
 * capture phase. The handler gets the same arguments as `on<signal>` and is removed with the node.
 */
export function listen(type, handler, options = {}) {
  return (node) => {
    renderer.addEventListener(node, type, handler, options);
    return () => renderer.removeEventListener(node, type, handler, options);
  };
}

/** The listeners of `el` registered under `type` with exactly `flags`, in registration order. */
function listenersFor(el, type, flags) {
  return (el.listeners[type] ?? []).filter((l) => l.flags === flags).map((l) => l.f);
}

/**
 * The host reports that the native object behind handle `h` died (`reason`: 'closed' | 'parent' |
 * 'owner' | 'replaced' | 'shutdown' | 'unknown'). The node stays in the JS tree but loses its
 * handle, so nothing further is sent for it: no remove, no destroy. The backend reports every
 * descendant separately. Unknown handles (already destroyed here) are ignored.
 */
export function gone(h, reason) {
  const el = byHandle.get(h) ?? borrowedNodes.get(h)?.deref();
  borrowedNodes.delete(h);
  if (!el) return;
  byHandle.delete(h);
  el.h = null;
  el.np = null;
  el.lastLabel = '';
  el.dead = reason;
  el.mounted = el.shown = false;
  el.tparent = el.pending = null;
}

/**
 * The event object handed to listeners of a signal whose handler result matters (sync signals,
 * and their deferred async delivery). Svelte ignores handler return values, so the result of the
 * native signal comes from flags on this object:
 *   e.preventDefault() / e.stopPropagation()   result `true` (stop: veto the close, key handled)
 *   e.result = v                               explicit result; wins over the flags
 * `args` are the native signal arguments; listeners receive them after the event, `(e, ...args)`.
 * When `deferred` is true the toolkit did not wait for JS (JS was already running), so the flags
 * are ignored; they stay settable so the same handler code runs unchanged.
 */
export class NativeEvent {
  constructor(type, signal, target, args, deferred) {
    this.type = type;
    this.signal = signal;
    this.target = target;
    this.args = args;
    this.deferred = deferred;
    this.cancelable = !deferred;
    this.defaultPrevented = false;
    this.cancelBubble = false;
    this.result = undefined;
    this.immediateStopped = false;
  }

  preventDefault() {
    if (this.cancelable) this.defaultPrevented = true;
  }

  stopPropagation() {
    if (this.cancelable) this.cancelBubble = true;
  }

  stopImmediatePropagation() {
    this.stopPropagation();
    this.immediateStopped = true;
  }
}

/** The host value for a finished sync event: `null` means "no decision, use the default". */
function syncResult(ev) {
  if (ev.result !== undefined) return ev.result;
  return ev.defaultPrevented || ev.cancelBubble ? true : null;
}

/**
 * Deliver a sync native signal (`call_sync`): run the listeners now, in registration order, with
 * a NativeEvent, and return the result for the toolkit (see NativeEvent). An exception from a
 * listener propagates; the host reports it and the toolkit gets the default.
 */
export function dispatchSync(h, signal, args = [], flags = 0) {
  const el = byHandle.get(h);
  if (!el) throw new Error(`dispatchSync: unknown handle ${h}`);
  const ev = new NativeEvent(eventFor(signal), signal, el, args, false);
  run: for (const type of new Set([eventFor(signal), signal])) {
    for (const f of listenersFor(el, type, flags)) {
      const r = f.call(el, ev, ...args);
      if (r !== null && typeof r === 'object' && typeof r.then === 'function') {
        console.warn(`sync handler for '${signal}' returned a Promise: flags set after its first await come too late and are ignored`);
      }
      if (ev.immediateStopped) break run;
    }
  }
  return syncResult(ev);
}

/**
 * Deliver a native signal to JS (asynchronously, from the event queue). Handlers registered for the signal (`onclicked`) and for its
 * DOM-style alias (`onclick`) run with the raw args, in registration order, `this` = the node.
 *
 * Returns `{ results, event }`:
 *   results  array with each handler's return value (Promises are passed through unresolved)
 *   event    args[0] when it is a non-null object (the host's event object, with whatever flags
 *            such as defaultPrevented/cancelBubble/returnValue the handlers set on it), else null
 * Exceptions from handlers propagate to the caller.
 */
export function dispatch(h, signal, args = [], deferred = false, flags = 0) {
  const el = byHandle.get(h);
  if (!el) throw new Error(`dispatch: unknown handle ${h}`);
  if (deferred) {
    // A sync signal that was delivered late: same listener signature as dispatchSync, result ignored.
    const ev = new NativeEvent(eventFor(signal), signal, el, args, true);
    const results = [];
    for (const type of new Set([eventFor(signal), signal])) {
      for (const f of listenersFor(el, type, flags)) results.push(f.call(el, ev, ...args));
    }
    return { results, event: ev };
  }
  // Mirror host-owned state so Svelte writing the same value back is a no-op.
  if (signal === 'changed' && el.tag === 'entry' && typeof args[0] === 'string') el.props.set('text', args[0]);
  const results = [];
  for (const type of new Set([eventFor(signal), signal])) {
    for (const f of listenersFor(el, type, flags)) results.push(f.apply(el, args));
  }
  const first = args[0];
  return { results, event: first !== null && typeof first === 'object' ? first : null };
}
