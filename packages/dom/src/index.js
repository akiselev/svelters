// @svelters/dom: the smallest DOM that stock Svelte 5 client output (fragments: 'tree') runs on,
// translating tree/attribute/listener mutations into SveltersHost ops.
// Mapping rules live in docs/notes/p1-js-lane.md. Plain ES2022, no Node built-ins.

const NS_HTML = 'http://www.w3.org/1999/xhtml';

/** Kinds whose Text children are folded into the `label` prop. */
export const LABEL_KINDS = new Set(['label', 'button']);

/** DOM-ish event name -> native signal. Anything not listed passes through unchanged. */
export const EVENT_TO_SIGNAL = { click: 'clicked', change: 'changed' };
const SIGNAL_TO_EVENT = Object.fromEntries(Object.entries(EVENT_TO_SIGNAL).map(([e, s]) => [s, e]));

/** Attributes that stay JS-side and never become host props. */
const JS_ONLY_ATTRS = new Set(['class', 'style', 'id']);

/** Props whose string attribute value is coerced before reaching the host. */
const INT_PROPS = new Set(['spacing']);

const signalFor = (type) => EVENT_TO_SIGNAL[type] ?? type;
const eventFor = (signal) => SIGNAL_TO_EVENT[signal] ?? signal;

function coerce(name, value) {
  if (INT_PROPS.has(name)) {
    const n = Number(value);
    if (!Number.isInteger(n)) throw new TypeError(`prop ${name} expects an integer, got ${JSON.stringify(value)}`);
    return n;
  }
  return value;
}

/**
 * Build a DOM bound to `host`. Returns the classes, `document`, the JS-only `root` element
 * to mount into, and `dispatch(handle, signal, args)` for native events.
 */
export function createDom(host) {
  const byHandle = new Map(); // handle -> Element, realized elements only
  const sweepSet = new Set(); // elements natively detached since the last sweep
  let sweepQueued = false;

  const enqueue = (fn) => {
    if (typeof queueMicrotask === 'function') queueMicrotask(fn);
    else Promise.resolve().then(fn);
  };

  function isAttached(el) {
    let n = el;
    while (n.parentNode) n = n.parentNode;
    return n === root;
  }

  function markDetached(el) {
    sweepSet.add(el);
    if (!sweepQueued) {
      sweepQueued = true;
      enqueue(sweep);
    }
  }

  function destroyTree(el) {
    for (const c of el._c) if (c.nodeType === 1) destroyTree(c);
    if (el._h != null) {
      byHandle.delete(el._h);
      host.destroy(el._h);
      el._h = null;
      el._lastLabel = '';
    }
  }

  // Native widgets removed from the tree are destroyed once the current job ends, unless they
  // were re-attached (a move is remove + insert within one flush).
  function sweep() {
    sweepQueued = false;
    const els = [...sweepSet];
    sweepSet.clear();
    for (const el of els) if (!isAttached(el)) destroyTree(el);
  }

  function labelText(el) {
    let s = '';
    for (const c of el._c) if (c.nodeType === 3) s += c._v;
    return s;
  }

  function updateLabel(el) {
    if (el._h == null || !LABEL_KINDS.has(el.localName)) return;
    const s = labelText(el);
    if (s !== el._lastLabel) {
      el._lastLabel = s;
      host.setProp(el._h, 'label', s);
    }
  }

  // Create the native widget (replaying stored props/listeners) and, if it was just created,
  // insert its element children. Text and comments stay JS-only.
  function realize(el) {
    const fresh = el._h == null;
    syncDelegated(el);
    if (fresh) {
      el._h = host.create(el.localName);
      byHandle.set(el._h, el);
      for (const [k, v] of el._props) host.setProp(el._h, k, v);
      for (const sig of el._signals) host.listen(el._h, sig, true);
      updateLabel(el);
    }
    for (const c of el._c) {
      if (c.nodeType !== 1) continue;
      realize(c);
      if (fresh) host.insert(el._h, c._h, null, c._slot ?? undefined);
    }
  }

  // Svelte 5.57 delegates events like click: `$.delegated()` stores handlers in a map under a
  // module-private symbol on the element (`el[Symbol('events')][type] = fn`) and listens on
  // the mount target and document only. Scan own symbol props to find them, so the native
  // `listen` op is issued for the element itself. Runs on every realize (handlers are set
  // while the template is built, before it is inserted).
  function syncDelegated(el) {
    const now = new Set();
    for (const s of Object.getOwnPropertySymbols(el)) {
      const m = el[s];
      if (m && typeof m === 'object') {
        for (const type of Object.keys(m)) if (typeof m[type] === 'function') now.add(type);
      }
    }
    for (const type of now) {
      if (!el._dl.has(type)) {
        el._dl.add(type);
        bumpListener(el, type, 1);
      }
    }
    for (const type of [...el._dl]) {
      if (!now.has(type)) {
        el._dl.delete(type);
        bumpListener(el, type, -1);
      }
    }
  }

  function nativeInsert(parent, child) {
    let before = null;
    for (let n = child.nextSibling; n; n = n.nextSibling) {
      if (n.nodeType === 1 && n._h != null) {
        before = n._h;
        break;
      }
    }
    host.insert(parent._h, child._h, before, child._slot ?? undefined);
  }

  function childAttached(parent, n) {
    if (n.nodeType === 3) {
      updateLabel(parent);
    } else if (n.nodeType === 1) {
      if (parent === root) {
        if (n.localName !== 'window') throw new Error(`only <window> may be mounted at top level, got <${n.localName}>`);
        realize(n);
        host.call(n._h, 'present', []);
      } else if (parent._h != null) {
        realize(n);
        nativeInsert(parent, n);
      }
    }
  }

  function childDetached(parent, n) {
    if (n.nodeType === 3) {
      updateLabel(parent);
    } else if (n.nodeType === 1 && n._h != null) {
      if (parent === root) markDetached(n);
      else if (parent._h != null) {
        host.remove(parent._h, n._h);
        markDetached(n);
      }
    }
  }

  function setNativeProp(el, name, value) {
    if (el._props.has(name) && el._props.get(name) === value) return;
    el._props.set(name, value);
    if (el._h != null) host.setProp(el._h, name, value);
  }

  // listener bookkeeping: native listen on 0 -> 1, off on 1 -> 0
  function bumpListener(el, type, delta) {
    if (el === root || el.nodeType !== 1) return;
    const n = (el._count[type] ?? 0) + delta;
    el._count[type] = n;
    const sig = signalFor(type);
    if (delta > 0 && n === 1) {
      el._signals.add(sig);
      if (el._h != null) host.listen(el._h, sig, true);
    } else if (delta < 0 && n === 0) {
      el._signals.delete(sig);
      if (el._h != null) host.listen(el._h, sig, false);
    }
  }

  class EventTarget {
    addEventListener(type, fn) {
      ((this._l ??= {})[type] ??= []).push(fn);
      bumpListener(this, type, 1);
    }
    removeEventListener(type, fn) {
      const a = this._l?.[type];
      const i = a ? a.indexOf(fn) : -1;
      if (i >= 0) {
        a.splice(i, 1);
        bumpListener(this, type, -1);
      }
    }
  }

  class Node extends EventTarget {
    parentNode = null;
    constructor() {
      super();
      this._c = [];
    }
    get firstChild() { return this._c[0] ?? null; }
    get lastChild() { return this._c[this._c.length - 1] ?? null; }
    get nextSibling() {
      const p = this.parentNode;
      return p ? (p._c[p._c.indexOf(this) + 1] ?? null) : null;
    }
    get previousSibling() {
      const p = this.parentNode;
      return p ? (p._c[p._c.indexOf(this) - 1] ?? null) : null;
    }
    get isConnected() { return isAttached(this); }
    get ownerDocument() { return document; }
    get childNodes() { return this._c; }
    _ins(nodes, i) {
      for (const n of nodes) {
        if (n.nodeType === 11) {
          const kids = [...n._c];
          n._c = [];
          for (const c of kids) c.parentNode = null;
          this._ins(kids, i);
          i += kids.length;
          continue;
        }
        n.remove();
        n.parentNode = this;
        this._c.splice(i++, 0, n);
        childAttached(this, n);
      }
    }
    append(...ns) { this._ins(ns, this._c.length); }
    appendChild(n) { this.append(n); return n; }
    removeChild(n) { n.remove(); return n; }
    insertBefore(n, ref) {
      if (ref == null) this.append(n);
      else ref.before(n);
      return n;
    }
    // Detach the moved nodes first: removing a node that sits before `this` in the same parent
    // shifts `this`, so the index must be computed afterwards (keyed {#each} reorders hit this).
    before(...ns) {
      const p = this.parentNode;
      if (!p) return;
      for (const n of ns) if (n !== this && n.nodeType !== 11) n.remove();
      p._ins(ns, p._c.indexOf(this));
    }
    after(...ns) {
      const p = this.parentNode;
      if (!p) return;
      for (const n of ns) if (n !== this && n.nodeType !== 11) n.remove();
      p._ins(ns, p._c.indexOf(this) + 1);
    }
    remove() {
      const p = this.parentNode;
      if (!p) return;
      p._c.splice(p._c.indexOf(this), 1);
      this.parentNode = null;
      childDetached(p, this);
    }
    get textContent() { return this.nodeValue ?? this._c.map((c) => c.textContent).join(''); }
    set textContent(v) {
      for (const c of [...this._c]) c.remove();
      if (v) this.append(document.createTextNode(String(v)));
    }
    cloneNode(deep) {
      const n = this._shallow();
      if (deep) for (const c of this._c) n.append(c.cloneNode(true));
      return n;
    }
  }

  class CharacterData extends Node {
    constructor(v) { super(); this._v = String(v); }
    get nodeValue() { return this._v; }
    set nodeValue(v) {
      this._v = String(v);
      if (this.parentNode) updateLabel(this.parentNode);
    }
    get data() { return this._v; }
    set data(v) { this.nodeValue = v; }
  }
  class Text extends CharacterData {
    nodeType = 3; nodeName = '#text';
    _shallow() { return new Text(this._v); }
  }
  class Comment extends CharacterData {
    nodeType = 8; nodeName = '#comment';
    _shallow() { return new Comment(this._v); }
  }
  class DocumentFragment extends Node {
    nodeType = 11; nodeName = '#document-fragment';
    _shallow() { return new DocumentFragment(); }
  }

  class Element extends Node {
    nodeType = 1;
    namespaceURI = NS_HTML;
    constructor(tag) {
      super();
      this.localName = tag;
      this.nodeName = tag.toUpperCase();
      this._a = {}; // attributes as set, for getAttribute
      this._props = new Map(); // host props, insertion-ordered
      this._signals = new Set(); // native signals with >= 1 listener
      this._count = {}; // DOM event type -> listener count
      this._dl = new Set(); // delegated event types counted as listeners
      this._slot = null;
      this._h = null; // native handle once realized
      this._lastLabel = '';
      const el = this;
      this.style = {
        set cssText(v) { el._a.style = v; },
        get cssText() { return el._a.style ?? ''; },
        setProperty() {},
        removeProperty() {},
      };
      this.classList = {
        toggle(k, on) {
          const s = new Set((el._a.class ?? '').split(' ').filter(Boolean));
          if (on) s.add(k); else s.delete(k);
          el._a.class = [...s].join(' ');
        },
      };
    }
    set className(v) { this._a.class = String(v); }
    get className() { return this._a.class ?? ''; }
    setAttribute(k, v) {
      v = String(v);
      this._a[k] = v;
      if (k === 'slot' || k === 'data-slot') this._slot = v;
      else if (!JS_ONLY_ATTRS.has(k)) setNativeProp(this, k, coerce(k, v));
    }
    setAttributeNS(ns, k, v) { this.setAttribute(k, v); }
    getAttribute(k) { return this._a[k] ?? null; }
    hasAttribute(k) { return k in this._a; }
    removeAttribute(k) {
      delete this._a[k];
      if (k === 'slot' || k === 'data-slot') this._slot = null;
      else if (this._props.has(k)) {
        this._props.delete(k);
        if (this._h != null) host.setProp(this._h, k, null);
      }
    }
    _shallow() {
      const e = new Element(this.localName);
      e._a = { ...this._a };
      e._props = new Map(this._props);
      e._slot = this._slot;
      return e;
    }
  }

  class Document extends Node {
    nodeType = 9; nodeName = '#document';
    createElement(t) { return new Element(String(t).toLowerCase()); }
    createElementNS(ns, t) { return new Element(String(t).toLowerCase()); }
    createTextNode(v) { return new Text(v); }
    createComment(v) { return new Comment(v); }
    createDocumentFragment() { return new DocumentFragment(); }
    importNode(n, deep) { return n.cloneNode(deep); }
  }

  const document = new Document();
  document.body = new Element('body');
  const root = new Element('svelters-root');

  function dispatch(h, signal, args = []) {
    const el = byHandle.get(h);
    if (!el) throw new Error(`dispatch: unknown handle ${h}`);
    // Mirror host-owned state so Svelte writing the same value back is a no-op.
    if (signal === 'changed' && el.localName === 'entry' && typeof args[0] === 'string') {
      el._props.set('text', args[0]);
    }
    const type = eventFor(signal);
    const path = [];
    for (let n = el; n; n = n.parentNode) path.push(n);
    path.push(document);
    const ev = {
      type, target: el, args, detail: args[0], cancelBubble: false, defaultPrevented: false,
      composedPath: () => path,
      stopPropagation() { this.cancelBubble = true; },
      preventDefault() { this.defaultPrevented = true; },
    };
    const results = [];
    // DOM semantics: one event object, bubbling to root/document so delegated handlers run.
    // Handlers registered under the raw signal name (`onchanged`) also get the event object.
    for (const t of new Set([type, signal])) {
      ev.type = t;
      for (const n of path) {
        for (const f of [...(n._l?.[t] ?? [])]) {
          const r = f.call(n, ev);
          if (n === el) results.push(r);
        }
        if (ev.cancelBubble) break;
      }
    }
    const first = args[0];
    return { results, event: first !== null && typeof first === 'object' ? first : ev };
  }

  return { document, root, dispatch, Node, Element, Text, Comment, CharacterData, DocumentFragment, EventTarget };
}

/** Define the globals Svelte's client runtime touches, bound to `host`. */
export function installDom(host, g = globalThis) {
  const dom = createDom(host);
  Object.assign(g, {
    window: g,
    document: dom.document,
    Node: dom.Node,
    Element: dom.Element,
    Text: dom.Text,
    Comment: dom.Comment,
    CharacterData: dom.CharacterData,
    DocumentFragment: dom.DocumentFragment,
    EventTarget: dom.EventTarget,
    HTMLMediaElement: class HTMLMediaElement {},
  });
  Object.defineProperty(g, 'navigator', { value: { userAgent: 'svelters' }, configurable: true });
  if (typeof g.requestAnimationFrame !== 'function') {
    g.requestAnimationFrame = (f) => g.setTimeout(() => f(0), 16);
  }
  g.__svelters_root = dom.root;
  g.__svelters_dispatch = dom.dispatch;
  return dom;
}
