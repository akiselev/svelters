// @svelters/renderer: a Svelte custom renderer (sveltejs/svelte#18042 API, vendored fork) that
// drives a SveltersHost. It keeps a JS tree of fragment/element/text/comment nodes and
// translates it into host ops. Comments, text, fragments and `sv-slot` proxies stay JS-only;
// only elements become native widgets, created lazily when first inserted under a native parent.
// Mapping rules and the dispatch return shape: docs/notes/p1-js-lane.md. Plain ES2022, no globals.
import { createRenderer } from 'svelte/renderer';
import { BINDINGS } from './bindings.generated.js';
import { coerceAttribute } from './coerce.js';
import { kindRef, methodRef, propRef } from './hostops.js';

export { BINDINGS };

/** Kinds whose Text children are folded into the `label` prop. */
export const LABEL_KINDS = new Set(['label', 'button']);

/** DOM-ish event name -> native signal. Anything not listed passes through unchanged. */
export const EVENT_TO_SIGNAL = { click: 'clicked', change: 'changed', close: 'close-request' };
const SIGNAL_TO_EVENT = Object.fromEntries(Object.entries(EVENT_TO_SIGNAL).map(([e, s]) => [s, e]));
const signalFor = (type) => EVENT_TO_SIGNAL[type] ?? type;
const eventFor = (signal) => SIGNAL_TO_EVENT[signal] ?? signal;

/** Attributes that stay JS-side and never become host props. */
const JS_ONLY_ATTRS = new Set(['class', 'style', 'id']);

const PROXY_TAG = 'sv-slot';

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
      this.listeners = {}; // event type -> handlers
      this.counts = {}; // native signal -> listener count
      this.signals = new Set(); // native signals with >= 1 listener
      this.slot = null; // from data-slot / slot
      this.h = null; // native handle once realized
      this.dead = null; // why the native widget is gone: 'destroyed' or a native death reason
      this.np = null; // native parent this element is inserted under
      this.dead = null; // reason the native object died (see `gone`), cleared when re-realized
      this.lastLabel = '';
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
    host().destroy(el.h);
    el.h = null;
    el.dead = 'destroyed';
    el.np = null;
    el.lastLabel = '';
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

function updateLabel(el) {
  if (el.kind !== 'element' || el.h == null || !LABEL_KINDS.has(el.tag)) return;
  const s = labelText(el);
  if (s !== el.lastLabel) {
    el.lastLabel = s;
    host().setProp(el.h, propRef(host(), el.tag, 'label'), s);
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
  for (const sig of el.signals) hs.listen(el.h, sig, true);
  updateLabel(el);
  for (const { el: c, slot } of flat(el, null, [])) {
    realize(c);
    hs.insert(el.h, c.h, null, slot ?? undefined);
    c.np = el;
  }
}

/** First native element in or under `s` (looking through sv-slot proxies), or null. */
function firstFlat(s) {
  if (s.kind !== 'element') return null;
  if (!isProxy(s)) return s;
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
}

function attachNative(el) {
  const { parent, slot } = placement(el);
  if (!parent) return;
  if (parent === root) {
    if (el.tag !== 'window') throw new Error(`only <window> may be mounted at top level, got <${el.tag}>`);
    realize(el);
    host().call(el.h, methodRef(host(), el.tag, 'present'), []);
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
    if (pl.parent === root) markDetached(t);
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
  el.props.set(name, value);
  if (el.h != null) host().setProp(el.h, propRef(host(), el.tag, name), value);
}

// listener bookkeeping per native signal: listen on 0 -> 1, off on 1 -> 0
function bumpListener(el, type, delta) {
  if (el === root || isProxy(el)) return;
  const sig = signalFor(type);
  const n = (el.counts[sig] ?? 0) + delta;
  el.counts[sig] = n;
  if (delta > 0 && n === 1) {
    el.signals.add(sig);
    if (el.h != null) host().listen(el.h, sig, true);
  } else if (delta < 0 && n === 0) {
    el.signals.delete(sig);
    if (el.h != null) host().listen(el.h, sig, false);
  }
}

const nativeIn = {
  string: (v) => (typeof v === 'string' ? v : v !== null && typeof v === 'object' ? String(v.detail ?? '') : String(v ?? '')),
  bool: Boolean,
  number: Number,
};
const nativeOut = {
  string: (v) => (v == null ? '' : String(v)),
  bool: Boolean,
  number: (v) => (v == null ? 0 : Number(v)),
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
    if (key === 'slot' || key === 'data-slot') el.slot = String(value);
    else if (!JS_ONLY_ATTRS.has(key) && !isProxy(el)) setNativeProp(el, key, coerceAttribute(host(), el.tag, key, value));
  },
  removeAttribute(el, name) {
    delete el.attrs[name];
    if (name === 'slot' || name === 'data-slot') el.slot = null;
    else if (el.props.has(name)) {
      el.props.delete(name);
      if (el.h != null) host().setProp(el.h, propRef(host(), el.tag, name), null);
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
  addEventListener(target, type, handler) {
    if (target.borrowed) throw new Error('events of a borrowed native object cannot be listened to through the renderer');
    (target.listeners[type] ??= []).push(handler);
    bumpListener(target, type, 1);
  },
  removeEventListener(target, type, handler) {
    const a = target.listeners[type];
    const i = a ? a.indexOf(handler) : -1;
    if (i >= 0) {
      a.splice(i, 1);
      bumpListener(target, type, -1);
    }
  },
  bind,
});

export default renderer;

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
export function dispatchSync(h, signal, args = []) {
  const el = byHandle.get(h);
  if (!el) throw new Error(`dispatchSync: unknown handle ${h}`);
  const ev = new NativeEvent(eventFor(signal), signal, el, args, false);
  run: for (const type of new Set([eventFor(signal), signal])) {
    for (const f of [...(el.listeners[type] ?? [])]) {
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
export function dispatch(h, signal, args = [], deferred = false) {
  const el = byHandle.get(h);
  if (!el) throw new Error(`dispatch: unknown handle ${h}`);
  if (deferred) {
    // A sync signal that was delivered late: same listener signature as dispatchSync, result ignored.
    const ev = new NativeEvent(eventFor(signal), signal, el, args, true);
    const results = [];
    for (const type of new Set([eventFor(signal), signal])) {
      for (const f of [...(el.listeners[type] ?? [])]) results.push(f.call(el, ev, ...args));
    }
    return { results, event: ev };
  }
  // Mirror host-owned state so Svelte writing the same value back is a no-op.
  if (signal === 'changed' && el.tag === 'entry' && typeof args[0] === 'string') el.props.set('text', args[0]);
  const results = [];
  for (const type of new Set([eventFor(signal), signal])) {
    for (const f of [...(el.listeners[type] ?? [])]) results.push(f.apply(el, args));
  }
  const first = args[0];
  return { results, event: first !== null && typeof first === 'object' ? first : null };
}
