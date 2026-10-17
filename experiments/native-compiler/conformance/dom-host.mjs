// A SveltersHost (plans/architecture/backend-abi.md "JS-side host ops") over a real DOM (jsdom),
// for the kinds of metadata/semantics/dom.json. Handles are numbers; `#text` is a Text node.
// Events are dispatched synchronously from the DOM listener (a browser has no host job queue),
// so `button.click()` in a Svelte test runs the handlers before it returns, as with Svelte's own
// DOM runtime.

/** Props written as DOM properties (their attribute, if any, is the default, not the state). */
const PROPERTIES = new Set(['value', 'checked', 'selected', 'defaultValue', 'defaultChecked', 'defaultSelected', 'indeterminate', 'muted', 'open']);

/** clsx-style class value: string, array (token lists from `$n.token`), object, falsy. */
function classValue(v) {
  if (v == null || v === false) return '';
  if (typeof v === 'string') return v;
  if (Array.isArray(v)) return v.map(classValue).filter(Boolean).join(' ');
  if (typeof v === 'object') return Object.keys(v).filter((k) => v[k]).join(' ');
  return String(v);
}

export class DomHost {
  constructor(document) {
    this.doc = document;
    this.nodes = new Map();
    this.next = 1;
    this.listeners = new Map(); // `${h} ${signal}` -> fn
    this.log = []; // op counts, for the report
  }

  node(h) {
    const n = this.nodes.get(h);
    if (!n) throw new Error(`stale or foreign handle ${h}`);
    return n;
  }

  /** Registers an existing DOM node (the mount target) and returns its handle. */
  adopt(n) {
    const h = this.next++;
    this.nodes.set(h, n);
    return h;
  }

  create(kind) {
    const n = kind === '#text' ? this.doc.createTextNode('') : this.doc.createElement(kind);
    return this.adopt(n);
  }

  insert(parent, child, before, _slot) {
    this.node(parent).insertBefore(this.node(child), before == null ? null : this.node(before));
  }

  remove(parent, child) {
    const p = this.nodes.get(parent);
    const c = this.nodes.get(child);
    if (p && c && c.parentNode === p) p.removeChild(c);
  }

  destroy(h) {
    const n = this.nodes.get(h);
    if (!n) return;
    for (const [key, fn] of this.listeners) {
      if (key.startsWith(`${h} `)) {
        const signal = key.slice(key.indexOf(' ') + 1);
        const [type, capture] = eventType(signal);
        n.removeEventListener(type, fn, capture);
        this.listeners.delete(key);
      }
    }
    this.nodes.delete(h);
  }

  setProp(h, name, value) {
    const n = this.nodes.get(h);
    if (!n) return;
    if (n.nodeType === 3) {
      n.data = value == null ? '' : String(value);
      return;
    }
    if (name === 'class') {
      const c = classValue(value);
      if (c === '') n.removeAttribute('class');
      else n.setAttribute('class', c);
      return;
    }
    if (PROPERTIES.has(name) && name in n) {
      if (name === 'value') n.value = value == null ? '' : value;
      else n[name] = value;
      return;
    }
    if (typeof value === 'boolean') {
      // bool attributes (spec type bool): present or absent
      if (name in n && typeof n[name] === 'boolean') n[name] = value;
      else n.toggleAttribute(name, value);
      return;
    }
    if (value == null) n.removeAttribute(name);
    else n.setAttribute(name === 'htmlFor' ? 'for' : name, String(value));
  }

  getProp(h, name) {
    const n = this.node(h);
    if (n.nodeType === 3) return n.data;
    if (name === 'class') return (n.getAttribute('class') ?? '').split(/\s+/).filter(Boolean);
    if (name === 'value' && n.localName === 'input' && (n.type === 'number' || n.type === 'range')) {
      // Svelte's bind:value on numeric inputs (to_number)
      return n.value === '' ? null : +n.value;
    }
    if (name in n && typeof n[name] !== 'function') return n[name];
    return n.getAttribute(name);
  }

  call(h, method, args) {
    const n = this.node(h);
    return n[method](...args);
  }

  listen(h, signal, on) {
    const n = this.node(h);
    const key = `${h} ${signal}`;
    const [type, capture] = eventType(signal);
    if (on) {
      if (this.listeners.has(key)) return;
      const fn = (e) => globalThis.__svelters_dispatch(h, signal, [e]);
      this.listeners.set(key, fn);
      n.addEventListener(type, fn, capture);
    } else {
      const fn = this.listeners.get(key);
      if (!fn) return;
      n.removeEventListener(type, fn, capture);
      this.listeners.delete(key);
    }
  }
}

function eventType(signal) {
  return signal.endsWith('capture') ? [signal.slice(0, -'capture'.length), true] : [signal, false];
}
