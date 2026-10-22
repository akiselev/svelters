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
    const p = this.node(parent);
    p.insertBefore(this.node(child), before == null ? null : this.node(before));
    // A <select>'s value is written before its options exist (children are inserted after the
    // node's props); re-select it as options arrive, as Svelte's select binding does on mutation.
    const select = p.localName === 'select' ? p : p.localName === 'optgroup' && p.parentNode?.localName === 'select' ? p.parentNode : null;
    if (select && select.__svelters_value !== undefined) selectValue(select, select.__svelters_value);
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
    if (name === 'value' && n.localName === 'select') {
      n.__svelters_value = value;
      selectValue(n, value);
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
    if (name === 'value' && n.localName === 'select') {
      const v = n.multiple ? [...n.selectedOptions].map((o) => o.value) : n.selectedOptions[0]?.value;
      n.__svelters_value = v;
      return v;
    }
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

  /**
   * The per-element style sink (ABI v2.3 `style`), only the TR1 visual channels: what a native
   * transition shows, as the element's inline style. The X6 dialect ops are not used by the dom
   * backend (its `class` / `style` are props): TARGET answers none.
   */
  style(op, h, text) {
    if (op === 0) return 0;
    const n = this.nodes.get(h);
    if (!n || n.nodeType !== 1) return 0;
    switch (op) {
      case 4: // OPACITY
        n.style.opacity = text ?? '';
        return 0;
      case 8: {
        // TRANSFORM "tx ty sx sy ox oy"
        if (text == null) {
          n.style.transform = '';
          n.style.transformOrigin = '';
          return 0;
        }
        const [tx, ty, sx, sy, ox, oy] = text.split(' ').map(Number);
        n.style.transform = `translate(${tx}px, ${ty}px) scale(${sx}, ${sy})`;
        n.style.transformOrigin = ox === 0.5 && oy === 0.5 ? '' : `${ox * 100}% ${oy * 100}%`;
        return 0;
      }
      case 9: {
        // CLIP "w h" (px, -1 = unclipped)
        const [w, hh] = text == null ? [-1, -1] : text.split(' ').map(Number);
        n.style.overflow = text == null ? '' : 'hidden';
        n.style.width = w >= 0 ? `${w}px` : '';
        n.style.height = hh >= 0 ? `${hh}px` : '';
        return 0;
      }
      case 10: // INERT
        n.inert = text != null;
        return 0;
      default:
        throw new Error(`unknown style op ${op}`);
    }
  }

  /**
   * TR1: the element's computed style, which the native ports of `svelte/transition` read where
   * Svelte's read `getComputedStyle(node)` (jsdom: `opacity` is '' unless set inline, so Svelte's
   * own `fade` animates `t * 0` there, and the samples expect it).
   */
  computedStyle(h) {
    const n = this.nodes.get(h);
    if (!n || n.nodeType !== 1) return null;
    return n.ownerDocument.defaultView.getComputedStyle(n);
  }

  /** ABI v2.8 `bounds`: the element's rect (jsdom: zeros unless a test overrides getBoundingClientRect). */
  bounds(h) {
    const n = this.nodes.get(h);
    if (!n || typeof n.getBoundingClientRect !== 'function') return null;
    const r = n.getBoundingClientRect();
    return [r.left, r.top, r.right - r.left, r.bottom - r.top];
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

/** Svelte's select_option: select the option(s) whose value matches (an array for `multiple`). */
function selectValue(select, value) {
  for (const o of select.options) {
    o.selected = select.multiple ? Array.isArray(value) && value.includes(o.value) : o.value === String(value);
  }
  if (!select.multiple && ![...select.options].some((o) => o.selected)) select.selectedIndex = -1;
}

function eventType(signal) {
  return signal.endsWith('capture') ? [signal.slice(0, -'capture'.length), true] : [signal, false];
}
